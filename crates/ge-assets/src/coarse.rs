//! Lazy checked views of optional static-mesh coarse sphere trees.
use crate::{binary::Bytes, edb::Entity};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    bytes: Bytes<'a>,
    pub root: usize,
    /// A present tree can retain a null pool pointer until it is addressed.
    pub pool: Option<usize>,
}
impl View<'_> {
    pub fn node_count(&self, node: usize) -> Result<u16> {
        self.bytes.u16(node)
    }
    pub fn pool_index(&self, node: usize) -> Result<u16> {
        self.bytes
            .u16(node.checked_add(2).context("coarse node offset overflow")?)
    }
    fn pool_address(&self, index: usize) -> Result<usize> {
        self.pool
            .context("addressed coarse tree has a null pool")?
            .checked_add(
                index
                    .checked_mul(16)
                    .context("coarse pool stride overflow")?,
            )
            .context("coarse pool address overflow")
    }
    pub fn sphere(&self, index: u16, slot: u16) -> Result<[f32; 4]> {
        let at = self.pool_address(usize::from(index) + usize::from(slot))?;
        Ok([
            self.bytes.f32(at)?,
            self.bytes.f32(at + 4)?,
            self.bytes.f32(at + 8)?,
            self.bytes.f32(at + 12)?,
        ])
    }
    /// Signed halfword offsets are relative to their own fields, including zero.
    pub fn child_target(&self, node: usize, slot: u16) -> Result<usize> {
        let field = node
            .checked_add(4)
            .and_then(|v| v.checked_add(usize::from(slot) * 2))
            .context("coarse child field overflow")?;
        let target = field as i64 + i64::from(self.bytes.u16(field)? as i16);
        ensure!(
            target >= 0 && target <= self.bytes.0.len() as i64,
            "coarse child relative pointer out of bounds"
        );
        Ok(target as usize)
    }
    fn bounds(&self, index: u16) -> Result<Bounds> {
        let at = self.pool_address(usize::from(index))?;
        let mut minimum = [0.; 3];
        let mut maximum = [0.; 3];
        for axis in 0..3 {
            // Containment checks used lanes lazily. Preserve NaN/Inf bits here;
            // neither padded W lane belongs to this reader's contract.
            minimum[axis] = f32::from_bits(self.bytes.u32(at + axis * 4)?);
            maximum[axis] = f32::from_bits(self.bytes.u32(at + 16 + axis * 4)?);
        }
        Ok(Bounds { minimum, maximum })
    }
    /// Root leaf uses pool base, irrespective of the root's stored index.
    pub fn root_bounds(&self) -> Result<Bounds> {
        self.bounds(0)
    }
    pub fn leaf_bounds(&self, index: u16) -> Result<Bounds> {
        self.bounds(index)
    }
}

pub(crate) fn read(data: &[u8], entity: Entity) -> Result<Option<View<'_>>> {
    ensure!(
        entity.entity_type == 0x601,
        "coarse tree requires static mesh type 0x601"
    );
    read_at(data, entity.address as usize)
}
pub(crate) fn read_at(data: &[u8], address: usize) -> Result<Option<View<'_>>> {
    let bytes = Bytes(data);
    ensure!(
        bytes.u32(address)? == 0x601,
        "coarse tree requires static mesh type 0x601"
    );
    read_geometry_at(data, address)
}

/// The split-line child gate uses the same base geometry +0x44 header and pool
/// as the static mesh gate. No root descriptor or entity-directory lookup is
/// required, and unknown geometry layouts remain unsupported.
pub(crate) fn read_geometry_at(data: &[u8], address: usize) -> Result<Option<View<'_>>> {
    let bytes = Bytes(data);
    ensure!(
        matches!(bytes.u32(address)?, 0x601 | 0x603 | 0x608),
        "coarse tree requires supported geometry type 0x601/0x603/0x608"
    );
    let field = address
        .checked_add(0x44)
        .context("coarse geometry field overflow")?;
    if bytes.u32(field)? == 0 {
        return Ok(None);
    }
    let header = bytes.relative(field)?;
    let pool = if bytes.u32(header)? == 0 {
        None
    } else {
        Some(bytes.relative(header)?)
    };
    Ok(Some(View {
        bytes,
        root: header
            .checked_add(4)
            .context("coarse root offset overflow")?,
        pool,
    }))
}

#[cfg(test)]
#[path = "tests/coarse.rs"]
mod tests;
