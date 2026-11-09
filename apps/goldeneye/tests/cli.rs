use std::process::Command;

#[test]
fn mission_context_rejects_invalid_source_choices_before_opening_assets() {
    for flags in [
        ["--difficulty", "4"],
        ["--difficulty", "-1"],
        ["--difficulty", "operative"],
        ["--level", "not-a-hash"],
        ["--start-point-id", "-1"],
        ["--start-point-id", "not-an-id"],
        ["--animation-request", "not-a-hash"],
        ["--animation-request", "0900071e"],
        ["--animation-previous", "0"],
        ["--animation-fallback", "0900065e"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["mission-context", "nonexistent-mission-preflight.rvz"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("error:"), "{error}");
        assert!(
            !error.contains("opening nonexistent-mission-preflight.rvz"),
            "{error}"
        );
        assert!(!error.contains("panicked"), "{error}");
    }
}

#[test]
fn initialized_layer_snapshot_conflicts_with_explicit_mode_history_before_asset_io() {
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args([
            "mission-context",
            "nonexistent-layer-preflight.rvz",
            "--initialize-animation-layers",
            "--animation-request",
            "0900071e",
            "--animation-previous",
            "0",
            "--animation-fallback",
            "0900065e",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("cannot be used with"), "{error}");
    assert!(
        !error.contains("opening nonexistent-layer-preflight.rvz"),
        "{error}"
    );
}

#[test]
fn raw_frame_decode_requires_owned_layer_initialization_before_asset_io() {
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args([
            "mission-context",
            "nonexistent-frame-preflight.rvz",
            "--decode-initial-frame",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("--initialize-animation-layers"), "{error}");
    assert!(
        !error.contains("opening nonexistent-frame-preflight.rvz"),
        "{error}"
    );
}

#[test]
fn prewarm_requires_fresh_owned_layers_and_conflicts_with_constructor_decode() {
    for flags in [
        vec!["--evaluate-prewarm-pose"],
        vec![
            "--evaluate-prewarm-pose",
            "--initialize-animation-layers",
            "--decode-initial-frame",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["mission-context", "nonexistent-prewarm-preflight.rvz"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("error:"), "{error}");
        assert!(
            !error.contains("opening nonexistent-prewarm-preflight.rvz"),
            "{error}"
        );
    }
}

#[test]
fn camera_bone_queries_require_committed_pose_and_one_explicit_cache_boundary() {
    for flags in [
        vec!["--evaluate-uncached-camera-bone"],
        vec!["--evaluate-cached-camera-bone"],
        vec![
            "--initialize-animation-layers",
            "--evaluate-prewarm-pose",
            "--evaluate-uncached-camera-bone",
            "--evaluate-cached-camera-bone",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["mission-context", "nonexistent-camera-preflight.rvz"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("error:"), "{error}");
        assert!(
            !error.contains("opening nonexistent-camera-preflight.rvz"),
            "{error}"
        );
    }
}

#[test]
fn menu_smoke_options_fail_before_asset_or_window_access() {
    for flags in [
        vec!["--hidden"],
        vec!["--frames", "0"],
        vec!["--frames", "2", "--smoke-resize"],
        vec!["--menu-inputs", "confirm"],
        vec!["--menu-inputs", "up", "--frames", "10"],
        vec![
            "--skip-startup",
            "--menu-inputs",
            "up,confirm",
            "--frames",
            "12",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["play", "nonexistent-menu-preflight.rvz"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            !error.contains("opening nonexistent-menu-preflight.rvz"),
            "{error}"
        );
        assert!(!error.contains("panicked"), "{error}");
    }
}

struct CollisionBoxFiles {
    snapshot: std::path::PathBuf,
    image: std::path::PathBuf,
}
impl CollisionBoxFiles {
    fn new(label: &str) -> Self {
        let snapshot = std::env::temp_dir().join(format!(
            "goldeneye-collision-box-{label}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&snapshot)
            .unwrap();
        let image = snapshot.with_extension("unavailable.rvz");
        assert!(!image.exists());
        Self { snapshot, image }
    }
    fn call(&self) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .arg("collision-box")
            .arg(&self.image)
            .args([
                "missing.edb",
                "--map",
                "85000000",
                "--zone",
                "0",
                "--snapshot",
            ])
            .arg(&self.snapshot)
            .output()
            .unwrap()
    }
    fn assert_preflight_error(&self, expected: &str) {
        let output = self.call();
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(error.contains(expected), "expected {expected}: {error}");
        assert!(
            !error.contains(self.image.file_name().unwrap().to_str().unwrap()),
            "image accessed before snapshot preflight: {error}"
        );
    }
}
impl Drop for CollisionBoxFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.snapshot);
    }
}
fn collision_box_snapshot() -> serde_json::Value {
    serde_json::json!({"world_origin":[0,0,0],"radius":1,
        "world_from_query":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],
        "boxes":[{"center":[0,0,0],"axes":[[1,0,0],[0,1,0]],"half_extents":[0.25,0.25,0.25]}],
        "filter_slot":0,"filter_flags":0,"record_capacity":1,"contact_capacity":2})
}

fn snapshot_command(files: &CollisionBoxFiles, command: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .arg(command)
        .arg(&files.image)
        .arg("--snapshot")
        .arg(&files.snapshot)
        .output()
        .unwrap()
}
#[test]
fn world_line_preflight_and_empty_manager_do_not_open_an_image() {
    let files = CollisionBoxFiles::new("world-line");
    let mut snapshot = serde_json::json!({"worlds":[],"line":{"origin":[0,1,0],"delta":[0,-2,0],
        "maximum_parameter":1,"filter_slot":0,"filter_flags":0,"required_any":0,"excluded_any":0,"stop_first_raw":0}});
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "collision-world-line");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["selected_world"].is_null() && report["query"].is_null());
    snapshot["line"]["maximum_parameter"] = serde_json::json!(1e100);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "collision-world-line");
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success() && error.contains("line snapshot values must be finite"));
    assert!(!error.contains(files.image.file_name().unwrap().to_str().unwrap()));
    snapshot["line"]["maximum_parameter"] = serde_json::json!(1);
    snapshot["line"]["unexpected"] = serde_json::json!(1);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "collision-world-line");
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success() && error.contains("unknown field"));
    assert!(!error.contains(files.image.file_name().unwrap().to_str().unwrap()));
}
#[test]
fn grenade_world_line_keeps_pre_lift_bounds_and_validates_before_image_access() {
    let files = CollisionBoxFiles::new("grenade-world-line");
    let mut snapshot = serde_json::json!({"worlds":[],"cached_apex":[0,1,0],"target":[0,0,0],"small_lift_raw":255});
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "grenade-world-line");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["blocked"], false);
    assert_eq!(
        report["typed_request"]["constructor_delta"],
        serde_json::json!([0., -1., 0.])
    );
    assert_eq!(
        report["typed_request"]["initial_world_bounds"]["minimum"][1],
        0.
    );
    assert!(report["typed_request"]["delta"][1].as_f64().unwrap() > -1.);
    assert_eq!(report["typed_request"]["filter_slot"], 1);
    assert_eq!(report["typed_request"]["filter_flags"], 0);
    snapshot["cached_apex"][1] = serde_json::json!(1e100);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "grenade-world-line");
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success() && error.contains("nonfinite grenade clearance input"));
    assert!(!error.contains(files.image.file_name().unwrap().to_str().unwrap()));
}
#[test]
fn world_box_snapshot_preflight_and_empty_manager_do_not_open_an_image() {
    let files = CollisionBoxFiles::new("world-manager");
    let mut snapshot =
        serde_json::json!({"worlds":[],"request_flags_raw":2,"query":collision_box_snapshot()});
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "collision-world-box");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["selected_world"].is_null() && report["query"].is_null());
    snapshot["query"]["record_capacity"] = serde_json::json!(1_000_001);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "collision-world-box");
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success() && error.contains("mesh snapshot exceeds host count budget"));
    assert!(!error.contains(files.image.file_name().unwrap().to_str().unwrap()));
    snapshot["query"]["record_capacity"] = serde_json::json!(1);
    snapshot["worlds"] = serde_json::json!([{"resource":"missing.edb","map":2231369728u32,"bounds":{"minimum":[0,0,0],"maximum":[1,1,1]}}]);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "collision-world-box");
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains(files.image.file_name().unwrap().to_str().unwrap())
    );
}
#[test]
fn grenade_ascent_rejection_and_empty_world_query_do_not_open_an_image() {
    let files = CollisionBoxFiles::new("grenade-ascent");
    let mut snapshot = serde_json::json!({"worlds":[],"start":[0,0,0],"target":[10,0,0],"angle_index":0,"small_lift_raw":1});
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "grenade-ascent-box");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["blocked"], false);
    assert_eq!(report["typed_request"]["record_capacity"], 64);
    snapshot["target"] = serde_json::json!([1, 100, 0]);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "grenade-ascent-box");
    assert!(result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["blocked"].is_null() && report["world_query"].is_null());
    snapshot["angle_index"] = serde_json::json!(5);
    std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let result = snapshot_command(&files, "grenade-ascent-box");
    let error = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success() && error.contains("angle index outside observed table"));
    assert!(!error.contains(files.image.file_name().unwrap().to_str().unwrap()));
}

