use super::*;

fn context(level: u32, difficulty: u8) -> Value {
    let difficulty_label = ["Operative", "Agent", "007", "007 Classic"][usize::from(difficulty)];
    let (text, objects, io, intro, banks, map) = if level == TUTORIAL {
        (
            0x0801_0019u32,
            0u32,
            0u32,
            0u32,
            [0x2d50_0079u32, u32::MAX, u32::MAX],
            0x8500_0000u32,
        )
    } else {
        (
            0x0801_000bu32,
            0x0109_0048u32,
            0x0106_0036u32,
            0x4210_003fu32,
            [0x2d50_001cu32, u32::MAX, 0x2d50_005bu32],
            0x0500_0024u32,
        )
    };
    let (next_file, next_map, ordinal, schema_index) = if level == TUTORIAL {
        (0x018b_002au32, 0x0500_0070u32, 40, 10)
    } else {
        (0x018b_0006u32, 0x0500_0000u32, 0, 0)
    };
    let mut value = json!({
        "state":"resources_retained", "catalog":{
            "resource_hash":0x0109_0006u32,"payload_bytes":4096,
            "primary_sheet_hash":0x1400_0011u32,"secondary_sheet_hash":0x1400_003au32,
            "primary_rows_read":3,"secondary_rows_read":2,"primary_rows_registered":2,
            "secondary_rows_registered":2,"duplicate_rows":1
        },"selection":{
            "level_hash":level,"difficulty":difficulty,
            "difficulty_label":difficulty_label,
            "source_row_index":1,"source_row_address":512,"text_page_hash":text,
            "mission_object_resource_raw":objects,"io_resource_raw":io,"intro_movie_hash":intro,
            "outro_movie_hash":0,"bank_hashes":banks,"tuning_hash":level,"character_request":0x4d0001b9u32
        },"resource":{
            "resource_hash":level,"payload_bytes":8192,"base_bytes":4096,
            "requested_map_reference":0x8500_0000u32,"resolved_map_hash":map,
            "next_link":{"file":next_file,"map":next_map,"locator":{
                "map_reference":0x8500_0000u32,"map_directory_index":0,"map_hash":map,
                "map_address":256,"trigger_block":512,"trigger_ordinal":ordinal,"record_address":768,
                "schema_index":schema_index,"schema_address":1024,"native_type_index":62,"flags_raw":3}},
            "entity_descriptors":2,"script_descriptors":1,"textures":0,
            "maps":[{"directory_index":0,"hash":map,"zones":1,"bsp_present":false,"bsp_nodes":0,
                "placements":1,"first_placement_reference":0x8200_0000u32,
                "first_placement_entity":{"directory_index":0,"hash":0x0200_0358u32,"entity_type":0x607}}]
        },"resources_retained":true,"world_registered":false,"player_initialized":false,
        "reference_compared":false,"scope":"Authored metadata-only context fixture."
    });
    let root = &value["resource"];
    let resources = json!([
        {"ordinal":0,"file_hash":level,"requested_map_reference":0x85000000u32,"resolved_map_hash":map,
         "map_directory_index":0,"payload_bytes":8192,"base_bytes":4096,"zones":1,"bsp_present":false,"bsp_nodes":0,
         "placements":1,"entity_descriptors":2,"script_descriptors":1,"textures":0,"next_link":root["next_link"]},
        {"ordinal":1,"file_hash":next_file,"requested_map_reference":0x85000000u32,"resolved_map_hash":next_map,
         "map_directory_index":0,"payload_bytes":2048,"base_bytes":1024,"zones":1,"bsp_present":false,"bsp_nodes":0,
         "placements":0,"entity_descriptors":1,"script_descriptors":0,"textures":0,"next_link":null}
    ]);
    value["residency"] = json!({"complete":true,"stop_reason":"no_link","repeated_file":null,
        "retained_files":2,"total_payload_bytes":10240,"file_limit":8,"per_file_byte_limit":16384,"total_byte_limit":32768,
        "resources":resources,"scope":"Authored retained ownership."});
    value["registration"] = json!({"prepared":true,"preference_snapshot":"source_initializer_once","native_init_timing_verified":false,
        "rows":[{"source_ordinal":0,"file_hash":level,"map_reference":0x85000000u32,"flags_raw":[255,0,1],"subtype_allowlist":null},
                {"source_ordinal":1,"file_hash":next_file,"map_reference":0x85000000u32,"flags_raw":[255,1,1],"subtype_allowlist":[0x1f000088u32]}],
        "scope":"Authored prepared rows; no instances created."});
    value["map_instances"] = json!({"bound":true,"preregistration_slots":3,"loaded_instances":2,
        "base_animators_initialized":false,"trigger_factories_run":false,"navigation_initialized":false,
        "instances":[
            {"source_ordinal":0,"loaded_ordinal":0,"wrapper_id":1,"instance_id":1,"preregistration_index":1,"file_hash":level,
             "requested_map_reference":0x85000000u32,"canonical_map_reference":0x85000000u32,"stored_map_hash":map,"map_directory_index":0,
             "zones":1,"bsp_present":false,"bsp_nodes":0,"placements_bound":1,"navigation_placements":1,
             "declared_bounds_sha256":"ab".repeat(32),"placement_matrices_sha256":"bc".repeat(32)},
            {"source_ordinal":1,"loaded_ordinal":1,"wrapper_id":2,"instance_id":2,"preregistration_index":2,"file_hash":next_file,
             "requested_map_reference":0x85000000u32,"canonical_map_reference":0x85000000u32,"stored_map_hash":next_map,"map_directory_index":0,
             "zones":1,"bsp_present":false,"bsp_nodes":0,"placements_bound":0,"navigation_placements":0,
             "declared_bounds_sha256":"ab".repeat(32),"placement_matrices_sha256":"bc".repeat(32)}],"scope":"Authored map data binding."});
    value["spawn"] = json!({"state":"prepared_startpoint","requested_id_before":1,"requested_id_after":1,
        "registration_rows":2,"eligible_files":1,"filtered_files":1,"prepared_entries":3,"visits":1,
        "selection_found":true,"used_fallback":false,"selected_catalog_index":0,
        "selected":{"file_hash":level,"source_ordinal":0,"prepared_row_index":0,"locator":{
            "map_reference":0x85000000u32,"map_directory_index":0,"map_hash":map,"map_address":256,"trigger_block":512,
            "trigger_ordinal":0,"record_address":768,"schema_index":0,"schema_address":1024,"native_type_index":28}},
        "pose_sha256":"44e09bcdcf1a9e77a8a1b353e5692bc94b55fdacb68b1d218433a985b13d73ff",
        "pose_sha256_encoding":"position_u32be[4] || rotation_u32be[4] || rotation_mode_u8","rotation_mode":3,
        "catalog_membership_verified":false,"catalog_created":false,"creation_inputs_verified":false,"camera_frame_ready":false,
        "scope":"Authored hash-only prepared selection."});
    value["player_components"] = json!({
        "state":"fp_skeleton_constructor_initialized","requested_character":0x4d0001b9u32,
        "catalog_resource":0x01090013u32,"catalog_payload_bytes":32768,
        "character_sheet":0x14000021u32,"character_group":1,"character_row_stride":24,
        "character_rows":8,"duplicate_character_rows":1,"selected_character":0x4d0001b9u32,
        "selected_source_index":1,"selected_source_address":1024,"native_default_fallback_used":false,
        "component_resource":0x01020016u32,"component_payload_bytes":16384,"component_reference":0,
        "skin_resource":0x010a0050u32,"skin_payload_bytes":8192,"skin_reference":0x0d00016fu32,
        "skin_directory_index":0,"skin_hash":0x0d00016fu32,"skin_body_address":512,
        "retained_resources":3,"retained_payload_bytes":57344,"bone_count":48,"root_count":1,
        "topology_sha256":"a7d3e7cca6f71ca5cfa486550224158647942af521946626e25a37f506f21c0f",
        "constructor_state_sha256":"2b2bdad0125d0bb6b2f8c289e44be757bf1c5a46c3292a4049316ffedc5de3bb",
        "camera_tag":0x0e000028u32,"camera_tag_found":true,"camera_bone":47,
        "camera_parent_chain_length":3,
        "camera_parent_chain_sha256":"c5688be2fdb2cb91c673a6f9caab734be0fe55c9877c7ec353b98650f936a932",
        "skeleton_constructor_initialized":true,"animation_sampled":false,
        "player_physics_initialized":false,"camera_frame_ready":false,"mode_plan":null,"initial_layers":null,"camera_bone_transform":null,"scope":"Authored FP constructor binding."
    });
    value
}

