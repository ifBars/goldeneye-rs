use super::*;
use serde_json::json;
pub(in crate::menu::game_session) fn pending_action() -> Value {
    let mut action = super::super::player_dispatch::tests::returned_action();
    action["stage"] = json!("fp_command_pending");
    let mut selector = super::super::player_selector::tests::fixture();
    selector["requested"] = action["fp_mapped"].clone();
    selector["fallback"] = json!(0x0900_065e);
    selector["tracks_player_modes"] = json!(true);
    let mut command = super::super::player_command::tests::fixture();
    command["stage"] = json!("child_factory_pending");
    command["child_reference"] = json!(0x0300_00ab);
    command["factory_class"] = json!(3);
    command["children_created"] = json!(0);
    command["child_header"] = Value::Null;
    action["fp_dispatch"] = json!({"stage":"command_pending","component_file":0x0102_0016,"qualification_skipped":true,"capture_performed":false,"initial_heads_empty":true,"fallback":0x0900_065e,"fallback_writes":4,
        "selector":selector,"command":command,"mask_capture":null,"mask_commands_returned":0,"setter_return_word":null,"history_updated":false,"primary_changed":null,"coordinate_written":null,"state_sha256":"f".repeat(64)});
    action
}
pub(in crate::menu::game_session) fn returned_action() -> Value {
    let mut action = pending_action();
    action["stage"] = json!("initial_action_suffix_pending");
    action["mode_history_updated"] = json!(true);
    let mut command = super::super::player_command::tests::returned::pair().0;
    command["head_copies"] = json!(0);
    command["effective_flags_or"] = json!(32);
    command["last_completed_child"]["parent_phase_copied"] = json!(false);
    let fp = &mut action["fp_dispatch"];
    fp["stage"] = json!("returned");
    fp["command"] = command;
    let selector = &mut fp["selector"];
    selector["stage"] = json!("returned");
    selector["commands_executed"] = json!(1);
    selector["command_effects_complete"] = json!(true);
    selector["pending_ordinal"] = Value::Null;
    selector["pending_opcode"] = Value::Null;
    selector["return_word"] = json!(0x0900_065e);
    fp["setter_return_word"] = json!(0x0900_065e);
    fp["history_updated"] = json!(true);
    fp["primary_changed"] = json!(false);
    fp["coordinate_written"] = json!(true);
    action
}
pub(in crate::menu::game_session) fn native_action() -> Value {
    let mut action = returned_action();
    action["fp_mapped"] = json!(0x0900_071e);
    let fp = &mut action["fp_dispatch"];
    fp["mask_commands_returned"] = json!(1);
    fp["mask_capture"] = super::super::player_mask::tests::fixture();
    let selector = &mut fp["selector"];
    selector["requested"] = json!(0x0900_071e);
    selector["target_index"] = json!(13);
    selector["transition_rows"] = json!(5);
    selector["commands_issued"] = json!(2);
    selector["commands_executed"] = json!(2);
    selector["return_word"] = json!(0x0900_000b);
    fp["setter_return_word"] = json!(0x0900_000b);
    fp["primary_changed"] = json!(true);
    let command = &mut fp["command"];
    command["set_reference"] = json!(9);
    command["set_index"] = json!(9);
    command["effective_flags_or"] = json!(39);
    let child = &mut command["last_completed_child"];
    child["descriptor_index"] = json!(187);
    let phase = &mut child["phase_zero"];
    phase["frame_range"] = json!(76);
    phase["position"]["descriptor_index"] = json!(11);
    action
}
#[test]
fn historical_mask_uses_its_own_active_span_after_a_later_set_raises_current_count() {
    let action = native_action();
    super::super::player_action::validate(&action).unwrap();
    assert_eq!(
        action["fp_dispatch"]["mask_capture"]["active_slots_before"],
        1
    );
    assert_eq!(action["fp_dispatch"]["command"]["active_slots"], 2);
    for masks in [0, 2, 3] {
        let mut bad = action.clone();
        bad["fp_dispatch"]["mask_commands_returned"] = json!(masks);
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted mask count {masks}"
        );
    }
    let mut bad = action;
    bad["fp_dispatch"]["mask_capture"]["mask_raw"] = json!(4);
    assert!(super::super::player_action::validate(&bad).is_err());
}
#[test]
fn pending_fp_command_preserves_tp_history_and_uses_its_own_receiver() {
    let action = pending_action();
    super::super::player_action::validate(&action).unwrap();
    for (key, value) in [
        ("component_file", json!(0x0102_0017)),
        ("qualification_skipped", json!(false)),
        ("capture_performed", json!(true)),
        ("initial_heads_empty", json!(false)),
        ("fallback", json!(0x0900_06c0)),
        ("fallback_writes", json!(2)),
        ("setter_return_word", json!(5)),
        ("history_updated", json!(true)),
        ("primary_changed", json!(false)),
        ("coordinate_written", json!(false)),
    ] {
        let mut bad = action.clone();
        bad["fp_dispatch"][key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn mask_command_retains_real_operand_without_inventing_controls_or_return() {
    let mut action = pending_action();
    let fp = &mut action["fp_dispatch"];
    fp["selector"]["pending_opcode"] = json!(0x0c00_0005);
    let command = &mut fp["command"];
    command["stage"] = json!("mask_control_pending");
    command["opcode"] = json!(0x0c00_0005);
    command["mask"] = json!(3);
    for key in [
        "set_reference",
        "set_index",
        "set_groups",
        "pending_group",
        "pending_slot",
        "child_reference",
        "factory_class",
        "child_flags",
        "child_divisor",
    ] {
        command[key] = Value::Null;
    }
    command["groups_visited"] = json!(0);
    command["weight_writes"] = json!(0);
    command["effective_flags_or"] = json!(0);
    super::super::player_action::validate(&action).unwrap();
    let mut bad = action.clone();
    bad["fp_dispatch"]["command"]["command_returned"] = json!(true);
    assert!(super::super::player_action::validate(&bad).is_err());
    let mut bad = action;
    bad["fp_dispatch"]["command"]["children_created"] = json!(1);
    assert!(super::super::player_action::validate(&bad).is_err());
}
#[test]
fn returned_mask_requires_capture_refresh_and_membership_before_cursor_acknowledgement() {
    let mut action = pending_action();
    action["stage"] = json!("fp_setter_tail_pending");
    let fp = &mut action["fp_dispatch"];
    fp["stage"] = json!("setter_tail_pending");
    let selector = &mut fp["selector"];
    selector["stage"] = json!("returned");
    selector["commands_executed"] = json!(1);
    selector["command_effects_complete"] = json!(true);
    selector["pending_ordinal"] = Value::Null;
    selector["pending_opcode"] = Value::Null;
    selector["return_word"] = json!(0x0900_065e);
    let command = &mut fp["command"];
    command["stage"] = json!("command_returned");
    command["opcode"] = json!(0x0c00_0005);
    command["mask"] = json!(3);
    command["mask_effects"] = super::super::player_mask::tests::fixture();
    for key in [
        "set_reference",
        "set_index",
        "set_groups",
        "pending_group",
        "pending_slot",
        "child_reference",
        "factory_class",
        "child_flags",
        "child_divisor",
    ] {
        command[key] = Value::Null;
    }
    for key in ["groups_visited", "weight_writes", "effective_flags_or"] {
        command[key] = json!(0);
    }
    command["command_returned"] = json!(true);
    command["selector_acknowledged"] = json!(true);
    let fp = &mut action["fp_dispatch"];
    fp["mask_commands_returned"] = json!(1);
    fp["mask_capture"] = fp["command"]["mask_effects"].clone();
    super::super::player_action::validate(&action).unwrap();
    let mut bad = action.clone();
    bad["fp_dispatch"]["command"]["mask_effects"] = Value::Null;
    assert!(super::super::player_action::validate(&bad).is_err());
    let mut bad = action.clone();
    bad["fp_dispatch"]["command"]["mask_effects"]["refresh"]["refresh_ordinal"] = json!(1);
    assert!(super::super::player_action::validate(&bad).is_err());
    let mut bad = action;
    bad["fp_dispatch"]["command"]["mask_effects"]["controls_inserted"] = json!(0);
    assert!(super::super::player_action::validate(&bad).is_err());
}
#[test]
fn returned_fp_dispatch_leaves_the_enclosing_action_and_gameplay_incomplete() {
    let action = returned_action();
    super::super::player_action::validate(&action).unwrap();
    for value in [true, false] {
        let mut skipped = action.clone();
        skipped["fp_dispatch"]["coordinate_written"] = json!(value);
        super::super::player_action::validate(&skipped).unwrap();
    }
    for (key, value) in [
        ("callback_returned", json!(true)),
        ("mode_history_updated", json!(false)),
        ("stage", json!("complete")),
    ] {
        let mut bad = action.clone();
        bad[key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
    for key in action["fp_dispatch"].as_object().unwrap().keys() {
        let mut bad = action.clone();
        bad["fp_dispatch"].as_object_mut().unwrap().remove(key);
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted missing {key}"
        );
    }
    for (key, value) in [
        ("setter_return_word", json!(0)),
        ("history_updated", json!(false)),
        ("primary_changed", Value::Null),
        ("coordinate_written", Value::Null),
        ("state_sha256", json!("short")),
        ("source_bytes", json!([1, 2, 3])),
    ] {
        let mut bad = action.clone();
        bad["fp_dispatch"][key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn setter_and_dispatch_tail_are_separate_from_selection_and_from_each_other() {
    let mut action = returned_action();
    action["stage"] = json!("fp_dispatch_tail_pending");
    action["fp_dispatch"]["stage"] = json!("dispatch_tail_pending");
    action["fp_dispatch"]["coordinate_written"] = Value::Null;
    super::super::player_action::validate(&action).unwrap();
    action["stage"] = json!("fp_setter_tail_pending");
    action["mode_history_updated"] = json!(false);
    let fp = &mut action["fp_dispatch"];
    fp["stage"] = json!("setter_tail_pending");
    fp["setter_return_word"] = Value::Null;
    fp["history_updated"] = json!(false);
    fp["primary_changed"] = Value::Null;
    super::super::player_action::validate(&action).unwrap();
    let mut bad = action.clone();
    bad["fp_dispatch"]["coordinate_written"] = json!(false);
    assert!(super::super::player_action::validate(&bad).is_err());
    let mut bad = action;
    bad["fp_dispatch"] = Value::Null;
    assert!(super::super::player_action::validate(&bad).is_err());
}
#[test]
fn zero_selector_return_skips_tracked_history_but_still_runs_dispatch_tail() {
    let mut action = pending_action();
    action["stage"] = json!("initial_action_suffix_pending");
    let fp = &mut action["fp_dispatch"];
    fp["stage"] = json!("returned");
    fp["command"] = Value::Null;
    let selector = &mut fp["selector"];
    selector["stage"] = json!("returned");
    selector["target_index"] = Value::Null;
    selector["attempts"] = json!(0);
    selector["lookups"] = json!(1);
    selector["transition_rows"] = json!(0);
    selector["commands_issued"] = json!(0);
    selector["pending_ordinal"] = Value::Null;
    selector["pending_opcode"] = Value::Null;
    selector["return_word"] = json!(0);
    fp["setter_return_word"] = json!(0);
    fp["coordinate_written"] = json!(false);
    super::super::player_action::validate(&action).unwrap();
}
