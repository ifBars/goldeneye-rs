//! Closed constructor diagnostics and the separate optional reset-prewarm sample.
use super::super::{closed, number};
use anyhow::{Result, ensure};
use serde_json::Value;
mod prewarm;

const MEMBERSHIP: &str = "a946ac509ecd9a432014688c51bff29fdd67f152bbe9cdfdee9cf4bdfde8525c";
const BINDINGS: &str = "a6a0cf2678060917d94cfe0878939a4024e5c786cbd60bfe50deafd49ee6aa09";
const DOL: &str = "0d101e5eba1aca38847c03d7a353f5156f3ec2d9025d6c6773d105561532208b";
const DATA: &str = "76d3456f33f013817ea80ddec93635c466d25bba233929800399b4d3e703836d";
const CAMERA_RAW: &str = "0d89e648ebd5afd1cc431430ddbad91c44fbd151c1f82f529fe910d31fa86193";
// Independent private frame-zero polynomial reconstruction plus matching host
// probes. This fingerprints source/host raw decoding, not native PPC sqrt parity.
const RAW_FRAME: &str = "1235865dc2f207e8d432d5c606f821a752e2d98d092c14a5fb71fa827170c6d8";

pub(super) fn validate(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "boundary",
            "inventory",
            "mode_requests",
            "active_layers",
            "live_nodes",
            "controls",
            "script_children",
            "bound_clips",
            "unique_animations",
            "captured_bones",
            "selection_mask",
            "clear_mask",
            "slot_zero_nodes",
            "slot_one_nodes",
            "membership_sha256",
            "bindings_sha256",
            "bindings_encoding",
            "post_selector_scalars_applied",
            "post_selector_head_writes",
            "raw_frame",
            "prewarm_pose",
            "animation_ticked",
            "pose_sampled",
            "player_initialized",
            "camera_frame_ready",
            "scope",
        ],
    )?;
    for field in ["membership_sha256", "bindings_sha256"] {
        digest(value, field)?;
    }
    ensure!(
        value["state"] == "fresh_fp_layers_initialized"
            && value["boundary"] == "fresh_fp_constructor_callback_snapshot"
            && value["bindings_encoding"] == "owned_animation_nodes_be_v1",
        "initial layers changed the explicit fresh callback boundary/encoding"
    );
    for (field, expected) in [
        ("mode_requests", 2),
        ("active_layers", 2),
        ("live_nodes", 3),
        ("controls", 1),
        ("script_children", 2),
        ("bound_clips", 5),
        ("unique_animations", 4),
        ("captured_bones", 48),
        ("selection_mask", 0xfb),
        ("clear_mask", 0xfb),
        ("slot_zero_nodes", 2),
        ("slot_one_nodes", 1),
        ("post_selector_head_writes", 2),
    ] {
        ensure!(
            number(value, field)? == expected,
            "owned fresh FP layer source count/mask differs: {field}"
        );
    }
    let prewarm = !value["prewarm_pose"].is_null();
    for field in ["animation_ticked", "pose_sampled"] {
        ensure!(
            value[field] == prewarm,
            "animation update/sample flag differs from validated optional prewarm: {field}"
        );
    }
    for field in ["player_initialized", "camera_frame_ready"] {
        ensure!(
            value[field] == false,
            "fresh layer snapshot promoted unavailable runtime work: {field}"
        );
    }
    ensure!(
        value["post_selector_scalars_applied"] == true,
        "source post-selector head scalar writes were not applied"
    );
    ensure!(
        value["membership_sha256"] == MEMBERSHIP && value["bindings_sha256"] == BINDINGS,
        "owned fresh FP membership or binding fingerprint differs from source-bound audit"
    );
    inventory(&value["inventory"])?;
    if !value["raw_frame"].is_null() {
        ensure!(
            !prewarm,
            "raw constructor decode and mutated prewarm boundaries were mixed"
        );
        raw_frame(&value["raw_frame"])?;
    }
    if prewarm {
        prewarm::validate(&value["prewarm_pose"])?;
    }
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|scope| !scope.is_empty() && scope.len() <= 2048),
        "initial layer boundary scope missing"
    );
    Ok(())
}