#[test]
fn fp_binding_rejects_wrong_dependency_and_constructor_fingerprints() {
    for (field, bad) in [
        ("skin_resource", json!(0x01020016u32)),
        ("requested_character", json!(0x4d0001b8u32)),
        ("character_group", json!(0)),
        ("retained_payload_bytes", json!(1)),
        ("topology_sha256", json!("aa".repeat(32))),
        ("constructor_state_sha256", json!("bb".repeat(32))),
        ("camera_parent_chain_sha256", json!("cc".repeat(32))),
    ] {
        let mut fixture = context(TUTORIAL, 0);
        fixture["player_components"][field] = bad;
        assert!(validate(&fixture, TUTORIAL, 0).is_err(), "{field}");
    }
}

fn mode_plan() -> Value {
    json!({"state":"planned","input_source":"explicit_host_state",
        "requested_mode":0x0900071eu32,"previous_mode":0,"fallback_mode":0x0900065eu32,
        "target_index":13,"return_word":0x0900000bu32,"attempts":1,"matched_transitions":1,
        "planned_commands":2,"mask_operations":1,"start_sets":1,"layer_groups":1,"child_requests":1,
        "previous_matches":1,"fallback_matches":0,
        "plan_sha256":"a86afa571d8aec797d77d50a86ec14c5f45d1d1ff7efdd57decc02750b7b4299",
        "plan_encoding":"mode_plan_be_v1","initial_request_verified":false,"runtime_children_created":false,
        "animation_sampled":false,"camera_frame_ready":false,"scope":"Authored explicit caller plan fixture."})
}

