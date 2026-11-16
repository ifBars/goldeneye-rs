use super::*;
pub(in crate::menu::game_session) fn preload(c: &Value) -> Value {
    let mut caches = c["initializer_prefix"]["caches"].clone();
    caches["headers"][0]["invalidation_count_reads"] = json!(1);
    caches["headers"][0]["invalidation_pointer_reads"] = json!(1);
    caches["state_sha256"] = json!("e".repeat(64));
    let current_call = json!({
            "stage":"awaiting_temporary","ordinal":0,"source_ordinal":2,"weapon":0x0200_0101,"generic":0x0900_0765,"alternate":false,"variant":0,
            "mapping":{"output":0x0900_0101,"mapped":true,"steps":1,"map_rows_read":1,"state_sha256":"a".repeat(64)},
            "raw_flags_read":1,"raw_keys_read":0,"raw_index":0,"raw_indices_written":true,"entry_keys_read":2,"entry_index":0,
            "occupied_written":true,"weapon_written":true,"mapping_written":true,"selected_key_empty":true,"invalidated":true,
            "extraction_returned":true,"reference_outcome":"class_four","reference":0x8400_0101u32,"extraction":{
                "requested_index":3,"baseline_index":0,"transition_ordinal":1,"command_ordinal":0,"set_index":4,"group_ordinal":0,"child_ordinal":0,
                "transitions_read":2,"commands_read":1,"groups_read":1,"children_read":1},"return_word":null,"return_reason":null,"state_sha256":"b".repeat(64)});
    let allocations = json!([{"kind":"raw","native_bytes":512,"payload_bytes":512,"known_payload_bytes":44,"header_words":[null,null,null,null]},
            {"kind":"array34","native_bytes":7088,"payload_bytes":7072,"known_payload_bytes":136,"header_words":[208,34,null,null]},
            {"kind":"array64","native_bytes":56080,"payload_bytes":56064,"known_payload_bytes":0,"header_words":[876,64,null,null]}]);
    json!({"stage":"awaiting_temporary","pending_dependency":"preload_temporary_bind","entry_flag":1,"cache_enabled":1,
        "raw_flags_written":32,"entry_keys_written":34,"registry_count":4,"source_row":2,"rows_visited":3,"enabled_rows":1,
        "current_weapon":0x0200_0101,"call_in_row":0,"completed_calls":[],"current_call":current_call,"allocations":allocations,
        "caches":caches,"flag_cleared":false,"helper_returned":false,"temporary":null,"script_publication":null,"state_sha256":"f".repeat(64)})
}
pub(in crate::menu::game_session) fn component() -> Value {
    let mut c = super::super::player_initializer_prefix::tests::completed_component();
    c["preload"] = preload(&c);
    c["stage"] = json!("player_preload_temporary_pending");
    c["pending_dependency"] = json!("preload_temporary_bind");
    c
}
fn temporary(reference: u32) -> Value {
    json!({"stage":"awaiting_evaluation","pending_dependency":"temporary_animation_evaluation","identity":3,"source_receiver":2,"player":1,
        "component_file":0x0102_0016,"reference":reference,"skin_file":0x010a_0056,"skin_reference":0x0d00_0001,
        "attached":false,"callbacks_bound":false,"skeleton_bound":true,"bones":2,"auxiliary_words":0,"skeleton_backlink_bound":true,
        "pose_callback_assigned":false,"frame_stamp_known":false,"semantic_flags":[0,11],"component_flags":0,"active_slots":1,
        "factory_returned":true,"head_inserted":true,"script":{"stage":"ReturnedWithoutBody","slot":0,"factory_flags":3,"reference":reference,
            "descriptor_index":3,"body_bound":false,"initializer_returned":true,"phase_zero":null,"clips":[],"state_sha256":"a".repeat(64)},
        "forward":{"stage":"Returned","phase_cleared":true,"body_bound":false,"duration_raw":null,"fps_bits":null,"result_bits":null},
        "maintenance_skipped":true,"reverse_skipped":true,"bind_return":null,"refresh_returned":false,"evaluation":null,"state_sha256":"b".repeat(64)})
}
pub(in crate::menu::game_session) fn native_preload(c: &Value) -> Value {
    let mut p = preload(c);
    p["registry_count"] = json!(38);
    p["current_weapon"] = json!(0x0200_016e);
    let call = &mut p["current_call"];
    call["weapon"] = json!(0x0200_016e);
    call["mapping"]["output"] = json!(0x0900_09a1);
    call["reference"] = json!(0x8400_00efu32);
    call["extraction"] = json!({"requested_index":86,"baseline_index":0,"transition_ordinal":8,"command_ordinal":0,"set_index":220,
        "group_ordinal":0,"child_ordinal":0,"transitions_read":9,"commands_read":1,"groups_read":1,"children_read":1});
    let mut t = temporary(0x8400_00ef);
    t["skin_file"] = json!(0x010a_0050);
    t["skin_reference"] = json!(0x0d00_016f);
    t["bones"] = json!(48);
    let s = &mut t["script"];
    s["descriptor_index"] = json!(239);
    t["stage"] = json!("bind_returned");
    t["pending_dependency"] = Value::Null;
    t["bind_return"] = json!(1);
    let mut e = returned_evaluation(&t);
    let receipts = [
        &c["bind_refresh"],
        &c["first_person"]["bind_refresh"],
        &c["initial_action"]["tp_capture"]["refresh"],
        &c["initial_action"]["fp_dispatch"]["mask_capture"]["refresh"],
    ];
    e["ordinal"] = json!(
        receipts
            .iter()
            .filter_map(|r| r["refresh_ordinal"].as_u64())
            .max()
            .unwrap_or(0)
            + 1
    );
    e["allocation_ordinal"] = json!(
        receipts
            .iter()
            .filter_map(|r| r["allocation_ordinal"].as_u64())
            .max()
            .unwrap_or(0)
            + 1
    );
    e["slots"][0]["tail"] = json!(1);
    e["slots"][0]["nodes"][0]["node"] = json!(1);
    e["guards"][0]["node"] = json!(1);
    t["evaluation"] = e;
    p["pending_dependency"] = json!("preload_temporary_post_bind");
    p["temporary"] = t;
    p["script_publication"] = json!({"file":0x0102_0016,"generation":0,
        "descriptor_index":239,"section_index":9,"section_key":0x0800_0009,
        "callback_mask":0x819c,"fresh_base":true,"request_count":0,"active_requests":0,
        "retained_sections":0,"history_unknown":false,"raw_job":0,"body_state":"known_null",
        "native_history_verified":false});
    p
}
#[test]
fn prefix_keeps_unknown_cache_payload_and_initializer_history_separate() {
    let c = component();
    validate(&c["preload"], &c).unwrap();
    player_initializer_prefix::validate(&c["initializer_prefix"], &c).unwrap();
    assert_eq!(
        c["initializer_prefix"]["caches"]["headers"][0]["invalidation_count_reads"],
        0
    );
    assert_eq!(
        c["preload"]["caches"]["headers"][0]["invalidation_count_reads"],
        1
    );
    for (key, value) in [
        ("raw_flags_written", json!(31)),
        ("entry_keys_written", json!(33)),
        ("flag_cleared", json!(true)),
        ("helper_returned", json!(true)),
        ("cache_enabled", json!(0)),
        ("call_in_row", json!(1)),
        ("current_weapon", json!(0)),
        ("rows_visited", json!(1)),
        ("source_row", json!(5)),
        ("pending_dependency", json!("player_created")),
    ] {
        let mut bad = c.clone();
        bad["preload"][key] = value;
        assert!(validate(&bad["preload"], &bad).is_err(), "accepted {key}");
    }
    for key in c["preload"].as_object().unwrap().keys() {
        let mut bad = c.clone();
        bad["preload"].as_object_mut().unwrap().remove(key);
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted missing {key}"
        );
    }
}
#[test]
fn actual_source_widths_do_not_initialize_padding_array64_or_evaluated_matrices() {
    let c = component();
    for (index, key, value) in [
        (0, "known_payload_bytes", json!(43)),
        (0, "known_payload_bytes", json!(512)),
        (1, "known_payload_bytes", json!(7072)),
        (1, "header_words", json!([208, 34, 0, 0])),
        (1, "payload_bytes", json!(7088)),
        (2, "known_payload_bytes", json!(4)),
    ] {
        let mut bad = c.clone();
        bad["preload"]["allocations"][index][key] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted allocation{index} {key}"
        );
    }
    let mut bad = c.clone();
    bad["preload"]["caches"]["headers"][0]["invalidation_pointer_reads"] = json!(0);
    assert!(validate(&bad["preload"], &bad).is_err());
    let mut bad = c.clone();
    bad["initializer_prefix"]["caches"]["headers"][0]["invalidation_count_reads"] = json!(1);
    bad["initializer_prefix"]["caches"]["headers"][0]["invalidation_pointer_reads"] = json!(1);
    assert!(validate(&bad["preload"], &bad).is_err());
    let mut bad = c.clone();
    bad["preload"]["caches"]["headers"][0]["backings"][0]["known_bytes"] = json!(72);
    bad["preload"]["caches"]["headers"][0]["backings"][0]["matrix_payload_known_bytes"] = json!(64);
    assert!(validate(&bad["preload"], &bad).is_err());
}
#[test]
fn call_receipt_rejects_reordering_fabricated_returns_or_unknown_reference_as_zero() {
    let c = component();
    for (key, value) in [
        ("weapon_written", json!(false)),
        ("occupied_written", json!(false)),
        ("mapping_written", json!(false)),
        ("invalidated", json!(false)),
        ("selected_key_empty", json!(false)),
        ("extraction_returned", json!(false)),
        ("reference", json!(0)),
        ("raw_flags_read", json!(0)),
        ("raw_keys_read", json!(1)),
        ("source_ordinal", json!(1)),
        ("variant", json!(1)),
        ("return_word", json!(1)),
        ("alternate", json!(true)),
        ("ordinal", json!(1)),
    ] {
        let mut bad = c.clone();
        bad["preload"]["current_call"][key] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted call {key}"
        );
    }
    let mut known_none = c;
    let p = &mut known_none["preload"]["current_call"];
    p["reference_outcome"] = json!("no_reference");
    p["reference"] = Value::Null;
    p["extraction"] = Value::Null;
    validate(&known_none["preload"], &known_none).unwrap();
    known_none["preload"]["current_call"]["reference"] = json!(0);
    assert!(validate(&known_none["preload"], &known_none).is_err());
    let p = &mut known_none["preload"]["current_call"];
    p["reference"] = Value::Null;
    p["reference_outcome"] = Value::Null;
    p["extraction_returned"] = json!(false);
    assert!(validate(&known_none["preload"], &known_none).is_err());
}
#[test]
fn closed_nested_fields_and_extraction_counts_are_required() {
    let c = component();
    for section in ["current_call", "allocations", "caches"] {
        let mut bad = c.clone();
        bad["preload"][section] = Value::Null;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted missing {section}"
        );
    }
    for section in [
        "current_call",
        "current_call/mapping",
        "current_call/extraction",
    ] {
        let path = format!("/preload/{section}");
        let keys: Vec<_> = c
            .pointer(&path)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for key in keys {
            let mut bad = c.clone();
            bad.pointer_mut(&path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&key);
            assert!(
                validate(&bad["preload"], &bad).is_err(),
                "accepted missing {section}/{key}"
            );
        }
    }
    let mut bad = c;
    bad["preload"]["current_call"]["extraction"]["transitions_read"] = json!(1);
    assert!(validate(&bad["preload"], &bad).is_err());
}
#[test]
fn temporary_owner_is_distinct_unattached_and_does_not_complete_preload() {
    let mut c = component();
    c["preload"]["temporary"] = temporary(0x8400_0101);
    validate(&c["preload"], &c).unwrap();
    for (key, value) in [
        ("identity", json!(2)),
        ("source_receiver", json!(0)),
        ("attached", json!(true)),
        ("callbacks_bound", json!(true)),
        ("skeleton_backlink_bound", json!(false)),
        ("semantic_flags", json!([0, 3])),
        ("head_inserted", json!(false)),
        ("bind_return", json!(1)),
        ("refresh_returned", json!(true)),
        ("reference", json!(0)),
        ("pending_dependency", json!("player_created")),
        ("forward", Value::Null),
    ] {
        let mut bad = c.clone();
        bad["preload"]["temporary"][key] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted temporary {key}"
        );
    }
    for key in c["preload"]["temporary"].as_object().unwrap().keys() {
        let mut bad = c.clone();
        bad["preload"]["temporary"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted missing temporary {key}"
        );
    }
    let mut bad = c;
    bad["preload"]["temporary"]["script"]["stage"] = json!("ReturnedInitialized");
    assert!(validate(&bad["preload"], &bad).is_err());
}
#[test]
fn unknown_nonbase_script_body_retains_descriptor_and_real_residency_dependency() {
    let mut c = component();
    let mut t = temporary(0x8400_0101);
    t["stage"] = json!("awaiting_initializer");
    t["pending_dependency"] = json!("temporary_script_body_residency");
    for k in [
        "factory_returned",
        "head_inserted",
        "maintenance_skipped",
        "reverse_skipped",
    ] {
        t[k] = json!(false);
    }
    t["forward"] = Value::Null;
    t["script"]["stage"] = json!("AwaitingScriptBody");
    t["script"]["body_bound"] = Value::Null;
    t["script"]["initializer_returned"] = json!(false);
    c["preload"]["temporary"] = t;
    validate(&c["preload"], &c).unwrap();
    for (key, value) in [
        ("body_bound", json!(false)),
        ("body_bound", json!(true)),
        ("descriptor_index", Value::Null),
        ("initializer_returned", json!(true)),
    ] {
        let mut bad = c.clone();
        bad["preload"]["temporary"]["script"][key] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted pending body {key}"
        );
    }
}
#[test]
fn no_body_factory_return_requires_known_absence_and_exact_returned_stage() {
    let mut c = component();
    c["preload"]["temporary"] = temporary(0x8400_0101);
    validate(&c["preload"], &c).unwrap();
    for (key, value) in [
        ("body_bound", Value::Null),
        ("body_bound", json!(true)),
        ("initializer_returned", json!(false)),
        ("stage", json!("AwaitingInitializer")),
    ] {
        let mut bad = c.clone();
        bad["preload"]["temporary"]["script"][key] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted no-body factory {key}"
        );
    }
}
#[test]
fn pending_forward_body_keeps_factory_history_and_stops_callbacks() {
    for had_play_scan in [false, true] {
        let mut c = component();
        let mut t = temporary(0x8400_0101);
        t["stage"] = json!("forward_tick");
        t["pending_dependency"] = json!("temporary_script_body_residency");
        t["maintenance_skipped"] = json!(false);
        t["reverse_skipped"] = json!(false);
        t["script"]["body_bound"] = Value::Null;
        t["forward"]["stage"] = json!("AwaitingBody");
        t["forward"]["body_bound"] = Value::Null;
        if had_play_scan {
            t["script"]["stage"] = json!("ReturnedWithoutPlay");
            t["script"]["phase_zero"] = json!({"stage":"returned_without_play","delta_bits":1f32.to_bits(),"duration_raw":60,"frame_bits":0,"entries_visited":1,
                "selected_offset":null,"normal_requested":null,"secondary_requested":null,"normal_bound":0,"secondary_bound":0,"clip_records":0,
                "heap_allocated":false,"initial_index_raw":null,"initial_index_clamped":null,"frame_range":null,"setup_returned":false,"flags_applied":false,
                "pending_file":null,"position":null});
        }
        c["preload"]["temporary"] = t;
        validate(&c["preload"], &c).unwrap();
        for (key, value) in [
            ("body_bound", json!(false)),
            ("body_bound", json!(true)),
            ("stage", json!("Returned")),
            ("phase_cleared", json!(false)),
            ("duration_raw", json!(0)),
            ("fps_bits", json!(0)),
            ("result_bits", json!(0)),
        ] {
            let mut bad = c.clone();
            bad["preload"]["temporary"]["forward"][key] = value;
            assert!(
                validate(&bad["preload"], &bad).is_err(),
                "accepted pending forward {key}"
            );
        }
        for key in ["maintenance_skipped", "reverse_skipped"] {
            let mut bad = c.clone();
            bad["preload"]["temporary"][key] = json!(true);
            assert!(
                validate(&bad["preload"], &bad).is_err(),
                "accepted advanced {key}"
            );
        }
        let mut bad = c;
        bad["preload"]["temporary"]["script"]["initializer_returned"] = json!(false);
        assert!(validate(&bad["preload"], &bad).is_err());
    }
}

