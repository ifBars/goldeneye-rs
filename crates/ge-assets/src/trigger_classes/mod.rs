//! Narrow typed native classification from identity-checked supplied DATA5.
//! No native descriptor table is embedded, and no arbitrary-address/byte API
//! escapes the retained owner. Unknown indices are unsupported, never filtered.
use crate::supplied_data::Data;
use anyhow::{Result, bail};

const DESCRIPTOR_BASE: u32 = 0x8056_c3b8;

/// Source-proven lookup policy, distinct from a disabled/filtered descriptor.
/// Callers can retain unsupported classification outcomes without treating a
/// corrupt reached supported descriptor as the same condition.
pub const fn supports(native_type_index: u32) -> bool {
    matches!(
        native_type_index,
        0 | 2
            | 3
            | 4
            | 6
            | 8
            | 9
            | 12
            | 17
            | 28
            | 29
            | 35
            | 38
            | 39
            | 40
            | 47
            | 62
            | 63
            | 72
            | 73
            | 89
    )
}

pub struct View<'a> {
    data: &'a Data,
}

/// Only source-proven classification fields; no original descriptor arrays or
/// function pointers. No Serialize implementation is provided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub native_type_index: u32,
    pub source_address: u32,
    pub class: u32,
    pub subtype: u32,
    pub enable_byte_raw: i8,
}

impl<'a> View<'a> {
    pub(crate) fn from_data(data: &'a Data) -> Self {
        Self { data }
    }

    /// Only individually source-anchored descriptor indices are admitted. A
    /// host work budget or DATA section end is not a native table count.
    pub fn descriptor(&self, native_type_index: u32) -> Result<Descriptor> {
        if !supports(native_type_index) {
            bail!(
                "unsupported native trigger classification index {native_type_index}; complete descriptor table extent is unproven"
            );
        }
        let address = DESCRIPTOR_BASE + native_type_index * 16;
        Ok(Descriptor {
            native_type_index,
            source_address: address,
            class: self.data.u32(address)?,
            subtype: self.data.u32(address + 4)?,
            enable_byte_raw: self.data.u8(address + 12)? as i8,
        })
    }
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