#[test]
fn explicit_mode_probe_requires_source_fingerprint_without_changing_default_context() {
    let mut fixture = context(TUTORIAL, 0);
    assert!(fixture["player_components"]["mode_plan"].is_null());
    validate(&fixture, TUTORIAL, 0).unwrap();
    fixture["player_components"]["mode_plan"] = mode_plan();
    validate(&fixture, TUTORIAL, 0).unwrap();
    player::validate_mode_probe(
        &fixture["player_components"]["mode_plan"],
        [0x0900071e, 0, 0x0900065e],
    )
    .unwrap();
    fixture["player_components"]["mode_plan"]["plan_sha256"] = json!("aa".repeat(32));
    assert!(
        player::validate_mode_probe(
            &fixture["player_components"]["mode_plan"],
            [0x0900071e, 0, 0x0900065e]
        )
        .is_err()
    );
}

#[test]
fn mode_probe_rejects_readiness_promotion_invalid_counts_and_source_array_exports() {
    for (field, bad) in [
        ("initial_request_verified", json!(true)),
        ("runtime_children_created", json!(true)),
        ("animation_sampled", json!(true)),
        ("camera_frame_ready", json!(true)),
        ("attempts", json!(0)),
        ("matched_transitions", json!(2)),
        ("planned_commands", json!(3)),
        ("child_requests", json!(16385)),
        ("target_index", json!(null)),
        ("commands", json!([])),
    ] {
        let mut fixture = context(TUTORIAL, 0);
        fixture["player_components"]["mode_plan"] = mode_plan();
        fixture["player_components"]["mode_plan"][field] = bad;
        assert!(validate(&fixture, TUTORIAL, 0).is_err(), "{field}");
    }
}

#[test]
fn fp_constructor_diagnostics_never_promote_sampled_pose_player_or_camera_readiness() {
    for field in [
        "native_default_fallback_used",
        "animation_sampled",
        "player_physics_initialized",
        "camera_frame_ready",
    ] {
        let mut fixture = context(TUTORIAL, 0);
        fixture["player_components"][field] = json!(true);
        assert!(validate(&fixture, TUTORIAL, 0).is_err(), "{field}");
    }
    let mut fixture = context(TUTORIAL, 0);
    fixture["player_components"]["bone_arrays"] = json!([]);
    assert!(validate(&fixture, TUTORIAL, 0).is_err());
    fixture["player_components"]
        .as_object_mut()
        .unwrap()
        .remove("bone_arrays");
    fixture["player_components"]["camera_bone"] = json!(null);
    assert!(validate(&fixture, TUTORIAL, 0).is_err());
}