#[test]
fn current_publication_proof_is_owner_bound_and_distinct_from_initializer_history() {
    let mut c = component();
    c["preload"] = native_preload(&c);
    c["pending_dependency"] = json!("preload_temporary_post_bind");
    let history = c["initializer_prefix"].clone();
    validate(&c["preload"], &c).unwrap();
    assert_eq!(c["initializer_prefix"], history);
    for (key, value) in [
        ("file", json!(0x0102_0017)),
        ("descriptor_index", json!(238)),
        ("descriptor_index", json!(32767)),
        ("section_index", json!(0)),
        ("section_index", json!(32767)),
        ("section_key", json!(0x0800_0000)),
        ("section_key", json!(0x0700_0009)),
        ("callback_mask", json!(0)),
        ("callback_mask", Value::Null),
        ("fresh_base", json!(false)),
        ("active_requests", json!(1)),
        ("retained_sections", json!(1)),
        ("history_unknown", json!(true)),
        ("raw_job", json!(1)),
        ("raw_job", Value::Null),
        ("native_history_verified", json!(true)),
        ("body_state", json!("resident")),
        ("request_count", json!(-1)),
    ] {
        let mut bad = c.clone();
        bad["preload"]["script_publication"][key] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted publication {key}"
        );
    }
    for key in c["preload"]["script_publication"]
        .as_object()
        .unwrap()
        .keys()
    {
        let mut bad = c.clone();
        bad["preload"]["script_publication"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted missing publication {key}"
        );
    }
    let mut pending = c;
    pending["preload"]["script_publication"]["body_state"] = json!("pending");
    pending["preload"]["script_publication"]["fresh_base"] = json!(false);
    pending["preload"]["script_publication"]["callback_mask"] = Value::Null;
    pending["preload"]["script_publication"]["raw_job"] = Value::Null;
    pending["preload"]["script_publication"]["history_unknown"] = json!(true);
    validate(&pending["preload"], &pending).unwrap();
}

