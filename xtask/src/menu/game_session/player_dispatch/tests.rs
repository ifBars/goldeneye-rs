use super::*;
use serde_json::json;

pub(in crate::menu::game_session) fn fixture() -> Value {
    json!({"stage":"awaiting_setter","captured_position_bits":0,"setter_return_word":null,"history_updated":false,"primary_changed":null,"scalar_visits":0,"scalar_writes":0,"updates_started":0,"updates_completed":0,"pending_slot":null,"script_invocations":0,"rate_queries":0,"rate_returns":0,"clip_reuses":0,"position_returns":0,"pending_dependency":null,"rate_bits":[null,null],"state_sha256":"e".repeat(64)})
}
pub(in crate::menu::game_session) fn returned_action() -> Value {
    let mut action = super::super::player_action::tests::fixture();
    action["stage"] = json!("fp_dispatch_pending");
    let selector = &mut action["tp_selector"];
    selector["stage"] = json!("returned");
    selector["commands_executed"] = json!(1);
    selector["command_effects_complete"] = json!(true);
    selector["pending_ordinal"] = Value::Null;
    selector["pending_opcode"] = Value::Null;
    selector["return_word"] = json!(0x0900_06c0);
    let mut command = super::super::player_command::tests::returned::pair().0;
    for key in [
        "set_groups",
        "groups_visited",
        "weight_writes",
        "children_created",
        "children_completed",
        "children_inserted",
        "head_copies",
    ] {
        command[key] = json!(2);
    }
    command["active_slots"] = json!(4);
    command["last_completed_child"]["slot"] = json!(3);
    action["tp_command"] = command;
    action["tp_dispatch"] = json!({"stage":"returned","captured_position_bits":0,"setter_return_word":0x0900_06c0,"history_updated":false,"primary_changed":null,"scalar_visits":3,"scalar_writes":2,"updates_started":2,"updates_completed":2,"pending_slot":null,"script_invocations":2,"rate_queries":2,"rate_returns":2,"clip_reuses":2,"position_returns":2,"pending_dependency":null,"rate_bits":[0x3f000000u32,0x3f000000u32],"state_sha256":"e".repeat(64)});
    action
}

#[test]
fn real_dispatcher_return_leaves_fp_and_enclosing_callback_pending() {
    let action = returned_action();
    super::super::player_action::validate(&action).unwrap();
    for (key, value) in [
        ("callback_returned", json!(true)),
        ("mode_history_updated", json!(true)),
        ("stage", json!("complete")),
    ] {
        let mut bad = action.clone();
        bad[key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn rejects_missing_fields_unbounded_counters_nonfinite_rates_and_early_returns() {
    let action = returned_action();
    let s = &action["tp_dispatch"];
    validate(s, &action).unwrap();
    for key in s.as_object().unwrap().keys() {
        let mut bad = s.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(validate(&bad, &action).is_err(), "accepted missing {key}");
    }
    for (key, value) in [
        ("stage", json!("complete")),
        ("captured_position_bits", json!(1)),
        ("setter_return_word", json!(0)),
        ("history_updated", json!(true)),
        ("primary_changed", json!(false)),
        ("scalar_visits", json!(2)),
        ("scalar_writes", json!(4)),
        ("updates_started", json!(3)),
        ("updates_completed", json!(1)),
        ("pending_slot", json!(3)),
        ("script_invocations", json!(3)),
        ("rate_queries", json!(3)),
        ("rate_returns", json!(1)),
        ("clip_reuses", json!(3)),
        ("position_returns", json!(3)),
        ("pending_dependency", json!("clip_rate")),
        ("rate_bits", json!([0x7fc00000u32, 0x3f000000u32])),
        ("rate_bits", json!([null, null])),
        ("rate_bits", json!([0x3f000000u32])),
        ("rate_bits", json!([true, 0x3f000000u32])),
        ("state_sha256", json!("short")),
        ("source_pose", json!([1, 2, 3])),
    ] {
        let mut bad = s.clone();
        bad[key] = value;
        assert!(validate(&bad, &action).is_err(), "accepted {key}");
    }
    for key in COUNTERS {
        let mut bad = s.clone();
        bad[key] = json!(-1);
        assert!(validate(&bad, &action).is_err(), "accepted signed {key}");
    }
}
#[test]
fn pending_provider_retains_prefix_without_claiming_return() {
    let mut action = returned_action();
    action["stage"] = json!("tp_dispatch_pending");
    let s = &mut action["tp_dispatch"];
    s["stage"] = json!("script_update_pending");
    s["updates_started"] = json!(1);
    s["updates_completed"] = json!(0);
    s["pending_slot"] = json!(2);
    s["script_invocations"] = json!(1);
    s["rate_queries"] = json!(1);
    for key in ["rate_returns", "clip_reuses", "position_returns"] {
        s[key] = json!(0);
    }
    s["rate_bits"] = json!([null, null]);
    s["pending_dependency"] = json!("animation_body");
    super::super::player_action::validate(&action).unwrap();
    for (key, value) in [
        ("updates_completed", json!(1)),
        ("pending_slot", json!(3)),
        ("pending_dependency", json!("missing_provider_is_success")),
        ("pending_dependency", Value::Null),
    ] {
        let mut bad = action.clone();
        bad["tp_dispatch"][key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn tracks_branch_and_zero_return_control_history_writes() {
    let mut action = returned_action();
    action["tp_selector"]["tracks_player_modes"] = json!(true);
    assert!(validate(&action["tp_dispatch"], &action).is_err());
    action["mode_history_updated"] = json!(true);
    action["tp_dispatch"]["history_updated"] = json!(true);
    action["tp_dispatch"]["primary_changed"] = json!(false);
    validate(&action["tp_dispatch"], &action).unwrap();
    action["tp_selector"]["return_word"] = json!(0);
    action["tp_dispatch"]["setter_return_word"] = json!(0);
    assert!(validate(&action["tp_dispatch"], &action).is_err());
    action["mode_history_updated"] = json!(false);
    action["tp_dispatch"]["history_updated"] = json!(false);
    action["tp_dispatch"]["primary_changed"] = Value::Null;
    validate(&action["tp_dispatch"], &action).unwrap();
}
