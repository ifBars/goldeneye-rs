use anyhow::{Context, Result, ensure};

use crate::mission::world::{InstanceId, WrapperId};

use super::Budget;

/// Placement lookup key, distinct from entity and map references. A zero top
/// byte selects a placement index; every other value is a hash, including MAX.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceInstanceKey(u32);

impl SourceInstanceKey {
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub key: SourceInstanceKey,
    pub entity_reference_raw: u32,
    pub position_bits: [u32; 3],
}

/// Loaded-vector order is significant. This fresh-instance scope rejects
/// duplicate semantic instance handles rather than inventing alias behavior.
pub struct MapView<'a> {
    pub instance: InstanceId,
    pub wrapper: WrapperId,
    pub placements: &'a [Placement],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementLocation {
    pub instance: InstanceId,
    pub index: usize,
}

/// A checked descriptor supplied by the owning level-resource adapter. Type607
/// is the supported NavMesh format; this is not a general native RTTI engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    pub directory_index: usize,
    pub entity_type: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub level_wrapper: WrapperId,
    pub source: PlacementLocation,
    pub found: PlacementLocation,
    pub key: SourceInstanceKey,
    pub source_entity_reference_raw: u32,
    pub source_entity_directory_index: usize,
    /// Source wrapper+18 caches this RAW reference, not its directory index.
    /// Its constructor sentinel is MAX; the placement key has no such sentinel.
    pub cached_entity_reference_raw: u32,
    pub entity_directory_index: usize,
    /// Captured found-placement XYZ, with source wrapper W forced positive zero.
    pub translation_bits: [u32; 4],
}

/// Resolve one source placement from the initial registration map. Missing or
/// non-NavMesh INITIAL entities skip registration. Once eligible, required
/// global lookup/revalidation failures are errors, never empty initialized
/// owners. The resolver is always called with the original LEVEL owner, even
/// when first-match placement lookup finds another loaded map.
///
/// This models a newly allocated wrapper whose cached reference is MAX. It does
/// not repeat key lookup for an existing bound wrapper or reset native history.
pub fn resolve_binding(
    level_wrapper: WrapperId,
    source: PlacementLocation,
    maps: &[MapView<'_>],
    mut resolve_entity: impl FnMut(WrapperId, u32) -> Result<Option<Entity>>,
    budget: Budget,
) -> Result<Option<Binding>> {
    validate_maps(maps, budget)?;
    let source_map = maps
        .iter()
        .find(|map| map.instance == source.instance)
        .context("navigation source map instance is absent")?;
    let source_placement = source_map
        .placements
        .get(source.index)
        .context("navigation source placement is absent")?;
    let Some(initial) = resolve_entity(level_wrapper, source_placement.entity_reference_raw)?
    else {
        return Ok(None);
    };
    if initial.entity_type != 0x607 {
        return Ok(None);
    }

    let (found, placement) = find_placement(source_placement.key, maps)
        .context("eligible navigation wrapper required placement lookup failed")?;
    let resolved = resolve_entity(level_wrapper, placement.entity_reference_raw)?
        .context("eligible navigation wrapper required entity is absent")?;
    ensure!(
        resolved.entity_type == 0x607,
        "eligible navigation wrapper required entity is not NavMesh"
    );
    Ok(Some(Binding {
        level_wrapper,
        source,
        found,
        key: source_placement.key,
        source_entity_reference_raw: source_placement.entity_reference_raw,
        source_entity_directory_index: initial.directory_index,
        cached_entity_reference_raw: placement.entity_reference_raw,
        entity_directory_index: resolved.directory_index,
        translation_bits: [
            placement.position_bits[0],
            placement.position_bits[1],
            placement.position_bits[2],
            0,
        ],
    }))
}

fn validate_maps(maps: &[MapView<'_>], budget: Budget) -> Result<()> {
    ensure!(maps.len() <= budget.maps, "navigation map budget exhausted");
    let mut placements = 0usize;
    for (index, map) in maps.iter().enumerate() {
        ensure!(
            !maps[..index]
                .iter()
                .any(|earlier| earlier.instance == map.instance),
            "navigation fresh map instance identity is repeated"
        );
        placements = placements
            .checked_add(map.placements.len())
            .context("navigation placement count overflow")?;
        ensure!(
            placements <= budget.placements,
            "navigation placement budget exhausted"
        );
    }
    Ok(())
}

fn find_placement<'a>(
    key: SourceInstanceKey,
    maps: &'a [MapView<'_>],
) -> Option<(PlacementLocation, &'a Placement)> {
    for map in maps {
        let selected = if key.raw() >> 24 == 0 {
            usize::try_from(key.raw())
                .ok()
                .filter(|&index| index < map.placements.len())
        } else {
            map.placements
                .iter()
                .position(|placement| placement.key == key)
        };
        if let Some(index) = selected {
            return Some((
                PlacementLocation {
                    instance: map.instance,
                    index,
                },
                &map.placements[index],
            ));
        }
    }
    None
}
