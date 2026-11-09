//! Verify source selection/projection fingerprints without exporting poses.
use super::{TUTORIAL, closed, number};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(super) fn validate(
    value: &Value,
    level: u32,
    requested: u32,
    registration: &Value,
    residency: &Value,
) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "requested_id_before",
            "requested_id_after",
            "registration_rows",
            "eligible_files",
            "filtered_files",
            "prepared_entries",
            "visits",
            "selection_found",
            "used_fallback",
            "selected_catalog_index",
            "selected",
            "pose_sha256",
            "pose_sha256_encoding",
            "rotation_mode",
            "catalog_membership_verified",
            "catalog_created",
            "creation_inputs_verified",
            "camera_frame_ready",
            "scope",
        ],
    )?;
    let rows = registration["rows"]
        .as_array()
        .context("spawn registration rows absent")?;
    let resources = residency["resources"]
        .as_array()
        .context("spawn resource owners absent")?;
    let entries = number(value, "prepared_entries")?;
    ensure!(
        value["requested_id_before"] == requested
            && number(value, "requested_id_after")? <= u64::from(u32::MAX)
            && number(value, "registration_rows")? == rows.len() as u64
            && number(value, "eligible_files")? + number(value, "filtered_files")?
                == rows.len() as u64
            && entries <= 4096
            && number(value, "visits")? <= entries * 2
            && value["catalog_membership_verified"] == false
            && value["catalog_created"] == false
            && value["creation_inputs_verified"] == false
            && value["camera_frame_ready"] == false
            && value["pose_sha256_encoding"]
                == "position_u32be[4] || rotation_u32be[4] || rotation_mode_u8",
        "spawn preparation invented runtime readiness or invalid selection/budget data"
    );
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "spawn scope missing"
    );
    if value["selection_found"] == false {
        ensure!(
            value["state"] == "missing_startpoint"
                && value["used_fallback"] == false
                && value["requested_id_after"] == requested
                && value["selected_catalog_index"].is_null()
                && value["selected"].is_null()
                && value["pose_sha256"].is_null()
                && value["rotation_mode"].is_null(),
            "missing StartPoint fabricated a usable pose"
        );
        ensure!(
            level != TUTORIAL,
            "required Tutorial StartPoint selection missing"
        );
        return Ok(());
    }
    ensure!(
        value["selection_found"] == true
            && value["state"] == "prepared_startpoint"
            && value["rotation_mode"] == 3
            && number(value, "selected_catalog_index")? < entries
            && number(value, "visits")? > 0
            && value["pose_sha256"]
                .as_str()
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit())),
        "spawn pose/provenance missing"
    );
    let after = number(value, "requested_id_after")?;
    ensure!(
        value["used_fallback"].is_boolean()
            && if value["used_fallback"] == true {
                after == 0 && requested != 0
            } else {
                after == u64::from(requested)
            },
        "spawn fallback did not preserve the source request-ID effect"
    );
    let selected = &value["selected"];
    closed(
        selected,
        &[
            "file_hash",
            "source_ordinal",
            "prepared_row_index",
            "locator",
        ],
    )?;
    let ordinal = number(selected, "source_ordinal")? as usize;
    let row_index = number(selected, "prepared_row_index")? as usize;
    let source = resources
        .get(ordinal)
        .context("selected StartPoint owner missing")?;
    let row = rows
        .get(row_index)
        .context("selected StartPoint row missing")?;
    ensure!(
        selected["file_hash"] == source["file_hash"]
            && row["file_hash"] == selected["file_hash"]
            && number(row, "source_ordinal")? == ordinal as u64,
        "selected StartPoint no longer belongs to its admitted row"
    );
    if let Some(allowlist) = row["subtype_allowlist"].as_array() {
        ensure!(
            allowlist.iter().any(|v| *v == 0x1f00_0081u32),
            "StartPoint selected from an excluded subtype list"
        );
    }
    let loc = &selected["locator"];
    closed(
        loc,
        &[
            "map_reference",
            "map_directory_index",
            "map_hash",
            "map_address",
            "trigger_block",
            "trigger_ordinal",
            "record_address",
            "schema_index",
            "schema_address",
            "native_type_index",
        ],
    )?;
    ensure!(
        loc["map_reference"] == 0x8500_0000u32
            && loc["map_directory_index"] == 0
            && loc["map_hash"] == source["resolved_map_hash"]
            && loc["native_type_index"] == 28
            && number(loc, "trigger_ordinal")? < 65536
            && number(loc, "schema_index")? < 65536,
        "StartPoint source class/map binding differs"
    );
    for field in [
        "map_address",
        "trigger_block",
        "record_address",
        "schema_address",
    ] {
        ensure!(
            (0xc8..number(source, "payload_bytes")?).contains(&number(loc, field)?),
            "StartPoint address outside retained owner"
        );
    }
    if level == TUTORIAL {
        ensure!(
            selected["file_hash"] == TUTORIAL && ordinal == 0 && row_index == 0,
            "Tutorial StartPoint came from the wrong map context"
        );
        if requested == 1 {
            // Opaque fingerprint from an independent source trace and private
            // asset read. No original pose words or coordinates are embedded.
            ensure!(
                loc["trigger_ordinal"] == 0
                    && value["used_fallback"] == false
                    && value["pose_sha256"]
                        == "44e09bcdcf1a9e77a8a1b353e5692bc94b55fdacb68b1d218433a985b13d73ff",
                "Tutorial initial pose differs from the actual source producer"
            );
        } else if requested == 99 || requested == 0 {
            ensure!(
                loc["trigger_ordinal"] == 82
                    && after == 0
                    && value["used_fallback"] == (requested != 0),
                "Tutorial ID-zero selection/fallback differs"
            );
        }
    }
    Ok(())
}
