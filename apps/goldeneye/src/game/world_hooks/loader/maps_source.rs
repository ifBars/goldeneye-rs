//! Checked per-operation adapter over retained mission data. Mutable hierarchies
//! live in Cache; no self-reference or borrowed EDB crosses an owner boundary.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    candidates::{Entries, Hierarchy, Node},
    edb::Edb,
    entity_spatial::SpatialHeader,
    mission::map_runtime,
    scene::Placement,
};
use ge_core::{
    collision::{
        bounds::{Bounds, EntityBounds},
        refresh::RefreshHierarchy,
        world::StoredBounds,
    },
    mission::{
        maps::{publication, sections},
        world::{Key, WrapperId},
    },
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, btree_map::Entry};

struct Zone {
    section_index: usize,
    root_reference: u32,
    placements: Option<Vec<usize>>,
    root: Option<publication::Root>,
    tree: Option<usize>,
}

// Native runtime +80/+84 are separately owned current/original references.
// Engine/map-on/light/group and the raw flags remain in the retained source.
struct BoundPlacement {
    source: Placement,
    current_entity_reference: u32,
    original_entity_reference: u32,
    wrapper: WrapperId,
    key: u32,
    ordinal: usize,
}

pub(super) struct Cache {
    key: Key,
    resource_hash: u32,
    map_hash: u32,
    map_body: usize,
    zones: Vec<Zone>,
    placements: Vec<BoundPlacement>,
    // A single map-wide node owner preserves shared-node writes across zones.
    nodes: BTreeMap<usize, Node>,
    base_bounds: BTreeMap<u16, Option<StoredBounds>>,
}

impl Cache {
    pub fn new(edb: &Edb, stored_map_hash: u32, key: Key) -> Result<Self> {
        let descriptor = edb
            .mission_map_descriptor(
                key.map_reference.raw(),
                ge_assets::mission::world::Budget { maps: 128 },
            )?
            .context("publication map reference is absent")?;
        ensure!(
            descriptor.stored_hash == stored_map_hash
                && descriptor.canonical_reference == key.map_reference.raw(),
            "publication map owner/reference differs"
        );
        ensure!(
            edb.map_hashes()?
                .iter()
                .filter(|&&hash| hash == stored_map_hash)
                .count()
                == 1,
            "publication legacy map readers require an unambiguous stored map hash"
        );
        let selected = edb
            .mission_map_descriptor(
                stored_map_hash,
                ge_assets::mission::world::Budget { maps: 128 },
            )?
            .context("publication stored map hash is absent")?;
        ensure!(
            selected.body_address == descriptor.body_address,
            "publication stored and canonical map selectors differ"
        );
        let view = edb
            .mission_map_runtime(key.map_reference.raw(), map_runtime::Budget::default())?
            .context("publication runtime map is absent")?;
        ensure!(
            view.zone_count() <= 65536,
            "publication zone cache budget exceeded"
        );
        let mut zones = Vec::with_capacity(view.zone_count());
        for zone in 0..view.zone_count() {
            let input = view.zone_publication(zone)?;
            zones.push(Zone {
                section_index: input.section_index,
                root_reference: input.root_reference,
                placements: None,
                root: None,
                tree: None,
            });
        }
        // This reads serialized base-block metadata/TRS only. Entity and zone
        // geometry remain lazy until their actual logical publication gate.
        let placements = edb.map_placements(stored_map_hash)?;
        ensure!(
            placements.len() <= 1_000_000,
            "publication placement cache budget exceeded"
        );
        let placements = placements
            .into_iter()
            .enumerate()
            .map(|(ordinal, source)| BoundPlacement {
                current_entity_reference: source.object_hash,
                original_entity_reference: source.object_hash,
                wrapper: key.wrapper,
                key: source.hash,
                ordinal,
                source,
            })
            .collect();
        Ok(Self {
            key,
            resource_hash: edb.header.hash,
            map_hash: stored_map_hash,
            map_body: descriptor.body_address,
            zones,
            placements,
            nodes: BTreeMap::new(),
            base_bounds: BTreeMap::new(),
        })
    }

