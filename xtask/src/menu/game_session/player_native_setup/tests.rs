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
    // Earlier helper fixtures intentionally omit presentation-independent
    // arena receipts. Supply authored prior scopes for this native-order gate.
    for (path, ordinal) in [
        (vec!["bind_refresh"], 1),
        (vec!["first_person", "bind_refresh"], 2),
        (vec!["initial_action", "tp_capture", "refresh"], 3),
        (
            vec!["initial_action", "fp_dispatch", "mask_capture", "refresh"],
            4,
        ),
    ] {
        let mut field = &mut c;
        for key in path {
            field = &mut field[key];
        }
        *field = json!({"scope_released":true,"live_reservations_after":0,
            "refresh_ordinal":ordinal,"allocation_ordinal":ordinal});
    }
    c["preload"] = super::super::player_preload::tests::native_sampling_preload(&c);
    c["stage"] = json!("player_preload_temporary_pending");
    c["pending_dependency"] = json!("preload_temporary_evaluation_sampling");
    c["attachment"] = super::super::player_attachment::tests::fixture();
    c["attachment"]["registry_identities"] = json!(35);
    json!({"pending_dependency":"preload_temporary_evaluation_sampling","component_initialization":c})
}
#[test]
fn observed_tutorial_completes_32_temporaries_then_pauses_on_resident_sampling() {
    let p = fixture();
    validate(&p, 1).unwrap();
    let preload = &p["component_initialization"]["preload"];
    assert_eq!(
        preload["completion"]["first_completed_snapshot"]["temporary_identity"],
        3
    );
    assert_eq!(preload["temporary"]["identity"], 35);
    assert_eq!(preload["completed_calls"].as_array().unwrap().len(), 162);
    assert_eq!(preload["completion"]["entry_commit_count"], 32);
    assert_eq!(preload["cache_enabled"], 1);
}
#[test]
fn native_gate_rejects_fabricated_completion_readiness_and_wrong_sampling_owner() {
    let p = fixture();
    for (pointer, value) in [
        ("/pending_dependency", json!("preload_temporary_bind")),
        (
            "/component_initialization/preload/completion/entry_commit_count",
            json!(33),
        ),
        (
            "/component_initialization/preload/completion/destroyed_temporary_count",
            json!(31),
        ),
        (
            "/component_initialization/preload/completion/found_output_count",
            json!(191),
        ),
        (
            "/component_initialization/preload/flag_cleared",
            json!(true),
        ),
        (
            "/component_initialization/preload/helper_returned",
            json!(true),
        ),
        ("/component_initialization/preload/cache_enabled", json!(0)),
        ("/component_initialization/preload/source_row", json!(38)),
        (
            "/component_initialization/preload/current_call/entry_index",
            json!(31),
        ),
        (
            "/component_initialization/preload/temporary/player",
            json!(2),
        ),
        (
            "/component_initialization/preload/temporary/identity",
            json!(34),
        ),
        (
            "/component_initialization/preload/temporary/bind_return",
            json!(1),
        ),
        (
            "/component_initialization/preload/temporary/script/body_bound",
            json!(false),
        ),
        (
            "/component_initialization/preload/temporary/script/phase_zero/heap_allocated",
            json!(false),
        ),
        (
            "/component_initialization/preload/allocations/0/known_payload_bytes",
            json!(369),
        ),
    ] {
        let mut bad = p.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(validate(&bad, 1).is_err(), "accepted {pointer}");
    }
}
#[test]
fn first_completed_proof_remains_distinct_from_the_current_resident_script() {
    let p = fixture();
    for (pointer, value) in [
        ("/ordinal", json!(1)),
        ("/temporary_identity", json!(4)),
        ("/script_publication", Value::Null),
        ("/script_publication/section_key", json!(0x0800000a)),
        ("/temporary/script/body_bound", json!(true)),
        ("/temporary/evaluation/released", json!(false)),
        ("/post_bind/process/current_bits", json!(0x3f800000)),
        ("/post_bind/phase/header_known_bits", json!(0)),
        ("/post_bind/phase/header_expression_bound", json!(false)),
        ("/post_bind/evaluation/ordinal", json!(5)),
        ("/outputs/stores", json!(6)),
    ] {
        let mut bad = p.clone();
        let first = &mut bad["component_initialization"]["preload"]["completion"]["first_completed_snapshot"];
        *first.pointer_mut(pointer).unwrap() = value;
        assert!(validate(&bad, 1).is_err(), "accepted first {pointer}");
    }
}
#[test]
fn resident_evaluation_cannot_claim_sentinel_return_sampling_or_scratch_release() {
    let p = fixture();
    for (pointer, value) in [
        ("/arena_owner", json!(2)),
        ("/ordinal", json!(6)),
        ("/allocation_ordinal", json!(6)),
        ("/released", json!(true)),
        ("/lease_live", json!(false)),
        ("/aborted", json!(true)),
        ("/successful_samples", json!(1)),
        ("/committed", json!(true)),
        ("/node_visits", json!(1)),
        ("/guards/0/clip_reference", json!(u32::MAX)),
        ("/guards/0/animator_reference_read", json!(false)),
        ("/guards/0/pointer", json!("constructor_inline")),
        ("/guards/0/record", json!(1)),
        ("/guards/0/pending_boundary", Value::Null),
        ("/slots/0/masks_cleared", json!(false)),
        ("/prepares/0/body_reads", json!(2)),
    ] {
        let mut bad = p.clone();
        let e = &mut bad["component_initialization"]["preload"]["temporary"]["evaluation"];
        *e.pointer_mut(pointer).unwrap() = value;
        assert!(validate(&bad, 1).is_err(), "accepted evaluation {pointer}");
    }
}
fn advance_snapshot_for_reentry(snapshot: &mut Value) {
    snapshot["player"] = json!(2);
    snapshot["temporary"]["player"] = json!(2);
    for key in ["temporary_identity", "receiver"] {
        snapshot[key] = json!(snapshot[key].as_u64().unwrap() + 500);
    }
    for key in ["identity", "source_receiver"] {
        snapshot["temporary"][key] = json!(snapshot["temporary"][key].as_u64().unwrap() + 500);
    }
    snapshot["post_bind"]["temporary_identity"] = snapshot["temporary_identity"].clone();
    for k in ["ordinal", "allocation_ordinal"] {
        snapshot["temporary"]["evaluation"][k] =
            json!(snapshot["temporary"]["evaluation"][k].as_u64().unwrap() + 100);
        snapshot["post_bind"]["first_scope"][k] = snapshot["temporary"]["evaluation"][k].clone();
        snapshot["post_bind"]["evaluation"][k] =
            json!(snapshot["post_bind"]["evaluation"][k].as_u64().unwrap() + 100);
    }
}
#[test]
fn native_gate_accepts_retained_identity_counters_and_matching_new_attempt_player() {
    let mut p = fixture();
    let preload = &mut p["component_initialization"]["preload"];
    let progress = &mut preload["completion"];
    for row in progress["history"].as_array_mut().unwrap() {
        advance_snapshot_for_reentry(&mut row["snapshot"]);
        row["destruction"]["temporary_identity"] = row["snapshot"]["temporary_identity"].clone();
    }
    advance_snapshot_for_reentry(&mut progress["first_completed_snapshot"]);
    for k in ["ordinal", "allocation_ordinal"] {
        preload["temporary"]["evaluation"][k] =
            json!(preload["temporary"]["evaluation"][k].as_u64().unwrap() + 100);
    }
    preload["temporary"]["player"] = json!(2);
    for key in ["identity", "source_receiver"] {
        preload["temporary"][key] = json!(preload["temporary"][key].as_u64().unwrap() + 500);
    }
    p["component_initialization"]["attachment"]["registry_identities"] = json!(535);
    validate(&p, 2).unwrap();
    assert!(validate(&p, 1).is_err());
    assert_eq!(
        p["component_initialization"]["preload"]["completion"]["first_completed_snapshot"]["temporary_identity"],
        503
    );
    assert_eq!(
        p["component_initialization"]["preload"]["temporary"]["identity"],
        535
    );
    for (pointer, value) in [
        (
            "/component_initialization/preload/temporary/source_receiver",
            json!(2),
        ),
        (
            "/component_initialization/preload/temporary/identity",
            json!(536),
        ),
        (
            "/component_initialization/attachment/registry_identities",
            json!(35),
        ),
        (
            "/component_initialization/preload/completion/first_completed_snapshot/receiver",
            json!(501),
        ),
    ] {
        let mut bad = p.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            validate(&bad, 2).is_err(),
            "accepted shifted identity mismatch {pointer}"
        );
    }
}
