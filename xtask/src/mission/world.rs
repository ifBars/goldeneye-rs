//! Validate owned map data and separate source indices from loaded ordinals.
use super::{closed, number};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(super) fn validate(value: &Value, residency: &Value, registration: &Value) -> Result<()> {
    closed(
        value,
        &[
            "bound",
            "preregistration_slots",
            "loaded_instances",
            "base_animators_initialized",
            "trigger_factories_run",
            "navigation_initialized",
            "instances",
            "scope",
        ],
    )?;
    let instances = value["instances"]
        .as_array()
        .context("bound map instances missing")?;
    let resources = residency["resources"]
        .as_array()
        .context("resident resources missing")?;
    let rows = registration["rows"]
        .as_array()
        .context("prepared map rows missing")?;
    ensure!(
        value["bound"] == true
            && value["base_animators_initialized"] == false
            && value["trigger_factories_run"] == false
            && value["navigation_initialized"] == false
            && instances.len() == rows.len()
            && instances.len() == resources.len()
            && number(value, "loaded_instances")? == instances.len() as u64
            && number(value, "preregistration_slots")? == instances.len() as u64 + 1,
        "map data binding promoted incomplete runtime work or lost null slot/owners"
    );
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "map binding scope missing"
    );
    for (index, (instance, row)) in instances.iter().zip(rows).enumerate() {
        closed(
            instance,
            &[
                "source_ordinal",
                "loaded_ordinal",
                "wrapper_id",
                "instance_id",
                "preregistration_index",
                "file_hash",
                "requested_map_reference",
                "canonical_map_reference",
                "stored_map_hash",
                "map_directory_index",
                "zones",
                "bsp_present",
                "bsp_nodes",
                "placements_bound",
                "navigation_placements",
                "declared_bounds_sha256",
                "placement_matrices_sha256",
            ],
        )?;
        let ordinal = number(row, "source_ordinal")? as usize;
        let source = resources
            .get(ordinal)
            .context("bound map source owner missing")?;
        ensure!(
            number(instance, "source_ordinal")? == ordinal as u64
                && number(instance, "loaded_ordinal")? == index as u64
                && number(instance, "preregistration_index")? == index as u64 + 1
                && number(instance, "instance_id")? == index as u64 + 1
                && number(instance, "wrapper_id")? == ordinal as u64 + 1
                && instance["file_hash"] == source["file_hash"]
                && instance["requested_map_reference"] == source["requested_map_reference"]
                && instance["canonical_map_reference"] == 0x8500_0000u32
                && instance["stored_map_hash"] == source["resolved_map_hash"]
                && instance["map_directory_index"] == 0,
            "map binding identity, canonical reference or source index differs"
        );
        for field in ["zones", "bsp_present", "bsp_nodes"] {
            ensure!(
                instance[field] == source[field],
                "map binding spatial metadata differs for {field}"
            );
        }
        ensure!(
            instance["placements_bound"] == source["placements"]
                && number(instance, "navigation_placements")?
                    <= number(instance, "placements_bound")?,
            "map placements were lost or incorrectly classified"
        );
        for field in ["declared_bounds_sha256", "placement_matrices_sha256"] {
            ensure!(
                instance[field]
                    .as_str()
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit())),
                "map data hash missing {field}"
            );
        }
    }
    Ok(())
}