    pub fn key(&self) -> Key {
        self.key
    }
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }

    /// Canonical private state hashing, never an original-data serialization.
    pub fn fingerprint(&self, digest: &mut Sha256) {
        fn count(d: &mut Sha256, v: usize) {
            d.update((v as u64).to_be_bytes());
        }
        fn bounds(d: &mut Sha256, v: StoredBounds) {
            for x in v.minimum.into_iter().chain(v.maximum) {
                d.update(x.to_bits().to_be_bytes());
            }
        }
        digest.update(b"ge_map_publication_source_cache_v1\0");
        digest.update(self.key.wrapper.get().to_be_bytes());
        digest.update(self.key.map_reference.raw().to_be_bytes());
        digest.update(self.resource_hash.to_be_bytes());
        digest.update(self.map_hash.to_be_bytes());
        count(digest, self.map_body);
        count(digest, self.zones.len());
        for zone in &self.zones {
            count(digest, zone.section_index);
            digest.update(zone.root_reference.to_be_bytes());
            digest.update([u8::from(zone.placements.is_some())]);
            if let Some(indices) = &zone.placements {
                count(digest, indices.len());
                for &index in indices {
                    count(digest, index);
                }
            }
            digest.update([u8::from(zone.root.is_some())]);
            if let Some(root) = zone.root {
                digest.update([root.binding_count, root.dynamic]);
            }
            digest.update([u8::from(zone.tree.is_some())]);
            if let Some(root) = zone.tree {
                count(digest, root);
            }
        }
        count(digest, self.placements.len());
        for placement in &self.placements {
            digest.update(placement.current_entity_reference.to_be_bytes());
            digest.update(placement.original_entity_reference.to_be_bytes());
            digest.update(placement.wrapper.get().to_be_bytes());
            digest.update(placement.key.to_be_bytes());
            count(digest, placement.ordinal);
            let source = &placement.source;
            digest.update(source.hash.to_be_bytes());
            digest.update(source.object_hash.to_be_bytes());
            digest.update(source.flags.to_be_bytes());
            digest.update(source.engine_flags.to_be_bytes());
            digest.update(source.map_on.to_be_bytes());
            digest.update(source.light_set.to_be_bytes());
            digest.update(source.group.to_be_bytes());
            digest.update(source.unknown_word.to_be_bytes());
            for value in source
                .position
                .into_iter()
                .chain(source.rotation)
                .chain(source.scale)
                .chain(source.model)
            {
                digest.update(value.to_bits().to_be_bytes());
            }
        }
        count(digest, self.nodes.len());
        for (&address, node) in &self.nodes {
            count(digest, address);
            digest.update([node.flags_raw]);
            match &node.entries {
                Entries::Leaves(leaves) => {
                    digest.update([0]);
                    count(digest, leaves.len());
                    for leaf in leaves {
                        digest.update(leaf.placement.to_be_bytes());
                        digest.update([leaf.flags_raw, leaf.unknown_byte]);
                    }
                }
                Entries::Branches(branches) => {
                    digest.update([1]);
                    count(digest, branches.len());
                    for branch in branches {
                        count(digest, branch.target);
                        bounds(
                            digest,
                            StoredBounds {
                                minimum: branch.bounds[0],
                                maximum: branch.bounds[1],
                            },
                        );
                    }
                }
            }
        }
        count(digest, self.base_bounds.len());
        for (&placement, value) in &self.base_bounds {
            digest.update(placement.to_be_bytes());
            digest.update([u8::from(value.is_some())]);
            if let Some(value) = value {
                bounds(digest, *value);
            }
        }
    }
}

pub(super) struct Source<'a> {
    edb: &'a Edb,
    cache: &'a mut Cache,
    scope: publication::FreshScope,
    sections: &'a sections::Owner,
}

