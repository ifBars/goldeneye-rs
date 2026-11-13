use super::*;
use serde_json::json;

pub(in crate::menu::game_session) fn returned_component() -> Value {
    let mut component = super::super::player_repeat::tests::returned_component();
    component["stage"] = json!("player_constructor_tail_pending");
    component["pending_dependency"] = json!("player_constructor_tail");
    component["callback_tail"] = json!({"stage":"returned","logging_noop_returned":true,"e59_raw":0,"initial_state_704":0,"state_704":2,"physics_outcome":"flags_cleared","physics_flags_before":65535,"physics_flags_after":64764,"word_708":1,"flags_220_before":2147483648u32,"flags_220_after":2147483649u32,"callback_returned":true,"state_sha256":"a".repeat(64)});
    component
}
#[test]
fn actual_callback_return_keeps_both_invocations_historical_and_constructor_pending() {
    let component = returned_component();
    validate(&component["callback_tail"], &component).unwrap();
    assert_eq!(component["initial_action"]["callback_returned"], false);
    assert_eq!(component["repeat_action"]["callback_returned"], false);
    super::super::player_repeat::validate(&component["repeat_action"], &component).unwrap();
    for (key, value) in [
        ("callback_returned", json!(false)),
        ("logging_noop_returned", json!(false)),
        ("state_704", json!(3)),
        ("physics_flags_after", json!(65535)),
        ("word_708", json!(0)),
        ("flags_220_after", json!(2147483648u32)),
        ("state_sha256", json!("bad")),
        ("private_words", json!([1, 2, 3, 4])),
    ] {
        let mut bad = component.clone();
        bad["callback_tail"][key] = value;
        assert!(
            validate(&bad["callback_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in component["callback_tail"].as_object().unwrap().keys() {
        let mut bad = component.clone();
        bad["callback_tail"].as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad["callback_tail"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
    let mut missing = component;
    missing["callback_tail"] = Value::Null;
    assert!(validate(&missing["callback_tail"], &missing).is_err());
}
#[test]
fn tutorial_gate_requires_the_observed_actual_callback_tail() {
    let mut component = returned_component();
    component["callback_tail"]["physics_flags_before"] = json!(0x4000);
    component["callback_tail"]["physics_flags_after"] = json!(0x4000);
    component["callback_tail"]["flags_220_before"] = json!(0x17);
    component["callback_tail"]["flags_220_after"] = json!(0x17);
    let player = json!({"pending_dependency":"player_constructor_tail","component_initialization":component});
    super::super::validate_tutorial_callback_tail(&player).unwrap();
    for (key, value) in [
        ("initial_state_704", Value::Null),
        ("physics_flags_before", json!(0x4001)),
        ("flags_220_before", json!(0x16)),
        ("callback_returned", json!(false)),
    ] {
        let mut bad = player.clone();
        bad["component_initialization"]["callback_tail"][key] = value;
        assert!(
            super::super::validate_tutorial_callback_tail(&bad).is_err(),
            "accepted {key}"
        );
    }
    let mut bad = player;
    bad["component_initialization"]["repeat_action"]["callback_returned"] = json!(true);
    assert!(super::super::validate_tutorial_callback_tail(&bad).is_err());
}
#[test]
fn pending_physics_does_not_become_a_missing_owner_or_callback_return() {
    let mut component = returned_component();
    component["stage"] = json!("player_callback_tail_pending");
    let tail = &mut component["callback_tail"];
    tail["stage"] = json!("physics");
    tail["callback_returned"] = json!(false);
    for key in [
        "physics_outcome",
        "physics_flags_before",
        "physics_flags_after",
        "word_708",
        "flags_220_before",
        "flags_220_after",
    ] {
        tail[key] = Value::Null;
    }
    validate(&component["callback_tail"], &component).unwrap();
    for (key, value) in [
        ("physics_outcome", json!("absent")),
        ("callback_returned", json!(true)),
        ("word_708", json!(1)),
    ] {
        let mut bad = component.clone();
        bad["callback_tail"][key] = value;
        assert!(
            validate(&bad["callback_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn source_byte_width_missing_physics_and_final_flag_width_are_preserved() {
    let mut component = returned_component();
    component["callback_tail"]["initial_state_704"] = Value::Null;
    validate(&component["callback_tail"], &component).unwrap();
    component["callback_tail"]["e59_raw"] = json!(255);
    component["callback_tail"]["state_704"] = json!(3);
    component["callback_tail"]["physics_outcome"] = json!("absent");
    component["callback_tail"]["physics_flags_before"] = Value::Null;
    component["callback_tail"]["physics_flags_after"] = Value::Null;
    validate(&component["callback_tail"], &component).unwrap();
    for (key, value) in [
        ("e59_raw", json!(256)),
        ("physics_flags_before", json!(0)),
        ("flags_220_before", json!(4294967296u64)),
    ] {
        let mut bad = component.clone();
        bad["callback_tail"][key] = value;
        assert!(
            validate(&bad["callback_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn early_tail_prefixes_never_claim_later_store_fields() {
    let mut component = returned_component();
    component["stage"] = json!("player_callback_tail_pending");
    let tail = &mut component["callback_tail"];
    tail["stage"] = json!("logging");
    tail["logging_noop_returned"] = json!(false);
    tail["callback_returned"] = json!(false);
    for key in [
        "e59_raw",
        "initial_state_704",
        "state_704",
        "physics_outcome",
        "physics_flags_before",
        "physics_flags_after",
        "word_708",
        "flags_220_before",
        "flags_220_after",
    ] {
        tail[key] = Value::Null;
    }
    validate(&component["callback_tail"], &component).unwrap();
    component["callback_tail"]["stage"] = json!("derived_timer");
    component["callback_tail"]["logging_noop_returned"] = json!(true);
    validate(&component["callback_tail"], &component).unwrap();
    component["callback_tail"]["e59_raw"] = json!(0);
    assert!(validate(&component["callback_tail"], &component).is_err());
}
