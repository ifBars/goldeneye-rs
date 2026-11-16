//! Authored source-order calls and receipts; no original payloads or poses.
use super::*;
pub(in crate::menu::game_session) fn fixture(c: &Value) -> Value {
    let mut p = super::native_preload(c);
    let base = p["current_call"].clone();
    let mut calls = Vec::new();
    let mut snapshots = Vec::new();
    for i in 0..162usize {
        let mut call = base.clone();
        let raw = i / 6;
        call["ordinal"] = json!(i);
        call["source_ordinal"] = json!(2 + raw);
        call["alternate"] = json!(i % 2 == 1);
        call["variant"] = json!((i % 6) / 2);
        call["generic"] = json!(if i % 2 == 1 { 0x090007a7 } else { 0x09000765 });
        call["raw_index"] = json!(raw);
        call["raw_flags_read"] = json!(raw + 1);
        call["raw_indices_written"] = json!(i % 6 == 0);
        call["raw_keys_read"] = json!(raw + usize::from(i % 6 != 0));
        call["stage"] = json!("returned");
        call["return_word"] = json!(1);
        if i < 32 {
            call["entry_index"] = json!(i);
            call["entry_keys_read"] = json!(i + 2);
            call["mapping"]["output"] = json!(0x090009a1u32 + i as u32);
            call["return_reason"] = json!("temporary_completed");
            let mut temporary = p["temporary"].clone();
            let mut post = p["post_bind"].clone();
            temporary["identity"] = json!(3 + i);
            temporary["evaluation"]["ordinal"] = json!(5 + 2 * i);
            temporary["evaluation"]["allocation_ordinal"] = json!(5 + 2 * i);
            post["temporary_identity"] = json!(3 + i);
            post["first_scope"]["ordinal"] = json!(5 + 2 * i);
            post["first_scope"]["allocation_ordinal"] = json!(5 + 2 * i);
            post["evaluation"]["ordinal"] = json!(6 + 2 * i);
            post["evaluation"]["allocation_ordinal"] = json!(6 + 2 * i);
            post["process"]["reads"] = json!(i + 1);
            post["capture"]["read_sequence"] = json!(i + 1);
            let outputs = json!({"stage":"done","pending_dependency":"preload_temporary_destructor","iteration":6,"hash_cursor":6,"hash_reads":6,"lookups":6,"body_reads":6,"tag_walks":6,"stores":30,"found":6,"missing":0,"outcomes":["found","found","found","found","found","found"],"state_sha256":"d".repeat(64)});
            let snapshot = json!({"ordinal":i,"source_ordinal":call["source_ordinal"],"player":1,"receiver":2,"weapon":call["weapon"],"generic":call["generic"],"alternate":call["alternate"],"variant":call["variant"],"entry":call["entry_index"],"mapped":call["mapping"]["output"],"reference":call["reference"],"temporary_identity":3+i,"temporary":temporary,"post_bind":post,"outputs":outputs,"script_publication":p["script_publication"]});
            snapshots.push(snapshot);
        } else {
            call["entry_index"] = json!(0);
            call["entry_keys_read"] = json!(2);
            call["selected_key_empty"] = json!(false);
            call["invalidated"] = json!(false);
            call["extraction_returned"] = json!(false);
            call["reference_outcome"] = Value::Null;
            call["reference"] = Value::Null;
            call["extraction"] = Value::Null;
            call["return_reason"] = json!("existing_entry");
        }
        calls.push(call);
    }
    let mut current = base;
    for (k, v) in [
        ("ordinal", json!(162)),
        ("source_ordinal", json!(36)),
        ("weapon", json!(0x02000551)),
        ("raw_index", json!(27)),
        ("raw_flags_read", json!(28)),
        ("raw_keys_read", json!(27)),
        ("entry_index", json!(32)),
        ("entry_keys_read", json!(34)),
        ("reference", json!(0x840000b7u32)),
    ] {
        current[k] = v;
    }
    current["mapping"]["output"] = json!(0x0900086d);
    current["extraction"] = json!({"requested_index":41,"baseline_index":0,"transition_ordinal":42,"command_ordinal":0,"set_index":21,"group_ordinal":0,"child_ordinal":0,"transitions_read":43,"commands_read":1,"groups_read":1,"children_read":1});
    let mut t = p["temporary"].clone();
    t["identity"] = json!(35);
    t["reference"] = current["reference"].clone();
    t["stage"] = json!("evaluating_bind");
    t["pending_dependency"] = json!("temporary_evaluation_sampling");
    t["bind_return"] = Value::Null;
    let script = &mut t["script"];
    script["stage"] = json!("ReturnedInitialized");
    script["reference"] = current["reference"].clone();
    script["descriptor_index"] = json!(183);
    script["body_bound"] = json!(true);
    script["phase_zero"] = json!({"stage":"initialized","delta_bits":1f32.to_bits(),"duration_raw":33,"frame_bits":0,"entries_visited":2,"selected_offset":0,"normal_requested":4,"secondary_requested":0,"normal_bound":4,"secondary_bound":0,"clip_records":4,"heap_allocated":true,"initial_index_raw":0,"initial_index_clamped":0,"frame_range":33,"setup_returned":true,"flags_applied":true,"pending_file":null,"position":{"selected_index":0,"file_bound":true,"generation_observed":0,"metadata_bound":true,"cache_generation":0,"descriptor_index":3,"body_bound":true,"timing_flags":0,"animation_fps":30,"script_fps_bits":30f32.to_bits(),"frame_bits":0,"previous_position_bits":0,"normalized_position_bits":0,"root_motion_pending":false}});
    script["clips"]=json!((0..4).map(|_|json!({"file":0x01020016,"reference":0x83000019u32,"cache_generation":0,"start_raw":0,"end_raw":33,"blend_bits":0})).collect::<Vec<_>>());
    t["forward"] = json!({"stage":"Returned","phase_cleared":true,"body_bound":true,"duration_raw":33,"fps_bits":30f32.to_bits(),"result_bits":0});
    let e = &mut t["evaluation"];
    for (k, v) in [
        ("stage", json!("pending")),
        ("pending_boundary", json!("Node")),
        ("released", json!(false)),
        ("lease_live", json!(true)),
        ("ordinal", json!(69)),
        ("allocation_ordinal", json!(69)),
        ("cursor_after", Value::Null),
        ("live_after", Value::Null),
        ("node_visits", json!(0)),
        ("nonempty_layers", json!(0)),
        ("advanced_context", json!(false)),
    ] {
        e[k] = v;
    }
    e["slots"][0]["nodes"] = json!([]);
    e["slots"][0]["flags_after"] = Value::Null;
    e["guards"] = json!([{"node":1,"pointer":"record","record":0,"clip_reference":0x83000019u32,"animator_reference_read":true,"animator_reference":0x840000b7u32,"result":null,"pending_boundary":"Sampling"}]);
    p["current_call"] = current;
    p["temporary"] = t;
    p["post_bind"] = Value::Null;
    p["script_publication"] = Value::Null;
    p["completed_calls"] = json!(calls);
    for (k, v) in [
        ("source_row", json!(36)),
        ("rows_visited", json!(37)),
        ("enabled_rows", json!(28)),
        ("current_weapon", json!(0x02000551)),
        (
            "pending_dependency",
            json!("preload_temporary_evaluation_sampling"),
        ),
    ] {
        p[k] = v;
    }
    p["allocations"][0]["known_payload_bytes"] = json!(368);
    p["allocations"][1]["known_payload_bytes"] = json!(6664);
    p["caches"]["headers"][0]["invalidation_count_reads"] = json!(33);
    p["caches"]["headers"][0]["invalidation_pointer_reads"] = json!(33);
    let history=snapshots.iter().skip(16).map(|s|json!({"snapshot":s,"destruction":{"stage":"destroyed","pending_dependency":null,"temporary_identity":s["temporary_identity"],"ordinal":s["ordinal"],"children":1,"skeleton":{"bones":48,"auxiliary_words":0,"block_bytes":1536},"snapshot_words":100,"snapshot_digest":"0123456789abcdef","state_sha256":"e".repeat(64)}})).collect::<Vec<_>>();
    p["completion"] = json!({"history_limit":16,"completed_temporary_count":32,"discarded_temporary_count":16,"found_output_count":192,"missing_output_count":0,"destroyed_temporary_count":32,"entry_commit_count":32,"history":history,"pending_snapshot":null,"first_completed_snapshot":snapshots[0],"destruction":null});
    p
}