fn returned_evaluation(t: &Value) -> Value {
    json!({"stage":"returned","pending_boundary":null,"reserved":true,"released":true,"lease_live":false,"aborted":false,"arena_owner":1,"ordinal":4,"allocation_ordinal":4,"reservation_start":0,"cursor_after":0,"live_after":0,
      "slots":[{"slot":0,"weight_bits":0x3f800000,"branch":"nonempty","tail":8,"prepared":true,"masks_cleared":true,"nodes":[{"node":8,"result":0}],"successes":0,"flags_after":0}],
      "prepares":[{"file":t["skin_file"],"reference":t["skin_reference"],"generation":0,"descriptor_bound":true,"body_resident":true,"auxiliary_words":0,"lookups":1,"body_reads":1,"returned":"AuxiliaryZero","pause":null}],
      "guards":[{"node":8,"pointer":"constructor_inline","record":null,"clip_reference":u32::MAX,"animator_reference_read":false,"animator_reference":null,"result":0,"pending_boundary":null}],
      "node_visits":1,"nonempty_layers":1,"successful_samples":0,"advanced_context":true,"commit_register":0,"blended_layers":0,"committed":false,"state_sha256":"9".repeat(64)})
}
#[test]
fn genuine_nonempty_bind_requires_its_own_release_and_preserves_unfinished_caller() {
    let mut c = component();
    let mut t = temporary(0x8400_0101);
    t["stage"] = json!("bind_returned");
    t["pending_dependency"] = Value::Null;
    t["bind_return"] = json!(1);
    t["evaluation"] = returned_evaluation(&t);
    c["preload"]["temporary"] = t;
    c["preload"]["pending_dependency"] = json!("preload_temporary_post_bind");
    c["pending_dependency"] = json!("preload_temporary_post_bind");
    validate(&c["preload"], &c).unwrap();
    for (field, value) in [
        ("released", json!(false)),
        ("lease_live", json!(true)),
        ("committed", json!(true)),
        ("live_after", json!(1)),
        ("node_visits", json!(0)),
        ("successful_samples", json!(1)),
    ] {
        let mut bad = c.clone();
        bad["preload"]["temporary"]["evaluation"][field] = value;
        assert!(
            validate(&bad["preload"], &bad).is_err(),
            "accepted forged {field}"
        );
    }
    let mut bad = c.clone();
    bad["preload"]["temporary"]["evaluation"]["slots"][0]["masks_cleared"] = json!(false);
    assert!(validate(&bad["preload"], &bad).is_err());
    let mut bad = c.clone();
    bad["preload"]["temporary"]["evaluation"]["guards"][0]["animator_reference_read"] = json!(true);
    assert!(validate(&bad["preload"], &bad).is_err());
    let mut bad = c.clone();
    bad["preload"]["temporary"]["evaluation"] = Value::Null;
    assert!(validate(&bad["preload"], &bad).is_err());
    assert_eq!(c["preload"]["entry_flag"], 1);
    assert_eq!(c["preload"]["helper_returned"], false);
}
#[test]
fn pending_and_aborted_nonempty_evaluation_cannot_claim_bind_return() {
    let mut c = component();
    let mut t = temporary(0x8400_0101);
    let mut e = returned_evaluation(&t);
    e["stage"] = json!("pending");
    e["pending_boundary"] = json!("Prepare");
    e["released"] = json!(false);
    e["lease_live"] = json!(true);
    e["node_visits"] = json!(0);
    e["nonempty_layers"] = json!(0);
    e["guards"] = json!([]);
    e["prepares"] = json!([]);
    e["slots"][0]["nodes"] = json!([]);
    e["slots"][0]["prepared"] = json!(false);
    e["slots"][0]["masks_cleared"] = json!(false);
    t["stage"] = json!("evaluating_bind");
    t["pending_dependency"] = json!("temporary_evaluation_skin");
    t["evaluation"] = e;
    c["preload"]["temporary"] = t;
    validate(&c["preload"], &c).unwrap();
    // EvaluatingBind is outside the legacy numeric stage range which checked
    // skeleton binding. Its retained scope must enforce that owner itself.
    let mut unbound = c.clone();
    let temporary = &mut unbound["preload"]["temporary"];
    temporary["skeleton_bound"] = json!(false);
    temporary["skeleton_backlink_bound"] = json!(false);
    temporary["bones"] = Value::Null;
    temporary["auxiliary_words"] = Value::Null;
    assert!(validate(&unbound["preload"], &unbound).is_err());
    c["preload"]["temporary"]["evaluation"]["stage"] = json!("aborted");
    c["preload"]["temporary"]["evaluation"]["pending_boundary"] = Value::Null;
    c["preload"]["temporary"]["evaluation"]["lease_live"] = json!(false);
    c["preload"]["temporary"]["evaluation"]["aborted"] = json!(true);
    c["preload"]["temporary"]["stage"] = json!("aborted");
    c["preload"]["temporary"]["pending_dependency"] = Value::Null;
    validate(&c["preload"], &c).unwrap();
    c["preload"]["temporary"]["bind_return"] = json!(1);
    assert!(validate(&c["preload"], &c).is_err());
}
