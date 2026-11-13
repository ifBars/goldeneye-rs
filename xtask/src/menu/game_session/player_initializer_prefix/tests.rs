use super::*;
use serde_json::json;
fn backing(kind: &str, bytes: u32, known: u32) -> Value {
    json!({"kind":kind,"native_bytes":bytes,"known_bytes":known,"matrix_payload_known_bytes":if kind=="matrix"{json!(known-8)}else{Value::Null}})
}
fn header(n: u32, desc: u32) -> Value {
    json!({"count":n,"association_bound":false,"backings":[backing("matrix",16+64*n,8),backing("validity",n,n),backing("offsets",2*n,2*n),backing("descendants",desc,desc),null],"invalidation_count_reads":0,"invalidation_pointer_reads":0})
}
fn build(slot: &str, n: u32, visits: u32) -> Value {
    json!({"slot":slot,"stage":"returned","bone_count":n,"roots_completed":n,"descendant_visits":visits,"descendant_bytes":visits+n,"offsets_written":n,"allocations":5,"releases":1,"waiting_skeleton":false,"failed":false,"returned":true,"state_sha256":"a".repeat(64)})
}
pub(in crate::menu::game_session) fn completed_component() -> Value {
    let mut c = super::super::player_component_tail::tests::completed_component();
    c["stage"] = json!("player_animation_preload_pending");
    c["pending_dependency"] = json!("player_animation_preload");
    c["initializer_prefix"] = json!({"stage":"awaiting_preload","pending_dependency":"player_animation_preload","item_flags_before":0,"item_flags_after":12,"mode_written":true,"dirty_cleared":true,"scalars_written":true,"receiver_searches":2,"receivers_found":[true,true],"cache_enabled":1,"invalidated":false,"caches":{"selected":"first_person","counts":[2,1],"matrix_backings":[true,true],"associations":[false,false],"native_bytes":237,"allocations":8,"invocation_count":1,"headers":[header(2,3),header(1,1)],"trace":{"ordinal":1,"stage":"returned","receivers_found":[true,true],"selected_before":null,"builds":[build("first_person",2,1),build("third_person",1,0)],"missing_receiver_skip":false,"reused_selected":false,"selection_returned":true,"selected_after":"first_person","failed":false,"returned":true,"state_sha256":"b".repeat(64)},"state_sha256":"c".repeat(64)},"state_sha256":"d".repeat(64)});
    c
}
#[test]
fn actual_cache_build_leaves_preload_flag_and_unwritten_matrix_payload_pending() {
    let c = completed_component();
    validate(&c["initializer_prefix"], &c).unwrap();
    for (key, value) in [
        ("cache_enabled", json!(0)),
        ("invalidated", json!(true)),
        ("item_flags_after", json!(0)),
        ("receiver_searches", json!(1)),
        ("mode_written", json!(false)),
        ("preload_returned", json!(true)),
        ("player_created", json!(true)),
    ] {
        let mut bad = c.clone();
        bad["initializer_prefix"][key] = value;
        assert!(
            validate(&bad["initializer_prefix"], &bad).is_err(),
            "accepted {key}"
        );
    }
    for key in c["initializer_prefix"].as_object().unwrap().keys() {
        let mut bad = c.clone();
        bad["initializer_prefix"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["initializer_prefix"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
}
#[test]
fn actual_backings_and_source_topology_counts_must_match_allocation_totals() {
    let c = completed_component();
    for (key, value) in [
        ("native_bytes", json!(238)),
        ("allocations", json!(10)),
        ("selected", json!("third_person")),
        ("invocation_count", json!(2)),
        ("matrices_ready", json!(true)),
    ] {
        let mut bad = c.clone();
        bad["initializer_prefix"]["caches"][key] = value;
        assert!(
            validate(&bad["initializer_prefix"], &bad).is_err(),
            "accepted cache {key}"
        );
    }
    for (key, value) in [
        ("known_bytes", json!(9)),
        ("matrix_payload_known_bytes", json!(1)),
        ("native_bytes", json!(145)),
    ] {
        let mut bad = c.clone();
        bad["initializer_prefix"]["caches"]["headers"][0]["backings"][0][key] = value;
        assert!(
            validate(&bad["initializer_prefix"], &bad).is_err(),
            "accepted matrix {key}"
        );
    }
    let mut bad = c.clone();
    bad["initializer_prefix"]["caches"]["trace"]["builds"][0]["descendant_bytes"] = json!(4);
    assert!(validate(&bad["initializer_prefix"], &bad).is_err());
    let mut bad = c;
    bad["initializer_prefix"]["caches"]["headers"][0]["backings"][4] = backing("temporary", 4, 3);
    assert!(validate(&bad["initializer_prefix"], &bad).is_err());
}
#[test]
fn persistent_selected_reuse_does_not_rebuild_or_claim_a_new_pose() {
    let mut c = completed_component();
    let cache = &mut c["initializer_prefix"]["caches"];
    cache["invocation_count"] = json!(2);
    let t = &mut cache["trace"];
    t["ordinal"] = json!(2);
    t["selected_before"] = json!("first_person");
    t["builds"] = json!([null, null]);
    t["reused_selected"] = json!(true);
    t["selection_returned"] = json!(false);
    validate(&c["initializer_prefix"], &c).unwrap();
    c["initializer_prefix"]["caches"]["trace"]["selection_returned"] = json!(true);
    assert!(validate(&c["initializer_prefix"], &c).is_err());
}
#[test]
fn current_build_matrix_stride_must_match_its_actual_skeleton_count() {
    let mut component = completed_component();
    let cache = &mut component["initializer_prefix"]["caches"];
    // Both the backing shape and summed owner total remain internally valid.
    // The retained build still belongs to a two-bone skeleton.
    cache["headers"][0]["backings"][0]["native_bytes"] = json!(16 + 64 * 3);
    cache["native_bytes"] = json!(237 + 64);
    assert!(validate(&component["initializer_prefix"], &component).is_err());
}
#[test]
fn missing_receiver_skips_flag_and_invalidation_but_flag_zero_really_invalidates() {
    let mut c = completed_component();
    c["stage"] = json!("player_initializer_tail_pending");
    c["pending_dependency"] = json!("player_initializer_tail");
    let p = &mut c["initializer_prefix"];
    p["stage"] = json!("awaiting_tail");
    p["pending_dependency"] = json!("player_initializer_tail");
    p["receivers_found"] = json!([true, false]);
    p["cache_enabled"] = Value::Null;
    p["caches"] = Value::Null;
    validate(&c["initializer_prefix"], &c).unwrap();
    c["initializer_prefix"]["invalidated"] = json!(true);
    assert!(validate(&c["initializer_prefix"], &c).is_err());
    let mut c = completed_component();
    c["stage"] = json!("player_initializer_tail_pending");
    let p = &mut c["initializer_prefix"];
    p["stage"] = json!("awaiting_tail");
    p["pending_dependency"] = json!("player_initializer_tail");
    p["cache_enabled"] = json!(0);
    p["invalidated"] = json!(true);
    validate(&c["initializer_prefix"], &c).unwrap();
    c["initializer_prefix"]["invalidated"] = json!(false);
    assert!(validate(&c["initializer_prefix"], &c).is_err());
}
#[test]
fn a_missing_skeleton_is_pending_not_a_zero_bone_return() {
    let mut c = completed_component();
    c["stage"] = json!("player_transform_caches_pending");
    let p = &mut c["initializer_prefix"];
    p["stage"] = json!("caches");
    p["pending_dependency"] = json!("player_transform_caches");
    p["cache_enabled"] = Value::Null;
    let cache = &mut p["caches"];
    cache["selected"] = Value::Null;
    cache["counts"] = json!([0, 0]);
    cache["matrix_backings"] = json!([false, false]);
    cache["native_bytes"] = json!(0);
    cache["allocations"] = json!(0);
    let h = json!({"count":0,"association_bound":false,"backings":[null,null,null,null,null],"invalidation_count_reads":0,"invalidation_pointer_reads":0});
    cache["headers"] = json!([h.clone(), h]);
    let t = &mut cache["trace"];
    t["stage"] = json!("first_person");
    t["returned"] = json!(false);
    t["selection_returned"] = json!(false);
    t["selected_after"] = Value::Null;
    t["builds"] = json!([{"slot":"first_person","stage":"skeleton","bone_count":null,"roots_completed":0,"descendant_visits":0,"descendant_bytes":0,"offsets_written":0,"allocations":0,"releases":0,"waiting_skeleton":true,"failed":false,"returned":false,"state_sha256":"a".repeat(64)},null]);
    validate(&c["initializer_prefix"], &c).unwrap();
    c["initializer_prefix"]["caches"]["trace"]["builds"][0]["bone_count"] = json!(0);
    assert!(validate(&c["initializer_prefix"], &c).is_err());
}
