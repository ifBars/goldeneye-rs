mod menu;
mod mission;
mod report;
mod startup;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize, Deserialize)]
struct Receipt {
    schema_version: u32,
    kind: String,
    source_hash: String,
    time_unix: u64,
    os: String,
    arch: String,
    passed: bool,
    details: Value,
}

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace root")?;
    env::set_current_dir(root)?;
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => check(),
        Some("coverage") => coverage(),
        Some("smoke") => smoke(Path::new(
            &args.next().context("usage: cargo xtask smoke <disc>")?,
        )),
        Some("resources") => resources(Path::new(
            &args.next().context("usage: cargo xtask resources <disc>")?,
        )),
        Some("navigation") => navigation_smoke(Path::new(
            &args
                .next()
                .context("usage: cargo xtask navigation <disc>")?,
        )),
        Some("paths") => path_smoke(Path::new(
            &args.next().context("usage: cargo xtask paths <disc>")?,
        )),
        Some("render") => render_smoke(Path::new(
            &args.next().context("usage: cargo xtask render <disc>")?,
        )),
        Some("viewer") => viewer_smoke(Path::new(
            &args.next().context("usage: cargo xtask viewer <disc>")?,
        )),
        Some("menu") => menu::run(Path::new(
            &args.next().context("usage: cargo xtask menu <disc>")?,
        )),
        Some("startup") => startup::run(Path::new(
            &args.next().context("usage: cargo xtask startup <disc>")?,
        )),
        Some("mission") => mission::run(Path::new(
            &args.next().context("usage: cargo xtask mission <disc>")?,
        )),
        Some("report") => report::generate(),
        Some("package") => package(),
        Some("notices") => notices(),
        None | Some("help") | Some("--help") => {
            println!(
                "cargo xtask <check|coverage|smoke <disc>|resources <disc>|navigation <disc>|paths <disc>|render <disc>|viewer <disc>|menu <disc>|startup <disc>|mission <disc>|report|package|notices>"
            );
            Ok(())
        }
        Some(other) => bail!("unknown task {other}; use cargo xtask --help"),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cargo(args: &[&str]) -> Result<()> {
    println!("cargo {}", args.join(" "));
    ensure!(
        Command::new("cargo").args(args).status()?.success(),
        "cargo {} failed",
        args.join(" ")
    );
    Ok(())
}

