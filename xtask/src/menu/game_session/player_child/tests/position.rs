use super::*;
fn initialized() -> Value {
    let mut child = fixture();
    child["phase_zero"]["stage"] = json!("initialized");
    child["phase_zero"]["frame_range"] = json!(30);
    child["phase_zero"]["setup_returned"] = json!(true);
    child["phase_zero"]["flags_applied"] = json!(true);
    child["phase_zero"]["position"] = super::super::position::fixture();
    for key in [
        "phase_zero_initialized",
        "initializer_returned",
        "factory_returned",
    ] {
        child[key] = json!(true);
    }
    child
}
#[test]
fn initialized_position_does_not_return_its_parent_command_and_rejects_fabricated_state() {
    validate(&initialized(), &json!("child_effects_pending")).unwrap();
    assert!(validate(&initialized(), &json!("child_initializer_pending")).is_err());
    let base = super::super::position::fixture();
    for (key, value) in [
        ("selected_index", json!(1)),
        ("file_bound", json!(false)),
        ("generation_observed", json!(7)),
        ("metadata_bound", json!(false)),
        ("cache_generation", json!(3)),
        ("descriptor_index", Value::Null),
        ("body_bound", Value::Null),
        ("timing_flags", json!(1)),
        ("animation_fps", json!(256)),
        ("script_fps_bits", Value::Null),
        ("frame_bits", Value::Null),
        ("previous_position_bits", Value::Null),
        ("normalized_position_bits", json!(0x7fc00000)),
        ("root_motion_pending", json!(true)),
        ("payload", json!([1])),
    ] {
        let mut v = initialized();
        v["phase_zero"]["position"][key] = value;
        assert!(
            validate(&v, &json!("child_effects_pending")).is_err(),
            "accepted {key}"
        );
    }
    for key in base.as_object().unwrap().keys() {
        let mut v = initialized();
        v["phase_zero"]["position"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&v, &json!("child_effects_pending")).is_err(),
            "accepted absent {key}"
        );
    }
}
#[test]
fn pending_body_and_timing_keep_completed_cache_but_cannot_claim_position_or_return() {
    for stage in ["animation_body_pending", "timing_curve_pending"] {
        let mut v = initialized();
        for key in [
            "phase_zero_initialized",
            "initializer_returned",
            "factory_returned",
        ] {
            v[key] = json!(false);
        }
        v["phase_zero"]["stage"] = json!(stage);
        let p = &mut v["phase_zero"]["position"];
        for key in [
            "timing_flags",
            "animation_fps",
            "script_fps_bits",
            "frame_bits",
            "previous_position_bits",
            "normalized_position_bits",
        ] {
            p[key] = Value::Null;
        }
        if stage == "animation_body_pending" {
            p["body_bound"] = Value::Null;
        } else {
            p["timing_flags"] = json!(1);
        }
        validate(&v, &json!("child_initializer_pending")).unwrap();
        v["phase_zero_initialized"] = json!(true);
        assert!(validate(&v, &json!("child_effects_pending")).is_err());
    }
}
