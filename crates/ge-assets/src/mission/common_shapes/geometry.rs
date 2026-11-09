//! Full source type0 fields, addressed only by the geometry consumer. The
//! metadata-only selector reader deliberately does not call this module.
use super::{Binding, Budget};
use crate::{binary::Bytes, edb::Table};
use anyhow::{Result, ensure};
use std::fmt;

/// Checked view bound to the immutable EDB data that resolved its selector.
/// Raw vectors remain source inputs, not transformed geometry or bounds.
/// Neither this view nor its original arrays implement Serialize.
pub struct Type0Geometry<'a> {
    _owner: &'a [u8],
    binding: Binding,
    word_04_raw: u16,
    dimensions_bits: [u32; 3],
    center_bits: [u32; 3],
    orientation_bits: [u32; 4],
}

impl fmt::Debug for Type0Geometry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Do not accidentally dump the retained owner or source vectors.
        f.debug_struct("Type0Geometry")
            .field("binding", &self.binding)
            .field("word_04_raw", &self.word_04_raw)
            .finish_non_exhaustive()
    }
}

impl Type0Geometry<'_> {
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn word_04_raw(&self) -> u16 {
        self.word_04_raw
    }
    pub fn type_raw(&self) -> u8 {
        self.binding.type_raw
    }
    pub fn dimensions_bits(&self) -> [u32; 3] {
        self.dimensions_bits
    }
    pub fn center_bits(&self) -> [u32; 3] {
        self.center_bits
    }
    pub fn orientation_bits(&self) -> [u32; 4] {
        self.orientation_bits
    }
}

pub(crate) fn read<'a>(
    data: &'a [u8],
    maps: &Table,
    reference: u32,
    raw_selector: u32,
    budget: Budget,
) -> Result<Option<Type0Geometry<'a>>> {
    // Resolve from THIS owner each time, never from a caller-supplied Binding
    // locator or a borrowed lookup into a different world resource.
    let Some(binding) = super::read(data, maps, reference, raw_selector, budget)? else {
        return Ok(None);
    };
    ensure!(
        budget.addressed_bytes >= 48,
        "common shape geometry byte budget exhausted"
    );
    let row = Bytes(Bytes(data).range(binding.locator.descriptor_address, 48)?);
    Ok(Some(Type0Geometry {
        _owner: data,
        binding,
        word_04_raw: row.u16(4)?,
        dimensions_bits: [row.u32(8)?, row.u32(0xc)?, row.u32(0x10)?],
        center_bits: [row.u32(0x14)?, row.u32(0x18)?, row.u32(0x1c)?],
        orientation_bits: [
            row.u32(0x20)?,
            row.u32(0x24)?,
            row.u32(0x28)?,
            row.u32(0x2c)?,
        ],
    }))
}

#[cfg(test)]
#[path = "tests/geometry.rs"]
mod tests;
