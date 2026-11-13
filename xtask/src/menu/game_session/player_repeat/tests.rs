use super::*;
use serde_json::json;

fn trace() -> Value {
    json!({"stage":"returned","opcode":0x0c00_0002,"set_reference":9,"set_index":9,"set_groups":1,"groups_visited":1,"weight_writes":1,"mask":null,"pending_slot":null,"child_reference":null,"children_created":1,"children_completed":1,"children_inserted":1,"head_copies":0,"effective_flags_or":39,"command_returned":true,"selector_acknowledged":true,"state_sha256":"a".repeat(64)})
}
fn call_fixture(target: &str, requested: u32, tracks: bool, previous: u32) -> Value {
    let mut selector = super::super::player_selector::tests::fixture();
    selector["stage"] = json!("returned");
    selector["requested"] = json!(requested);
    selector["previous"] = json!(previous);
    selector["fallback"] = json!(0x0900_000b);
    selector["tracks_player_modes"] = json!(tracks);
    selector["commands_executed"] = json!(1);
    selector["pending_ordinal"] = Value::Null;
    selector["pending_opcode"] = Value::Null;
    selector["return_word"] = json!(0x0900_000b);
    selector["command_effects_complete"] = json!(true);
    json!({"target":target,"component_file":if target=="first_person" {0x0102_0016}else{0x0102_0017},"generic_request":0x0900_0765,"requested":requested,"parameter_bits":0x7f7f_ffff,"tracks_player_modes":tracks,"fallback":0x0900_000b,"fallback_writes":5,"previous":previous,"stage":"returned","selector":selector,"completed_commands":[trace()],"pending_command":null,"setter_return_word":0x0900_000b,"history_updated":tracks,"state_sha256":"b".repeat(64)})
}
pub(in crate::menu::game_session) fn pending_component() -> Value {
    let initial = super::super::player_suffix::tests::returned_action();
    let mut calls = [
        call_fixture("third_person", 0x0900_0767, false, 0x0900_071e),
        call_fixture("third_person_secondary", 0x0900_0768, false, 0x0900_071e),
        call_fixture("first_person", 0x0900_0766, true, 0),
    ];
    calls[2]["fallback_writes"] = json!(6);
    json!({"stage":"action_dispatch_repeat_pending","pending_dependency":"initial_action_repeat","active_slots":2,"initial_action":initial,"repeat_action":{"stage":"suffix_pending","generic_request":0x0900_0765,"parameter_bits":0x7f7f_ffff,"weapon_key":0x0200_00ab,"variant":0,"remap_calls":3,"map_rows_read":20,"fp_mapped":0x0900_0766,"tp_mapped":0x0900_0767,"tp_secondary_mapped":0x0900_0768,"mapping_state_sha256":"c".repeat(64),"dispatch":{"phase":"returned","captured_position_bits":0,"captured_head_present":true,"captured_matched":true,"tp_coordinate_written":true,"seek_bits":0,"seek_node_present":true,"seek_returned":true,"override_writes":2,"fp_coordinate_bits":0,"fp_coordinate_written":true,"tp_active_slots":2,"fp_active_slots":2,"calls":calls,"state_sha256":"d".repeat(64)},"suffix":null,"action_invocation_returned":false,"callback_returned":false,"state_sha256":"e".repeat(64)}})
}
pub(in crate::menu::game_session) fn returned_component() -> Value {
    let mut component = pending_component();
    component["stage"] = json!("player_callback_tail_pending");
    component["pending_dependency"] = json!("player_callback_tail");
    let suffix = component["initial_action"]["suffix"].clone();
    let repeated = &mut component["repeat_action"];
    repeated["stage"] = json!("returned");
    repeated["action_invocation_returned"] = json!(true);
    repeated["suffix"] = suffix;
    repeated["suffix"]["repeat_generic"] = Value::Null;
    repeated["suffix"]["repeat_parameter_bits"] = Value::Null;
    repeated["suffix"]["parameter_visits"] = json!(12);
    repeated["suffix"]["parameter_writes"] = json!(12);
    component
}
fn native_component() -> Value {
    let mut component = returned_component();
    component["active_slots"] = json!(4);
    let repeated = &mut component["repeat_action"];
    repeated["map_rows_read"] = json!(3);
    repeated["fp_mapped"] = json!(0x0900_0b70);
    repeated["tp_mapped"] = json!(0x0900_0b70);
    repeated["tp_secondary_mapped"] = json!(0x0900_0b73);
    repeated["dispatch"]["tp_active_slots"] = json!(4);
    repeated["suffix"]["parameter_visits"] = json!(14);
    repeated["suffix"]["parameter_writes"] = json!(14);
    for (ordinal, index, attempts, lookups, rows, set, requested) in [
        (0, 169, 2, 3, 150, 156, 0x0900_0b70u32),
        (1, 172, 2, 3, 153, 159, 0x0900_0b73),
        (2, 143, 1, 2, 139, 85, 0x0900_0b70),
    ] {
        let call = &mut repeated["dispatch"]["calls"][ordinal];
        call["requested"] = json!(requested);
        call["selector"]["requested"] = json!(requested);
        call["selector"]["target_index"] = json!(index);
        call["selector"]["attempts"] = json!(attempts);
        call["selector"]["lookups"] = json!(lookups);
        call["selector"]["transition_rows"] = json!(rows);
        call["completed_commands"][0]["set_reference"] = json!(set);
        call["completed_commands"][0]["set_index"] = json!(set);
        call["completed_commands"][0]["effective_flags_or"] = json!(35);
    }
    component
}
#[test]
fn native_tutorial_repeat_gate_requires_the_observed_second_invocation() {
    let component = native_component();
    let player =
        json!({"pending_dependency":"player_callback_tail","component_initialization":component});
    super::super::validate_tutorial_repeat(&player).unwrap();
    for (key, value) in [
        ("map_rows_read", json!(4)),
        ("fp_mapped", json!(0x0900_0766)),
        ("callback_returned", json!(true)),
    ] {
        let mut bad = player.clone();
        bad["component_initialization"]["repeat_action"][key] = value;
        assert!(
            super::super::validate_tutorial_repeat(&bad).is_err(),
            "accepted {key}"
        );
    }
    for (key, value) in [
        ("transition_rows", json!(149)),
        ("commands_executed", json!(0)),
        ("return_word", json!(0x0900_06c0)),
    ] {
        let mut bad = player.clone();
        bad["component_initialization"]["repeat_action"]["dispatch"]["calls"][0]["selector"][key] =
            value;
        assert!(
            super::super::validate_tutorial_repeat(&bad).is_err(),
            "accepted selector {key}"
        );
    }
    for (key, value) in [
        ("set_reference", json!(157)),
        ("effective_flags_or", json!(39)),
        ("children_completed", json!(0)),
    ] {
        let mut bad = player.clone();
        bad["component_initialization"]["repeat_action"]["dispatch"]["calls"][0]["completed_commands"]
            [0][key] = value;
        assert!(
            super::super::validate_tutorial_repeat(&bad).is_err(),
            "accepted body {key}"
        );
    }
    let mut bad = player;
    bad["component_initialization"]["repeat_action"]["suffix"]["parameter_writes"] = json!(13);
    assert!(super::super::validate_tutorial_repeat(&bad).is_err());
}
#[test]
fn repeated_return_keeps_first_snapshot_and_callback_completion_separate() {
    let pending = pending_component();
    validate(&pending["repeat_action"], &pending).unwrap();
    let returned = returned_component();
    validate(&returned["repeat_action"], &returned).unwrap();
    assert_eq!(pending["initial_action"], returned["initial_action"]);
    assert_ne!(
        returned["active_slots"],
        returned["initial_action"]["tp_command"]["active_slots"]
    );
    for (key, value) in [
        ("callback_returned", json!(true)),
        ("action_invocation_returned", json!(false)),
        ("generic_request", json!(0x0900_071e)),
        ("parameter_bits", json!(0)),
        ("dispatch", Value::Null),
        ("suffix", Value::Null),
        ("raw_words", json!([1, 2, 3, 4])),
    ] {
        let mut bad = returned.clone();
        bad["repeat_action"][key] = value;
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in returned["repeat_action"].as_object().unwrap().keys() {
        let mut bad = returned.clone();
        bad["repeat_action"].as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
}
#[test]
fn pending_call_never_becomes_a_setter_return_or_acknowledged_body() {
    let mut component = pending_component();
    let repeated = &mut component["repeat_action"];
    repeated["stage"] = json!("dispatch_pending");
    let dispatch = &mut repeated["dispatch"];
    dispatch["phase"] = json!("first_person");
    dispatch["fp_coordinate_bits"] = Value::Null;
    dispatch["fp_coordinate_written"] = Value::Null;
    let call = &mut dispatch["calls"][2];
    call["stage"] = json!("command_pending");
    call["setter_return_word"] = Value::Null;
    call["history_updated"] = json!(false);
    call["completed_commands"] = json!([]);
    call["selector"]["stage"] = json!("command_pending");
    call["selector"]["commands_executed"] = json!(0);
    call["selector"]["pending_ordinal"] = json!(0);
    call["selector"]["pending_opcode"] = json!(0x0c00_0002);
    call["selector"]["return_word"] = Value::Null;
    call["selector"]["command_effects_complete"] = json!(false);
    let mut pending = trace();
    pending["selector_acknowledged"] = json!(false);
    call["pending_command"] = pending;
    validate(&component["repeat_action"], &component).unwrap();
    for (key, value) in [
        ("setter_return_word", json!(0x0900_000b)),
        ("history_updated", json!(true)),
        ("stage", json!("returned")),
    ] {
        let mut bad = component.clone();
        bad["repeat_action"]["dispatch"]["calls"][2][key] = value;
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted {key}"
        );
    }
    let mut bad = component.clone();
    bad["repeat_action"]["dispatch"]["calls"][2]["pending_command"]["selector_acknowledged"] =
        json!(true);
    assert!(validate(&bad["repeat_action"], &bad).is_err());
}
#[test]
fn source_order_receiver_history_and_current_slots_are_strict() {
    let component = returned_component();
    for (key, value) in [
        ("override_writes", json!(1)),
        ("seek_returned", json!(false)),
        ("seek_bits", json!(1)),
        ("captured_head_present", json!(false)),
        ("tp_active_slots", json!(4)),
        ("calls", json!([])),
        ("fp_coordinate_written", Value::Null),
        ("state_sha256", json!("bad")),
    ] {
        let mut bad = component.clone();
        bad["repeat_action"]["dispatch"][key] = value;
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted dispatch {key}"
        );
    }
    for (key, value) in [
        ("target", json!("first_person")),
        ("component_file", json!(0x0102_0016)),
        ("requested", json!(0)),
        ("history_updated", json!(true)),
        ("previous", json!(0)),
        ("state_sha256", json!("bad")),
    ] {
        let mut bad = component.clone();
        bad["repeat_action"]["dispatch"]["calls"][0][key] = value;
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted call {key}"
        );
    }
    for key in component["repeat_action"]["dispatch"]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = component.clone();
        bad["repeat_action"]["dispatch"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted missing dispatch {key}"
        );
    }
    for key in component["repeat_action"]["dispatch"]["calls"][0]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = component.clone();
        bad["repeat_action"]["dispatch"]["calls"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted missing call {key}"
        );
    }
    for key in component["repeat_action"]["dispatch"]["calls"][0]["completed_commands"][0]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = component.clone();
        bad["repeat_action"]["dispatch"]["calls"][0]["completed_commands"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["repeat_action"], &bad).is_err(),
            "accepted missing command trace {key}"
        );
    }
}
#[test]
fn missing_receipt_and_selecting_cursor_preserve_the_actual_prefix() {
    let mut component = pending_component();
    component["repeat_action"]["stage"] = json!("dispatch_pending");
    let dispatch = &mut component["repeat_action"]["dispatch"];
    dispatch["phase"] = json!("first_third_person");
    dispatch["calls"].as_array_mut().unwrap().truncate(1);
    dispatch["tp_coordinate_written"] = Value::Null;
    dispatch["seek_bits"] = Value::Null;
    dispatch["seek_node_present"] = json!(false);
    dispatch["seek_returned"] = json!(false);
    dispatch["override_writes"] = json!(0);
    dispatch["fp_coordinate_bits"] = Value::Null;
    dispatch["fp_coordinate_written"] = Value::Null;
    let call = &mut dispatch["calls"][0];
    call["stage"] = json!("selecting");
    call["completed_commands"] = json!([]);
    call["setter_return_word"] = Value::Null;
    let cursor = &mut call["selector"];
    cursor["stage"] = json!("selecting");
    cursor["target_index"] = Value::Null;
    cursor["attempts"] = json!(0);
    cursor["lookups"] = json!(0);
    cursor["transition_rows"] = json!(0);
    cursor["commands_issued"] = json!(0);
    cursor["commands_executed"] = json!(0);
    cursor["return_word"] = Value::Null;
    cursor["command_effects_complete"] = json!(false);
    validate(&component["repeat_action"], &component).unwrap();
    component["repeat_action"] = Value::Null;
    assert!(validate(&component["repeat_action"], &component).is_err());
}
