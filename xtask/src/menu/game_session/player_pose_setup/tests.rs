use super::*;
use serde_json::json;
pub(in crate::menu::game_session) fn completed_component() -> Value {
    let mut c = super::super::player_constructor_tail::tests::completed_component();
    c["constructor_tail"]["pose_input_raw"] = json!(1);
    c["constructor_tail"]["pose_input_signed"] = json!(1);
    c["stage"] = json!("player_component_tail_pending");
    c["pending_dependency"] = json!("player_component_tail");
    c["pose_setup"] = json!({"stage":"returned","pending_dependency":null,"input_raw":1,"input_signed":1,"helper_returned":true,"entry_702":0,"blend_ba8":0,"bab_written":true,"tp_enabled":false,"fp_enabled":true,"receiver_searches":[true,true],"enable_writes":[{"component_file":0x0102_0017,"reference":0x0400_0101,"enabled":false,"reference_matched":true,"flags_before":32791,"flags_after":32790,"invalidated_8e":false},{"component_file":0x0102_0016,"reference":0x0400_0102,"enabled":true,"reference_matched":true,"flags_before":32790,"flags_after":32791,"invalidated_8e":true}],"character_bound":true,"character_scalars_read":2,"character_vector_read":true,"session_scale_multiple":false,"scale_source":"startup_ones","scale_stored":true,"scale_invalidated":true,"dirty_before":51,"dirty_after":19,"ca4_written":true,"mode_afc_absent":true,"force_nodes_checked":2,"user_target_bound":true,"user_lookup":{"visited":1,"matched":false,"association_state":"known_null","association_origin":"local_user_constructor","state_sha256":"a".repeat(64)},"false_arm_b00_absent":true,"backlink_nodes_checked":2,"attachment_absent":true,"weapon_global_read":true,"weapon_global_byte":null,"global_argument":null,"global_equivalence_used":true,"global_attachment_absent":true,"fade_inputs_read":true,"fade_argument_ready":true,"fade_attachment_absent":true,"final_session_multiple":false,"state_sha256":"b".repeat(64)});
    c
}
#[test]
fn nested_helper_return_preserves_unknown_global_and_constructor_scope() {
    let c = completed_component();
    validate(&c["pose_setup"], &c).unwrap();
    assert!(c["pose_setup"]["weapon_global_byte"].is_null());
    assert_eq!(c["initial_action"]["callback_returned"], false);
    for (key, value) in [
        ("helper_returned", json!(false)),
        ("global_argument", json!(0)),
        ("global_equivalence_used", json!(false)),
        ("global_attachment_absent", json!(false)),
        ("character_scalars_read", json!(1)),
        ("scale_invalidated", json!(false)),
        ("scale_source", json!("character")),
        ("camera_ready", json!(true)),
        ("vectors", json!([1, 2, 3, 4])),
    ] {
        let mut bad = c.clone();
        bad["pose_setup"][key] = value;
        assert!(
            validate(&bad["pose_setup"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in c["pose_setup"].as_object().unwrap().keys() {
        let mut bad = c.clone();
        bad["pose_setup"].as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad["pose_setup"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
}
#[test]
fn known_global_arguments_and_actual_semantic_enable_widths_are_distinct() {
    for byte in [0, 255] {
        let mut c = completed_component();
        let p = &mut c["pose_setup"];
        p["weapon_global_byte"] = json!(byte);
        p["global_argument"] = json!(if byte == 0 { 1 } else { 0 });
        p["global_equivalence_used"] = json!(false);
        validate(&c["pose_setup"], &c).unwrap();
    }
    let c = completed_component();
    for (key, value) in [
        ("flags_after", json!(32791)),
        ("flags_before", json!(65536)),
        ("invalidated_8e", json!(true)),
        ("reference", json!(0x0200_0001)),
        ("component_file", json!(0x0102_0016)),
    ] {
        let mut bad = c.clone();
        bad["pose_setup"]["enable_writes"][0][key] = value;
        assert!(
            validate(&bad["pose_setup"], &bad).is_err(),
            "accepted enable {key}"
        );
    }
}
#[test]
fn local_user_lookup_requires_its_actual_lifetime_not_created_or_ready_flags() {
    let c = completed_component();
    for (key, value) in [
        ("visited", json!(0)),
        ("matched", json!(true)),
        ("association_state", json!("unavailable")),
        ("association_origin", json!("created_byte_zero")),
        ("state_sha256", json!("bad")),
    ] {
        let mut bad = c.clone();
        bad["pose_setup"]["user_lookup"][key] = value;
        assert!(
            validate(&bad["pose_setup"], &bad).is_err(),
            "accepted lookup {key}"
        );
    }
}
#[test]
fn partial_scale_stores_precede_invalidation_and_later_noops() {
    let mut c = completed_component();
    c["stage"] = json!("player_pose_setup_pending");
    c["pending_dependency"] = json!("player_pose_setup");
    let p = &mut c["pose_setup"];
    p["stage"] = json!("scale_invalidate");
    p["helper_returned"] = json!(false);
    p["scale_invalidated"] = json!(false);
    for key in [
        "dirty_before",
        "dirty_after",
        "mode_afc_absent",
        "user_lookup",
        "false_arm_b00_absent",
        "attachment_absent",
        "global_attachment_absent",
        "fade_attachment_absent",
        "final_session_multiple",
    ] {
        p[key] = Value::Null;
    }
    for key in [
        "ca4_written",
        "user_target_bound",
        "weapon_global_read",
        "global_equivalence_used",
        "fade_inputs_read",
        "fade_argument_ready",
    ] {
        p[key] = json!(false);
    }
    p["force_nodes_checked"] = json!(0);
    p["backlink_nodes_checked"] = json!(0);
    validate(&c["pose_setup"], &c).unwrap();
    c["pose_setup"]["scale_stored"] = json!(false);
    assert!(validate(&c["pose_setup"], &c).is_err());
}
