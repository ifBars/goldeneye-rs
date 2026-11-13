use super::*;
use serde_json::json;
fn resident(index: u32) -> Value {
    json!({"outcome":"resident","directory_index":index,"attribute_present":true,"state_sha256":"a".repeat(64)})
}
fn pending() -> Value {
    json!({"outcome":"pending","directory_index":null,"attribute_present":null,"state_sha256":"b".repeat(64)})
}
fn helper_fixture(target: &str) -> Value {
    json!({"target":target,"receiver_searched":true,"receiver_bound":true,"lookups":[resident(0),resident(1)],"applied_slots":if target=="first_person" {json!([0,1,2])}else{json!([0,1,2,3])},"returned":true,"state_sha256":"c".repeat(64)})
}
pub(in crate::menu::game_session) fn returned_action() -> Value {
    let mut action = super::super::player_fp::tests::native_action();
    action["stage"] = json!("action_dispatch_repeat_pending");
    action["action_invocation_returned"] = json!(true);
    action["suffix"] = json!({"stage":"returned","classification":1,"mask_helpers":[helper_fixture("first_person"),helper_fixture("third_person")],"parameter_visits":11,"parameter_writes":11,"pending_dependency":null,"repeat_generic":0x0900_0765,"repeat_parameter_bits":0x7f7f_ffff,"state_sha256":"d".repeat(64)});
    action
}
#[test]
fn first_action_return_keeps_enclosing_callback_and_repeat_execution_pending() {
    let action = returned_action();
    super::super::player_action::validate(&action).unwrap();
    assert_eq!(action["callback_returned"], false);
    assert_eq!(action["generic_request"], 0x0900_071e);
    for (key, value) in [
        ("callback_returned", json!(true)),
        ("action_invocation_returned", json!(false)),
        ("stage", json!("complete")),
        ("suffix", Value::Null),
    ] {
        let mut bad = action.clone();
        bad[key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
    for (key, value) in [
        ("repeat_generic", json!(0x0900_071e)),
        ("repeat_parameter_bits", json!(0)),
        ("parameter_visits", json!(15)),
        ("parameter_writes", json!(12)),
        ("pending_dependency", json!("still_pending")),
        ("classification", json!(4)),
        ("state_sha256", json!("short")),
        ("mask_words", json!([1, 2, 3, 4])),
    ] {
        let mut bad = action.clone();
        bad["suffix"][key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
    for key in action["suffix"].as_object().unwrap().keys() {
        let mut bad = action.clone();
        bad["suffix"].as_object_mut().unwrap().remove(key);
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted missing {key}"
        );
    }
}
#[test]
fn pending_mask_body_never_becomes_absence_or_a_completed_helper() {
    let mut action = returned_action();
    action["stage"] = json!("initial_action_suffix_pending");
    action["action_invocation_returned"] = json!(false);
    let s = &mut action["suffix"];
    s["stage"] = json!("first_person_masks");
    s["parameter_visits"] = json!(0);
    s["parameter_writes"] = json!(0);
    s["pending_dependency"] = json!("bone_masks");
    s["repeat_generic"] = Value::Null;
    s["repeat_parameter_bits"] = Value::Null;
    let fp = &mut s["mask_helpers"][0];
    fp["lookups"][1] = pending();
    fp["applied_slots"] = json!([]);
    fp["returned"] = json!(false);
    let tp = &mut s["mask_helpers"][1];
    tp["receiver_searched"] = json!(false);
    tp["receiver_bound"] = json!(false);
    tp["lookups"] = json!([null, null]);
    tp["applied_slots"] = json!([]);
    tp["returned"] = json!(false);
    super::super::player_action::validate(&action).unwrap();
    for change in [0, 1, 2] {
        let mut bad = action.clone();
        match change {
            0 => bad["suffix"]["mask_helpers"][0]["returned"] = json!(true),
            1 => bad["suffix"]["mask_helpers"][0]["lookups"][1]["outcome"] = json!("missing_body"),
            _ => bad["suffix"]["parameter_visits"] = json!(1),
        }
        assert!(super::super::player_action::validate(&bad).is_err());
    }
}
#[test]
fn confirmed_missing_resource_returns_without_mask_writes_but_absent_attribute_stays_resident() {
    let mut action = returned_action();
    let fp = &mut action["suffix"]["mask_helpers"][0];
    fp["lookups"][0] = json!({"outcome":"missing_descriptor","directory_index":null,"attribute_present":null,"state_sha256":"e".repeat(64)});
    fp["lookups"][1] = Value::Null;
    fp["applied_slots"] = json!([]);
    super::super::player_action::validate(&action).unwrap();
    let mut bad = action;
    bad["suffix"]["mask_helpers"][0]["applied_slots"] = json!([0]);
    assert!(super::super::player_action::validate(&bad).is_err());
    let mut absent_attribute = returned_action();
    absent_attribute["suffix"]["mask_helpers"][0]["lookups"][0]["attribute_present"] = json!(false);
    super::super::player_action::validate(&absent_attribute).unwrap();
}
#[test]
fn helper_order_metadata_and_physical_slot_prefix_are_strict() {
    let action = returned_action();
    for (key, value) in [
        ("target", json!("third_person")),
        ("receiver_searched", json!(false)),
        ("receiver_bound", json!(false)),
        ("applied_slots", json!([0, 2, 1])),
        ("returned", json!(false)),
        ("state_sha256", json!("short")),
        ("words", json!([1, 2, 3, 4])),
    ] {
        let mut bad = action.clone();
        bad["suffix"]["mask_helpers"][0][key] = value;
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted {key}"
        );
    }
    for key in action["suffix"]["mask_helpers"][0]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = action.clone();
        bad["suffix"]["mask_helpers"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted missing {key}"
        );
    }
    for key in action["suffix"]["mask_helpers"][0]["lookups"][0]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = action.clone();
        bad["suffix"]["mask_helpers"][0]["lookups"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            super::super::player_action::validate(&bad).is_err(),
            "accepted missing lookup {key}"
        );
    }
}
