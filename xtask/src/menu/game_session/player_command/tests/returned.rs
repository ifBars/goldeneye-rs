use super::*;
pub(in crate::menu::game_session) fn pair() -> (Value, Value) {
    let mut s = fixture();
    let mut a = super::super::super::player_action::tests::fixture();
    a["stage"] = json!("tp_setter_tail_pending");
    a["tp_selector"]["commands_executed"] = json!(1);
    let mut h = s["child_header"].clone();
    for key in [
        "phase_zero_initialized",
        "initializer_returned",
        "factory_returned",
        "layer_inserted",
        "parent_phase_copied",
        "parent_scalars_initialized",
        "parent_word_stored",
    ] {
        h[key] = json!(true);
    }
    h["phase_zero"]["stage"] = json!("initialized");
    h["phase_zero"]["frame_range"] = json!(30);
    h["phase_zero"]["setup_returned"] = json!(true);
    h["phase_zero"]["flags_applied"] = json!(true);
    h["phase_zero"]["position"] = json!({"selected_index":0,"file_bound":true,"generation_observed":0,"metadata_bound":true,"cache_generation":0,"descriptor_index":3,
        "body_bound":true,"timing_flags":0,"animation_fps":30,"script_fps_bits":30f32.to_bits(),"frame_bits":0,"previous_position_bits":0,"normalized_position_bits":0,"root_motion_pending":false});
    s["last_completed_child"] = h;
    s["child_header"] = Value::Null;
    s["stage"] = json!("command_returned");
    for key in [
        "pending_group",
        "pending_slot",
        "child_reference",
        "factory_class",
        "child_flags",
        "child_divisor",
    ] {
        s[key] = Value::Null;
    }
    for key in ["children_completed", "children_inserted", "head_copies"] {
        s[key] = json!(1);
    }
    s["active_slots"] = json!(2);
    s["effective_flags_or"] = json!(36);
    for key in [
        "command_returned",
        "cleanup_skipped",
        "selector_acknowledged",
    ] {
        s[key] = json!(true);
    }
    (s, a)
}
#[test]
fn completed_body_needs_real_parent_membership_and_only_returns_at_the_setter_boundary() {
    let (s, a) = pair();
    validate(&s, &a).unwrap();
    for (key, value) in [
        ("children_completed", json!(0)),
        ("children_inserted", json!(0)),
        ("head_copies", json!(2)),
        ("active_slots", json!(1)),
        ("effective_flags_or", json!(0)),
        ("last_completed_child", Value::Null),
        ("command_returned", json!(false)),
        ("cleanup_complete", json!(true)),
        ("cleanup_skipped", json!(false)),
        ("selector_acknowledged", json!(false)),
    ] {
        let mut v = s.clone();
        v[key] = value;
        assert!(validate(&v, &a).is_err(), "accepted {key}");
    }
    for key in [
        "layer_inserted",
        "parent_scalars_initialized",
        "parent_word_stored",
    ] {
        let mut v = s.clone();
        v["last_completed_child"][key] = json!(false);
        assert!(validate(&v, &a).is_err());
    }
    let mut a = a;
    a["tp_selector"]["commands_executed"] = json!(0);
    assert!(validate(&s, &a).is_err());
}
