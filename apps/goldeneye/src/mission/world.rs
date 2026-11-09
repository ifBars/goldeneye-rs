//! Owned map spatial/placement data bound through the fresh source registry.
//! Base animator execution and trigger/player factories are separate stages.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, mission::world::MapDescriptor, scene::Placement, spatial::MapSpatial};
use ge_core::mission::{
    registration::RegistrationPlan,
    world::{Budget, InstanceId, Key, MapReference, Registry, WrapperId},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) struct RuntimePlacement {
    pub source: Placement,
    pub entity_directory_index: usize,
    pub entity_type: u32,
}

pub(crate) struct MapDataInstance {
    pub source_ordinal: usize,
    pub descriptor: MapDescriptor,
    pub spatial: MapSpatial,
    pub placements: Vec<RuntimePlacement>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct InstanceEvidence {
    source_ordinal: usize,
    loaded_ordinal: usize,
    wrapper_id: u64,
    instance_id: u64,
    preregistration_index: u8,
    file_hash: u32,
    requested_map_reference: u32,
    canonical_map_reference: u32,
    stored_map_hash: u32,
    map_directory_index: usize,
    zones: usize,
    bsp_present: bool,
    bsp_nodes: usize,
    placements_bound: usize,
    navigation_placements: usize,
    declared_bounds_sha256: String,
    placement_matrices_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct Evidence {
    bound: bool,
    preregistration_slots: usize,
    loaded_instances: usize,
    base_animators_initialized: bool,
    trigger_factories_run: bool,
    navigation_initialized: bool,
    instances: Vec<InstanceEvidence>,
    scope: &'static str,
}

pub(super) struct WorldBindings {
    registry: Registry,
    instances: Vec<MapDataInstance>,
    evidence: Evidence,
}

pub(crate) struct TriggerMap<'a> {
    pub source: &'a Edb,
    pub data: &'a MapDataInstance,
    pub loaded: &'a ge_core::mission::world::Loaded,
    pub flags: [u8; 3],
    pub filter: Option<&'a [u32]>,
}

impl WorldBindings {
    pub fn bind(resources: &[Edb], plan: &RegistrationPlan) -> Result<Self> {
        ensure!(
            !resources.is_empty() && resources.len() <= 128 && plan.rows().len() == resources.len(),
            "map binding resource/plan count differs"
        );
        let mut registry = Registry::fresh(Budget {
            preregistered_slots: 129,
            loaded_instances: 128,
        })?;
        let mut instances = Vec::with_capacity(resources.len());
        let mut rows = Vec::with_capacity(resources.len());
        let mut used = BTreeSet::new();
        for row in plan.rows() {
            let source_ordinal = row.source_index();
            ensure!(
                used.insert(source_ordinal),
                "map binding source owner repeated"
            );
            let edb = resources
                .get(source_ordinal)
                .context("map binding source owner absent")?;
            let request = row.request();
            ensure!(
                request.file == edb.header.hash,
                "map binding resource identity differs"
            );
            let descriptor = edb
                .mission_map_descriptor(
                    request.map_reference,
                    ge_assets::mission::world::Budget { maps: 128 },
                )?
                .context("map binding reference is absent")?;
            let map_reference =
                MapReference::from_directory_index(u32::try_from(descriptor.directory_index)?)?;
            ensure!(
                map_reference.raw() == descriptor.canonical_reference,
                "map normalization differs from registry key"
            );
            let wrapper = WrapperId::new(source_ordinal as u64 + 1)?;
            let key = Key {
                wrapper,
                map_reference,
            };
            let spatial = edb.map_spatial(descriptor.stored_hash)?;
            let source_placements = edb.map_placements(descriptor.stored_hash)?;
            let entities = edb.entities()?;
            let mut placements = Vec::with_capacity(source_placements.len());
            for source in source_placements {
                let index = edb
                    .entity_reference_index(source.object_hash)?
                    .context("map placement entity reference is absent")?;
                let entity = entities
                    .get(index)
                    .context("map placement entity directory index is absent")?;
                placements.push(RuntimePlacement {
                    source,
                    entity_directory_index: index,
                    entity_type: entity.entity_type,
                });
            }
            // Construction binds the map and registers its canonical key before
            // publication. Slot zero remains the constructor's null entry.
            registry.preregister(key)?;
            let instance = InstanceId::new(instances.len() as u64 + 1)?;
            let published = registry.publish_constructed(key, instance)?;
            ensure!(
                !published.reused && published.loaded_ordinal == instances.len(),
                "fresh map instance unexpectedly reused"
            );
            instances.push(MapDataInstance {
                source_ordinal,
                descriptor,
                spatial,
                placements,
            });
            let owned = instances
                .last()
                .context("constructed map data instance absent")?;
            let descriptor = owned.descriptor;
            let spatial = &owned.spatial;
            let placements = &owned.placements;
            let source_ordinal = owned.source_ordinal;
            ensure!(
                placements
                    .iter()
                    .all(|p| p.entity_directory_index < entities.len()),
                "bound placement directory index is absent"
            );
            let mut bounds_hash = Sha256::new();
            for side in descriptor.declared_bounds_bits {
                for bits in side {
                    bounds_hash.update(bits.to_be_bytes());
                }
            }
            let mut matrices_hash = Sha256::new();
            for placement in placements {
                for value in placement.source.model {
                    matrices_hash.update(value.to_bits().to_be_bytes());
                }
            }
            rows.push(InstanceEvidence {
                source_ordinal,
                loaded_ordinal: published.loaded_ordinal,
                wrapper_id: wrapper.get(),
                instance_id: instance.get(),
                preregistration_index: published.slot,
                file_hash: edb.header.hash,
                requested_map_reference: request.map_reference,
                canonical_map_reference: descriptor.canonical_reference,
                stored_map_hash: descriptor.stored_hash,
                map_directory_index: descriptor.directory_index,
                zones: spatial.zones.len(),
                bsp_present: spatial.bsp_present,
                bsp_nodes: spatial.nodes.len(),
                placements_bound: placements.len(),
                navigation_placements: placements
                    .iter()
                    .filter(|placement| placement.entity_type == 0x607)
                    .count(),
                declared_bounds_sha256: format!("{:x}", bounds_hash.finalize()),
                placement_matrices_sha256: format!("{:x}", matrices_hash.finalize()),
            });
        }
        let evidence = Evidence {
            bound: true,
            preregistration_slots: registry.slots().len(),
            loaded_instances: instances.len(),
            base_animators_initialized: false,
            trigger_factories_run: false,
            navigation_initialized: false,
            instances: rows,
            scope: "Owned source map spatial/BSP data and placement matrices with resolved entity descriptors, bound through a fresh semantic wrapper/instance registry. Source preregistration indices include null slot zero; loaded ordinals do not. Stored map bounds are retained, never replaced with fitted mesh bounds. Base animator execution, navigation registration, trigger factories and player/camera initialization remain incomplete; these data bindings are not a ready gameplay world.",
        };
        Ok(Self {
            registry,
            instances,
            evidence,
        })
    }
    pub fn evidence(&self) -> &Evidence {
        debug_assert_eq!(self.registry.loaded().len(), self.instances.len());
        &self.evidence
    }
    pub fn instances(&self) -> &[MapDataInstance] {
        &self.instances
    }
    pub fn trigger_maps<'a>(
        &'a self,
        resources: &'a [Edb],
        plan: &'a RegistrationPlan,
    ) -> Result<Vec<TriggerMap<'a>>> {
        ensure!(
            self.instances.len() == self.registry.loaded().len()
                && plan.rows().len() == self.instances.len(),
            "trigger bridge owner counts differ"
        );
        self.instances
            .iter()
            .zip(self.registry.loaded())
            .zip(plan.rows())
            .enumerate()
            .map(|(index, ((data, loaded), row))| {
                let source = resources
                    .get(data.source_ordinal)
                    .context("trigger source owner absent")?;
                ensure!(
                    row.source_index() == data.source_ordinal
                        && row.request().file == source.header.hash
                        && loaded.key.map_reference.raw() == data.descriptor.canonical_reference,
                    "trigger bridge provenance differs"
                );
                Ok(TriggerMap {
                    source,
                    data,
                    loaded,
                    flags: row.flags_raw(),
                    filter: plan.row_filter(index)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "tests/world.rs"]
mod tests;
