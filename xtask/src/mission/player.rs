//! Verify the real FP skin dependency and constructor without promoting a pose.
use super::{closed, number};
use anyhow::{Result, ensure};
use serde_json::Value;
mod animation;
mod camera;
mod runtime;

pub(super) fn validate_initial_layers(value: &Value) -> Result<()> {
    runtime::validate(value)
}

pub(super) fn validate_mode_probe(value: &Value, inputs: [u32; 3]) -> Result<()> {
    animation::validate(value)?;
    for (field, expected) in ["requested_mode", "previous_mode", "fallback_mode"]
        .into_iter()
        .zip(inputs)
    {
        ensure!(
            value[field] == expected,
            "mode probe caller input changed: {field}"
        );
    }
    let expected = match inputs {
        [0x0900_071e, 0, 0x0900_065e] => {
            "a86afa571d8aec797d77d50a86ec14c5f45d1d1ff7efdd57decc02750b7b4299"
        }
        [0x0900_0b70, 0, 0x0900_000b] => {
            "c0c9b39936256d8a69cffbd924a073cbcd4219433cbc31bc4841fa43587c6ee5"
        }
        [0x0900_071e, 0x09ff_fffe, 0] => {
            "7ba421a26ab7768d23c0a7056de18154e4e024fee247254d28876e75df15fbe3"
        }
        [0x09ff_ffff, 0, 0] => "aa76b45bc259f86fcee45e55d2cf57b72c185f65381f5bc2f3f0a4be6ab087f1",
        _ => anyhow::bail!("unknown authored mission mode probe"),
    };
    ensure!(
        value["plan_sha256"] == expected,
        "mode probe differs from independent private source command/derived-plan fingerprint"
    );
    Ok(())
}

pub(super) fn validate(value: &Value, selection: &Value) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "requested_character",
            "catalog_resource",
            "catalog_payload_bytes",
            "character_sheet",
            "character_group",
            "character_row_stride",
            "character_rows",
            "duplicate_character_rows",
            "selected_character",
            "selected_source_index",
            "selected_source_address",
            "native_default_fallback_used",
            "component_resource",
            "component_payload_bytes",
            "component_reference",
            "skin_resource",
            "skin_payload_bytes",
            "skin_reference",
            "skin_directory_index",
            "skin_hash",
            "skin_body_address",
            "retained_resources",
            "retained_payload_bytes",
            "bone_count",
            "root_count",
            "topology_sha256",
            "constructor_state_sha256",
            "camera_tag",
            "camera_tag_found",
            "camera_bone",
            "camera_parent_chain_length",
            "camera_parent_chain_sha256",
            "skeleton_constructor_initialized",
            "animation_sampled",
            "player_physics_initialized",
            "camera_frame_ready",
            "mode_plan",
            "initial_layers",
            "camera_bone_transform",
            "scope",
        ],
    )?;
    ensure!(
        value["state"] == "fp_skeleton_constructor_initialized"
            && value["requested_character"] == selection["character_request"]
            && value["selected_character"] == value["requested_character"]
            && value["requested_character"] == 0x4d00_01b9u32
            && value["catalog_resource"] == 0x0109_0013u32
            && value["character_sheet"] == 0x1400_0021u32
            && value["character_group"] == 1
            && value["character_row_stride"] == 24
            && value["native_default_fallback_used"] == false
            && value["component_resource"] == 0x0102_0016u32
            && value["component_reference"] == 0
            && value["skin_resource"] == 0x010a_0050u32
            && value["skin_reference"] == 0x0d00_016fu32
            && value["skin_hash"] == 0x0d00_016fu32
            && value["skin_directory_index"] == 0,
        "FP skeleton binding no longer follows the selected source character/skin"
    );
    let catalog_bytes = number(value, "catalog_payload_bytes")?;
    let component_bytes = number(value, "component_payload_bytes")?;
    let skin_bytes = number(value, "skin_payload_bytes")?;
    ensure!(
        (0xc8..=1024 * 1024).contains(&catalog_bytes)
            && (0xc8..=8 * 1024 * 1024).contains(&component_bytes)
            && (0xc8..=16 * 1024 * 1024).contains(&skin_bytes)
            && value["retained_resources"] == 3
            && number(value, "retained_payload_bytes")?
                == catalog_bytes + component_bytes + skin_bytes
            && number(value, "character_rows")? > 0
            && number(value, "character_rows")? + number(value, "duplicate_character_rows")?
                <= 4096
            && number(value, "selected_source_index")?
                < number(value, "character_rows")? + number(value, "duplicate_character_rows")?
            && value["selected_source_index"] == 1
            && number(value, "selected_source_address")? >= 0xc8
            && number(value, "selected_source_address")? + 24 <= catalog_bytes
            && (0xc8..skin_bytes).contains(&number(value, "skin_body_address")?),
        "FP retained owners or source provenance exceed their bounds"
    );
    ensure!(
        value["bone_count"] == 48
            && value["root_count"] == 1
            && value["camera_tag"] == 0x0e00_0028u32
            && value["camera_tag_found"] == true
            && value["camera_bone"] == 47
            && value["camera_parent_chain_length"] == 3
            && value["skeleton_constructor_initialized"] == true
            && value["animation_sampled"] == !value["initial_layers"]["prewarm_pose"].is_null()
            && value["player_physics_initialized"] == false
            && value["camera_frame_ready"] == false,
        "FP constructor/tag binding changed or promoted incomplete player/camera work"
    );
    // Independent private source reads establish these opaque fingerprints.
    // No source topology, pose arrays or original transforms are exported.
    for (field, expected) in [
        (
            "topology_sha256",
            "a7d3e7cca6f71ca5cfa486550224158647942af521946626e25a37f506f21c0f",
        ),
        (
            "constructor_state_sha256",
            "2b2bdad0125d0bb6b2f8c289e44be757bf1c5a46c3292a4049316ffedc5de3bb",
        ),
        (
            "camera_parent_chain_sha256",
            "c5688be2fdb2cb91c673a6f9caab734be0fe55c9877c7ec353b98650f936a932",
        ),
    ] {
        ensure!(
            value[field] == expected,
            "FP source binding fingerprint differs: {field}"
        );
    }
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "FP constructor scope missing"
    );
    if !value["mode_plan"].is_null() {
        animation::validate(&value["mode_plan"])?;
    }
    if !value["initial_layers"].is_null() {
        validate_initial_layers(&value["initial_layers"])?;
    }
    if !value["camera_bone_transform"].is_null() {
        camera::validate(&value["camera_bone_transform"], value)?;
    }
    Ok(())
}