impl<'a> Source<'a> {
    pub fn new(
        edb: &'a Edb,
        cache: &'a mut Cache,
        scope: publication::FreshScope,
        sections: &'a sections::Owner,
    ) -> Result<Self> {
        ensure!(
            cache.key.wrapper == sections.wrapper() && cache.resource_hash == edb.header.hash,
            "publication source owner differs"
        );
        let section_count = usize::from(
            edb.tables
                .first()
                .context("publication section directory absent")?
                .count,
        );
        ensure!(
            sections.section_count() == section_count,
            "logical publication section directory differs"
        );
        let descriptor = edb
            .mission_map_descriptor(
                cache.key.map_reference.raw(),
                ge_assets::mission::world::Budget { maps: 128 },
            )?
            .context("retained publication map disappeared")?;
        ensure!(
            descriptor.stored_hash == cache.map_hash && descriptor.body_address == cache.map_body,
            "retained publication map binding changed"
        );
        Ok(Self {
            edb,
            cache,
            scope,
            sections,
        })
    }
    fn view(&self) -> Result<map_runtime::View<'_>> {
        self.edb
            .mission_map_runtime(
                self.cache.key.map_reference.raw(),
                map_runtime::Budget::default(),
            )?
            .context("publication runtime map is absent")
    }
    fn zone(&self, zone: usize) -> Result<&Zone> {
        self.cache
            .zones
            .get(zone)
            .context("publication zone outside retained map")
    }
    fn published_zone(&self, zone: usize) -> Result<()> {
        ensure!(
            self.sections.status(self.zone(zone)?.section_index)?
                == sections::Status::BasePublished,
            "publication attempted to read unrequested zone geometry"
        );
        Ok(())
    }
    fn links(&mut self, zone: usize) -> Result<&[usize]> {
        self.published_zone(zone)?;
        if self.zone(zone)?.placements.is_none() {
            let values = self.view()?.zone_placement_indices(zone)?;
            ensure!(
                values.iter().all(|&i| i < self.cache.placements.len()),
                "publication zone link outside placements"
            );
            self.cache.zones[zone].placements = Some(values);
        }
        self.cache.zones[zone]
            .placements
            .as_deref()
            .context("publication links unavailable")
    }
    fn placement(&self, placement: usize) -> Result<&Placement> {
        Ok(&self.bound_placement(placement)?.source)
    }
    fn bound_placement(&self, placement: usize) -> Result<&BoundPlacement> {
        self.cache
            .placements
            .get(placement)
            .context("publication placement outside retained map")
    }
    fn entity_index(&self, placement: usize, original: bool) -> Result<Option<usize>> {
        let bound = self.bound_placement(placement)?;
        ensure!(
            bound.wrapper == self.cache.key.wrapper && bound.ordinal == placement,
            "publication placement owner differs"
        );
        let reference = if original {
            bound.original_entity_reference
        } else {
            bound.current_entity_reference
        };
        let Some(index) = self.edb.entity_reference_index(reference)? else {
            return Ok(None);
        };
        // entity_reference() enumerates all body type tags, so it cannot be
        // used here before logical publication. Check only this descriptor.
        let section = self.edb.entity_section_at(index)?;
        ensure!(
            self.sections.status(section)? == sections::Status::BasePublished,
            "placement geometry belongs to an unrequested section"
        );
        Ok(Some(index))
    }
    fn node(&self, node: usize) -> Result<&Node> {
        self.cache
            .nodes
            .get(&node)
            .context("publication hierarchy node absent")
    }
}

fn spatial(header: &SpatialHeader) -> Bounds {
    Bounds {
        minimum: [
            header.bounds[0][0],
            header.bounds[0][1],
            header.bounds[0][2],
            f32::from_bits(header.packed_w_raw[0]),
        ],
        maximum: [
            header.bounds[1][0],
            header.bounds[1][1],
            header.bounds[1][2],
            f32::from_bits(header.packed_w_raw[1]),
        ],
    }
}

impl publication::Source for Source<'_> {
    fn key(&self) -> Key {
        self.cache.key
    }
    fn fresh_scope(&self) -> publication::FreshScope {
        self.scope
    }
    fn zone_section_index(&mut self, zone: usize) -> Result<usize> {
        Ok(self.zone(zone)?.section_index)
    }
    fn zone_root(&mut self, zone: usize) -> Result<publication::Root> {
        self.published_zone(zone)?;
        let root = self.view()?.zone_root(zone)?;
        let root = publication::Root {
            binding_count: root.binding_count,
            dynamic: root.dynamic,
        };
        self.cache.zones[zone].root = Some(root);
        Ok(root)
    }
    fn zone_placement_count(&mut self, zone: usize) -> Result<usize> {
        Ok(self.links(zone)?.len())
    }
    fn zone_placement(&mut self, zone: usize, ordinal: usize) -> Result<usize> {
        self.links(zone)?
            .get(ordinal)
            .copied()
            .context("publication link ordinal outside list")
    }
    fn zone_tree_root(&mut self, zone: usize) -> Result<usize> {
        if let Some(root) = self.zone(zone)?.tree {
            return Ok(root);
        }
        // Hierarchy records are checked base-block metadata, not the zone's
        // unrequested geometry payload. Source refreshes all trees after a change.
        let Hierarchy { root, nodes } = self
            .edb
            .map_candidate_hierarchy(self.cache.map_hash, zone)?
            .context("required publication hierarchy is absent")?;
        let added = nodes
            .keys()
            .filter(|address| !self.cache.nodes.contains_key(*address))
            .count();
        ensure!(
            self.cache
                .nodes
                .len()
                .checked_add(added)
                .context("publication node count overflow")?
                <= 65536,
            "publication shared hierarchy node budget exceeded"
        );
        for (address, node) in nodes {
            if let Entry::Vacant(entry) = self.cache.nodes.entry(address) {
                entry.insert(node);
            }
        }
        self.cache.zones[zone].tree = Some(root);
        Ok(root)
    }
    fn placement_entity(&mut self, placement: usize) -> Result<Option<EntityBounds>> {
        self.entity_index(placement, false)?
            .map(|index| {
                let header = self.edb.entity_spatial_at(index)?;
                Ok(EntityBounds {
                    base: spatial(&header),
                    component_mask: header.components_raw as u8,
                })
            })
            .transpose()
    }
    fn placement_matrix(&mut self, placement: usize) -> Result<[f32; 16]> {
        Ok(self.placement(placement)?.model)
    }
    fn placement_component(&mut self, _: usize, _: u8) -> Result<Bounds> {
        anyhow::bail!(
            "publication component requires a checked published component-target provider"
        )
    }
    fn placement_special(&mut self, _: usize) -> Result<Option<Bounds>> {
        anyhow::bail!("publication slot-six aggregate requires a checked published target provider")
    }
}

