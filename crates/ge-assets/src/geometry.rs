//! Lazy checked metadata for identified static geometry collector implementations.
//! This view resolves local references but does not infer runtime map membership.
use crate::{binary::Bytes, edb::Table, reference};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct GeometrySphere {
    pub center: [f32; 3],
    pub radius: f32,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GeometryBounds {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
    /// Homogeneous lanes are preserved without float validation; affine bounds
    /// transformation ignores them. They are not extra XYZ endpoints.
    pub packed_w_raw: [u32; 2],
}
pub struct GeometryView<'a> {
    data: &'a [u8],
    tables: &'a [Table],
}
impl<'a> GeometryView<'a> {
    pub(crate) fn new(data: &'a [u8], tables: &'a [Table]) -> Self {
        Self { data, tables }
    }
    fn field(address: usize, offset: usize) -> Result<usize> {
        address
            .checked_add(offset)
            .context("geometry field offset overflow")
    }
    pub fn node_type(&self, address: usize) -> Result<u32> {
        Bytes(self.data).u32(address)
    }
    /// The map collector reads this common base field before selecting a component.
    pub fn flags(&self, address: usize) -> Result<u32> {
        Bytes(self.data).u32(Self::field(address, 4)?)
    }
    pub fn component_mask(&self, address: usize) -> Result<u8> {
        Ok(Bytes(self.data).u32(Self::field(address, 0x40)?)? as u8)
    }
    pub fn component_target(&self, address: usize, slot: u8) -> Result<Option<usize>> {
        crate::entity_spatial::component_at(self.data, address, slot)?
            .map(|component| {
                self.common_type(component.address)?;
                Ok(component.address)
            })
            .transpose()
    }
    fn common_type(&self, address: usize) -> Result<()> {
        let kind = self.node_type(address)?;
        ensure!(
            matches!(kind, 0x601 | 0x603 | 0x607 | 0x608),
            "unsupported geometry spatial type {kind:#x}"
        );
        Ok(())
    }
    fn expect_type(&self, address: usize, expected: u32) -> Result<()> {
        ensure!(
            self.node_type(address)? == expected,
            "geometry body type does not match requested operation"
        );
        Ok(())
    }
    pub fn sphere(&self, address: usize) -> Result<GeometrySphere> {
        self.common_type(address)?;
        let b = Bytes(self.data);
        let at = Self::field(address, 0x30)?;
        Ok(GeometrySphere {
            center: [
                b.f32(at)?,
                b.f32(Self::field(at, 4)?)?,
                b.f32(Self::field(at, 8)?)?,
            ],
            radius: b.f32(Self::field(at, 12)?)?,
        })
    }
    pub fn bounds(&self, address: usize) -> Result<GeometryBounds> {
        self.common_type(address)?;
        let b = Bytes(self.data);
        let at = Self::field(address, 0x10)?;
        Ok(GeometryBounds {
            minimum: [
                b.f32(at)?,
                b.f32(Self::field(at, 4)?)?,
                b.f32(Self::field(at, 8)?)?,
            ],
            maximum: [
                b.f32(Self::field(at, 16)?)?,
                b.f32(Self::field(at, 20)?)?,
                b.f32(Self::field(at, 24)?)?,
            ],
            packed_w_raw: [b.u32(Self::field(at, 12)?)?, b.u32(Self::field(at, 28)?)?],
        })
    }
    pub fn split_child_count(&self, address: usize) -> Result<u32> {
        self.expect_type(address, 0x603)?;
        Bytes(self.data).u32(Self::field(address, 0x54)?)
    }
    pub fn split_child(&self, address: usize, index: u32) -> Result<usize> {
        ensure!(
            index < self.split_child_count(address)?,
            "geometry split child index out of bounds"
        );
        let offset = (index as usize)
            .checked_mul(4)
            .context("geometry child index offset overflow")?;
        let at = Self::field(Self::field(address, 0x5c)?, offset)?;
        let b = Bytes(self.data);
        ensure!(b.u32(at)? != 0, "null geometry split child");
        let target = b.relative(at)?;
        self.node_type(target)?;
        Ok(target)
    }
    pub fn link_count(&self, address: usize) -> Result<u8> {
        self.expect_type(address, 0x608)?;
        Ok(Bytes(self.data).range(Self::field(address, 0x54)?, 1)?[0])
    }
    /// Link rows use a 0x14-byte stride, but address only their first reference
    /// word. Lookup uses table ONE and its sixteen-byte directory descriptors.
    /// Local serialized target +8 replaces the original loader-filled pointer
    /// at descriptor +0xc; that runtime word is never interpreted here.
    pub fn link_target(&self, address: usize, index: u8) -> Result<Option<usize>> {
        ensure!(
            index < self.link_count(address)?,
            "geometry link index out of bounds"
        );
        let b = Bytes(self.data);
        let at = Self::field(address, 0x58 + usize::from(index) * 0x14)?;
        let key = b.u32(at)?;
        self.resolve_reference(key)
    }
    /// Resolve a local sixteen-byte reference-directory entry without selecting
    /// from the root entity directory. External binding remains unsupported.
    pub fn resolve_reference(&self, key: u32) -> Result<Option<usize>> {
        let b = Bytes(self.data);
        let table = self
            .tables
            .get(1)
            .context("missing geometry reference directory")?;
        let Some(index) = reference::resolve(self.data, table, 16, key)? else {
            return Ok(None);
        };
        let descriptor = table
            .offset
            .checked_add(
                index
                    .checked_mul(16)
                    .context("geometry reference index overflow")?,
            )
            .context("geometry reference descriptor overflow")?;
        b.range(descriptor, 16)?;
        ensure!(
            b.u32(descriptor)? == 0,
            "external geometry link references are not implemented"
        );
        let target = b.u32(Self::field(descriptor, 8)?)? as usize;
        // The caller's original entity-flag gate precedes virtual dispatch.
        // Preserve unknown type tags here; supported-type checks belong to the
        // requested geometry operation after that gate.
        self.node_type(target)?;
        Ok(Some(target))
    }
    pub fn optional_context_index(&self, address: usize) -> Result<u16> {
        self.common_type(address)?;
        Bytes(self.data).u16(Self::field(address, 0x52)?)
    }
}

#[cfg(test)]
#[path = "tests/geometry.rs"]
mod tests;
