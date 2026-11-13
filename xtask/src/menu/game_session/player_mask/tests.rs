use super::*;
use serde_json::json;
pub(in crate::menu::game_session) fn fixture() -> Value {
    let mut refresh = super::super::player_refresh::tests::fixture();
    refresh["refresh_ordinal"] = json!(4);
    refresh["allocation_ordinal"] = json!(4);
    json!({"mask_raw":3,"active_slots_before":1,"slots_visited":1,"selected_mask":3,"clear_mask":251,"controls_allocated":1,"controls_inserted":1,"capture_returned":true,"refresh":refresh,"state_sha256":"d".repeat(64)})
}
#[test]
fn mask_three_visits_one_active_slot_instead_of_its_highest_set_bit() {
    validate(&fixture(), "command_returned", 3, 1).unwrap();
    for (key, value) in [
        ("active_slots_before", json!(2)),
        ("slots_visited", json!(2)),
        ("controls_inserted", json!(2)),
        ("controls_allocated", json!(2)),
        ("selected_mask", json!(4)),
        ("capture_returned", json!(false)),
        ("refresh", Value::Null),
    ] {
        let mut bad = fixture();
        bad[key] = value;
        assert!(
            validate(&bad, "command_returned", 3, 1).is_err(),
            "accepted {key}"
        );
    }
}
#[test]
fn backing_failure_retains_refresh_before_capture_and_loop_fields() {
    let mut e = fixture();
    for key in ["active_slots_before", "selected_mask", "clear_mask"] {
        e[key] = Value::Null;
    }
    e["slots_visited"] = json!(0);
    e["controls_inserted"] = json!(0);
    e["capture_returned"] = json!(false);
    validate(&e, "mask_backing_pending", 3, 1).unwrap();
    let mut bad = e.clone();
    bad["selected_mask"] = json!(3);
    assert!(validate(&bad, "mask_backing_pending", 3, 1).is_err());
    e["refresh"] = Value::Null;
    validate(&e, "mask_refresh_pending", 3, 1).unwrap();
    assert!(validate(&e, "mask_backing_pending", 3, 1).is_err());
    e["refresh"] = fixture()["refresh"].clone();
    e["capture_returned"] = json!(true);
    validate(&e, "mask_effects_pending", 3, 1).unwrap();
}
#[test]
fn pending_destruction_does_not_publish_an_insertion_or_return() {
    let mut e = fixture();
    e["controls_inserted"] = json!(0);
    validate(&e, "mask_destroy_pending", 3, 1).unwrap();
    assert!(validate(&e, "command_returned", 3, 1).is_err());
    e["clear_mask"] = json!(0);
    assert!(validate(&e, "mask_destroy_pending", 3, 1).is_err());
}
#[test]
fn clear_only_return_never_allocates_or_claims_capture() {
    let mut e = fixture();
    e["mask_raw"] = json!(0);
    e["selected_mask"] = json!(0);
    e["controls_allocated"] = json!(0);
    e["controls_inserted"] = json!(0);
    e["capture_returned"] = json!(false);
    e["refresh"] = Value::Null;
    validate(&e, "command_returned", 0, 1).unwrap();
    e["controls_allocated"] = json!(1);
    assert!(validate(&e, "command_returned", 0, 1).is_err());
    validate(&Value::Null, "mask_clear_pending", 0, 1).unwrap();
    assert!(validate(&fixture(), "mask_clear_pending", 0, 1).is_err());
}
#[test]
fn rejects_missing_fields_unowned_refresh_and_private_payloads() {
    let e = fixture();
    for key in e.as_object().unwrap().keys() {
        let mut bad = e.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad, "command_returned", 3, 1).is_err(),
            "accepted missing {key}"
        );
    }
    let mut bad = e.clone();
    bad["refresh"]["scope_released"] = json!(false);
    assert!(validate(&bad, "command_returned", 3, 1).is_err());
    let mut bad = e;
    bad["pose"] = json!([1, 2, 3]);
    assert!(validate(&bad, "command_returned", 3, 1).is_err());
}
