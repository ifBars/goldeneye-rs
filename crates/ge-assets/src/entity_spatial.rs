//! Checked common spatial headers for currently identified static entity types.
use crate::{binary::Bytes, edb::Entity};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SpatialHeader {
    pub address: usize,
    pub entity_type: u32,
    pub flags_raw: u32,
    /// Signed render-sort override at common offset eight; retained independently
    /// from flags and stored bounds. Zero selects the computed-center path.
    pub sort_field_raw: i32,
    pub bounds: [[f32; 3]; 2],
    /// The affine bounds helper ignores these stored homogeneous lanes.
    pub packed_w_raw: [u32; 2],
    /// Low byte is the component mask; upper bits encode a signed relative link.
    /// This reader preserves the word without following any component pointers.
    pub components_raw: u32,
}
pub(crate) fn read(data: &[u8], entity: Entity) -> Result<SpatialHeader> {
    read_at(data, entity.address as usize)
}
pub(crate) fn read_at(data: &[u8], address: usize) -> Result<SpatialHeader> {
    let b = Bytes(data);
    let entity_type = b.u32(address)?;
    ensure!(
        matches!(entity_type, 0x601 | 0x603 | 0x607 | 0x608),
        "unsupported static spatial header type"
    );
    b.range(address, 0x44)?;
    let mut bounds = [[0.; 3]; 2];
    let mut packed_w_raw = [0; 2];
    for side in 0..2 {
        let start = address + 0x10 + side * 16;
        for (axis, value) in bounds[side].iter_mut().enumerate() {
            *value = b.f32(start + axis * 4)?;
        }
        packed_w_raw[side] = b.u32(start + 12)?;
    }
    Ok(SpatialHeader {
        flags_raw: b.u32(address + 4)?,
        sort_field_raw: b.u32(address + 8)? as i32,
        bounds,
        packed_w_raw,
        components_raw: b.u32(address + 0x40)?,
        address,
        entity_type,
    })
}

#[derive(Debug, Serialize)]
pub struct Component {
    pub slot: u8,
    pub address: usize,
    pub entity_type: u32,
    pub descriptor_low_byte_raw: u8,
}
fn relative24(data: &[u8], at: usize, word: u32) -> Result<usize> {
    let target = at as i64 + i64::from((word as i32) >> 8);
    ensure!(
        target >= 0 && target < data.len() as i64,
        "component relative pointer out of bounds"
    );
    Ok(target as usize)
}
/// Compact table ordered by set bits. A masked-out slot reads no table pointer
/// or descriptor. Targets may be non-root bodies; no recursive traversal occurs.
pub(crate) fn component(data: &[u8], entity: Entity, slot: u8) -> Result<Option<Component>> {
    component_at(data, entity.address as usize, slot)
}
pub(crate) fn component_at(data: &[u8], address: usize, slot: u8) -> Result<Option<Component>> {
    ensure!(slot < 8, "invalid static component slot");
    let b = Bytes(data);
    ensure!(
        matches!(b.u32(address)?, 0x601 | 0x603 | 0x607 | 0x608),
        "unsupported static component parent type"
    );
    let at = address
        .checked_add(0x40)
        .ok_or_else(|| anyhow::anyhow!("component header offset overflow"))?;
    let word = b.u32(at)?;
    let mask = word as u8;
    if mask & (1 << slot) == 0 {
        return Ok(None);
    }
    let table = relative24(data, at, word)?;
    let rank = (mask & ((1u8 << slot) - 1)).count_ones() as usize;
    let descriptor = table
        .checked_add(rank * 4)
        .ok_or_else(|| anyhow::anyhow!("component table offset overflow"))?;
    let word = b.u32(descriptor)?;
    let address = relative24(data, descriptor, word)?;
    Ok(Some(Component {
        slot,
        address,
        entity_type: b.u32(address)?,
        descriptor_low_byte_raw: word as u8,
    }))
}

#[cfg(test)]
#[path = "tests/entity_spatial.rs"]
mod tests;
