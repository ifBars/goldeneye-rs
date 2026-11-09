use super::*;
use serde_json::json;

fn fixture() -> Value {
    json!({
        "state":"fresh_fp_layers_initialized", "boundary":"fresh_fp_constructor_callback_snapshot",
        "inventory":{
            "inventory_key":0x0200_0830u32,"records":1,"selected_record":0,"count":1,"limit":1,
            "flags":0,"ordinal_raw":255,"guns_resource":0x0109_000fu32,"guns_payload_bytes":7_765_056,
            "metadata_source_index":0,"metadata_primary_key":0x4b00_002cu32,"weapon_registry_keys":38,
            "weapon_order_keys":29,"selected_order_index":27,"primary_request":0x0900_071eu32,
            "secondary_request":0x0900_0b70u32,"static_data":{
                "game_id":"SJBE52","revision":0,"main_dol_partition_offset":261_888,
                "dol_bytes":6_087_648,"dol_sha256":DOL,"data_section_index":5,
                "data_virtual_address":0x8056_bba0u32,"data_bytes":383_136,"data_sha256":DATA
            }
        },
        "mode_requests":2,"active_layers":2,"live_nodes":3,"controls":1,"script_children":2,
        "bound_clips":5,"unique_animations":4,"captured_bones":48,"selection_mask":251,
        "clear_mask":251,"slot_zero_nodes":2,"slot_one_nodes":1,"membership_sha256":MEMBERSHIP,
        "bindings_sha256":BINDINGS,"bindings_encoding":"owned_animation_nodes_be_v1",
        "post_selector_scalars_applied":true,"post_selector_head_writes":2,"raw_frame":null,"prewarm_pose":null,
        "animation_ticked":false,"pose_sampled":false,
        "player_initialized":false,"camera_frame_ready":false,"scope":"Authored boundary metadata."
    })
}

fn raw_fixture() -> Value {
    json!({
        "state":"raw_frame_decoded","source_child":0x8400_0022u32,"selected_clip":0,
        "blend_fraction_bits":0,"requested_format_flags_raw":0,
        "animation_reference":0x8300_000au32,"animation_directory_index":10,"frame":0,
        "rotations":48,"translations":6,"exact_branch_rotations":1,"unverified_sqrt_rotations":47,
        "raw_frame_sha256":RAW_FRAME,"camera_raw_sha256":CAMERA_RAW,
        "camera_raw_exact_branch":true,"raw_encoding":"raw_polynomial_frame_be_v1",
        "cached_frames":1,"decode_calls":1,"cache_hits":0,
        "root_bias_applied":false,"skin_mapping_applied":false,"layer_blending_applied":false,
        "skeleton_committed":false,"native_numeric_parity_verified":false,
        "first_displayed_pose_verified":false,"camera_frame_ready":false,
        "scope":"Authored raw decoder boundary; exact branch labels do not assert native numerical parity."
    })
}

#[test]
fn source_boundary_has_owned_objects_and_hashes_but_never_implies_tick_pose_or_camera() {
    let valid = fixture();
    validate(&valid).unwrap();
    for field in [
        "animation_ticked",
        "pose_sampled",
        "player_initialized",
        "camera_frame_ready",
    ] {
        let mut changed = valid.clone();
        changed[field] = json!(true);
        assert!(
            validate(&changed).is_err(),
            "promoted readiness accepted: {field}"
        );
    }
    for field in [
        "active_layers",
        "live_nodes",
        "bound_clips",
        "unique_animations",
        "captured_bones",
        "selection_mask",
        "slot_zero_nodes",
        "slot_one_nodes",
        "post_selector_head_writes",
    ] {
        let mut changed = valid.clone();
        changed[field] = json!(0);
        assert!(
            validate(&changed).is_err(),
            "changed source count accepted: {field}"
        );
    }
    for field in ["membership_sha256", "bindings_sha256"] {
        for bad in [
            json!("a".repeat(63)),
            json!("g".repeat(64)),
            json!("A".repeat(64)),
            json!("0".repeat(64)),
            Value::Null,
        ] {
            let mut changed = valid.clone();
            changed[field] = bad;
            assert!(
                validate(&changed).is_err(),
                "invalid or differing digest accepted: {field}"
            );
        }
    }
    let mut changed = valid;
    changed["boundary"] = json!("live_player_ready");
    assert!(validate(&changed).is_err());
}

