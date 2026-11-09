//! Grouped map portal inputs, separately from BSP binding and rendered geometry.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct PortalRow {
    pub prefix_raw: u16,
    pub target_zone_raw: i16,
    pub portal_index_raw: i16,
    pub extra_rows_raw: u8,
    pub tail_raw: u8,
}
#[derive(Debug, Serialize)]
pub struct PortalGroup {
    /// Only the first row supplies the target and group length. Other rows'
    /// corresponding words remain raw; their portal indices are still used.
    pub target_zone: i16,
    pub rows: Vec<PortalRow>,
}
#[derive(Debug, Serialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub enum PortalShape {
    InlineQuad { vertices: [[f32; 3]; 4] },
    LinkedPolygon { vertices: Vec<[f32; 3]> },
}
impl PortalShape {
    pub fn vertices(&self) -> &[[f32; 3]] {
        match self {
            Self::InlineQuad { vertices } => vertices,
            Self::LinkedPolygon { vertices } => vertices,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct MapPortals {
    pub map_hash: u32,
    pub declared_portals: usize,
    pub zone_groups: Vec<Vec<PortalGroup>>,
    /// Only shapes referenced by nonnegative target groups are decoded.
    pub shapes: BTreeMap<u16, PortalShape>,
}
/// Query view: selected-zone rows and addressed shapes are separate operations.
/// The full inventory reader deliberately validates more data than a query uses.
pub struct QueryView<'a> {
    data: &'a [u8],
    map: usize,
    pub zone_count: usize,
}
pub(crate) fn query_view<'a>(data: &'a [u8], tables: &[Table], hash: u32) -> Result<QueryView<'a>> {
    let map = crate::spatial::map_address(data, tables, hash)?;
    let zone_count = Bytes(data).u32(map + 0x84)? as usize;
    ensure!(zone_count <= 65536, "invalid portal zone count");
    Ok(QueryView {
        data,
        map,
        zone_count,
    })
}
impl QueryView<'_> {
    /// Preserve raw indices/targets; validate their use after ordered radius gates.
    pub fn groups(&self, zone: usize) -> Result<Vec<PortalGroup>> {
        ensure!(
            zone < self.zone_count,
            "portal query zone outside directory"
        );
        let b = Bytes(self.data);
        let at = self.map + 0x88 + zone * 0x8c;
        let count = b.u32(at + 0x20)? as usize;
        ensure!(count <= 1_000_000, "zone portal-row budget exceeded");
        if count == 0 {
            return Ok(Vec::new());
        }
        ensure!(b.u32(at + 0x24)? != 0, "missing zone portal rows");
        let start = b.relative(at + 0x24)?;
        b.range(start, count * 8)?;
        let mut groups = Vec::new();
        let mut cursor = 0;
        while cursor < count {
            let row = start + cursor * 8;
            let size = usize::from(b.range(row + 6, 1)?[0]) + 1;
            ensure!(
                size <= count - cursor,
                "portal group extends beyond zone rows"
            );
            let rows = (0..size)
                .map(|offset| {
                    let p = row + offset * 8;
                    Ok(PortalRow {
                        prefix_raw: b.u16(p)?,
                        target_zone_raw: b.u16(p + 2)? as i16,
                        portal_index_raw: b.u16(p + 4)? as i16,
                        extra_rows_raw: b.range(p + 6, 1)?[0],
                        tail_raw: b.range(p + 7, 1)?[0],
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            groups.push(PortalGroup {
                target_zone: b.u16(row + 2)? as i16,
                rows,
            });
            cursor += size;
        }
        Ok(groups)
    }
    pub fn shape(&self, index: i16) -> Result<PortalShape> {
        let b = Bytes(self.data);
        let count = b.u32(self.map + 0x38)? as usize;
        ensure!(count <= 1_000_000, "invalid declared portal count");
        ensure!(
            index >= 0 && (index as usize) < count,
            "addressed portal index outside directory"
        );
        ensure!(
            b.u32(self.map + 0x3c)? != 0,
            "missing addressed portal directory"
        );
        let start = b.relative(self.map + 0x3c)?;
        read_shape(b, start + index as usize * 0x44, 65536)
    }
}
fn position(b: Bytes<'_>, at: usize) -> Result<[f32; 3]> {
    Ok([b.f32(at)?, b.f32(at + 4)?, b.f32(at + 8)?])
}
fn read_shape(b: Bytes<'_>, at: usize, vertex_budget: usize) -> Result<PortalShape> {
    b.range(at, 0x14)?;
    Ok(if b.u32(at + 0x10)? == 0 {
        PortalShape::InlineQuad {
            vertices: [
                position(b, at + 0x14)?,
                position(b, at + 0x20)?,
                position(b, at + 0x2c)?,
                position(b, at + 0x38)?,
            ],
        }
    } else {
        let linked = b.relative(at + 0x10)?;
        let count = b.u32(linked + 8)? as usize;
        ensure!(
            (3..=65536).contains(&count),
            "invalid linked portal vertex count"
        );
        ensure!(count <= vertex_budget, "map portal vertex budget exceeded");
        PortalShape::LinkedPolygon {
            vertices: (0..count)
                .map(|i| position(b, linked + 0x10 + i * 32))
                .collect::<Result<Vec<_>>>()?,
        }
    })
}
pub(crate) fn read(data: &[u8], tables: &[Table], hash: u32) -> Result<MapPortals> {
    let b = Bytes(data);
    let map = crate::spatial::map_address(data, tables, hash)?;
    let zones = b.u32(map + 0x84)? as usize;
    ensure!(zones <= 65536, "invalid portal zone count");
    b.range(map + 0x88, zones * 0x8c)?;
    let declared = b.u32(map + 0x38)? as usize;
    ensure!(declared <= 1_000_000, "invalid declared portal count");
    let mut result = MapPortals {
        map_hash: hash,
        declared_portals: declared,
        zone_groups: Vec::with_capacity(zones),
        shapes: BTreeMap::new(),
    };
    let mut requested = BTreeSet::new();
    let mut total_rows = 0usize;
    for index in 0..zones {
        let zone = map + 0x88 + index * 0x8c;
        let count = b.u32(zone + 0x20)? as usize;
        total_rows = total_rows
            .checked_add(count)
            .ok_or_else(|| anyhow::anyhow!("portal row count overflow"))?;
        ensure!(total_rows <= 1_000_000, "map portal-row budget exceeded");
        let mut groups = Vec::new();
        if count > 0 {
            ensure!(b.u32(zone + 0x24)? != 0, "missing zone portal rows");
            let start = b.relative(zone + 0x24)?;
            b.range(start, count * 8)?;
            let mut cursor = 0;
            while cursor < count {
                let at = start + cursor * 8;
                let target = b.u16(at + 2)? as i16;
                let size = usize::from(b.range(at + 6, 1)?[0]) + 1;
                ensure!(
                    size <= count - cursor,
                    "portal group extends beyond zone rows"
                );
                if target >= 0 {
                    ensure!(
                        (target as usize) < zones,
                        "portal target zone outside directory"
                    );
                }
                let mut rows = Vec::with_capacity(size);
                for i in 0..size {
                    let row = at + i * 8;
                    let portal = b.u16(row + 4)? as i16;
                    if target >= 0 {
                        ensure!(
                            portal >= 0 && (portal as usize) < declared,
                            "addressed portal index outside directory"
                        );
                        requested.insert(portal as u16);
                    }
                    rows.push(PortalRow {
                        prefix_raw: b.u16(row)?,
                        target_zone_raw: b.u16(row + 2)? as i16,
                        portal_index_raw: portal,
                        extra_rows_raw: b.range(row + 6, 1)?[0],
                        tail_raw: b.range(row + 7, 1)?[0],
                    });
                }
                groups.push(PortalGroup {
                    target_zone: target,
                    rows,
                });
                cursor += size;
            }
        }
        result.zone_groups.push(groups);
    }
    let mut vertices_read = 0usize;
    if !requested.is_empty() {
        ensure!(
            b.u32(map + 0x3c)? != 0,
            "missing addressed portal directory"
        );
        let start = b.relative(map + 0x3c)?;
        for index in requested {
            let at = start + usize::from(index) * 0x44;
            let shape = read_shape(b, at, 1_000_000 - vertices_read)?;
            vertices_read += shape.vertices().len();
            ensure!(
                vertices_read <= 1_000_000,
                "map portal vertex budget exceeded"
            );
            result.shapes.insert(index, shape);
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests/map_portals.rs"]
mod tests;
