//! Checked metadata for retained mission files, separate from world activation.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, mission};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize)]
pub(super) struct EntityLocator {
    directory_index: usize,
    hash: u32,
    entity_type: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct MapEvidence {
    pub directory_index: usize,
    pub hash: u32,
    pub zones: usize,
    pub bsp_present: bool,
    pub bsp_nodes: usize,
    pub placements: usize,
    pub first_placement_reference: Option<u32>,
    pub first_placement_entity: Option<EntityLocator>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ResourceEvidence {
    pub resource_hash: u32,
    pub payload_bytes: u32,
    pub base_bytes: u32,
    pub requested_map_reference: u32,
    pub resolved_map_hash: Option<u32>,
    pub next_link: Option<mission::maps::LinkedMap>,
    pub maps: Vec<MapEvidence>,
    pub entity_descriptors: usize,
    pub script_descriptors: usize,
    pub textures: usize,
}

/// Only static data is addressed. The residency adapter performs linked lookup
/// after acknowledging the actual retained resource and fills next_link then.
pub(super) fn read(edb: &Edb, map_reference: u32) -> Result<ResourceEvidence> {
    ensure!(
        map_reference == 0x8500_0000,
        "unsupported mission map reference"
    );
    let maps = map_metadata(edb)?;
    Ok(ResourceEvidence {
        resource_hash: edb.header.hash,
        payload_bytes: edb.header.payload_bytes,
        base_bytes: edb.header.base_bytes,
        requested_map_reference: map_reference,
        resolved_map_hash: maps.first().map(|map| map.hash),
        next_link: None,
        maps,
        entity_descriptors: edb.entities()?.len(),
        script_descriptors: usize::from(
            edb.tables
                .get(5)
                .context("mission script directory absent")?
                .count,
        ),
        textures: edb.textures.len(),
    })
}

pub(super) fn map_metadata(edb: &Edb) -> Result<Vec<MapEvidence>> {
    let hashes = edb.map_hashes()?;
    ensure!(hashes.len() <= 128, "mission map metadata budget exhausted");
    let mut seen = BTreeSet::new();
    let mut maps = Vec::with_capacity(hashes.len());
    for (directory_index, hash) in hashes.into_iter().enumerate() {
        ensure!(seen.insert(hash), "mission map hash is ambiguous");
        let spatial = edb.map_spatial(hash)?;
        let placements = edb.map_placements(hash)?;
        let reference = placements.first().map(|placement| placement.object_hash);
        let entity = reference
            .map(|reference| edb.entity_reference(reference))
            .transpose()?
            .flatten();
        maps.push(MapEvidence {
            directory_index,
            hash,
            zones: spatial.zones.len(),
            bsp_present: spatial.bsp_present,
            bsp_nodes: spatial.nodes.len(),
            placements: placements.len(),
            first_placement_reference: reference,
            first_placement_entity: entity.map(|entity| EntityLocator {
                directory_index: entity.directory_index,
                hash: entity.entity.hash,
                entity_type: entity.entity.entity_type,
            }),
        });
    }
    Ok(maps)
}