fn output(program: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {}", program.display()))?;
    if !output.status.success() {
        bail!(
            "{} failed: {}",
            program.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn source_hash() -> Result<String> {
    let mut files = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.lock"),
        PathBuf::from("rust-toolchain.toml"),
        PathBuf::from("tools/dashboard/package.json"),
        PathBuf::from("tools/dashboard/bun.lock"),
        PathBuf::from("tools/dashboard/dashboard.test.mjs"),
    ];
    for root in [
        "crates",
        "apps",
        "xtask",
        ".cargo",
        ".github",
        ".vscode",
        "third-party",
    ] {
        collect(Path::new(root), &mut files)?;
    }
    files.sort();
    let mut hash = Sha256::new();
    for path in files {
        let name = path.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&path)?;
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.update(output(Path::new("rustc"), &["--version"])?);
    Ok(format!("{:x}", hash.finalize()))
}

fn collect(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        ensure!(
            !entry.file_type()?.is_symlink(),
            "source symlinks are unsupported: {}",
            entry.path().display()
        );
        if entry.file_type()?.is_dir() {
            collect(&entry.path(), files)?;
        } else {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn record(kind: &str, hash: String, passed: bool, details: Value) -> Result<()> {
    ensure!(
        hash == source_hash()?,
        "source changed while {kind} ran; evidence not recorded"
    );
    let receipt = Receipt {
        schema_version: 1,
        kind: kind.into(),
        source_hash: hash,
        time_unix: now(),
        os: env::consts::OS.into(),
        arch: env::consts::ARCH.into(),
        passed,
        details,
    };
    write_json(
        &Path::new("local/evidence").join(format!("{kind}.json")),
        &receipt,
    )
}

fn check() -> Result<()> {
    let hash = source_hash()?;
    let commands: &[(&str, &[&str])] = &[
        ("format", &["fmt", "--all", "--", "--check"]),
        (
            "lint",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("tests", &["test", "--workspace", "--locked"]),
        (
            "release",
            &["build", "--release", "-p", "goldeneye", "--locked"],
        ),
    ];
    let mut steps = Vec::new();
    let mut passed = true;
    for (name, args) in commands {
        let start = Instant::now();
        let result = cargo(args);
        steps.push(
            json!({"id": name, "passed": result.is_ok(), "seconds": start.elapsed().as_secs_f64()}),
        );
        if result.is_err() {
            passed = false;
            break;
        }
    }
    record("check", hash, passed, json!({"steps": steps}))?;
    report::generate()?;
    ensure!(passed, "checks failed; see output and dashboard");
    Ok(())
}

fn coverage() -> Result<()> {
    let hash = source_hash()?;
    fs::create_dir_all("local/coverage")?;
    let result = cargo(&[
        "llvm-cov",
        "--workspace",
        "--exclude",
        "xtask",
        "--locked",
        "--json",
        "--summary-only",
        "--ignore-filename-regex",
        r"[/\\]tests([/\\]|\.rs$)",
        "--output-path",
        "local/coverage/summary.json",
        "--fail-under-lines",
        "50",
    ]);
    let mut details = json!({
        "scope": "production crates and executable; excludes xtask, test modules and test fixtures",
        "measurement_version": 2
    });
    let mut passed = result.is_ok();
    if passed {
        let data: Value = serde_json::from_slice(&fs::read("local/coverage/summary.json")?)?;
        for file in data["data"][0]["files"]
            .as_array()
            .context("missing LLVM coverage file list")?
        {
            let path = file["filename"]
                .as_str()
                .context("missing LLVM coverage filename")?
                .replace('\\', "/");
            ensure!(
                !path.split('/').any(|part| part == "tests")
                    && !path.ends_with("/tests.rs")
                    && !path.ends_with("/fixtures.rs"),
                "test-only code entered production coverage: {path}"
            );
        }
        details["totals"] = data["data"][0]["totals"].clone();
        ensure!(
            details["totals"]["lines"]["percent"].as_f64().is_some(),
            "missing LLVM coverage totals"
        );
        passed = cargo(&[
            "llvm-cov",
            "report",
            "--html",
            "--ignore-filename-regex",
            r"[/\\]tests([/\\]|\.rs$)",
            "--output-dir",
            "local/coverage",
        ])
        .is_ok();
    }
    record("coverage", hash, passed, details)?;
    report::generate()?;
    ensure!(
        passed,
        "coverage failed; install cargo-llvm-cov and llvm-tools-preview if missing"
    );
    Ok(())
}

fn executable(profile: &str) -> PathBuf {
    Path::new("target")
        .join(profile)
        .join(format!("goldeneye{}", env::consts::EXE_SUFFIX))
}

fn resources(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
    let result = (|| -> Result<Value> {
        let output = Command::new(executable("release"))
            .args([
                "scan-edb",
                disc.to_str().context("disc path must be UTF-8")?,
                "--decode-textures",
                "--decode-graphs",
                "--decode-collision",
            ])
            .stderr(std::process::Stdio::inherit())
            .output()?;
        let summary: Value =
            serde_json::from_slice(&output.stdout).context("EDB audit did not emit JSON")?;
        let errors = summary["errors"]
            .as_array()
            .context("audit errors missing")?
            .len();
        let path = Path::new("local/resource-audits").join(format!(
            "{}-{}.json",
            now(),
            std::process::id()
        ));
        write_json(&path, &summary)?;
        let mut details = summary;
        details
            .as_object_mut()
            .context("audit summary")?
            .remove("errors");
        details["error_count"] = json!(errors);
        details["passed"] = json!(
            output.status.success()
                && errors == 0
                && details["resources"] == details["parsed"]
                && details["decoded_pixels"] == true
                && details["grenade_prototype"]["hash"].as_u64() == Some(0x0400_0d8e)
                && details["collision"]["found"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["collision"]["found"] == details["collision"]["decoded"]
                && details["collision"]["box_triangle"]["queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["collision"]["box_triangle"]["far_queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["collision"]["box_triangle"]["far_queries"]
                    == details["collision"]["box_triangle"]["far_separated"]
                && details["map_spatial"]["maps"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["map_spatial"]["maps"] == details["map_spatial"]["decoded"]
                && details["map_spatial"]["validated_queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["map_spatial"]["portals"]["maps"] == details["map_spatial"]["maps"]
                && details["map_spatial"]["portals"]["distance_calls"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["map_spatial"]["portals"]["stable_expansions"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["initial_placements"]["maps"] == details["map_spatial"]["maps"]
                && details["initial_placements"]["placements"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["initial_placements"]["probed_points"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["collision_frames"]["maps"] == details["initial_placements"]["maps"]
                && details["collision_frames"]["placements"]
                    == details["initial_placements"]["placements"]
                && details["collision_frames"]["rigid_frames"]
                    == details["initial_placements"]["placements"]
                && details["collision_frames"]["general_frames"]
                    == details["initial_placements"]["placements"]
                && details["collision_frames"]["validated_general_frames"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["collision_frames"]["query_probes"]
                    .as_u64()
                    .zip(details["initial_placements"]["probed_points"].as_u64())
                    .is_some_and(|(queries, points)| points.checked_mul(2) == Some(queries))
                && details["initial_bounds"]["maps"] == details["initial_placements"]["maps"]
                && details["initial_bounds"]["base_bounds_probed"]
                    == details["initial_placements"]["placements"]
                && details["initial_bounds"]["unsupported_components"].as_u64() == Some(0)
                && details["initial_bounds"]["assembled_bounds_probed"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["candidate_hierarchies"]["maps"] == details["map_spatial"]["maps"]
                && details["candidate_hierarchies"]["refreshed_hierarchies"]
                    == details["candidate_hierarchies"]["zones"]
                && details["candidate_hierarchies"]["stable_queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["static_geometry"]["maps"] == details["map_spatial"]["maps"]
                && details["static_geometry"]["queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["static_geometry"]["records_collected"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["static_geometry"]["frame_origins_prepared"]
                    == details["static_geometry"]["records_collected"]
                && details["static_geometry"]["full_rejections"].as_u64() == Some(0)
                && details["static_mesh_contacts"]["maps"] == details["map_spatial"]["maps"]
                && details["static_mesh_contacts"]["queries"]
                    == details["static_geometry"]["queries"]
                && details["static_mesh_contacts"]["records_collected"]
                    == details["static_geometry"]["records_collected"]
                && details["static_mesh_contacts"]["primitive_queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["static_mesh_contacts"]["contacts_appended"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["static_mesh_contacts"]["contacts_validated"]
                    == details["static_mesh_contacts"]["contacts_appended"]
                && details["static_mesh_contacts"]["record_capacity_rejections"].as_u64()
                    == Some(0)
                && details["static_mesh_contacts"]["contact_capacity_mesh_skips"].as_u64()
                    == Some(0)
                && details["static_mesh_contacts"]["contact_capacity_after_hit_full"].as_u64()
                    == Some(0)
                && details["world_box_requests"]["maps"] == details["map_spatial"]["maps"]
                && details["world_box_requests"]["sample_origins"]
                    == details["static_geometry"]["queries"]
                && details["world_box_requests"]["single_queries"]
                    == details["world_box_requests"]["sample_origins"]
                && details["world_box_requests"]["expanded_queries"]
                    == details["world_box_requests"]["sample_origins"]
                && details["world_box_requests"]["direct_single_record_checks"]
                    == details["world_box_requests"]["single_queries"]
                && details["world_box_requests"]["direct_single_contact_checks"]
                    == details["world_box_requests"]["single_queries"]
                && details["world_box_requests"]["expanded_record_sequence_checks"]
                    == details["world_box_requests"]["expanded_queries"]
                && details["world_box_requests"]["contacts"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["world_box_requests"]["contacts_validated"]
                    == details["world_box_requests"]["contacts"]
                && details["world_box_requests"]["record_capacity_rejections"].as_u64() == Some(0)
                && details["world_box_requests"]["contact_capacity_mesh_skips"].as_u64() == Some(0)
                && details["world_box_requests"]["contact_capacity_after_hit_full"].as_u64()
                    == Some(0)
                && details["world_line_requests"]["maps"] == details["map_spatial"]["maps"]
                && details["world_line_requests"]["sample_origins"]
                    == details["static_geometry"]["queries"]
                && details["world_line_requests"]["sample_origins"]
                    .as_u64()
                    .and_then(|n| n.checked_mul(3))
                    == details["world_line_requests"]["queries"].as_u64()
                && details["world_line_requests"]["sample_origins"]
                    .as_u64()
                    .and_then(|n| n.checked_mul(2))
                    == details["world_line_requests"]["nearest_queries"].as_u64()
                && details["world_line_requests"]["sample_origins"]
                    == details["world_line_requests"]["any_hit_queries"]
                && details["world_line_requests"]["mode_comparisons"]
                    == details["world_line_requests"]["sample_origins"]
                && details["world_line_requests"]["hits"]
                    .as_u64()
                    .zip(details["world_line_requests"]["misses"].as_u64())
                    .and_then(|(hits, misses)| hits.checked_add(misses))
                    == details["world_line_requests"]["queries"].as_u64()
                && details["world_line_requests"]["hits"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["world_line_requests"]["final_hits_validated"]
                    == details["world_line_requests"]["hits"]
                && details["world_line_requests"]["f64_plane_checks"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["graphs"]["found"].as_u64().is_some_and(|n| n > 0)
                && details["graphs"]["found"] == details["graphs"]["decoded"]
                && details["map_navigation"]["maps"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["map_navigation"]["maps"] == details["map_navigation"]["decoded"]
                && details["map_navigation"]["ready"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["map_navigation"]["registrations"].as_u64()
                    == details["map_navigation"]["ready"]
                        .as_u64()
                        .zip(details["map_navigation"]["unavailable"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
        );
        println!(
            "EDB audit: {}/{} resources; {} local images; {} decoded mips; {errors} errors. {}",
            details["parsed"],
            details["resources"],
            details["local_images"],
            details["decoded_mips"],
            path.display()
        );
        println!(
            "Triangle graphs: {}/{} bodies decoded; {} triangles, {} links; runtime semantics unverified",
            details["graphs"]["decoded"],
            details["graphs"]["found"],
            details["graphs"]["triangles"],
            details["graphs"]["directed_links"]
        );
        Ok(details)
    })();
    let (passed, details) = match result {
        Ok(details) => (details["passed"] == true, details),
        Err(error) => {
            eprintln!("EDB audit failed: {error:#}");
            (
                false,
                json!({"message":"EDB audit failed; inspect local terminal output."}),
            )
        }
    };
    record("resources", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "EDB/local texture audit failed");
    Ok(())
}

fn navigation_smoke(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
    let result = (|| -> Result<Value> {
        let output = Command::new(executable("release"))
            .args([
                "scan-nav",
                disc.to_str().context("disc path must be UTF-8")?,
            ])
            .stderr(std::process::Stdio::inherit())
            .output()?;
        let mut details: Value =
            serde_json::from_slice(&output.stdout).context("navigation audit did not emit JSON")?;
        let errors = details["errors"]
            .as_array()
            .context("missing navigation errors")?
            .len();
        let path = Path::new("local/navigation-audits").join(format!(
            "{}-{}.json",
            now(),
            std::process::id()
        ));
        write_json(&path, &details)?;
        details
            .as_object_mut()
            .context("navigation summary")?
            .remove("errors");
        details["error_count"] = json!(errors);
        details["passed"] = json!(
            output.status.success()
                && errors == 0
                && details["graphs"].as_u64().is_some_and(|n| n > 0)
                && details["graphs"] == details["probed"]
                && details["stats"]["queries"].as_u64().is_some_and(|n| n > 0)
                && [
                    "first_queries",
                    "nearest_queries",
                    "segment_queries",
                    "connected_queries",
                    "connected_neighbor_hits",
                    "connected_policy_checks"
                ]
                .into_iter()
                .all(|name| details["stats"][name].as_u64().is_some_and(|n| n > 0))
                && details["stats"]["first_queries"] == details["stats"]["nearest_queries"]
                && details["stats"]["hits"] == details["stats"]["queries"]
                && details["stats"]["misses"] == 0
                && details["reference_compared"] == false
        );
        println!(
            "Navigation query smoke: {}/{} navmeshes; {} sampled queries; {errors} errors. {}",
            details["probed"],
            details["graphs"],
            details["stats"]["queries"],
            path.display()
        );
        Ok(details)
    })();
    let (passed, details) = match result {
        Ok(d) => (d["passed"] == true, d),
        Err(error) => {
            eprintln!("Navigation audit failed: {error:#}");
            (
                false,
                json!({"message":"Navigation audit failed; inspect local terminal output."}),
            )
        }
    };
    record("navigation", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "navigation query smoke failed");
    Ok(())
}

fn path_smoke(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
    let result = (|| -> Result<Value> {
        let output = Command::new(executable("release"))
            .args([
                "scan-paths",
                disc.to_str().context("disc path must be UTF-8")?,
            ])
            .stderr(std::process::Stdio::inherit())
            .output()?;
        let mut details: Value =
            serde_json::from_slice(&output.stdout).context("path audit did not emit JSON")?;
        let errors = details["errors"]
            .as_array()
            .context("missing path errors")?
            .len();
        let path =
            Path::new("local/path-audits").join(format!("{}-{}.json", now(), std::process::id()));
        write_json(&path, &details)?;
        details
            .as_object_mut()
            .context("path summary")?
            .remove("errors");
        details["error_count"] = json!(errors);
        let actor_goals = &details["stats"]["actor_goals"];
        let actor_targets = &details["stats"]["actor_targets"];
        let actor_actions = &actor_targets["actions"];
        let action_sum = |parts: &[&str]| {
            parts.iter().try_fold(0u64, |sum, &part| {
                sum.checked_add(actor_actions[part].as_u64()?)
            })
        };
        let actor_actions_valid = actor_actions["calls"].as_u64()
            == actor_targets["samples"]
                .as_u64()
                .and_then(|n| n.checked_mul(2))
            && actor_actions["calls"].as_u64() == action_sum(&["aligned_calls", "gated_calls"])
            && actor_actions["calls"].as_u64() == action_sum(&["updates", "noops", "errors"])
            && actor_actions["updates"] == actor_actions["motion_validated"]
            && actor_actions["angle_probes"].as_u64()
                == actor_targets["samples"]
                    .as_u64()
                    .and_then(|n| n.checked_mul(40))
            && actor_actions["angle_probes"].as_u64()
                == action_sum(&["angle_validated", "angle_ambiguous"])
            && actor_actions["angle_validated"]
                .as_u64()
                .is_some_and(|n| n > 0);
        let target_sum = |parts: &[&str]| {
            parts.iter().try_fold(0u64, |sum, &part| {
                sum.checked_add(actor_targets[part].as_u64()?)
            })
        };
        let actor_targets_valid = actor_targets["samples"].as_u64()
            == details["graphs"].as_u64().and_then(|n| n.checked_mul(2))
            && actor_targets["samples"].as_u64()
                == target_sum(&[
                    "initial_misses",
                    "height_rejections",
                    "connected_misses",
                    "prepared",
                ])
            && actor_targets["samples"].as_u64()
                == target_sum(&["validated_initial", "ambiguous_initial"])
            && actor_targets["prepared"].as_u64()
                == target_sum(&["paths", "misses", "unsupported_searches", "budget_searches"])
            && actor_targets["paths"].as_u64()
                == target_sum(&["validated_paths", "ambiguous_paths"])
            && target_sum(&["prepared", "connected_misses"])
                == target_sum(&["validated_connected", "ambiguous_connected"])
            && actor_targets["manager_edges"] == details["stats"]["actor_edges"]["edges"];
        let goal_partition = |total: &str, parts: &[&str]| {
            actor_goals[total].as_u64().is_some_and(|total| {
                parts.iter().try_fold(0u64, |sum, &part| {
                    sum.checked_add(actor_goals[part].as_u64()?)
                }) == Some(total)
            })
        };
        let actor_goals_valid = actor_goals["queries"] == details["graphs"]
            && goal_partition(
                "pairs",
                &["validated_pairs", "ambiguous_pairs", "unsupported_pairs"],
            )
            && goal_partition(
                "queries",
                &["paths", "misses", "unsupported_queries", "budget_queries"],
            )
            && goal_partition(
                "queries",
                &[
                    "validated_queries",
                    "ambiguous_queries",
                    "unsupported_queries",
                    "budget_queries",
                ],
            )
            && actor_goals["validated_pairs"]
                .as_u64()
                .is_some_and(|n| n > 0);
        details["passed"] = json!(
            output.status.success()
                && errors == 0
                && actor_goals_valid
                && actor_targets_valid
                && actor_actions_valid
                && details["graphs"].as_u64().is_some_and(|n| n > 0)
                && details["graphs"] == details["probed"]
                && details["stats"]["queries"].as_u64().is_some_and(|n| n > 0)
                && details["stats"]["found"] == details["stats"]["queries"]
                && details["stats"]["actor_edges"]["queries"] == details["graphs"]
                && details["stats"]["actor_edges"]["queries"].as_u64()
                    == details["stats"]["actor_edges"]["found"]
                        .as_u64()
                        .zip(details["stats"]["actor_edges"]["misses"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["stats"]["actor_edges"]["queries"].as_u64()
                    == details["stats"]["actor_edges"]["validated_queries"]
                        .as_u64()
                        .zip(details["stats"]["actor_edges"]["ambiguous_queries"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["stats"]["actor_edges"]["edges"].as_u64()
                    == details["stats"]["actor_edges"]["validated_edges"]
                        .as_u64()
                        .zip(details["stats"]["actor_edges"]["ambiguous_edges"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["stats"]["actor_edges"]["validated_edges"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["goals"]["queries"].as_u64()
                    == details["stats"]["queries"]
                        .as_u64()
                        .and_then(|n| n.checked_mul(2))
                && details["stats"]["goals"]["found"] == details["stats"]["goals"]["queries"]
                && details["stats"]["goals"]["goal_tests"] == details["stats"]["goals"]["expanded"]
                && details["stats"]["goals"]["heuristic_centers"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["goals"]["multiple_goal_queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["world_queries"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["world_queries"] == details["stats"]["world_validated"]
                && details["stats"]["world_goals"]["requests"] == details["stats"]["world_queries"]
                && details["stats"]["world_goals"]["wrapper_calls"].as_u64()
                    == details["stats"]["world_goals"]["requests"]
                        .as_u64()
                        .zip(details["stats"]["world_goals"]["found"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b.checked_mul(2)?))
                && details["stats"]["world_goals"]["requests"].as_u64()
                    == details["stats"]["world_goals"]["found"]
                        .as_u64()
                        .zip(details["stats"]["world_goals"]["start_misses"].as_u64())
                        .zip(details["stats"]["world_goals"]["search_misses"].as_u64())
                        .and_then(|((a, b), c)| a.checked_add(b)?.checked_add(c))
                && details["stats"]["world_goals"]["waypoint_paths"]
                    == details["stats"]["world_goals"]["found"]
                && details["stats"]["world_goals"]["retained_start_misses"]
                    == details["stats"]["world_goals"]["found"]
                && details["stats"]["world_goals"]["recovery_replacements"]
                    == details["stats"]["world_goals"]["found"]
                && details["stats"]["world_goals"]["portals_checked"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["world_paths_found"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["longest_chain"]
                    .as_u64()
                    .is_some_and(|n| n > 1)
                && details["stats"]["waypoint_queries"] == details["stats"]["world_paths_found"]
                && details["stats"]["waypoint_portals_checked"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["route_consumer_paths"] == details["stats"]["waypoint_queries"]
                && details["stats"]["regeneration"]["samples"]
                    == details["stats"]["waypoint_queries"]
                && details["stats"]["regeneration"]["calls"].as_u64()
                    == details["stats"]["regeneration"]["samples"]
                        .as_u64()
                        .and_then(|n| n.checked_mul(6))
                && details["stats"]["regeneration"]["retained_misses"].as_u64()
                    == details["stats"]["regeneration"]["samples"]
                        .as_u64()
                        .and_then(|n| n.checked_mul(3))
                && details["stats"]["regeneration"]["successes"].as_u64()
                    == details["stats"]["regeneration"]["samples"]
                        .as_u64()
                        .and_then(|n| n.checked_mul(3))
                && details["stats"]["regeneration"]["final_pair_replacements"]
                    == details["stats"]["regeneration"]["samples"]
                && details["stats"]["regeneration"]["miss_positions_checked"]
                    == details["stats"]["regeneration"]["samples"]
                && details["stats"]["route_arrival_validated"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["route_corridor_validated"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["route_corridor_queries"].as_u64()
                    == details["stats"]["route_corridor_validated"]
                        .as_u64()
                        .zip(details["stats"]["route_corridor_ambiguous"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["stats"]["actor_query_calls"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["actor_dispatch_calls"].as_u64()
                    == details["stats"]["world_queries"]
                        .as_u64()
                        .and_then(|n| n.checked_mul(2))
                && details["stats"]["actor_dispatch_calls"].as_u64()
                    == details["stats"]["actor_acquisition_calls"]
                        .as_u64()
                        .zip(details["stats"]["actor_query_calls"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["stats"]["boundary"]["caches"] == details["graphs"]
                && details["stats"]["boundary"]["edges"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["boundary"]["topology_faces"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["boundary"]["validated"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["boundary"]["queries"].as_u64()
                    == details["stats"]["boundary"]["validated"]
                        .as_u64()
                        .zip(details["stats"]["boundary"]["ambiguous"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["stats"]["boundary"]["actor_validated"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["stats"]["boundary"]["actor_samples"].as_u64()
                    == details["stats"]["boundary"]["actor_validated"]
                        .as_u64()
                        .zip(details["stats"]["boundary"]["actor_ambiguous"].as_u64())
                        .and_then(|(a, b)| a.checked_add(b))
                && details["reference_compared"] == false
        );
        println!(
            "Path smoke: {}/{} navmeshes; {} sampled routes; {errors} errors. {}",
            details["probed"],
            details["graphs"],
            details["stats"]["queries"],
            path.display()
        );
        Ok(details)
    })();
    let (passed, details) = match result {
        Ok(d) => (d["passed"] == true, d),
        Err(error) => {
            eprintln!("Path audit failed: {error:#}");
            (
                false,
                json!({"message":"Path audit failed; inspect local terminal output."}),
            )
        }
    };
    record("paths", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "path smoke failed");
    Ok(())
}

fn viewer_smoke(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    let result = (|| -> Result<Value> {
        cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
        let folder =
            Path::new("local/viewer-smoke").join(format!("{}-{}", now(), std::process::id()));
        fs::create_dir_all(&folder)?;
        let png = folder.join("native-window.png");
        let disc = disc.to_str().context("disc path must be UTF-8")?;
        let raw = output(
            &executable("release"),
            &[
                "view-map",
                disc,
                "m_dam_gfx.edb",
                "--placements",
                "--eye",
                "-252",
                "7",
                "-8",
                "--target",
                "-270",
                "3",
                "-27",
                "--frames",
                "3",
                "--hidden",
                "--smoke-resize",
                "--capture",
                png.to_str().context("capture path")?,
            ],
        )?;
        let mut details: Value = serde_json::from_slice(&raw)?;
        write_json(&folder.join("native-window.json"), &details)?;
        ensure!(
            details["allow_incomplete"] == false
                && details["placements_loaded"] == 241
                && details["render_placements"] == 240
                && details["navigation_placements"] == 1
                && details["placements_not_loaded"] == 0
                && details["nav_meshes"] == 1,
            "native window did not preserve strict render/navigation placement decoding"
        );
        ensure!(
            details["window"]["hidden"] == true
                && details["window"]["frames_presented"] == 3
                && details["window"]["scene_uploads"] == 1
                && details["window"]["resize_count"]
                    .as_u64()
                    .is_some_and(|n| n > 0)
                && details["window"]["final_size"] == json!([960, 540])
                && details["window"]["capture_non_background_pixels"]
                    .as_u64()
                    .is_some_and(|n| n > 0),
            "native window/resize/capture checks failed"
        );
        ensure!(fs::metadata(&png)?.len() > 0, "native capture PNG missing");
        let reference = folder.join("offscreen.png");
        let raw = output(
            &executable("release"),
            &[
                "render-map",
                disc,
                "m_dam_gfx.edb",
                reference.to_str().context("reference path")?,
                "--placements",
                "--eye",
                "-252",
                "7",
                "-8",
                "--target",
                "-270",
                "3",
                "-27",
                "--width",
                "960",
                "--height",
                "540",
            ],
        )?;
        let offscreen: Value = serde_json::from_slice(&raw)?;
        write_json(&folder.join("offscreen.json"), &offscreen)?;
        let capture_hash = details["window"]["capture_rgba_sha256"]
            .as_str()
            .context("missing surface capture hash")?;
        ensure!(
            capture_hash.len() == 64 && offscreen["rgba_sha256"] == capture_hash,
            "native surface pixels differ from the shared offscreen path"
        );
        details["offscreen_rgba_sha256"] = offscreen["rgba_sha256"].clone();
        details["surface_capture_matches_offscreen"] = json!(true);
        println!(
            "Native window: 3 frames, 1 scene upload, 960x540 after resize; pixels match offscreen. {}",
            png.display()
        );
        Ok(details)
    })();
    let (passed, details) = match result {
        Ok(details) => (true, details),
        Err(error) => {
            eprintln!("Native window smoke failed: {error:#}");
            (
                false,
                json!({"message":"Native window smoke failed; inspect local terminal output."}),
            )
        }
    };
    record("viewer", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "native window smoke failed");
    Ok(())
}

fn render_smoke(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    let result = (|| -> Result<Value> {
        println!("Running the explicit native GPU regression fixture");
        let fixture = String::from_utf8(output(
            Path::new("cargo"),
            &[
                "test",
                "-p",
                "ge-render",
                "native_texture_depth_alpha_and_unaligned_readback",
                "--locked",
                "--",
                "--ignored",
                "--nocapture",
            ],
        )?)?;
        print!("{fixture}");
        ensure!(
            fixture.contains("native_texture_depth_alpha_and_unaligned_readback")
                && fixture.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "native GPU test did not report exactly one passing test"
        );
        cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
        let folder =
            Path::new("local/render-smoke").join(format!("{}-{}", now(), std::process::id()));
        fs::create_dir_all(&folder)?;
        let strict = Command::new(executable("release"))
            .args([
                "scene",
                disc.to_str().context("disc path must be UTF-8")?,
                "m_dam_gfx.edb",
                "--placements",
            ])
            .output()?;
        fs::write(folder.join("strict-scene.json"), &strict.stdout)?;
        fs::write(folder.join("strict-scene-error.txt"), &strict.stderr)?;
        ensure!(
            strict.status.success(),
            "strict Dam placement decoding failed: {}",
            String::from_utf8_lossy(&strict.stderr)
        );
        let strict_summary: Value = serde_json::from_slice(&strict.stdout)?;
        ensure!(
            strict_summary["placements_loaded"] == 241
                && strict_summary["render_placements"] == 240
                && strict_summary["navigation_placements"] == 1
                && strict_summary["placements_not_loaded"] == 0,
            "strict Dam render/navigation placement counts changed"
        );
        let png = folder.join("dam-zone0.png");
        let result = output(
            &executable("release"),
            &[
                "render-map",
                disc.to_str().context("disc path must be UTF-8")?,
                "m_dam_gfx.edb",
                png.to_str().context("render path must be UTF-8")?,
                "--placements",
                "--eye",
                "-252",
                "7",
                "-8",
                "--target",
                "-270",
                "3",
                "-27",
            ],
        )?;
        let mut details: Value = serde_json::from_slice(&result)?;
        write_json(&folder.join("render.json"), &details)?;
        // Public summaries contain counts/types, not original placement records or poses.
        let mut unsupported = std::collections::BTreeMap::<String, usize>::new();
        for p in details["unsupported_placements"]
            .as_array()
            .context("missing placement outcomes")?
        {
            let kind = p["status"]["entity_type"]
                .as_u64()
                .context("missing unsupported entity type")?;
            *unsupported.entry(format!("0x{kind:x}")).or_default() += 1;
        }
        details
            .as_object_mut()
            .context("render summary")?
            .remove("unsupported_placements");
        details["unsupported_entity_types"] = json!(unsupported);
        details["strict_placement_decode_passed"] = json!(strict.status.success());
        ensure!(
            details["render"]["triangles"]
                .as_u64()
                .is_some_and(|n| n > 0)
                && details["render"]["non_background_pixels"]
                    .as_u64()
                    .is_some_and(|n| n > 0),
            "render produced no geometry pixels"
        );
        ensure!(fs::metadata(&png)?.len() > 0, "render PNG missing");
        details["native_gpu_fixture_passed"] = json!(true);
        println!(
            "Native map inspection: {} submitted triangles, {} draws on {} ({}); {}",
            details["render"]["triangles"],
            details["render"]["draws"],
            details["render"]["adapter"],
            details["render"]["backend"],
            png.display()
        );
        Ok(details)
    })();
    let (passed, details) = match result {
        Ok(details) => (true, details),
        Err(error) => {
            eprintln!("Native render smoke failed: {error:#}");
            (
                false,
                json!({"message":"Native render smoke failed; inspect local terminal output."}),
            )
        }
    };
    record("render", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "native render smoke failed");
    Ok(())
}

fn smoke(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    cargo(&["build", "-p", "goldeneye", "--locked"])?;
    let result = asset_smoke(disc);
    let (passed, details) = match result {
        Ok(details) => (true, details),
        Err(error) => {
            eprintln!("Asset smoke failed: {error:#}");
            (
                false,
                json!({"message": "Asset smoke failed; inspect local terminal output."}),
            )
        }
    };
    record("assets", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "local asset smoke failed");
    Ok(())
}

fn asset_smoke(disc: &Path) -> Result<Value> {
    let path = disc.to_str().context("disc path must be UTF-8")?;
    let run = Path::new("local/smoke").join(format!("{}-{}", now(), std::process::id()));
    fs::create_dir_all(&run)?;
    let inventory_bytes = output(&executable("debug"), &["inspect", path])?;
    let inventory: Value = serde_json::from_slice(&inventory_bytes)?;
    fs::write(run.join("disc.json"), &inventory_bytes)?;
    let mut indexes = Vec::new();
    let mut entries = 0;
    for name in [
        "Filelist.bin",
        "File_COM.bin",
        "File_USA.bin",
        "File_FRE.bin",
        "File_SPA.bin",
    ] {
        let bytes = output(&executable("debug"), &["archive", path, name])?;
        let index: Value = serde_json::from_slice(&bytes)?;
        let count = index["entries"]
            .as_array()
            .context("missing entries")?
            .len();
        ensure!(count > 0, "empty index: {name}");
        fs::write(run.join(format!("{name}.json")), bytes)?;
        entries += count;
        indexes.push(json!({"index": name, "entries": count, "base_ranges_valid": true}));
    }
    println!(
        "Local asset smoke: {} files, {entries} entries across {} indexes",
        inventory["file_count"],
        indexes.len()
    );
    Ok(
        json!({"disc": inventory["disc"], "file_count": inventory["file_count"], "file_bytes": inventory["file_bytes"], "entries": entries, "indexes": indexes,
        "scope": "Disc identity, FST, v7 indexes and indexed base ranges only; no gameplay or full EDB validation."}),
    )
}

fn notices() -> Result<()> {
    let rustc = String::from_utf8(output(Path::new("rustc"), &["-vV"])?)?;
    let host = rustc
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .context("rustc host triple")?;
    let metadata: Value = serde_json::from_slice(&output(
        Path::new("cargo"),
        &[
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            host,
        ],
    )?)?;
    let mut text = format!(
        "# Third-party notices\n\nGenerated from Cargo.lock for {host}. Includes transitive and build dependencies for this host; no original game code.\n\n",
    );
    let mut packages: Vec<&Value> = metadata["packages"]
        .as_array()
        .context("packages")?
        .iter()
        .filter(|p| !p["source"].is_null())
        .collect();
    packages.sort_by_key(|p| p["name"].as_str().unwrap_or(""));
    for p in packages {
        text.push_str(&format!(
            "## {} {}\n\nDeclared license: {}\n\n",
            p["name"].as_str().unwrap_or("?"),
            p["version"].as_str().unwrap_or("?"),
            p["license"].as_str().unwrap_or("see upstream")
        ));
        let root = Path::new(p["manifest_path"].as_str().context("manifest path")?)
            .parent()
            .context("crate root")?;
        let mut license_paths = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
            if entry.file_type()?.is_file()
                && (name.starts_with("LICENSE")
                    || name.starts_with("COPYING")
                    || name.starts_with("NOTICE"))
            {
                license_paths.push(entry.path());
            }
            if entry.file_type()?.is_dir() && name == "LICENSES" {
                collect(&entry.path(), &mut license_paths)?;
            }
        }
        if let Some(file) = p["license_file"].as_str() {
            let path = root.join(file);
            if path.is_file() {
                license_paths.push(path);
            }
        }
        license_paths.sort();
        license_paths.dedup();
        // Some upstream crates omit workspace-root licenses from their published crate.
        if license_paths.is_empty() {
            let override_root = Path::new("third-party").join(format!(
                "{}-{}",
                p["name"].as_str().context("package name")?,
                p["version"].as_str().context("package version")?
            ));
            collect(&override_root, &mut license_paths)?;
        }
        ensure!(
            !license_paths.is_empty(),
            "no license text found for {}; inspect upstream before packaging",
            p["name"]
        );
        for path in license_paths {
            text.push_str(&format!(
                "### {}\n\n```text\n{}\n```\n\n",
                path.file_name().unwrap_or_default().to_string_lossy(),
                fs::read_to_string(&path)?
            ));
        }
    }
    fs::write("THIRD_PARTY_NOTICES.md", text)?;
    println!("Generated THIRD_PARTY_NOTICES.md");
    Ok(())
}

fn package() -> Result<()> {
    cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
    notices()?;
    let stage = Path::new("dist").join(format!("goldeneye-{}-{}", now(), std::process::id()));
    fs::create_dir_all("dist")?;
    fs::create_dir(&stage)?;
    let files = [
        (
            executable("release"),
            format!("goldeneye{}", env::consts::EXE_SUFFIX),
        ),
        ("docs/package-readme.md".into(), "README.md".into()),
        ("LICENSE".into(), "LICENSE".into()),
        (
            "THIRD_PARTY_NOTICES.md".into(),
            "THIRD_PARTY_NOTICES.md".into(),
        ),
    ];
    let mut sums = String::new();
    for (source, name) in files {
        let bytes = fs::read(source)?;
        sums.push_str(&format!("{:x}  {name}\n", Sha256::digest(&bytes)));
        fs::write(stage.join(name), bytes)?;
    }
    fs::write(stage.join("SHA256SUMS"), sums)?;
    println!(
        "Staged asset-free tools build: {} (gameplay is not implemented)",
        stage.display()
    );
    Ok(())
}