#[test]
fn all_source_difficulties_and_both_mission_identities_are_required() {
    for difficulty in 0..=3 {
        validate(&context(TUTORIAL, difficulty), TUTORIAL, difficulty).unwrap();
    }
    validate(&context(DAM, 0), DAM, 0).unwrap();
    assert!(validate(&context(TUTORIAL, 0), DAM, 0).is_err());
    assert!(validate(&context(TUTORIAL, 0), TUTORIAL, 1).is_err());
    let mut altered = context(TUTORIAL, 0);
    altered["selection"]["difficulty_label"] = json!("Easy");
    assert!(validate(&altered, TUTORIAL, 0).is_err());
}

#[test]
fn readiness_and_parity_claims_cannot_be_promoted_by_metadata() {
    for (pointer, invalid) in [
        ("/state", json!("playing")),
        ("/resources_retained", json!(false)),
        ("/world_registered", json!(true)),
        ("/player_initialized", json!(true)),
        ("/reference_compared", json!(true)),
        ("/world_registered", Value::Null),
    ] {
        let mut altered = context(TUTORIAL, 0);
        *altered.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&altered, TUTORIAL, 0).is_err(), "{pointer}");
    }
}

#[test]
fn catalog_requires_bounded_consistent_counts_and_addressed_source_rows() {
    for (pointer, invalid) in [
        ("/catalog/resource_hash", json!(0)),
        ("/catalog/primary_rows_read", json!(0)),
        ("/catalog/secondary_rows_read", json!(SECONDARY_LIMIT + 1)),
        ("/catalog/primary_rows_registered", json!(4)),
        ("/catalog/duplicate_rows", json!(0)),
        ("/selection/source_row_index", json!(3)),
        ("/selection/source_row_address", json!(4000)),
        ("/catalog/payload_bytes", json!(u64::MAX)),
    ] {
        let mut altered = context(TUTORIAL, 0);
        *altered.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&altered, TUTORIAL, 0).is_err(), "{pointer}");
    }
}

#[test]
fn source_movies_banks_and_coded_navmesh_binding_are_not_optional() {
    for (pointer, invalid) in [
        ("/selection/text_page_hash", json!(0)),
        ("/selection/bank_hashes", json!([0, 0, 0])),
        ("/selection/tuning_hash", Value::Null),
        (
            "/resource/maps/0/first_placement_reference",
            json!(0x0200_0358u32),
        ),
        (
            "/resource/maps/0/first_placement_entity/entity_type",
            json!(0x601),
        ),
        (
            "/resource/maps/0/first_placement_entity/directory_index",
            json!(2),
        ),
        ("/resource/maps/0/hash", json!(0x0500_0070u32)),
        ("/resource/resource_hash", json!(0x018b_002au32)),
        ("/resource/requested_map_reference", json!(0x0500_0070u32)),
        ("/resource/resolved_map_hash", Value::Null),
    ] {
        let mut altered = context(TUTORIAL, 0);
        *altered.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&altered, TUTORIAL, 0).is_err(), "{pointer}");
    }
    let mut altered = context(DAM, 0);
    altered["selection"]["intro_movie_hash"] = json!(0);
    assert!(validate(&altered, DAM, 0).is_err());
}

#[test]
fn closed_schema_rejects_original_data_exports_even_with_valid_metadata() {
    for pointer in [
        "",
        "/catalog",
        "/selection",
        "/resource",
        "/resource/maps/0",
        "/resource/maps/0/first_placement_entity",
    ] {
        let mut altered = context(TUTORIAL, 0);
        altered
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("words_raw".into(), json!([1, 2, 3, 4]));
        assert!(validate(&altered, TUTORIAL, 0).is_err(), "{pointer}");
    }
    let mut altered = context(TUTORIAL, 0);
    altered.as_object_mut().unwrap().remove("scope");
    assert!(validate(&altered, TUTORIAL, 0).is_err());
}

