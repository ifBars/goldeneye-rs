use super::*;
use serde_json::json;
pub(in crate::menu::game_session) fn completed_component() -> Value {
    let mut c = super::super::player_pose_setup::tests::completed_component();
    c["stage"] = json!("player_initializer_pending");
    c["pending_dependency"] = json!("player_initializer");
    c["component_tail"] = json!({"stage":"returned","pending_dependency":null,"helper_return":1,"helper_returned":true,"rotation_reads":[true,true],"rotation_stores":[true,true],"rotation_invalidated":true,"character_outcome":"bound","character_scalars_read":2,"character_scale_ready":true,"session_multiple":false,"scale_stores":[true,true],"scale_invalidated":true,"flags_before":48,"flags_after":51,"state_sha256":"c".repeat(64)});
    c
}
#[test]
fn outer_helper_return_is_separate_from_pose_and_initializer_completion() {
    let c = completed_component();
    validate(&c["component_tail"], &c).unwrap();
    for (key, value) in [
        ("helper_return", json!(0)),
        ("rotation_reads", json!([true, false])),
        ("rotation_stores", json!([false, true])),
        ("character_scalars_read", json!(1)),
        ("scale_stores", json!([true, false])),
        ("flags_after", json!(48)),
        ("player_created", json!(true)),
        ("scale_bits", json!([1, 2, 3, 4])),
    ] {
        let mut bad = c.clone();
        bad["component_tail"][key] = value;
        assert!(
            validate(&bad["component_tail"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in c["component_tail"].as_object().unwrap().keys() {
        let mut bad = c.clone();
        bad["component_tail"].as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad["component_tail"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
}
#[test]
fn explicit_absent_character_skips_all_later_scale_and_flag_work() {
    let mut c = completed_component();
    let t = &mut c["component_tail"];
    t["character_outcome"] = json!("known_absent");
    t["character_scalars_read"] = json!(0);
    t["character_scale_ready"] = json!(false);
    t["session_multiple"] = Value::Null;
    t["scale_stores"] = json!([false, false]);
    t["scale_invalidated"] = json!(false);
    t["flags_before"] = Value::Null;
    t["flags_after"] = Value::Null;
    validate(&c["component_tail"], &c).unwrap();
    c["component_tail"]["character_outcome"] = Value::Null;
    assert!(validate(&c["component_tail"], &c).is_err());
}
#[test]
fn rotation_reads_second_pair_first_and_paired_scale_stores_survive_pending_invalidation() {
    let mut c = completed_component();
    c["stage"] = json!("player_component_tail_pending");
    let t = &mut c["component_tail"];
    t["stage"] = json!("scale_invalidate");
    t["helper_return"] = Value::Null;
    t["helper_returned"] = json!(false);
    t["scale_invalidated"] = json!(false);
    t["flags_before"] = Value::Null;
    t["flags_after"] = Value::Null;
    validate(&c["component_tail"], &c).unwrap();
    c["component_tail"]["scale_stores"] = json!([true, false]);
    assert!(validate(&c["component_tail"], &c).is_err());
}