impl RefreshHierarchy for Source<'_> {
    type Node = usize;
    fn child_count(&mut self, node: usize) -> Result<u8> {
        match &self.node(node)?.entries {
            Entries::Branches(entries) => Ok(u8::try_from(entries.len())?),
            Entries::Leaves(_) => Ok(0),
        }
    }
    fn leaf_count(&mut self, node: usize) -> Result<u16> {
        let Entries::Leaves(entries) = &self.node(node)?.entries else {
            anyhow::bail!("publication node is not a leaf");
        };
        Ok(u16::try_from(entries.len())?)
    }
    fn leaf_placement(&mut self, node: usize, ordinal: u16) -> Result<u16> {
        let Entries::Leaves(entries) = &self.node(node)?.entries else {
            anyhow::bail!("publication node is not a leaf");
        };
        Ok(entries
            .get(usize::from(ordinal))
            .context("publication leaf ordinal outside node")?
            .placement)
    }
    fn placement_base_bounds(&mut self, placement: u16) -> Result<Option<StoredBounds>> {
        if let Some(value) = self.cache.base_bounds.get(&placement) {
            return Ok(*value);
        }
        let index = usize::from(placement);
        let value = if let Some(entity) = self.entity_index(index, true)? {
            let bounds = spatial(&self.edb.entity_spatial_at(entity)?)
                .transformed(&self.placement(index)?.model)?;
            Some(StoredBounds {
                minimum: bounds.minimum[..3].try_into()?,
                maximum: bounds.maximum[..3].try_into()?,
            })
        } else {
            None
        };
        self.cache.base_bounds.insert(placement, value);
        Ok(value)
    }
    fn branch_bounds(&mut self, node: usize, ordinal: u8) -> Result<StoredBounds> {
        let Entries::Branches(entries) = &self.node(node)?.entries else {
            anyhow::bail!("publication node is not internal");
        };
        let branch = entries
            .get(usize::from(ordinal))
            .context("publication branch ordinal outside node")?;
        Ok(StoredBounds {
            minimum: branch.bounds[0],
            maximum: branch.bounds[1],
        })
    }
    fn branch_target(&mut self, node: usize, ordinal: u8) -> Result<usize> {
        let Entries::Branches(entries) = &self.node(node)?.entries else {
            anyhow::bail!("publication node is not internal");
        };
        Ok(entries
            .get(usize::from(ordinal))
            .context("publication branch ordinal outside node")?
            .target)
    }
    fn write_branch_bounds(
        &mut self,
        node: usize,
        ordinal: u8,
        bounds: StoredBounds,
    ) -> Result<()> {
        let node = self
            .cache
            .nodes
            .get_mut(&node)
            .context("publication write node absent")?;
        let Entries::Branches(entries) = &mut node.entries else {
            anyhow::bail!("publication write node is not internal");
        };
        entries
            .get_mut(usize::from(ordinal))
            .context("publication write branch ordinal outside node")?
            .bounds = [bounds.minimum, bounds.maximum];
        Ok(())
    }
}