fn raw_frame(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "source_child",
            "selected_clip",
            "blend_fraction_bits",
            "requested_format_flags_raw",
            "animation_reference",
            "animation_directory_index",
            "frame",
            "rotations",
            "translations",
            "exact_branch_rotations",
            "unverified_sqrt_rotations",
            "raw_frame_sha256",
            "camera_raw_sha256",
            "camera_raw_exact_branch",
            "raw_encoding",
            "cached_frames",
            "decode_calls",
            "cache_hits",
            "root_bias_applied",
            "skin_mapping_applied",
            "layer_blending_applied",
            "skeleton_committed",
            "native_numeric_parity_verified",
            "first_displayed_pose_verified",
            "camera_frame_ready",
            "scope",
        ],
    )?;
    ensure!(
        value["state"] == "raw_frame_decoded"
            && value["raw_encoding"] == "raw_polynomial_frame_be_v1",
        "raw frame boundary/encoding differs"
    );
    for (field, expected) in [
        ("source_child", 0x8400_0022u64),
        ("selected_clip", 0),
        ("blend_fraction_bits", 0),
        ("requested_format_flags_raw", 0),
        ("animation_reference", 0x8300_000a),
        ("animation_directory_index", 10),
        ("frame", 0),
        ("rotations", 48),
        ("translations", 6),
        ("cached_frames", 1),
        ("decode_calls", 1),
        ("cache_hits", 0),
    ] {
        ensure!(
            number(value, field)? == expected,
            "raw initial clip/frame source input or count differs: {field}"
        );
    }
    let exact = number(value, "exact_branch_rotations")?;
    let approximate = number(value, "unverified_sqrt_rotations")?;
    ensure!(
        exact == 1 && approximate == 47 && exact.checked_add(approximate) == Some(48),
        "raw frame precision partition is inconsistent or conceals unverified host square roots"
    );
    for field in ["raw_frame_sha256", "camera_raw_sha256"] {
        digest(value, field)?;
    }
    ensure!(
        value["raw_frame_sha256"] == RAW_FRAME,
        "raw frame source/host fingerprint differs from independent frame-zero audit"
    );
    ensure!(
        value["camera_raw_exact_branch"] == true && value["camera_raw_sha256"] == CAMERA_RAW,
        "raw camera-bone fingerprint or source epsilon-branch identity differs"
    );
    for field in [
        "root_bias_applied",
        "skin_mapping_applied",
        "layer_blending_applied",
        "skeleton_committed",
        "native_numeric_parity_verified",
        "first_displayed_pose_verified",
        "camera_frame_ready",
    ] {
        ensure!(
            value[field] == false,
            "raw frame promoted unavailable mapping/pose/numeric/camera work: {field}"
        );
    }
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|scope| !scope.is_empty() && scope.len() <= 2048),
        "raw frame scope missing"
    );
    Ok(())
}

fn inventory(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "inventory_key",
            "records",
            "selected_record",
            "count",
            "limit",
            "flags",
            "ordinal_raw",
            "guns_resource",
            "guns_payload_bytes",
            "metadata_source_index",
            "metadata_primary_key",
            "weapon_registry_keys",
            "weapon_order_keys",
            "selected_order_index",
            "primary_request",
            "secondary_request",
            "static_data",
        ],
    )?;
    for (field, expected) in [
        ("inventory_key", 0x0200_0830),
        ("records", 1),
        ("selected_record", 0),
        ("count", 1),
        ("limit", 1),
        ("flags", 0),
        ("ordinal_raw", 0xff),
        ("guns_resource", 0x0109_000f),
        ("guns_payload_bytes", 7_765_056),
        ("metadata_source_index", 0),
        ("metadata_primary_key", 0x4b00_002c),
        ("weapon_registry_keys", 38),
        ("weapon_order_keys", 29),
        ("selected_order_index", 27),
        ("primary_request", 0x0900_071e),
        ("secondary_request", 0x0900_0b70),
    ] {
        ensure!(
            number(value, field)? == expected,
            "fresh default inventory or supplied remapping differs: {field}"
        );
    }
    static_data(&value["static_data"])
}

fn static_data(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "game_id",
            "revision",
            "main_dol_partition_offset",
            "dol_bytes",
            "dol_sha256",
            "data_section_index",
            "data_virtual_address",
            "data_bytes",
            "data_sha256",
        ],
    )?;
    digest(value, "dol_sha256")?;
    digest(value, "data_sha256")?;
    ensure!(
        value["game_id"] == "SJBE52"
            && value["revision"] == 0
            && value["dol_bytes"] == 6_087_648
            && value["dol_sha256"] == DOL
            && value["data_section_index"] == 5
            && value["data_virtual_address"] == 0x8056_bba0u32
            && value["data_bytes"] == 383_136
            && value["data_sha256"] == DATA,
        "supplied DOL/DATA identity differs from SJBE52 revision zero"
    );
    let offset = number(value, "main_dol_partition_offset")?;
    ensure!(
        offset >= 0x440
            && offset % 4 == 0
            && offset <= u64::from(u32::MAX) * 4
            && offset.checked_add(number(value, "dol_bytes")?).is_some(),
        "supplied main DOL offset exceeds its Wii boot-word address bounds"
    );
    Ok(())
}

fn digest(value: &Value, field: &str) -> Result<()> {
    ensure!(
        value[field].as_str().is_some_and(|hash| hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "initial layer digest is not lowercase SHA256: {field}"
    );
    Ok(())
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod tests;
