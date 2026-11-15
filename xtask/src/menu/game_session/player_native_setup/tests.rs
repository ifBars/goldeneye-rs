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
    c["preload"] = super::super::player_preload::tests::native_preload(&c);
    c["stage"] = json!("player_preload_temporary_pending");
    c["pending_dependency"] = json!("preload_temporary_bind");
    c["attachment"] = super::super::player_attachment::tests::fixture();
    c["attachment"]["registry_identities"] = json!(3);
    json!({"pending_dependency":"preload_temporary_bind","component_initialization":c})
}
#[test]
fn observed_tutorial_retains_fresh_null_publication_and_real_evaluation_dependency() {
    let p = fixture();
    validate(&p, 1).unwrap();
    for (key, value) in [
        ("weapon_global_byte", json!(0)),
        ("global_equivalence_used", json!(false)),
        ("force_nodes_checked", json!(1)),
    ] {
        let mut bad = p.clone();
        bad["component_initialization"]["pose_setup"][key] = value;
        assert!(validate(&bad, 1).is_err(), "accepted pose {key}");
    }
    let mut bad = p.clone();
    bad["component_initialization"]["pose_setup"]["enable_writes"][0]["flags_before"] = json!(0xb);
    assert!(validate(&bad, 1).is_err());
    let mut bad = p.clone();
    bad["component_initialization"]["initializer_prefix"]["caches"]["headers"][0]["backings"][0]
        ["matrix_payload_known_bytes"] = json!(64);
    assert!(validate(&bad, 1).is_err());
    let mut reused = p;
    let cache = &mut reused["component_initialization"]["initializer_prefix"]["caches"];
    cache["invocation_count"] = json!(2);
    let trace = &mut cache["trace"];
    trace["ordinal"] = json!(2);
    trace["selected_before"] = json!("first_person");
    trace["reused_selected"] = json!(true);
    trace["selection_returned"] = json!(false);
    trace["builds"] = json!([null, null]);
    reused["component_initialization"]["preload"] =
        super::super::player_preload::tests::native_preload(&reused["component_initialization"]);
    validate(&reused, 1).unwrap();
}
#[test]
fn native_gate_requires_actual_new_owner_and_null_proof_without_evaluation_return() {
    let p = fixture();
    validate(&p, 1).unwrap();
    for (key, value) in [
        ("stage", json!("awaiting_initializer")),
        ("factory_returned", json!(false)),
        ("head_inserted", json!(false)),
        ("identity", json!(2)),
        ("source_receiver", json!(1)),
        ("skin_file", json!(0x010a_0056)),
        ("bones", json!(37)),
        ("attached", json!(true)),
        ("frame_stamp_known", json!(true)),
        ("bind_return", json!(1)),
    ] {
        let mut bad = p.clone();
        bad["component_initialization"]["preload"]["temporary"][key] = value;
        assert!(
            validate(&bad, 1).is_err(),
            "accepted native temporary {key}"
        );
    }
    for (key, value) in [
        ("body_bound", Value::Null),
        ("body_bound", json!(true)),
        ("initializer_returned", json!(false)),
        ("descriptor_index", json!(0)),
        ("stage", json!("AwaitingScriptBody")),
    ] {
        let mut bad = p.clone();
        bad["component_initialization"]["preload"]["temporary"]["script"][key] = value;
        assert!(validate(&bad, 1).is_err(), "accepted native script {key}");
    }
    let mut bad = p.clone();
    bad["component_initialization"]["preload"]["temporary"] = Value::Null;
    assert!(validate(&bad, 1).is_err());
    let mut bad = p.clone();
    bad["component_initialization"]["attachment"]["item_components"] = json!(3);
    assert!(validate(&bad, 1).is_err());
    let mut bad = p;
    bad["component_initialization"]["preload"]["allocations"][0]["known_payload_bytes"] = json!(45);
    assert!(validate(&bad, 1).is_err());
}

#[test]
fn native_gate_binds_player_to_the_current_attempt_after_cancellation() {
    let mut player = fixture();
    player["component_initialization"]["preload"]["temporary"]["player"] = json!(2);
    validate(&player, 2).unwrap();
    assert!(validate(&player, 1).is_err());
    player["component_initialization"]["preload"]["temporary"]["player"] = json!(0);
    assert!(validate(&player, 0).is_err());
}

#[test]
fn native_gate_requires_exact_fresh_same_wrapper_publication_receipt() {
    let p = fixture();
    for (key, value) in [
        ("generation", json!(1)),
        ("request_count", json!(1)),
        ("section_index", json!(10)),
        ("section_key", json!(0x0800_000a)),
        ("callback_mask", json!(8)),
        ("body_state", json!("pending")),
    ] {
        let mut bad = p.clone();
        bad["component_initialization"]["preload"]["script_publication"][key] = value;
        assert!(
            validate(&bad, 1).is_err(),
            "accepted native publication {key}"
        );
    }
    let mut bad = p.clone();
    bad["component_initialization"]["preload"]["script_publication"] = Value::Null;
    assert!(validate(&bad, 1).is_err());
    let mut bad = p;
    bad["component_initialization"]["preload"]["temporary"]["stage"] = json!("bind_returned");
    bad["component_initialization"]["preload"]["temporary"]["bind_return"] = json!(1);
    assert!(validate(&bad, 1).is_err());
}