#[test]
fn malformed_map_counts_and_fake_first_placements_fail() {
    for (pointer, invalid) in [
        ("/resource/maps", json!([])),
        ("/resource/maps/0/directory_index", json!(1)),
        ("/resource/maps/0/placements", json!(0)),
        ("/resource/maps/0/bsp_nodes", json!(1)),
        ("/resource/maps/0/bsp_present", Value::Null),
        ("/resource/base_bytes", json!(9000)),
        ("/resource/entity_descriptors", json!(0)),
        ("/resource/script_descriptors", json!(u64::MAX)),
    ] {
        let mut altered = context(TUTORIAL, 0);
        *altered.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&altered, TUTORIAL, 0).is_err(), "{pointer}");
    }
}

#[test]
fn unknown_level_needs_a_clean_error_without_a_context_receipt() {
    let error = b"Error: requested mission level is absent";
    validate_unknown(false, b"", error, false).unwrap();
    for (success, stdout, stderr, exists) in [
        (true, b"".as_slice(), error.as_slice(), false),
        (false, b"{}".as_slice(), error.as_slice(), false),
        (false, b"".as_slice(), error.as_slice(), true),
        (false, b"".as_slice(), b"".as_slice(), false),
        (
            false,
            b"".as_slice(),
            b"thread mission panicked".as_slice(),
            false,
        ),
    ] {
        assert!(validate_unknown(success, stdout, stderr, exists).is_err());
    }
}

#[test]
fn linked_map_metadata_requires_native_class_first_match_and_bounded_provenance() {
    for (pointer, invalid) in [
        ("/resource/next_link", Value::Null),
        ("/resource/next_link/file", json!(0x0181_016bu32)),
        ("/resource/next_link/map", json!(0x8500_0000u32)),
        (
            "/resource/next_link/locator/map_reference",
            json!(0x0500_0070u32),
        ),
        (
            "/resource/next_link/locator/map_hash",
            json!(0x0500_0070u32),
        ),
        ("/resource/next_link/locator/map_directory_index", json!(1)),
        ("/resource/next_link/locator/trigger_ordinal", json!(41)),
        ("/resource/next_link/locator/schema_index", json!(11)),
        ("/resource/next_link/locator/native_type_index", json!(61)),
        ("/resource/next_link/locator/flags_raw", json!(0)),
        ("/resource/next_link/locator/record_address", json!(8190)),
        (
            "/resource/next_link/locator/schema_address",
            json!(u64::MAX),
        ),
    ] {
        let mut altered = context(TUTORIAL, 0);
        *altered.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&altered, TUTORIAL, 0).is_err(), "{pointer}");
    }
    for pointer in ["/resource/next_link", "/resource/next_link/locator"] {
        let mut altered = context(TUTORIAL, 0);
        altered
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("raw_record".into(), json!([1, 2, 3]));
        assert!(validate(&altered, TUTORIAL, 0).is_err());
    }
    validate_link(&Value::Null, 8192, 0x8500_0000, None).unwrap();
    let mut optional = context(TUTORIAL, 0)["resource"]["next_link"].clone();
    optional["locator"]["flags_raw"] = json!(1);
    optional["map"] = json!(u32::MAX);
    validate_link(&optional, 8192, 0x8500_0000, None).unwrap();
    optional["map"] = json!(0);
    assert!(validate_link(&optional, 8192, 0x8500_0000, None).is_err());
}

#[test]
fn residency_requires_actual_unique_owners_chain_order_byte_totals_and_real_termination() {
    for (pointer, invalid) in [
        ("/residency/complete", json!(false)),
        ("/residency/retained_files", json!(1)),
        ("/residency/total_payload_bytes", json!(10239)),
        ("/residency/total_byte_limit", json!(8192)),
        ("/residency/resources/1/file_hash", json!(TUTORIAL)),
        (
            "/residency/resources/1/requested_map_reference",
            json!(0x05000070u32),
        ),
        (
            "/residency/resources/1/resolved_map_hash",
            json!(0x85000000u32),
        ),
        ("/residency/resources/0/next_link", Value::Null),
        ("/residency/stop_reason", json!("repeated_file")),
    ] {
        let mut value = context(TUTORIAL, 0);
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&value, TUTORIAL, 0).is_err(), "{pointer}");
    }
}

