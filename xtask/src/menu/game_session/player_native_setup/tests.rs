use super::*;
fn fixture() -> Value {
    let mut c = super::super::player_initializer_prefix::tests::completed_component();
    let pose = &mut c["pose_setup"];
    pose["dirty_before"] = json!(0x13);
    pose["dirty_after"] = json!(0x13);
    for (i, r, b, a) in [(0, 0x8400_0063u32, 3, 2), (1, 0x8400_0022, 3, 3)] {
        let e = &mut pose["enable_writes"][i];
        e["reference"] = json!(r);
        e["flags_before"] = json!(b);
        e["flags_after"] = json!(a);
    }
    c["component_tail"]["flags_before"] = json!(0x13);
    c["component_tail"]["flags_after"] = json!(0x13);
    let cache = &mut c["initializer_prefix"]["caches"];
    cache["counts"] = json!([48, 37]);
    cache["native_bytes"] = json!(6377);
    for (i, n, visits, desc) in [(0, 48u32, 333, 381u32), (1, 37, 232, 269)] {
        let h = &mut cache["headers"][i];
        h["count"] = json!(n);
        h["backings"][0]["native_bytes"] = json!(16 + 64 * n);
        h["backings"][1]["native_bytes"] = json!(n);
        h["backings"][1]["known_bytes"] = json!(n);
        h["backings"][2]["native_bytes"] = json!(2 * n);
        h["backings"][2]["known_bytes"] = json!(2 * n);
        h["backings"][3]["native_bytes"] = json!(desc);
        h["backings"][3]["known_bytes"] = json!(desc);
        let b = &mut cache["trace"]["builds"][i];
        b["bone_count"] = json!(n);
        b["roots_completed"] = json!(n);
        b["offsets_written"] = json!(n);
        b["descendant_visits"] = json!(visits);
        b["descendant_bytes"] = json!(desc);
    }
    json!({"pending_dependency":"player_animation_preload","component_initialization":c})
}
#[test]
fn observed_tutorial_cache_and_nested_returns_remain_preload_pending() {
    let p = fixture();
    validate(&p).unwrap();
    for (key, value) in [
        ("weapon_global_byte", json!(0)),
        ("global_equivalence_used", json!(false)),
        ("force_nodes_checked", json!(1)),
    ] {
        let mut bad = p.clone();
        bad["component_initialization"]["pose_setup"][key] = value;
        assert!(validate(&bad).is_err(), "accepted pose {key}");
    }
    let mut bad = p.clone();
    bad["component_initialization"]["pose_setup"]["enable_writes"][0]["flags_before"] = json!(0xb);
    assert!(validate(&bad).is_err());
    let mut bad = p.clone();
    bad["component_initialization"]["initializer_prefix"]["caches"]["headers"][0]["backings"][0]
        ["matrix_payload_known_bytes"] = json!(64);
    assert!(validate(&bad).is_err());
    let mut reused = p;
    let cache = &mut reused["component_initialization"]["initializer_prefix"]["caches"];
    cache["invocation_count"] = json!(2);
    let trace = &mut cache["trace"];
    trace["ordinal"] = json!(2);
    trace["selected_before"] = json!("first_person");
    trace["reused_selected"] = json!(true);
    trace["selection_returned"] = json!(false);
    trace["builds"] = json!([null, null]);
    validate(&reused).unwrap();
}
