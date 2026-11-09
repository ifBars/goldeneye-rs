//! Validate actual retained ownership and source file-discovery invariants.
use super::{RESOURCE_LIMIT, TUTORIAL, closed, number, validate_link};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn validate(value: &Value, root: &Value, level: u32) -> Result<()> {
    closed(
        value,
        &[
            "complete",
            "stop_reason",
            "repeated_file",
            "retained_files",
            "total_payload_bytes",
            "file_limit",
            "per_file_byte_limit",
            "total_byte_limit",
            "resources",
            "scope",
        ],
    )?;
    let resources = value["resources"]
        .as_array()
        .context("resident resource list missing")?;
    let file_limit = number(value, "file_limit")?;
    let per_file = number(value, "per_file_byte_limit")?;
    let total_limit = number(value, "total_byte_limit")?;
    ensure!(
        value["complete"] == true
            && (1..=128).contains(&file_limit)
            && (0xc8..=RESOURCE_LIMIT).contains(&per_file)
            && (0xc8..=256 * 1024 * 1024).contains(&total_limit)
            && !resources.is_empty()
            && resources.len() as u64 <= file_limit
            && number(value, "retained_files")? == resources.len() as u64,
        "mission residency is incomplete or exceeds declared limits"
    );
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "residency scope missing"
    );
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for (index, resource) in resources.iter().enumerate() {
        closed(
            resource,
            &[
                "ordinal",
                "file_hash",
                "requested_map_reference",
                "resolved_map_hash",
                "map_directory_index",
                "payload_bytes",
                "base_bytes",
                "zones",
                "bsp_present",
                "bsp_nodes",
                "placements",
                "entity_descriptors",
                "script_descriptors",
                "textures",
                "next_link",
            ],
        )?;
        let file = number(resource, "file_hash")?;
        let map = number(resource, "resolved_map_hash")?;
        let payload = number(resource, "payload_bytes")?;
        ensure!(
            file <= u64::from(u32::MAX)
                && seen.insert(file)
                && map <= u64::from(u32::MAX)
                && number(resource, "ordinal")? == index as u64
                && resource["requested_map_reference"] == 0x8500_0000u32
                && resource["map_directory_index"] == 0
                && (0xc8..=per_file).contains(&payload)
                && (0xc8..=payload).contains(&number(resource, "base_bytes")?),
            "resident resource identity, coded map, order or size is invalid"
        );
        total = total
            .checked_add(payload)
            .context("resident byte sum overflow")?;
        ensure!(
            total <= total_limit,
            "resident payload exceeds aggregate budget"
        );
        for (field, limit) in [
            ("zones", 65_536),
            ("bsp_nodes", 1_000_000),
            ("placements", 1_000_000),
            ("entity_descriptors", 32_767),
            ("script_descriptors", 32_767),
            ("textures", 32_767),
        ] {
            ensure!(
                number(resource, field)? <= limit,
                "resident {field} exceeds metadata limits"
            );
        }
        ensure!(
            resource["bsp_present"].is_boolean()
                && (resource["bsp_present"] != false || resource["bsp_nodes"] == 0),
            "resident BSP diagnostics differ"
        );
        validate_link(&resource["next_link"], payload, map as u32, None)?;
        if index == 0 {
            ensure!(
                file == u64::from(level)
                    && resource["payload_bytes"] == root["payload_bytes"]
                    && resource["base_bytes"] == root["base_bytes"]
                    && resource["resolved_map_hash"] == root["resolved_map_hash"]
                    && resource["next_link"] == root["next_link"],
                "root resource was not retained consistently"
            );
        } else {
            let link = &resources[index - 1]["next_link"];
            ensure!(
                !link.is_null() && link["file"] == resource["file_hash"],
                "residency did not follow the previous file link"
            );
        }
    }
    ensure!(
        number(value, "total_payload_bytes")? == total,
        "resident payload total differs from actual owners"
    );
    let last = resources.last().context("resident list empty")?;
    match value["stop_reason"].as_str() {
        Some("no_link") => ensure!(
            last["next_link"].is_null() && value["repeated_file"].is_null(),
            "no-link stop is fabricated"
        ),
        Some("repeated_file") => {
            let file = number(&last["next_link"], "file")?;
            ensure!(
                seen.contains(&file) && value["repeated_file"] == file,
                "repeated-file stop did not identify an existing owner"
            );
        }
        _ => anyhow::bail!("unsupported residency completion reason"),
    }
    if level == TUTORIAL {
        ensure!(
            resources.len() == 2
                && resources[1]["file_hash"] == 0x018b_002au32
                && resources[1]["resolved_map_hash"] == 0x0500_0070u32
                && value["stop_reason"] == "no_link",
            "tutorial residency differs from its source two-file chain"
        );
    }
    Ok(())
}