#[test]
fn a_repeated_file_stop_must_refer_to_an_existing_owner_without_appending_it_twice() {
    let mut value = context(DAM, 0);
    let mut link = value["resource"]["next_link"].clone();
    link["file"] = json!(DAM);
    link["locator"]["map_hash"] = value["residency"]["resources"][1]["resolved_map_hash"].clone();
    value["residency"]["resources"][1]["next_link"] = link;
    value["residency"]["stop_reason"] = json!("repeated_file");
    value["residency"]["repeated_file"] = json!(DAM);
    assert!(validate(&value, DAM, 0).is_ok());
    value["residency"]["repeated_file"] = json!(0x12345678u32);
    assert!(validate(&value, DAM, 0).is_err());
}

#[test]
fn prepared_registration_cannot_invent_original_history_or_change_owned_row_bindings() {
    for (pointer, invalid) in [
        ("/registration/prepared", json!(false)),
        ("/registration/native_init_timing_verified", json!(true)),
        ("/registration/rows/1/source_ordinal", json!(0)),
        ("/registration/rows/0/flags_raw", json!([255, 1, 1])),
        ("/registration/rows/0/subtype_allowlist", json!([])),
        ("/registration/rows/1/subtype_allowlist", Value::Null),
    ] {
        let mut value = context(TUTORIAL, 0);
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&value, TUTORIAL, 0).is_err(), "{pointer}");
    }
    let mut value = context(TUTORIAL, 0);
    value["registration"]["world_instances_created"] = json!(true);
    assert!(validate(&value, TUTORIAL, 0).is_err());
}

#[test]
fn map_binding_requires_native_slot_offset_and_never_promotes_trigger_execution() {
    for (pointer, invalid) in [
        ("/map_instances/preregistration_slots", json!(2)),
        ("/map_instances/instances/0/preregistration_index", json!(0)),
        ("/map_instances/instances/1/loaded_ordinal", json!(2)),
        (
            "/map_instances/instances/1/canonical_map_reference",
            json!(0x05000070u32),
        ),
        ("/map_instances/instances/0/placements_bound", json!(0)),
        ("/map_instances/base_animators_initialized", json!(true)),
        ("/map_instances/trigger_factories_run", json!(true)),
    ] {
        let mut value = context(TUTORIAL, 0);
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&value, TUTORIAL, 0).is_err(), "{pointer}");
    }
}

#[test]
fn spawn_selection_fingerprint_does_not_establish_membership_creation_or_camera_readiness() {
    for (pointer, invalid) in [
        ("/spawn/pose_sha256", json!("de".repeat(32))),
        ("/spawn/rotation_mode", json!(0)),
        ("/spawn/selected/source_ordinal", json!(1)),
        ("/spawn/catalog_created", json!(true)),
        ("/spawn/catalog_membership_verified", json!(true)),
        ("/spawn/camera_frame_ready", json!(true)),
    ] {
        let mut value = context(TUTORIAL, 0);
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(validate(&value, TUTORIAL, 0).is_err(), "{pointer}");
    }
    let mut value = context(TUTORIAL, 0);
    value["spawn"]["position_bits"] = json!([1, 2, 3, 0]);
    assert!(validate(&value, TUTORIAL, 0).is_err());
}

#[test]
fn explicit_id_zero_fallback_has_request_side_effects_and_matching_source_locator() {
    let mut value = context(TUTORIAL, 0);
    value["spawn"]["requested_id_before"] = json!(99);
    value["spawn"]["requested_id_after"] = json!(0);
    value["spawn"]["used_fallback"] = json!(true);
    value["spawn"]["selected_catalog_index"] = json!(2);
    value["spawn"]["visits"] = json!(6);
    value["spawn"]["selected"]["locator"]["trigger_ordinal"] = json!(82);
    assert!(validate_for_request(&value, TUTORIAL, 0, 99).is_ok());
    value["spawn"]["requested_id_after"] = json!(99);
    assert!(validate_for_request(&value, TUTORIAL, 0, 99).is_err());
}
