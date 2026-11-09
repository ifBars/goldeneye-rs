//! Native map-reference normalization and stored runtime bounds. These are map
//! declarations, independent of fitted triangle bounds and trigger activation.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDescriptor {
    pub directory_index: usize,
    pub stored_hash: u32,
    pub requested_reference: u32,
    pub canonical_reference: u32,
    pub body_address: usize,
    /// Source map +0x6c..+0x80. Preserve raw float bits and inverted sentinels;
    /// no mesh fit, normalization, sorting or numerical repair occurs here.
    pub declared_bounds_bits: [[u32; 3]; 2],
}

pub fn read(
    data: &[u8],
    maps: &Table,
    reference: u32,
    budget: Budget,
) -> Result<Option<MapDescriptor>> {
    ensure!(maps.name == "maps", "map descriptor requires table six");
    ensure!(
        usize::from(maps.count) <= budget.maps,
        "map descriptor directory budget exhausted"
    );
    let Some(index) = crate::reference::resolve(data, maps, 16, reference)? else {
        return Ok(None);
    };
    let b = Bytes(data);
    let entry = maps
        .offset
        .checked_add(
            index
                .checked_mul(16)
                .context("map descriptor stride overflow")?,
        )
        .context("map descriptor address overflow")?;
    let body = b.u32(entry.checked_add(8).context("map body field overflow")?)? as usize;
    ensure!(
        b.u32(body)? == 0x500,
        "map descriptor body is not type 0x500"
    );
    let mut bounds = [[0; 3]; 2];
    for (side, vector) in bounds.iter_mut().enumerate() {
        for (axis, value) in vector.iter_mut().enumerate() {
            let offset = body
                .checked_add(0x6c + side * 12 + axis * 4)
                .context("map bounds address overflow")?;
            *value = b.u32(offset)?;
        }
    }
    let index32 = u32::try_from(index)?;
    ensure!(
        index32 <= 0x00ff_ffff,
        "map directory index is outside coded-reference width"
    );
    Ok(Some(MapDescriptor {
        directory_index: index,
        stored_hash: b.u32(entry)?,
        requested_reference: reference,
        canonical_reference: 0x8500_0000 | index32,
        body_address: body,
        declared_bounds_bits: bounds,
    }))
}

#[cfg(test)]
#[path = "tests/world.rs"]
mod tests;
