use super::*;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize)]
struct Criterion {
    id: String,
    domain: String,
    title: String,
    acceptance: String,
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Roadmap {
    schema_version: u32,
    target: String,
    scope_complete: bool,
    scope_note: String,
    next: String,
    criteria: Vec<Criterion>,
}

fn validate(roadmap: &Roadmap) -> Result<()> {
    ensure!(roadmap.schema_version == 1, "unsupported roadmap schema");
    ensure!(
        !roadmap.scope_complete,
        "full-game scope audit is not implemented; scope_complete must remain false"
    );
    ensure!(!roadmap.criteria.is_empty(), "empty acceptance scope");
    let mut ids = BTreeSet::new();
    for c in &roadmap.criteria {
        ensure!(
            !c.id.is_empty() && ids.insert(&c.id),
            "empty or duplicate criterion ID: {}",
            c.id
        );
        ensure!(
            !c.domain.is_empty() && !c.title.is_empty() && !c.acceptance.is_empty(),
            "missing acceptance fields: {}",
            c.id
        );
        ensure!(
            ["not_started", "in_progress", "implemented"].contains(&c.state.as_str()),
            "invalid state {} for {}: verified requires a reference-comparison runner",
            c.state,
            c.id
        );
    }
    Ok(())
}

fn state(receipt: &Receipt, hash: &str, time: u64) -> Result<&'static str> {
    ensure!(
        receipt.schema_version == 1 && receipt.source_hash.len() == 64,
        "invalid evidence schema/hash"
    );
    ensure!(receipt.time_unix <= time + 300, "receipt is in the future");
    if receipt.source_hash != hash
        || receipt.os != env::consts::OS
        || receipt.arch != env::consts::ARCH
    {
        return Ok("stale");
    }
    Ok(if receipt.passed { "passed" } else { "failed" })
}

fn receipt(kind: &str, hash: &str, time: u64) -> Result<Value> {
    let path = Path::new("local/evidence").join(format!("{kind}.json"));
    if !path.exists() {
        return Ok(json!({"state": "not_run"}));
    }
    let r: Receipt = serde_json::from_slice(&fs::read(&path)?)
        .with_context(|| format!("invalid receipt {}", path.display()))?;
    ensure!(r.kind == kind, "receipt kind mismatch");
    Ok(
        json!({"state": state(&r, hash, time)?, "time_unix": r.time_unix, "source_hash": r.source_hash,
        "os": r.os, "arch": r.arch, "details": r.details}),
    )
}

