use super::*;
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
struct Fixture {
    root: PathBuf,
    cases: PathBuf,
    reference: PathBuf,
    candidate: PathBuf,
    exe: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ge-parity-{}-{}-{stamp}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir_all(root.join("local/parity")).unwrap();
        fs::create_dir(root.join("progress")).unwrap();
        let f = Self {
            cases: root.join("progress/parity-cases.json"),
            reference: root.join("local/parity/reference.json"),
            candidate: root.join("local/parity/candidate.json"),
            exe: root.join("local/parity/candidate.exe"),
            root,
        };
        fs::write(
            f.root.join("local/parity/reference.exe"),
            b"authored original-provider fixture",
        )
        .unwrap();
        fs::write(&f.exe, b"authored candidate-provider fixture").unwrap();
        fs::write(
            f.root.join("local/parity/reference-config.json"),
            b"{\"backend\":\"authored-reference\"}",
        )
        .unwrap();
        fs::write(
            f.root.join("local/parity/candidate-config.json"),
            b"{\"backend\":\"authored-candidate\"}",
        )
        .unwrap();
        fs::write(
            f.root.join("local/parity/inputs.json"),
            b"{\"semantic_input\":\"no_buttons\"}",
        )
        .unwrap();
        let pixels = [180, 190, 200, 255, 200, 210, 220, 255];
        f.png("reference.png", 2, 1, &pixels);
        f.png("candidate.png", 2, 1, &pixels);
        f.write(&f.cases,&json!({"schema_version":1,"cases":[{"id":"frontend.menu.frame","title":"Authored menu frame","kind":"image_set",
            "samples":[{"id":"main","width":2,"height":1,"roi":null,"minimum_nonblack_fraction":0.02,"minimum_spatial_range":16}],
            "tolerances":{"max_mae":1.0,"bad_pixel_delta":4,"max_bad_pixel_fraction":0.01}}]}));
        for (role, path) in [("reference", &f.reference), ("candidate", &f.candidate)] {
            f.write(path,&json!({"schema_version":1,"role":role,"case_set_sha256":receipt::digest(&fs::read(&f.cases).unwrap()),
                "disc":{"game_id":"SJBE52","revision":0,"image_sha256":"b".repeat(64)},
                "producer":{"kind":if role=="reference"{"dolphin"}else{"goldeneye_rust"},"version":"synthetic-test-only",
                    "executable":f.artifact(&format!("{role}.exe"))},
                "configuration":f.artifact(&format!("{role}-config.json")),"input_contract":f.artifact("inputs.json"),
                "source_hash":if role=="reference"{Value::Null}else{json!(SOURCE)},
                "captures":[{"case_id":"frontend.menu.frame","sample_id":"main","image":f.artifact(&format!("{role}.png"))}]}));
        }
        f
    }
    fn write(&self, path: &Path, value: &Value) {
        fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
    fn read(&self, path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }
    fn artifact(&self, file: &str) -> Value {
        let path = format!("local/parity/{file}");
        let sha256 = receipt::digest(&fs::read(self.root.join(&path)).unwrap());
        json!({"path":path,"sha256":sha256})
    }
    fn png(&self, file: &str, width: u32, height: u32, pixels: &[u8]) {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
        fs::write(self.root.join("local/parity").join(file), bytes).unwrap();
    }
    fn refresh_image(&self, role: &str) {
        let path = if role == "reference" {
            &self.reference
        } else {
            &self.candidate
        };
        let mut p = self.read(path);
        p["captures"][0]["image"] = self.artifact(&format!("{role}.png"));
        self.write(path, &p);
    }
    fn refresh_cases(&self) {
        for path in [&self.reference, &self.candidate] {
            let mut p = self.read(path);
            p["case_set_sha256"] = json!(receipt::digest(&fs::read(&self.cases).unwrap()));
            self.write(path, &p);
        }
    }
    fn compare(&self) -> Summary {
        compare(
            &self.root,
            &self.cases,
            &self.reference,
            &self.candidate,
            SOURCE,
            &self.exe,
        )
        .unwrap()
    }
    fn current(&self) -> Summary {
        current(&self.root, &self.cases, SOURCE, &self.exe).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the unique directory created by this fixture is eligible.
        if self.root.parent() == Some(std::env::temp_dir().as_path())
            && self
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ge-parity-")
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
#[test]
fn no_run_keeps_denominator_and_real_comparison_computes_metrics() {
    let f = Fixture::new();
    let empty = f.current();
    assert_eq!(
        (empty.state.as_str(), empty.verified, empty.total),
        ("not_run", 0, 1)
    );
    let pass = f.compare();
    assert_eq!(
        (pass.state.as_str(), pass.verified, pass.total, pass.percent),
        ("passed", 1, 1, Some(100.0))
    );
    let metric = pass.cases[0].samples[0].metrics.as_ref().unwrap();
    assert_eq!(metric.mae, 0.0);
    assert_eq!(metric.pixels, 2);
    assert_eq!(metric.reference_nonblack_fraction, 1.0);
    assert_eq!(metric.reference_spatial_range, 20);
    assert_eq!(metric.candidate_spatial_range, 20);
    assert_eq!(f.current().run_id, pass.run_id);
    let public = serde_json::to_string(&pass).unwrap();
    assert!(!public.contains("local/parity"));
    assert!(!public.contains(&f.root.to_string_lossy().to_string()));
}
#[test]
fn latest_failed_comparison_never_reuses_older_pass_and_history_file_survives() {
    let f = Fixture::new();
    let pass = f.compare();
    let old = f
        .root
        .join("local/parity/runs")
        .join(format!("{}.json", pass.run_id.as_deref().unwrap()));
    f.png("candidate.png", 2, 1, &[255, 0, 0, 255, 255, 0, 0, 255]);
    f.refresh_image("candidate");
    let fail = f.compare();
    assert_eq!((fail.state.as_str(), fail.verified), ("failed", 0));
    assert!(old.exists());
    assert_eq!(f.current().state, "failed");
    assert_ne!(f.current().run_id, pass.run_id);
}
#[test]
fn blank_reference_or_candidate_never_passes_even_when_identical() {
    for role in ["reference", "candidate", "both"] {
        let f = Fixture::new();
        for side in ["reference", "candidate"] {
            if role == side || role == "both" {
                f.png(&format!("{side}.png"), 2, 1, &[0, 0, 0, 255, 0, 0, 0, 255]);
                f.refresh_image(side);
            }
        }
        let s = f.compare();
        assert_eq!(s.verified, 0);
        assert_eq!(s.state, "failed");
    }
}
#[test]
fn each_real_artifact_source_and_case_spec_participates_in_freshness() {
    for file in [
        "reference.png",
        "candidate.png",
        "reference-config.json",
        "candidate-config.json",
        "inputs.json",
        "reference.exe",
        "candidate.exe",
        "reference.json",
        "candidate.json",
    ] {
        let f = Fixture::new();
        f.compare();
        let path = f.root.join("local/parity").join(file);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        fs::write(path, bytes).unwrap();
        let s = f.current();
        assert_eq!(s.verified, 0, "accepted changed {file}");
        assert_eq!(s.state, "stale");
    }
    let f = Fixture::new();
    f.compare();
    let s = current(&f.root, &f.cases, &"c".repeat(64), &f.exe).unwrap();
    assert_eq!(s.reason.as_deref(), Some("stale_source"));
    assert_eq!(s.verified, 0);
    let mut cases = f.read(&f.cases);
    cases["cases"][0]["tolerances"]["max_mae"] = json!(2);
    f.write(&f.cases, &cases);
    let s = f.current();
    assert_eq!(s.reason.as_deref(), Some("stale_case_set"));
    assert_eq!(s.verified, 0);
}
#[test]
fn changed_case_denominator_is_current_while_prior_observation_stays_private_history() {
    let f = Fixture::new();
    let old = f.compare();
    let mut cases = f.read(&f.cases);
    let mut second = cases["cases"][0].clone();
    second["id"] = json!("frontend.other.frame");
    cases["cases"].as_array_mut().unwrap().push(second);
    f.write(&f.cases, &cases);
    let s = f.current();
    assert_eq!((s.total, s.verified), (2, 0));
    assert_eq!(s.measured_state.as_deref(), Some("passed"));
    assert_eq!(s.run_id, old.run_id);
}
#[test]
fn wrong_roles_disc_hashes_inputs_and_aliases_never_verify() {
    for mode in [
        "role",
        "disc",
        "disc_hash",
        "inputs",
        "producer",
        "source",
        "alias",
        "hash",
        "manual_pass",
    ] {
        let f = Fixture::new();
        let mut p = f.read(&f.candidate);
        match mode {
            "role" => p["role"] = json!("reference"),
            "disc" => p["disc"]["game_id"] = json!("WRONG"),
            "disc_hash" => p["disc"]["image_sha256"] = json!("f".repeat(64)),
            "inputs" => p["input_contract"] = f.artifact("candidate-config.json"),
            "producer" => p["producer"]["kind"] = json!("dolphin"),
            "source" => p["source_hash"] = json!("f".repeat(64)),
            "alias" => p["captures"][0]["image"] = f.artifact("reference.png"),
            "hash" => p["captures"][0]["image"]["sha256"] = json!("f".repeat(64)),
            "manual_pass" => p["passed"] = json!(true),
            _ => unreachable!(),
        }
        f.write(&f.candidate, &p);
        assert_eq!(f.compare().verified, 0, "accepted {mode}");
    }
}
#[test]
fn missing_required_sample_and_changed_dimensions_fail_without_partial_verification() {
    let f = Fixture::new();
    let mut p = f.read(&f.candidate);
    p["captures"] = json!([]);
    f.write(&f.candidate, &p);
    let s = f.compare();
    assert_eq!((s.state.as_str(), s.verified, s.total), ("missing", 0, 1));
    let f = Fixture::new();
    f.png("candidate.png", 1, 1, &[180, 190, 200, 255]);
    f.refresh_image("candidate");
    assert_eq!(f.compare().state, "failed");
    let f = Fixture::new();
    fs::remove_file(f.root.join("local/parity/reference.png")).unwrap();
    assert_eq!(f.compare().state, "missing");
}
#[test]
fn missing_case_keeps_complete_case_measured_and_requires_every_sample() {
    for role in ["reference", "candidate"] {
        let f = Fixture::new();
        let mut cases = f.read(&f.cases);
        let mut second = cases["cases"][0].clone();
        second["id"] = json!("frontend.other.frame");
        cases["cases"].as_array_mut().unwrap().push(second);
        f.write(&f.cases, &cases);
        f.refresh_cases();
        let result = f.compare();
        assert_eq!(
            (
                result.state.as_str(),
                result.verified,
                result.total,
                result.percent
            ),
            ("failed", 1, 2, Some(50.0))
        );
        assert_eq!(result.cases[0].state, "passed");
        assert!(result.cases[0].samples[0].metrics.is_some());
        assert_eq!(result.cases[1].state, "missing");
        assert_eq!(
            result.cases[1].samples[0].reason.as_deref(),
            Some("missing_reference_capture")
        );

        // A second missing sample in the otherwise complete case removes its pass.
        let mut cases = f.read(&f.cases);
        let mut extra = cases["cases"][0]["samples"][0].clone();
        extra["id"] = json!("extra");
        cases["cases"][0]["samples"]
            .as_array_mut()
            .unwrap()
            .push(extra);
        f.write(&f.cases, &cases);
        f.refresh_cases();
        let path = if role == "reference" {
            &f.candidate
        } else {
            &f.reference
        };
        let mut pack = f.read(path);
        let mut capture = pack["captures"][0].clone();
        capture["sample_id"] = json!("extra");
        let image_name = format!("{role}-extra.png");
        fs::copy(
            f.root.join("local/parity/candidate.png"),
            f.root.join("local/parity").join(&image_name),
        )
        .unwrap();
        capture["image"] = f.artifact(&image_name);
        pack["captures"].as_array_mut().unwrap().push(capture);
        f.write(path, &pack);
        let result = f.compare();
        assert_eq!(result.verified, 0);
        assert_eq!(result.cases[0].passed_samples, 1);
        assert_eq!(result.cases[0].total_samples, 2);
        assert_eq!(result.cases[0].state, "missing");
        assert_eq!(
            result.cases[0].samples[1].reason,
            Some(format!("missing_{role}_capture"))
        );
    }
}

#[test]
fn undeclared_capture_is_invalid_even_when_all_required_samples_are_present() {
    let f = Fixture::new();
    let mut pack = f.read(&f.candidate);
    pack["captures"][0]["sample_id"] = json!("undeclared");
    f.write(&f.candidate, &pack);
    let result = f.compare();
    assert_eq!(result.verified, 0);
    assert_eq!(result.reason.as_deref(), Some("unknown_capture_identity"));
}
#[test]
fn unpaired_provided_images_are_validated_and_remain_freshness_dependencies() {
    for role in ["reference", "candidate"] {
        let f = Fixture::new();
        let mut cases = f.read(&f.cases);
        let mut second = cases["cases"][0].clone();
        second["id"] = json!("frontend.other.frame");
        cases["cases"].as_array_mut().unwrap().push(second);
        f.write(&f.cases, &cases);
        f.refresh_cases();
        let path = if role == "reference" {
            &f.reference
        } else {
            &f.candidate
        };
        let mut pack = f.read(path);
        let mut capture = pack["captures"][0].clone();
        capture["case_id"] = json!("frontend.other.frame");
        let image = f.root.join("local/parity/unpaired.png");
        fs::copy(f.root.join("local/parity/candidate.png"), &image).unwrap();
        capture["image"] = f.artifact("unpaired.png");
        pack["captures"].as_array_mut().unwrap().push(capture);
        f.write(path, &pack);
        assert_eq!(f.compare().verified, 1);
        fs::write(&image, b"changed image").unwrap();
        assert_eq!(f.current().state, "stale");
        let result = f.compare();
        assert_eq!(result.state, "stale");
        assert_eq!(result.verified, 0);
        assert_eq!(result.reason.as_deref(), Some("artifact_hash_mismatch"));
    }
}
#[test]
fn roi_is_predeclared_and_never_auto_aligned_or_resampled() {
    let f = Fixture::new();
    let mut cases = f.read(&f.cases);
    cases["cases"][0]["samples"][0]["width"] = json!(3);
    cases["cases"][0]["samples"][0]["roi"] = json!({"x":0,"y":0,"width":2,"height":1});
    f.write(&f.cases, &cases);
    f.refresh_cases();
    f.png(
        "reference.png",
        3,
        1,
        &[180, 190, 200, 255, 200, 210, 220, 255, 50, 60, 70, 255],
    );
    f.png(
        "candidate.png",
        3,
        1,
        &[180, 190, 200, 255, 200, 210, 220, 255, 0, 0, 0, 255],
    );
    f.refresh_image("reference");
    f.refresh_image("candidate");
    let result = f.compare();
    assert_eq!(result.verified, 1);
    let metrics = result.cases[0].samples[0].metrics.as_ref().unwrap();
    assert_eq!(
        (
            metrics.reference_spatial_range,
            metrics.candidate_spatial_range
        ),
        (20, 20)
    );
    let mut cases = f.read(&f.cases);
    cases["cases"][0]["samples"][0]["roi"]["width"] = json!(4);
    f.write(&f.cases, &cases);
    assert!(
        compare(
            &f.root,
            &f.cases,
            &f.reference,
            &f.candidate,
            SOURCE,
            &f.exe
        )
        .is_err()
    );
}
#[test]
fn unsupported_kind_or_png_and_malformed_or_empty_case_set_never_pass() {
    let f = Fixture::new();
    let mut cases = f.read(&f.cases);
    cases["cases"][0]["kind"] = json!("audio");
    f.write(&f.cases, &cases);
    f.refresh_cases();
    let s = f.compare();
    assert_eq!((s.state.as_str(), s.verified), ("unsupported", 0));
    let f = Fixture::new();
    let mut bytes = Vec::new();
    let mut e = png::Encoder::new(&mut bytes, 2, 1);
    e.set_color(png::ColorType::Grayscale);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&[180, 190])
        .unwrap();
    fs::write(f.root.join("local/parity/candidate.png"), bytes).unwrap();
    f.refresh_image("candidate");
    assert_eq!(f.compare().state, "unsupported");
    for change in ["empty", "duplicate", "too_large", "threshold", "unknown"] {
        let f = Fixture::new();
        let mut cases = f.read(&f.cases);
        match change {
            "empty" => cases["cases"] = json!([]),
            "duplicate" => {
                let c = cases["cases"][0].clone();
                cases["cases"].as_array_mut().unwrap().push(c);
            }
            "too_large" => cases["cases"][0]["samples"][0]["width"] = json!(4097),
            "threshold" => {
                cases["cases"][0]["samples"][0]["minimum_nonblack_fraction"] = json!(-0.1)
            }
            "unknown" => cases["verified"] = json!(true),
            _ => unreachable!(),
        }
        f.write(&f.cases, &cases);
        assert!(
            current(&f.root, &f.cases, SOURCE, &f.exe).is_err(),
            "accepted {change}"
        );
    }
}
#[test]
fn immutable_receipt_tampering_missing_artifacts_and_path_escape_cannot_reuse_pass() {
    let f = Fixture::new();
    let s = f.compare();
    let path = f
        .root
        .join("local/parity/runs")
        .join(format!("{}.json", s.run_id.unwrap()));
    let mut r = f.read(&path);
    r["summary"]["verified"] = json!(128);
    f.write(&path, &r);
    assert_eq!(f.current().state, "invalid");
    let f = Fixture::new();
    f.compare();
    fs::remove_file(f.root.join("local/parity/reference-config.json")).unwrap();
    let s = f.current();
    assert_eq!((s.state.as_str(), s.verified), ("missing", 0));
    let f = Fixture::new();
    let mut p = f.read(&f.candidate);
    p["captures"][0]["image"]["path"] = json!("local/parity/../parity/candidate.png");
    f.write(&f.candidate, &p);
    assert_eq!(f.compare().state, "invalid");
}
#[test]
fn every_required_sample_must_pass_and_reusing_an_anchor_file_is_invalid() {
    let f = Fixture::new();
    let mut cases = f.read(&f.cases);
    let mut second = cases["cases"][0]["samples"][0].clone();
    second["id"] = json!("second");
    cases["cases"][0]["samples"]
        .as_array_mut()
        .unwrap()
        .push(second);
    f.write(&f.cases, &cases);
    f.refresh_cases();
    f.png(
        "reference-second.png",
        2,
        1,
        &[180, 190, 200, 255, 200, 210, 220, 255],
    );
    f.png("candidate-second.png", 2, 1, &[0, 0, 0, 255, 0, 0, 0, 255]);
    for (role, path) in [("reference", &f.reference), ("candidate", &f.candidate)] {
        let mut p = f.read(path);
        p["captures"].as_array_mut().unwrap().push(json!({"case_id":"frontend.menu.frame","sample_id":"second","image":f.artifact(&format!("{role}-second.png"))}));
        f.write(path, &p);
    }
    let result = f.compare();
    assert_eq!(
        (
            result.verified,
            result.cases[0].passed_samples,
            result.cases[0].total_samples
        ),
        (0, 1, 2)
    );
    let mut p = f.read(&f.candidate);
    p["captures"][1]["image"] = p["captures"][0]["image"].clone();
    f.write(&f.candidate, &p);
    assert_eq!(f.compare().state, "invalid");
}
#[test]
fn rgb_capture_normalizes_only_opaque_alpha_and_transparent_blank_is_rejected() {
    let f = Fixture::new();
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[180, 190, 200, 200, 210, 220])
        .unwrap();
    fs::write(f.root.join("local/parity/reference.png"), bytes).unwrap();
    f.refresh_image("reference");
    assert_eq!(f.compare().verified, 1);
    for role in ["reference", "candidate"] {
        f.png(
            &format!("{role}.png"),
            2,
            1,
            &[255, 255, 255, 0, 255, 255, 255, 0],
        );
        f.refresh_image(role);
    }
    let result = f.compare();
    assert_eq!(result.verified, 0);
    let metrics = result.cases[0].samples[0].metrics.as_ref().unwrap();
    assert_eq!(metrics.mae, 0.0);
    assert_eq!(metrics.reference_nonblack_fraction, 0.0);
}
#[test]
fn identical_flat_white_red_black_and_transparent_captures_are_not_appearance() {
    for pixel in [
        [255, 255, 255, 255],
        [255, 0, 0, 255],
        [0, 0, 0, 255],
        [255, 255, 255, 0],
    ] {
        let f = Fixture::new();
        let pixels = [pixel, pixel].concat();
        for role in ["reference", "candidate"] {
            f.png(&format!("{role}.png"), 2, 1, &pixels);
            f.refresh_image(role);
        }
        let result = f.compare();
        assert_eq!((result.state.as_str(), result.verified), ("failed", 0));
        let metrics = result.cases[0].samples[0].metrics.as_ref().unwrap();
        assert_eq!(metrics.mae, 0.0);
        assert_eq!(metrics.bad_pixel_fraction, 0.0);
        assert_eq!(
            (
                metrics.reference_spatial_range,
                metrics.candidate_spatial_range
            ),
            (0, 0)
        );
    }
}
#[test]
fn spatial_signal_is_required_independently_for_reference_and_candidate() {
    for flat_role in ["reference", "candidate"] {
        let f = Fixture::new();
        let mut cases = f.read(&f.cases);
        // Pixel differences are permitted here to isolate each producer's
        // independent appearance gate from ordinary comparison failures.
        cases["cases"][0]["tolerances"]["max_mae"] = json!(255);
        cases["cases"][0]["tolerances"]["max_bad_pixel_fraction"] = json!(1);
        f.write(&f.cases, &cases);
        f.refresh_cases();
        f.png(
            &format!("{flat_role}.png"),
            2,
            1,
            &[255, 0, 0, 255, 255, 0, 0, 255],
        );
        f.refresh_image(flat_role);
        let result = f.compare();
        assert_eq!(result.verified, 0);
        let metrics = result.cases[0].samples[0].metrics.as_ref().unwrap();
        assert_eq!(
            (
                metrics.reference_nonblack_fraction,
                metrics.candidate_nonblack_fraction
            ),
            (1.0, 1.0)
        );
        assert_eq!(
            (
                metrics.reference_spatial_range,
                metrics.candidate_spatial_range
            ),
            if flat_role == "reference" {
                (0, 20)
            } else {
                (20, 0)
            }
        );
    }
}
#[test]
fn spatial_threshold_uses_one_channels_range_and_is_part_of_freshness() {
    let f = Fixture::new();
    for role in ["reference", "candidate"] {
        f.png(
            &format!("{role}.png"),
            2,
            1,
            &[140, 150, 100, 255, 140, 150, 116, 255],
        );
        f.refresh_image(role);
    }
    let result = f.compare();
    assert_eq!(result.verified, 1);
    let metrics = result.cases[0].samples[0].metrics.as_ref().unwrap();
    assert_eq!(
        (
            metrics.reference_spatial_range,
            metrics.candidate_spatial_range
        ),
        (16, 16)
    );
    let mut cases = f.read(&f.cases);
    cases["cases"][0]["samples"][0]["minimum_spatial_range"] = json!(17);
    f.write(&f.cases, &cases);
    let stale = f.current();
    assert_eq!((stale.state.as_str(), stale.verified), ("stale", 0));
    assert_eq!(stale.reason.as_deref(), Some("stale_case_set"));
    f.refresh_cases();
    assert_eq!(f.compare().verified, 0);
}
#[test]
fn variation_outside_the_declared_roi_is_not_spatial_signal() {
    let f = Fixture::new();
    let mut cases = f.read(&f.cases);
    cases["cases"][0]["samples"][0]["width"] = json!(3);
    cases["cases"][0]["samples"][0]["roi"] = json!({"x":0,"y":0,"width":2,"height":1});
    f.write(&f.cases, &cases);
    f.refresh_cases();
    for role in ["reference", "candidate"] {
        f.png(
            &format!("{role}.png"),
            3,
            1,
            &[180, 190, 200, 255, 180, 190, 200, 255, 0, 255, 0, 255],
        );
        f.refresh_image(role);
    }
    let result = f.compare();
    assert_eq!(result.verified, 0);
    let metrics = result.cases[0].samples[0].metrics.as_ref().unwrap();
    assert_eq!(
        (
            metrics.reference_spatial_range,
            metrics.candidate_spatial_range
        ),
        (0, 0)
    );
}
#[test]
fn spatial_threshold_is_required_and_bounded_to_a_byte() {
    for value in [None, Some(json!(-1)), Some(json!(256)), Some(json!(16.5))] {
        let f = Fixture::new();
        let mut cases = f.read(&f.cases);
        let sample = cases["cases"][0]["samples"][0].as_object_mut().unwrap();
        match value {
            None => {
                sample.remove("minimum_spatial_range");
            }
            Some(value) => {
                sample.insert("minimum_spatial_range".into(), value);
            }
        }
        f.write(&f.cases, &cases);
        assert!(current(&f.root, &f.cases, SOURCE, &f.exe).is_err());
    }
}
