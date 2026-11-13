use super::*;
use serde_json::json;
pub(in crate::menu::game_session) fn completed_component() -> Value {
    let mut component = super::super::player_callback_tail::tests::returned_component();
    component["stage"] = json!("player_pose_setup_pending");
    component["pending_dependency"] = json!("player_pose_setup");
    component["constructor_tail"] = json!({"stage":"awaiting_pose","pending_dependency":"constructor_pose","saved_state_704":0,"state_restored":false,"script_reference":0x0400_1dac,"metadata_reads":[{"slot":23,"field":"type","value":0x4b20_0000},{"slot":23,"field":"inventory_key","value":0x0200_0830},{"slot":23,"field":"primary_key","value":0x4b00_002c}],"current_slot":0,"current_record_present":true,"current_inventory_key":0x0200_0830,"variant":0,"cleanup_checks":[true,true,true,true,true],"b84_written":true,"bd1_written":true,"actor_absent":true,"override_removals":[{"key":0x0600_056d,"count_raw":0,"removed":false},{"key":0x0600_067b,"count_raw":0,"removed":false}],"helper_return":1,"pose_input_raw":254,"pose_input_signed":-2,"state_sha256":"a".repeat(64)});
    component
}
#[test]
fn completed_helpers_only_reach_pending_pose_and_leave_snapshots_historical() {
    let component = completed_component();
    validate(&component["constructor_tail"], &component).unwrap();
    super::super::player_callback_tail::validate(&component["callback_tail"], &component).unwrap();
    super::super::player_repeat::validate(&component["repeat_action"], &component).unwrap();
    assert_eq!(component["initial_action"]["callback_returned"], false);
    assert_eq!(component["repeat_action"]["callback_returned"], false);
    assert_eq!(component["callback_tail"]["callback_returned"], true);
    for (key, value) in [
        ("stage", json!("complete")),
        ("pending_dependency", Value::Null),
        ("script_reference", json!(0)),
        ("helper_return", json!(0)),
        ("pose_input_signed", json!(254)),
        ("state_restored", json!(true)),
        ("player_created", json!(true)),
        ("private_words", json!([1, 2, 3, 4])),
    ] {
        let mut bad = component.clone();
        bad["constructor_tail"][key] = value;
        assert!(
            validate(&bad["constructor_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in component["constructor_tail"].as_object().unwrap().keys() {
        let mut bad = component.clone();
        bad["constructor_tail"].as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad["constructor_tail"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
    let mut bad = component;
    bad["constructor_tail"] = Value::Null;
    assert!(validate(&bad["constructor_tail"], &bad).is_err());
}
#[test]
fn tutorial_gate_requires_the_observed_constructor_cleanup_path() {
    let mut component = completed_component();
    component["constructor_tail"]["pose_input_raw"] = json!(1);
    component["constructor_tail"]["pose_input_signed"] = json!(1);
    let player =
        json!({"pending_dependency":"player_pose_setup","component_initialization":component});
    super::super::validate_tutorial_constructor_tail(&player).unwrap();
    for (key, value) in [
        ("helper_return", json!(0)),
        ("current_inventory_key", json!(0x0200_1e4d)),
        ("state_restored", json!(true)),
        ("bd1_written", json!(false)),
        ("pose_input_signed", json!(-1)),
    ] {
        let mut bad = player.clone();
        bad["component_initialization"]["constructor_tail"][key] = value;
        assert!(
            super::super::validate_tutorial_constructor_tail(&bad).is_err(),
            "accepted {key}"
        );
    }
    let mut bad = player.clone();
    bad["component_initialization"]["constructor_tail"]["metadata_reads"][0]["slot"] = json!(22);
    assert!(super::super::validate_tutorial_constructor_tail(&bad).is_err());
    let mut bad = player;
    bad["component_initialization"]["constructor_tail"]["override_removals"][0]["count_raw"] =
        json!(u32::MAX);
    assert!(super::super::validate_tutorial_constructor_tail(&bad).is_err());
}
fn child_pending() -> Value {
    let mut component = completed_component();
    component["stage"] = json!("player_constructor_tail_pending");
    component["pending_dependency"] = json!("player_constructor_tail");
    let tail = &mut component["constructor_tail"];
    tail["stage"] = json!("child_cleanup");
    tail["pending_dependency"] = json!("constructor_child_cleanup");
    tail["cleanup_checks"] = json!([true, true, false, null, null]);
    tail["bd1_written"] = json!(false);
    tail["actor_absent"] = Value::Null;
    tail["override_removals"] = json!([null, null]);
    tail["helper_return"] = Value::Null;
    tail["pose_input_raw"] = Value::Null;
    tail["pose_input_signed"] = Value::Null;
    component
}
#[test]
fn nonnull_child_link_retains_the_b84_store_before_its_release_dependency() {
    let component = child_pending();
    validate(&component["constructor_tail"], &component).unwrap();
    for (key, value) in [
        ("b84_written", json!(false)),
        ("bd1_written", json!(true)),
        ("helper_return", json!(1)),
        ("pending_dependency", json!("constructor_pose")),
    ] {
        let mut bad = component.clone();
        bad["constructor_tail"][key] = value;
        assert!(
            validate(&bad["constructor_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
    let mut bad = component;
    bad["constructor_tail"]["cleanup_checks"][2] = json!(true);
    assert!(validate(&bad["constructor_tail"], &bad).is_err());
}
#[test]
fn unsupported_type_and_normal_binding_keep_later_fields_unread() {
    let mut component = child_pending();
    let tail = &mut component["constructor_tail"];
    tail["stage"] = json!("select_script");
    tail["pending_dependency"] = json!("constructor_script_selection");
    tail["script_reference"] = Value::Null;
    tail["metadata_reads"] = json!([{"slot":23,"field":"type","value":0x4b20_0001},null,null]);
    for key in [
        "current_slot",
        "current_record_present",
        "current_inventory_key",
        "variant",
    ] {
        tail[key] = Value::Null;
    }
    tail["cleanup_checks"] = json!([null, null, null, null, null]);
    tail["b84_written"] = json!(false);
    validate(&component["constructor_tail"], &component).unwrap();
    let mut bad = component.clone();
    bad["constructor_tail"]["metadata_reads"][1] =
        json!({"slot":23,"field":"inventory_key","value":0x0200_0830});
    assert!(validate(&bad["constructor_tail"], &bad).is_err());
    let tail = &mut component["constructor_tail"];
    tail["stage"] = json!("cleanup_key");
    tail["pending_dependency"] = json!("constructor_component_binding");
    tail["script_reference"] = json!(0x0400_1dac);
    tail["metadata_reads"] = json!([null, null, null]);
    tail["current_slot"] = json!(-1);
    tail["current_record_present"] = json!(false);
    tail["current_inventory_key"] = json!(0);
    tail["variant"] = json!(0);
    validate(&component["constructor_tail"], &component).unwrap();
    component["constructor_tail"]["current_inventory_key"] = json!(0x0200_0830);
    assert!(validate(&component["constructor_tail"], &component).is_err());
}
#[test]
fn override_noops_preserve_signed_count_and_never_claim_removal_or_generation_writes() {
    let mut component = completed_component();
    component["constructor_tail"]["override_removals"][0]["count_raw"] = json!(u32::MAX);
    validate(&component["constructor_tail"], &component).unwrap();
    for (key, value) in [
        ("count_raw", json!(1)),
        ("removed", json!(true)),
        ("key", json!(0x0600_067b)),
        ("generation", json!(2)),
    ] {
        let mut bad = component.clone();
        bad["constructor_tail"]["override_removals"][0][key] = value;
        assert!(
            validate(&bad["constructor_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in component["constructor_tail"]["override_removals"][0]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = component.clone();
        bad["constructor_tail"]["override_removals"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["constructor_tail"], &bad).is_err(),
            "accepted missing removal {key}"
        );
    }
}
#[test]
fn saved_entry_predicate_does_not_use_the_callback_diagnostic_old_state() {
    let mut component = completed_component();
    component["constructor_tail"]["saved_state_704"] = json!(5);
    component["constructor_tail"]["state_restored"] = json!(true);
    component["callback_tail"]["initial_state_704"] = Value::Null;
    validate(&component["constructor_tail"], &component).unwrap();
    let mut bad = component;
    bad["constructor_tail"]["state_restored"] = json!(false);
    assert!(validate(&bad["constructor_tail"], &bad).is_err());
}
#[test]
fn helper_return_precedes_the_signed_pose_argument_read() {
    let mut component = completed_component();
    component["stage"] = json!("player_constructor_tail_pending");
    component["pending_dependency"] = json!("player_constructor_tail");
    let tail = &mut component["constructor_tail"];
    tail["stage"] = json!("pose_input");
    tail["pending_dependency"] = Value::Null;
    tail["pose_input_raw"] = Value::Null;
    tail["pose_input_signed"] = Value::Null;
    validate(&component["constructor_tail"], &component).unwrap();
    component["constructor_tail"]["helper_return"] = Value::Null;
    assert!(validate(&component["constructor_tail"], &component).is_err());
}