#[test]
fn collision_box_rejects_invalid_snapshot_values_before_opening_an_image() {
    let files = CollisionBoxFiles::new("values");
    let valid = collision_box_snapshot();
    std::fs::write(&files.snapshot, serde_json::to_vec(&valid).unwrap()).unwrap();
    let baseline = files.call();
    assert!(!baseline.status.success());
    assert!(
        String::from_utf8_lossy(&baseline.stderr)
            .contains(files.image.file_name().unwrap().to_str().unwrap())
    );
    let mut cases = Vec::new();
    let mut snapshot = valid.clone();
    snapshot["boxes"][0]["half_extents"][1] = serde_json::json!(-0.25);
    cases.push((snapshot, "invalid mesh snapshot oriented box"));
    let mut snapshot = valid.clone();
    snapshot["unexpected"] = serde_json::json!(true);
    cases.push((snapshot, "unknown field"));
    let mut snapshot = valid.clone();
    snapshot["boxes"][0]["unexpected"] = serde_json::json!(true);
    cases.push((snapshot, "unknown field"));
    for field in ["record_capacity", "contact_capacity"] {
        let mut snapshot = valid.clone();
        snapshot[field] = serde_json::json!(1_000_001);
        cases.push((snapshot, "mesh snapshot exceeds host count budget"));
    }
    let mut snapshot = valid.clone();
    snapshot["boxes"] = serde_json::json!(vec![valid["boxes"][0].clone(); 257]);
    cases.push((snapshot, "mesh snapshot exceeds host count budget"));
    for (snapshot, error) in cases {
        std::fs::write(&files.snapshot, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        files.assert_preflight_error(error);
    }
}

#[test]
fn collision_box_reports_malformed_oversized_and_missing_snapshots_before_image_access() {
    let files = CollisionBoxFiles::new("files");
    std::fs::write(&files.snapshot, b"{not-json}").unwrap();
    files.assert_preflight_error("parsing mesh query snapshot");
    std::fs::write(&files.snapshot, vec![b' '; 1_048_577]).unwrap();
    files.assert_preflight_error("mesh snapshot exceeds 1 MiB");
    std::fs::remove_file(&files.snapshot).unwrap();
    files.assert_preflight_error("opening mesh query snapshot");
}

#[test]
fn collision_collection_validates_query_and_capacity_before_asset_access() {
    for (radius, capacity, error) in [
        ("NaN", "1", "collection query values must be finite"),
        ("1", "1000001", "collection capacity exceeds host budget"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args([
                "collision-collect",
                "missing-collection-image.rvz",
                "missing.edb",
                "--map",
                "85000000",
                "--zone",
                "0",
                "--origin",
                "0",
                "0",
                "0",
                "--radius",
                radius,
                "--filter-slot",
                "0",
                "--filter-flags",
                "0",
                "--capacity",
                capacity,
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
    }
}

#[test]
fn collision_candidates_reject_nonfinite_queries_before_opening_an_image() {
    for (origin, radius) in [(["NaN", "0", "0"], "1"), (["0", "0", "0"], "inf")] {
        let result = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args([
                "collision-candidates",
                "missing-candidate-image.rvz",
                "missing.edb",
                "--map",
                "85000000",
                "--zone",
                "0",
                "--origin",
                origin[0],
                origin[1],
                origin[2],
                "--radius",
                radius,
            ])
            .output()
            .unwrap();
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains("candidate query values must be finite"),
            "{error}"
        );
    }
}

#[test]
fn grenade_snapshot_checks_precede_asset_loading_and_unknown_fields_are_rejected() {
    let path = std::env::temp_dir().join(format!(
        "goldeneye-grenade-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let valid = serde_json::json!({"source":{"entity_kind_raw":0,"location":0},"records":[],"rng_word":0,"body_flags_raw":8});
    let call = |bytes: Vec<u8>| {
        std::fs::write(&path, bytes).unwrap();
        Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args([
                "grenade-launch",
                "missing-grenade-image.rvz",
                "--snapshot",
                path.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    };
    let result = call(serde_json::to_vec(&valid).unwrap());
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("opening"));
    let mut cases = Vec::new();
    let mut value = valid.clone();
    value["source"]["location"] = serde_json::json!(8);
    cases.push((value, "outside observed table"));
    let mut value = valid.clone();
    value["unrecognized"] = serde_json::json!(true);
    cases.push((value, "unknown field"));
    let mut value = valid.clone();
    value["source"]["unrecognized"] = serde_json::json!(true);
    cases.push((value, "unknown field"));
    let mut value = valid.clone();
    value["rng_word"] = serde_json::json!(-1);
    cases.push((value, "invalid value"));
    let mut value = valid;
    value["steps"] = serde_json::json!(vec![
        serde_json::json!({"clock_delta":0.1,"time_scale":1});
        257
    ]);
    cases.push((value, "exceeds 256 spin steps"));
    for (snapshot, error) in cases {
        let result = call(serde_json::to_vec(&snapshot).unwrap());
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(error),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty());
    }
    let result = call(vec![b' '; 1_048_577]);
    assert!(String::from_utf8_lossy(&result.stderr).contains("exceeds 1 MiB"));
}

#[test]
fn actor_action_loads_assets_only_after_a_passing_gate_and_checks_snapshot_bounds() {
    let root = std::env::temp_dir().join(format!(
        "goldeneye-action-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let gated = r#"{"owner_translation":[0,0,0],"owner_yaw":0,"instance_translation":[0,0,0],"state":{"state_raw":77,"active_index":0,"mode_raw":0,"target":[10,0,0],"records":[]}}"#;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&root)
        .unwrap();
    std::fs::write(&root, gated).unwrap();
    let base = [
        "nav-actor-action",
        "unused.rvz",
        "unused.edb",
        "--index",
        "0",
        "--snapshot",
        root.to_str().unwrap(),
    ];
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let d: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(d["query"], serde_json::Value::Null);
    assert_eq!(d["action"]["return_code"], 1);
    assert_eq!(d["state_before"], d["state_after"]);
    for extra in [
        ["--max-face-tests", "0"],
        ["--max-expansions", "0"],
        ["--max-goal-steps", "0"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(extra)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(String::from_utf8_lossy(&r.stderr).contains("budgets must be positive"));
    }
    std::fs::write(
        &root,
        gated.replace("\"state_raw\":77", "\"state_raw\":77,\"unrecognized\":true"),
    )
    .unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("unknown field"));
    let accepted = gated.replace("[10,0,0]", "[0,0,10]").replace(
        "\"records\":[]",
        "\"records\":[{\"position\":[0,0,0],\"motion\":[7,8,9]}]",
    );
    std::fs::write(&root, accepted).unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
    std::fs::write(&root, vec![b' '; 1_048_577]).unwrap();
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .output()
        .unwrap();
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("exceeds 1 MiB"));
    std::fs::remove_file(root).unwrap();
}

#[test]
fn actor_target_requires_distinct_finite_frame_inputs_and_positive_work_guards() {
    let base = [
        "nav-actor-target",
        "unused.rvz",
        "unused.edb",
        "--index",
        "0",
        "--owner-translation",
        "0",
        "0",
        "0",
        "--instance-translation",
        "0",
        "0",
        "0",
        "--target",
        "1",
        "0",
        "1",
    ];
    for args in [
        vec![],
        vec!["--active-position", "NaN", "0", "0"],
        vec!["--active-position", "0", "0", "0", "--max-face-tests", "0"],
        vec!["--active-position", "0", "0", "0", "--max-expansions", "0"],
        vec!["--active-position", "0", "0", "0", "--max-goal-steps", "0"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(args)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
    }
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args(["--active-position", "0", "0", "0"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
}

#[test]
fn actor_path_requires_explicit_finite_origin_point_and_positive_independent_budgets() {
    let base = [
        "nav-actor-path",
        "unused.rvz",
        "unused.edb",
        "--index",
        "0",
        "--start-triangle",
        "0",
    ];
    for args in [
        vec![],
        vec!["--origin", "0", "0", "0"],
        vec!["--origin", "NaN", "0", "0", "--point", "1", "0", "1"],
        vec!["--origin", "0", "0", "0", "--point", "inf", "0", "1"],
        vec![
            "--origin",
            "0",
            "0",
            "0",
            "--point",
            "1",
            "0",
            "1",
            "--max-expansions",
            "0",
        ],
        vec![
            "--origin",
            "0",
            "0",
            "0",
            "--point",
            "1",
            "0",
            "1",
            "--max-goal-steps",
            "0",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("opening"));
    }
    let result = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args(["--origin", "0", "0", "0", "--point", "1", "0", "1"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&result.stderr).contains("opening"));
}

#[test]
fn predicate_dispatch_requires_a_complete_finite_authored_policy_before_asset_access() {
    let base = [
        "nav-request",
        "unused.rvz",
        "unused.edb",
        "--map",
        "85000000",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
        "--flags-raw",
        "300",
    ];
    for args in [
        vec!["--actor-edges"],
        vec!["--accept-triangle", "1"],
        vec!["--heuristic-point", "0", "0", "0"],
        vec![
            "--accept-triangle",
            "1",
            "--heuristic-point",
            "NaN",
            "0",
            "0",
        ],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(args)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
    }
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args([
            "--accept-triangle",
            "0",
            "--accept-triangle",
            "1",
            "--heuristic-point",
            "0",
            "0",
            "0",
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
}

#[test]
fn predicate_path_requires_explicit_accepted_faces_and_finite_point_before_asset_access() {
    let base = [
        "nav-goal-path",
        "unused.rvz",
        "unused.edb",
        "82000000",
        "--start-triangle",
        "0",
    ];
    for args in [
        vec![],
        vec!["--accept-triangle", "1"],
        vec!["--heuristic-point", "0", "0", "0"],
        vec![
            "--accept-triangle",
            "1",
            "--heuristic-point",
            "NaN",
            "0",
            "0",
        ],
        vec![
            "--accept-triangle",
            "1",
            "--heuristic-point",
            "0",
            "0",
            "0",
            "--max-expansions",
            "0",
        ],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(args)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
    }
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args([
            "--accept-triangle",
            "1",
            "--accept-triangle",
            "2",
            "--heuristic-point",
            "0",
            "0",
            "0",
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
}

#[test]
fn authored_dispatch_positions_require_exact_remaining_count_and_finite_values() {
    let base = [
        "nav-request",
        "unused.rvz",
        "unused.edb",
        "--map",
        "85000000",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
        "--flags-raw",
        "300",
    ];
    for args in [
        vec!["--steps", "3", "--next-start", "1", "0", "1"],
        vec!["--steps", "1", "--next-start", "1", "0", "1"],
        vec!["--next-start", "NaN", "0", "1"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(args)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
    }
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args([
            "--steps",
            "3",
            "--next-start",
            "1",
            "0",
            "1",
            "--next-start",
            "2",
            "0",
            "2",
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
}

#[test]
fn actor_boundary_diagnostic_requires_an_observed_band_and_explicit_mode_before_asset_access() {
    let base = [
        "nav-request",
        "unused.rvz",
        "unused.edb",
        "--map",
        "85000000",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
        "--flags-raw",
        "300",
    ];
    for extra in [
        vec!["--actor-boundary-band", "4.5"],
        vec!["--actor-mode", "0"],
        vec!["--actor-boundary-band", "4.4", "--actor-mode", "0"],
        vec!["--actor-boundary-band", "3.45", "--actor-mode", "-1"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(extra)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("opening"));
    }
}

#[test]
fn actor_request_chooses_static_map_or_explicit_frame_without_mixing_them() {
    let base = [
        "nav-request",
        "unused.rvz",
        "unused.edb",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
        "--flags-raw",
        "300",
    ];
    for flags in [
        vec![],
        vec!["--map", "85000000", "--translation", "0", "0", "0"],
        vec!["--map", "invalid"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(flags)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
    }
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args(["--map", "85000000"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
}

#[test]
fn actor_request_requires_explicit_flags_and_finite_bounded_dispatch_inputs() {
    let base = [
        "nav-request",
        "unused.rvz",
        "unused.edb",
        "--translation",
        "0",
        "0",
        "0",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
    ];
    for flags in [
        vec![],
        vec!["--flags-raw", "300", "--steps", "0"],
        vec!["--flags-raw", "300", "--steps", "17"],
        vec!["--flags-raw", "300", "--max-expansions", "0"],
        vec!["--flags-raw", "nope"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(flags)
            .output()
            .unwrap();
        assert!(!r.status.success());
        assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
    }
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .args(["--flags-raw", "300"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&r.stderr).contains("opening"));
    let r = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args([
            "nav-request",
            "unused.rvz",
            "unused.edb",
            "--translation",
            "NaN",
            "0",
            "0",
            "--start",
            "0",
            "0",
            "0",
            "--goal",
            "1",
            "0",
            "1",
            "--flags-raw",
            "300",
        ])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&r.stderr).contains("opening"));
}

#[test]
fn help_is_available_without_game_assets() {
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("inspect") && text.contains("archive") && text.contains("probe"));
}

#[test]
fn missing_image_is_an_error_not_a_successful_empty_inventory() {
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(["inspect", "nonexistent-image-for-cli-test.rvz"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("opening"));
}

#[test]
fn malformed_offset_is_rejected_before_asset_access() {
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(["probe", "unused.rvz", "Filelist.000", "--offset", "-1"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error"));
}

#[test]
fn incomplete_inspection_requires_explicit_placement_loading() {
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(["scene", "unused.rvz", "unused.edb", "--allow-incomplete"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--placements"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
}

#[test]
fn graph_selection_requires_exactly_one_hash_or_directory_index() {
    for selection in [vec![], vec!["82000001", "--index", "0"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["entity-graph", "unused.rvz", "unused.edb"])
            .args(selection)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
    // Explicit directory selection passes argument parsing and reaches image access.
    let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args([
            "entity-graph",
            "nonexistent-graph-cli-image.rvz",
            "unused.edb",
            "--index",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("opening"));
}

#[test]
fn native_smoke_options_are_bounded_before_asset_or_window_access() {
    for args in [
        vec!["--hidden"],
        vec!["--frames", "0"],
        vec!["--frames", "1", "--smoke-resize"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["view-map", "unused.rvz", "unused.edb"])
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}

#[test]
fn surface_query_requires_a_complete_numeric_ray_before_asset_access() {
    for flags in [
        vec!["--max-t", "2"],
        vec![
            "--origin",
            "0",
            "0",
            "0",
            "--direction",
            "0",
            "-1",
            "0",
            "--max-t",
            "bad",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["nav-ray", "unused.rvz", "unused.edb", "82000001"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}

#[test]
fn surface_query_mode_and_selected_segment_require_valid_arguments_before_asset_access() {
    for args in [
        vec![
            "nav-ray",
            "unused.rvz",
            "unused.edb",
            "82000001",
            "--origin",
            "0",
            "0",
            "0",
            "--direction",
            "0",
            "-1",
            "0",
            "--max-t",
            "2",
            "--selection",
            "unsupported",
        ],
        vec![
            "nav-segment",
            "unused.rvz",
            "unused.edb",
            "82000001",
            "--origin",
            "0",
            "0",
            "0",
            "--displacement",
            "0",
            "-1",
            "0",
        ],
        vec![
            "nav-segment",
            "unused.rvz",
            "unused.edb",
            "82000001",
            "--triangle",
            "0",
            "--origin",
            "0",
            "0",
            "0",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}

#[test]
fn connected_segment_requires_explicit_start_depth_and_positive_host_budget() {
    let base = [
        "nav-connected-segment",
        "unused.rvz",
        "unused.edb",
        "82000001",
        "--origin",
        "0",
        "0",
        "0",
        "--displacement",
        "0",
        "-1",
        "0",
    ];
    for flags in [
        vec!["--start-triangle", "0"],
        vec!["--depth", "2"],
        vec!["--start-triangle", "0", "--depth", "2", "--face-limit", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}

#[test]
fn path_selection_requires_endpoints_and_a_positive_host_budget_before_asset_access() {
    for flags in [
        vec!["--start-triangle", "0"],
        vec!["--goal-triangle", "1"],
        vec![
            "--start-triangle",
            "0",
            "--goal-triangle",
            "1",
            "--max-expansions",
            "0",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["nav-path", "unused.rvz", "unused.edb", "82000001"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}

#[test]
fn world_path_requires_explicit_frame_endpoints_and_positive_budget_before_asset_access() {
    for flags in [
        vec!["--start", "0", "0", "0", "--goal", "1", "0", "1"],
        vec!["--translation", "0", "0", "0", "--start", "0", "0", "0"],
        vec![
            "--translation",
            "0",
            "0",
            "0",
            "--start",
            "0",
            "0",
            "0",
            "--goal",
            "1",
            "0",
            "1",
            "--max-expansions",
            "0",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(["nav-world-path", "unused.rvz", "unused.edb", "82000001"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}

#[test]
fn waypoint_work_limits_require_waypoint_mode_and_positive_values() {
    let base = [
        "nav-world-path",
        "unused.rvz",
        "unused.edb",
        "82000001",
        "--translation",
        "0",
        "0",
        "0",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
    ];
    for flags in [
        vec!["--max-shortcut-tests", "1"],
        vec!["--max-fan-steps", "1"],
        vec!["--waypoints", "--max-shortcut-tests", "0"],
        vec!["--waypoints", "--max-fan-steps", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
    let normal = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
        .args(base)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&normal.stderr).contains("opening"));
}

#[test]
fn waypoint_probe_requires_mode_radius_and_finite_values_before_asset_access() {
    let base = [
        "nav-world-path",
        "unused.rvz",
        "unused.edb",
        "82000001",
        "--translation",
        "0",
        "0",
        "0",
        "--start",
        "0",
        "0",
        "0",
        "--goal",
        "1",
        "0",
        "1",
    ];
    for flags in [
        vec!["--probe-position", "0", "0", "0", "--corridor-radius", "1"],
        vec!["--waypoints", "--probe-position", "0", "0", "0"],
        vec!["--waypoints", "--corridor-radius", "1"],
        vec!["--waypoints", "--corridor-segments", "1"],
        vec![
            "--waypoints",
            "--probe-position",
            "0",
            "0",
            "--corridor-radius",
            "1",
        ],
        vec![
            "--waypoints",
            "--probe-position",
            "NaN",
            "0",
            "0",
            "--corridor-radius",
            "1",
        ],
        vec![
            "--waypoints",
            "--probe-position",
            "0",
            "0",
            "0",
            "--corridor-radius",
            "inf",
        ],
        vec![
            "--waypoints",
            "--probe-position",
            "0",
            "0",
            "0",
            "--corridor-radius",
            "3.4028235e38",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
    for radius in ["0", "-0.5", "1"] {
        let output = Command::new(env!("CARGO_BIN_EXE_goldeneye"))
            .args(base)
            .args([
                "--waypoints",
                "--probe-position",
                "0",
                "0",
                "0",
                "--corridor-radius",
                radius,
                "--corridor-segments",
                "0",
            ])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stderr).contains("opening"));
    }
}