#[test]
fn inventory_static_identity_and_closed_schema_reject_wrong_source_and_exported_arrays() {
    let valid = fixture();
    for field in [
        "inventory_key",
        "count",
        "flags",
        "ordinal_raw",
        "guns_payload_bytes",
        "metadata_primary_key",
        "weapon_registry_keys",
        "weapon_order_keys",
        "selected_order_index",
        "primary_request",
        "secondary_request",
    ] {
        let mut changed = valid.clone();
        changed["inventory"][field] = json!(number(&valid["inventory"], field).unwrap() + 1);
        assert!(
            validate(&changed).is_err(),
            "changed inventory accepted: {field}"
        );
    }
    for field in [
        "game_id",
        "revision",
        "dol_bytes",
        "dol_sha256",
        "data_section_index",
        "data_virtual_address",
        "data_bytes",
        "data_sha256",
    ] {
        let mut changed = valid.clone();
        changed["inventory"]["static_data"][field] = Value::Null;
        assert!(
            validate(&changed).is_err(),
            "missing static identity accepted: {field}"
        );
    }
    for offset in [0u64, 0x441, u64::MAX] {
        let mut changed = valid.clone();
        changed["inventory"]["static_data"]["main_dol_partition_offset"] = json!(offset);
        assert!(validate(&changed).is_err());
    }
    for path in [
        "/nodes",
        "/pose_bits",
        "/inventory/rows_raw",
        "/inventory/static_data/raw_dol",
        "/inventory/static_data/data_bytes_raw",
    ] {
        let mut changed = valid.clone();
        let (parent, key) = path.rsplit_once('/').unwrap();
        changed
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), json!([1, 2, 3]));
        assert!(
            validate(&changed).is_err(),
            "unexpected source export accepted: {path}"
        );
    }
}

#[test]
fn optional_raw_frame_requires_applied_scalars_and_closed_source_frame_metadata() {
    let mut valid = fixture();
    validate(&valid).unwrap();
    valid["raw_frame"] = raw_fixture();
    validate(&valid).unwrap();
    for (field, changed_value) in [
        ("post_selector_scalars_applied", json!(false)),
        ("post_selector_head_writes", json!(1)),
        ("raw_frame", json!("decoded")),
    ] {
        let mut changed = valid.clone();
        changed[field] = changed_value;
        assert!(
            validate(&changed).is_err(),
            "unsupported new field value accepted: {field}"
        );
    }
    for field in [
        "source_child",
        "selected_clip",
        "blend_fraction_bits",
        "requested_format_flags_raw",
        "animation_reference",
        "animation_directory_index",
        "frame",
        "rotations",
        "translations",
        "cached_frames",
        "decode_calls",
        "cache_hits",
    ] {
        let mut changed = valid.clone();
        changed["raw_frame"][field] = json!(number(&valid["raw_frame"], field).unwrap() + 1);
        assert!(
            validate(&changed).is_err(),
            "different raw source/frame accepted: {field}"
        );
    }
    for field in ["raw_frame_sha256", "camera_raw_sha256"] {
        for bad in ["A".repeat(64), "0".repeat(64)] {
            let mut changed = valid.clone();
            changed["raw_frame"][field] = json!(bad);
            assert!(validate(&changed).is_err());
        }
    }
    let mut changed = valid.clone();
    changed["raw_frame"]["camera_raw_sha256"] = json!("0".repeat(64));
    assert!(validate(&changed).is_err());
    let mut changed = valid.clone();
    changed["raw_frame"]["raw_encoding"] = json!("native_pose");
    assert!(validate(&changed).is_err());
    for key in [
        "rotations_raw",
        "translation_bits",
        "bone_arrays",
        "payload_bytes_raw",
    ] {
        let mut changed = valid.clone();
        changed["raw_frame"]
            .as_object_mut()
            .unwrap()
            .insert(key.into(), json!([0, 1]));
        assert!(
            validate(&changed).is_err(),
            "raw source export accepted: {key}"
        );
    }
}

#[test]
fn branch_precision_is_distinct_from_native_parity_and_all_pose_readiness_stays_false() {
    let mut valid = fixture();
    valid["raw_frame"] = raw_fixture();
    validate(&valid).unwrap();
    for field in [
        "root_bias_applied",
        "skin_mapping_applied",
        "layer_blending_applied",
        "skeleton_committed",
        "native_numeric_parity_verified",
        "first_displayed_pose_verified",
        "camera_frame_ready",
    ] {
        let mut changed = valid.clone();
        changed["raw_frame"][field] = json!(true);
        assert!(
            validate(&changed).is_err(),
            "raw frame readiness promoted: {field}"
        );
    }
    for (exact, approximate) in [(48, 0), (0, 48), (1, 46), (49, 0), (2, 46)] {
        let mut changed = valid.clone();
        changed["raw_frame"]["exact_branch_rotations"] = json!(exact);
        changed["raw_frame"]["unverified_sqrt_rotations"] = json!(approximate);
        assert!(
            validate(&changed).is_err(),
            "invalid/concealed precision partition accepted"
        );
    }
    let mut changed = valid;
    changed["raw_frame"]["camera_raw_exact_branch"] = json!(false);
    assert!(validate(&changed).is_err());
}