fn embedded_json(value: &Value) -> Result<String> {
    // JSON inside a script element must not allow user-controlled closing tags.
    Ok(serde_json::to_string(value)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029"))
}

fn render(value: &Value) -> Result<String> {
    let template = include_str!("dashboard.html");
    let marker = "\"__REPORT_JSON__\"";
    ensure!(
        template.matches(marker).count() == 1,
        "dashboard data marker must appear exactly once"
    );
    Ok(template.replace(marker, &embedded_json(value)?))
}

fn inspection_state(receipt: &Value) -> Value {
    // Complete placement decoding does not make the current shader/window a full renderer.
    if receipt["state"] == "passed" {
        json!("partial")
    } else {
        receipt["state"].clone()
    }
}

fn menu_checkpoint(receipt: &Value) -> Value {
    let state = receipt["state"].as_str().unwrap_or("not_run");
    // One measured launch smoke, separate from game completion or reference parity.
    let passed = u32::from(state == "passed");
    let percent = match state {
        "passed" => Some(100),
        "failed" => Some(0),
        _ => None,
    };
    json!({"id":"native_main_menu", "state":state, "passed":passed, "total":1,
        "percent":percent, "reference_compared":false})
}

pub fn generate() -> Result<()> {
    let raw = fs::read("progress/roadmap.json")?;
    let roadmap: Roadmap = serde_json::from_slice(&raw)?;
    validate(&roadmap)?;
    let hash = source_hash()?;
    let scope_hash = format!("{:x}", Sha256::digest(&raw));
    let time = now();
    let check = receipt("check", &hash, time)?;
    let coverage = receipt("coverage", &hash, time)?;
    let assets = receipt("assets", &hash, time)?;
    let resources = receipt("resources", &hash, time)?;
    let render_receipt = receipt("render", &hash, time)?;
    let viewer_receipt = receipt("viewer", &hash, time)?;
    let navigation_receipt = receipt("navigation", &hash, time)?;
    let paths_receipt = receipt("paths", &hash, time)?;
    let menu_receipt = receipt("menu", &hash, time)?;
    let startup_receipt = receipt("startup", &hash, time)?;
    let mission_receipt = receipt("mission", &hash, time)?;
    let implemented = roadmap
        .criteria
        .iter()
        .filter(|c| c.state == "implemented")
        .count();
    let in_progress = roadmap
        .criteria
        .iter()
        .filter(|c| c.state == "in_progress")
        .count();
    let total = roadmap.criteria.len();
    let mut groups: BTreeMap<&str, Value> = BTreeMap::new();
    for c in &roadmap.criteria {
        let group = groups
            .entry(&c.domain)
            .or_insert(json!({"total":0, "implemented":0, "in_progress":0, "verified":0}));
        group["total"] = json!(group["total"].as_u64().unwrap_or(0) + 1);
        if c.state != "not_started" {
            group[&c.state] = json!(group[&c.state].as_u64().unwrap_or(0) + 1);
        }
    }
    let mut latest = json!({
        "schema_version": 1, "generated_unix": time, "source_hash": hash, "scope_hash": scope_hash,
        "target": roadmap.target, "scope_complete": false, "scope_note": roadmap.scope_note, "next": roadmap.next,
        "parity": {"verified":0, "total":total, "percent":0.0, "implemented":implemented, "in_progress":in_progress,
            "implemented_percent": 100.0 * implemented as f64 / total as f64, "comparator_available":false},
        "groups": groups, "criteria":roadmap.criteria,
        "initial_checkpoint":menu_checkpoint(&menu_receipt),
        "checks":check, "coverage":coverage, "assets":assets,"resources":resources,"render":render_receipt,"viewer":viewer_receipt,"navigation":navigation_receipt,"paths":paths_receipt,"menu":menu_receipt,"startup":startup_receipt,"mission":mission_receipt,
        "compatibility": [
            {"edition":"Wii · SJBE52", "revision":"0", "container":"RVZ", "assets":"not_run", "resources":"not_run", "render":"not_run", "viewer":"not_run", "menu":"not_run", "startup":"not_run", "runtime":"not_implemented"},
            {"edition":"Other Wii editions", "revision":"Unverified", "container":"Unverified", "assets":"not_run", "resources":"not_run", "render":"not_run", "viewer":"not_run", "menu":"not_run", "startup":"not_run", "runtime":"not_implemented"}
        ]
    });
    if latest["assets"]["details"]["disc"]["game_id"] == "SJBE52"
        && latest["assets"]["details"]["disc"]["revision"] == 0
        && latest["assets"]["details"]["disc"]["format"] == "Rvz"
    {
        latest["compatibility"][0]["assets"] = latest["assets"]["state"].clone();
    }
    if latest["resources"]["details"]["disc"]["game_id"] == "SJBE52"
        && latest["resources"]["details"]["disc"]["revision"] == 0
        && latest["resources"]["details"]["disc"]["format"] == "Rvz"
    {
        latest["compatibility"][0]["resources"] = latest["resources"]["state"].clone();
    }
    if latest["render"]["details"]["disc"]["game_id"] == "SJBE52"
        && latest["render"]["details"]["disc"]["revision"] == 0
        && latest["render"]["details"]["disc"]["format"] == "Rvz"
    {
        latest["compatibility"][0]["render"] = inspection_state(&latest["render"]);
    }
    if latest["viewer"]["details"]["disc"]["game_id"] == "SJBE52"
        && latest["viewer"]["details"]["disc"]["revision"] == 0
        && latest["viewer"]["details"]["disc"]["format"] == "Rvz"
    {
        latest["compatibility"][0]["viewer"] = inspection_state(&latest["viewer"]);
    }
    if latest["menu"]["details"]["disc"]["game_id"] == "SJBE52"
        && latest["menu"]["details"]["disc"]["revision"] == 0
        && latest["menu"]["details"]["disc"]["format"] == "Rvz"
    {
        latest["compatibility"][0]["menu"] = latest["menu"]["state"].clone();
    }
    if latest["startup"]["details"]["disc"]["game_id"] == "SJBE52"
        && latest["startup"]["details"]["disc"]["revision"] == 0
        && latest["startup"]["details"]["disc"]["format"] == "Rvz"
    {
        latest["compatibility"][0]["startup"] = latest["startup"]["state"].clone();
    }
    let history_path = Path::new("progress/history.json");
    let mut history: Vec<Value> = if history_path.exists() {
        serde_json::from_slice(&fs::read(history_path)?)?
    } else {
        Vec::new()
    };
    let observation = json!({"source_hash":hash,"scope_hash":scope_hash,"parity_percent":0.0,"total":total,
        "implemented_percent": latest["parity"]["implemented_percent"], "coverage_percent":latest["coverage"]["details"]["totals"]["lines"]["percent"],
        "coverage_state":latest["coverage"]["state"], "check_state":latest["checks"]["state"], "asset_state":latest["assets"]["state"],
        "check_time":latest["checks"]["time_unix"],"coverage_time":latest["coverage"]["time_unix"],"asset_time":latest["assets"]["time_unix"],"resource_state":latest["resources"]["state"],"resource_time":latest["resources"]["time_unix"],"render_state":latest["render"]["state"],"render_time":latest["render"]["time_unix"],"viewer_state":latest["viewer"]["state"],"viewer_time":latest["viewer"]["time_unix"],"navigation_state":latest["navigation"]["state"],"navigation_time":latest["navigation"]["time_unix"]});
    let mut observation = observation;
    observation["paths_state"] = latest["paths"]["state"].clone();
    observation["paths_time"] = latest["paths"]["time_unix"].clone();
    observation["menu_state"] = latest["menu"]["state"].clone();
    observation["menu_time"] = latest["menu"]["time_unix"].clone();
    observation["startup_state"] = latest["startup"]["state"].clone();
    observation["startup_time"] = latest["startup"]["time_unix"].clone();
    observation["coverage_measurement_version"] =
        latest["coverage"]["details"]["measurement_version"].clone();
    let identity = format!("{:x}", Sha256::digest(serde_json::to_vec(&observation)?));
    if history.last().is_none_or(|h| h["id"] != identity) {
        let mut entry = observation;
        entry["id"] = json!(identity);
        entry["observed_unix"] = json!(time);
        history.push(entry);
        if history.len() > 120 {
            history.remove(0);
        }
        write_json(history_path, &history)?;
    }
    latest["history"] = json!(history);
    let html = render(&latest)?;
    write_json(Path::new("progress/latest.json"), &latest)?;
    fs::write("progress/index.html", html)?;
    println!(
        "Dashboard: progress/index.html | tracked parity 0/{total} | checks {} | coverage {} | assets {} | menu {} | startup {}",
        latest["checks"]["state"],
        latest["coverage"]["state"],
        latest["assets"]["state"],
        latest["menu"]["state"],
        latest["startup"]["state"]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roadmap() -> Roadmap {
        serde_json::from_str(include_str!("../../progress/roadmap.json")).unwrap()
    }
    #[test]
    fn menu_launch_percentage_requires_current_passing_evidence() {
        for (status, passed, percent) in [
            ("passed", 1, json!(100)),
            ("failed", 0, json!(0)),
            ("stale", 0, Value::Null),
            ("not_run", 0, Value::Null),
        ] {
            let checkpoint = menu_checkpoint(&json!({"state":status}));
            assert_eq!(checkpoint["state"], status);
            assert_eq!(checkpoint["passed"], passed);
            assert_eq!(checkpoint["total"], 1);
            assert_eq!(checkpoint["percent"], percent);
            assert_eq!(checkpoint["reference_compared"], false);
        }
    }
    #[test]
    fn complete_decode_counts_cannot_promote_inspection_to_full_compatibility() {
        let receipt = json!({"state":"passed","details":{"placements_not_loaded":0,"strict_placement_decode_passed":true}});
        assert_eq!(inspection_state(&receipt), "partial");
        for status in ["stale", "failed", "not_run"] {
            assert_eq!(inspection_state(&json!({"state":status})), status);
        }
    }
    #[test]
    fn manual_verified_claim_is_rejected() {
        let mut r = roadmap();
        r.criteria[0].state = "verified".into();
        assert!(validate(&r).is_err());
    }
    #[test]
    fn duplicate_ids_and_empty_denominators_are_rejected() {
        let mut r = roadmap();
        r.criteria[1].id = r.criteria[0].id.clone();
        assert!(validate(&r).is_err());
        r.criteria.clear();
        assert!(validate(&r).is_err());
    }
    #[test]
    fn changed_source_cannot_reuse_a_passing_receipt() {
        let r = Receipt {
            schema_version: 1,
            kind: "check".into(),
            source_hash: "a".repeat(64),
            time_unix: 100,
            os: env::consts::OS.into(),
            arch: env::consts::ARCH.into(),
            passed: true,
            details: json!({}),
        };
        assert_eq!(state(&r, &"a".repeat(64), 101).unwrap(), "passed");
        assert_eq!(state(&r, &"b".repeat(64), 101).unwrap(), "stale");
        assert!(state(&r, &"a".repeat(64), 0).is_ok());
        let future = Receipt {
            time_unix: 1000,
            ..r
        };
        assert!(state(&future, &"a".repeat(64), 0).is_err());
    }
    #[test]
    fn embedded_data_cannot_close_script_elements() {
        let input = json!({"title":"</script><script>alert(1)</script>&"});
        let output = embedded_json(&input).unwrap();
        assert!(!output.contains('<'));
        assert!(!output.contains('&'));
        assert_eq!(serde_json::from_str::<Value>(&output).unwrap(), input);
    }

    #[test]
    fn generated_html_contains_parseable_data_after_formatting() {
        let value = json!({"title":"</script>","parity":{"total":44,"verified":0}});
        let html = render(&value).unwrap();
        let marker = "<script id=\"report-data\" type=\"application/json\">";
        let data = html
            .split_once(marker)
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        assert_eq!(serde_json::from_str::<Value>(data).unwrap(), value);
        assert!(!html.contains("__REPORT_JSON__"));
    }
}
