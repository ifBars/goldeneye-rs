//! Static single-map navigation registrations, with native entity/instance reference rules.
use crate::{
    binary::Bytes,
    edb::{Table, read_entities},
    reference,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MapNavigation {
    pub map_hash: u32,
    pub placements: usize,
    pub unresolved_entity_references: usize,
    /// Registration order includes wrappers whose lazy binding cannot resolve.
    pub registrations: Vec<NavigationPlacement>,
}
#[derive(Debug, Serialize)]
pub struct NavigationPlacement {
    pub source_placement: usize,
    pub instance_key: u32,
    pub source_entity_index: usize,
    pub binding: NavigationBinding,
}
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NavigationBinding {
    Ready {
        resolved_placement: usize,
        entity_index: usize,
        translation: [f32; 3],
    },
    InstanceMissing,
    EntityMissing {
        resolved_placement: usize,
    },
    WrongEntityType {
        resolved_placement: usize,
        entity_index: usize,
        entity_type: u32,
    },
}
struct Source {
    key: u32,
    position: [f32; 3],
    entity: u32,
}

pub(crate) fn read(data: &[u8], tables: &[Table], map_hash: u32) -> Result<MapNavigation> {
    let b = Bytes(data);
    let maps = tables.get(6).context("missing map directory")?;
    b.range(maps.offset, maps.count as usize * 16)?;
    let mut matches = 0;
    for i in 0..maps.count as usize {
        matches += usize::from(b.u32(maps.offset + i * 16)? == map_hash);
    }
    ensure!(matches <= 1, "duplicate navigation map hash");
    let map = crate::scene::map_address(b, tables, map_hash)?;
    let count = b.u32(map + 0x48)? as usize;
    ensure!(count <= 1_000_000, "invalid navigation placement count");
    let start = b.relative(map + 0x4c)?;
    b.range(start, count * 60)?;
    // Position-only binding must not depend on an inspection rotation/scale convention.
    let sources = (0..count)
        .map(|i| {
            let p = start + i * 60;
            Ok(Source {
                key: b.u32(p)?,
                position: [b.f32(p + 4)?, b.f32(p + 8)?, b.f32(p + 12)?],
                entity: b.u32(p + 48)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let entities = read_entities(data, tables)?;
    let entity_table = tables.get(2).context("missing entity directory")?;
    let mut result = MapNavigation {
        map_hash,
        placements: count,
        unresolved_entity_references: 0,
        registrations: Vec::new(),
    };
    for (index, source) in sources.iter().enumerate() {
        let Some(entity_index) = reference::resolve(data, entity_table, 20, source.entity)? else {
            result.unresolved_entity_references += 1;
            continue;
        };
        if entities[entity_index].entity_type != 0x607 {
            continue;
        }
        // The single-map scope is explicit: native global lookup checks loaded maps in order.
        let resolved = if source.key >> 24 == 0 {
            ((source.key as usize) < count).then_some(source.key as usize)
        } else {
            sources.iter().position(|p| p.key == source.key)
        };
        let binding = if let Some(resolved) = resolved {
            if let Some(entity_index) =
                reference::resolve(data, entity_table, 20, sources[resolved].entity)?
            {
                if entities[entity_index].entity_type == 0x607 {
                    NavigationBinding::Ready {
                        resolved_placement: resolved,
                        entity_index,
                        translation: sources[resolved].position,
                    }
                } else {
                    NavigationBinding::WrongEntityType {
                        resolved_placement: resolved,
                        entity_index,
                        entity_type: entities[entity_index].entity_type,
                    }
                }
            } else {
                NavigationBinding::EntityMissing {
                    resolved_placement: resolved,
                }
            }
        } else {
            NavigationBinding::InstanceMissing
        };
        result.registrations.push(NavigationPlacement {
            source_placement: index,
            instance_key: source.key,
            source_entity_index: entity_index,
            binding,
        });
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests/navigation_placements.rs"]
mod tests;
