//! Static map BSP inputs used by collision zone selection, independently of rendering.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct BspNode {
    pub plane: [f32; 4],
    /// Positive values are node indices; zero/negative values encode -zone.
    pub children: [i16; 2],
}
#[derive(Debug, Serialize)]
pub struct SpatialZone {
    pub entity_reference: u32,
    /// Packed source XYZ bounds; not a live animator's transformed bounds.
    pub bounds: [[f32; 3]; 2],
}
#[derive(Debug, Serialize)]
pub struct MapSpatial {
    pub map_hash: u32,
    /// Only nodes reachable from root zero are addressed. Cycles/sharing remain
    /// intact; the runtime traversal has a separate checked work budget.
    pub nodes: BTreeMap<u16, BspNode>,
    pub bsp_present: bool,
    pub zones: Vec<SpatialZone>,
}
pub(crate) fn map_address(data: &[u8], tables: &[Table], hash: u32) -> Result<usize> {
    let b = Bytes(data);
    let table = tables.get(6).context("missing spatial map directory")?;
    b.range(table.offset, table.count as usize * 16)?;
    let matches = (0..table.count as usize)
        .map(|i| b.u32(table.offset + i * 16))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|&h| h == hash)
        .count();
    ensure!(matches <= 1, "ambiguous spatial map hash");
    crate::scene::map_address(b, tables, hash)
}
pub(crate) fn read(data: &[u8], tables: &[Table], hash: u32) -> Result<MapSpatial> {
    let b = Bytes(data);
    let map = map_address(data, tables, hash)?;
    let zone_count = b.u32(map + 0x84)? as usize;
    ensure!(zone_count <= 65536, "invalid spatial zone count");
    b.range(map + 0x88, zone_count * 0x8c)?;
    let mut result = MapSpatial {
        map_hash: hash,
        nodes: BTreeMap::new(),
        bsp_present: b.u32(map + 4)? != 0,
        zones: Vec::with_capacity(zone_count),
    };
    for i in 0..zone_count {
        let at = map + 0x88 + i * 0x8c;
        let mut bounds = [[0.; 3]; 2];
        for (side, vector) in bounds.iter_mut().enumerate() {
            for (axis, component) in vector.iter_mut().enumerate() {
                *component = b.f32(at + 0x68 + side * 12 + axis * 4)?;
            }
        }
        result.zones.push(SpatialZone {
            entity_reference: b.u32(at)?,
            bounds,
        });
    }
    if !result.bsp_present {
        // Preserve absence. Attempting a runtime lookup without a root is an
        // error, not an implicit zone-zero or clear-world result.
        return Ok(result);
    }
    let start = b.relative(map + 4)?;
    let mut pending = vec![0u16];
    let mut seen = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        ensure!(seen.len() <= 32768, "spatial BSP node budget exceeded");
        let at = start + usize::from(index) * 32;
        // Only the plane and two child words are used; trailing node words stay opaque.
        b.range(at, 20)?;
        let plane = [b.f32(at)?, b.f32(at + 4)?, b.f32(at + 8)?, b.f32(at + 12)?];
        let children = [b.u16(at + 16)? as i16, b.u16(at + 18)? as i16];
        for child in children {
            if child > 0 {
                pending.push(child as u16);
            } else {
                let zone = -i32::from(child);
                ensure!(
                    (zone as usize) < zone_count,
                    "BSP leaf outside zone directory"
                );
            }
        }
        result.nodes.insert(index, BspNode { plane, children });
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests/map_spatial.rs"]
mod tests;
