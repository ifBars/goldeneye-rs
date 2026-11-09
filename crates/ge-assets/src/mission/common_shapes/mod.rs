//! Checked metadata for common-trigger property27 initialization. This source
//! path retains a selector; it does not construct collision geometry or bounds.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use std::{error::Error, fmt};

mod geometry;
pub use geometry::Type0Geometry;
pub(crate) use geometry::read as read_geometry;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub maps: usize,
    /// Exclusive host selector limit, not a recovered native shape count.
    pub selectors: usize,
    /// Maximum addressed descriptor span: metadata needs seven bytes; the
    /// separate type0 geometry view requires 48 bytes. Map work is limited by `maps`.
    pub addressed_bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            maps: 32_768,
            selectors: 65_536,
            addressed_bytes: 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locator {
    pub requested_map_reference: u32,
    pub canonical_map_reference: u32,
    pub map_directory_index: usize,
    pub map_hash: u32,
    pub map_address: usize,
    pub trigger_block: usize,
    pub shape_directory: usize,
    pub descriptor_address: usize,
}

/// Source-addressed selector retained for the supported type0 initializer.
/// Unconsumed prefix words, dimensions and geometry remain private/unread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub raw_selector: u32,
    pub type_raw: u8,
    pub locator: Locator,
}

/// Explicit scope limitations, distinguishable from malformed source data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    Selector { raw_selector: u32 },
    Type { type_raw: u8, locator: Locator },
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selector { raw_selector } => write!(
                f,
                "unsupported negative common shape selector {raw_selector:#010x}"
            ),
            Self::Type { type_raw, .. } => {
                write!(f, "unsupported common shape initializer type {type_raw}")
            }
        }
    }
}
impl Error for Unsupported {}

pub(crate) fn read(
    data: &[u8],
    maps: &Table,
    reference: u32,
    raw_selector: u32,
    budget: Budget,
) -> Result<Option<Binding>> {
    ensure!(maps.name == "maps", "common shape requires map table six");
    ensure!(
        usize::from(maps.count) <= budget.maps,
        "common shape map directory budget exhausted"
    );
    let Some(index) = crate::reference::resolve(data, maps, 16, reference)? else {
        return Ok(None);
    };
    // Source has no absent/-1 sentinel here. Supporting unchecked preceding
    // memory or wrapped 32-bit multiplication would require another contract.
    if (raw_selector as i32) < 0 {
        return Err(Unsupported::Selector { raw_selector }.into());
    }
    let selector = usize::try_from(raw_selector)?;
    ensure!(
        selector < budget.selectors,
        "common shape selector budget exhausted"
    );
    ensure!(
        budget.addressed_bytes >= 7,
        "common shape addressed-byte budget exhausted"
    );
    let b = Bytes(data);
    let entry = maps
        .offset
        .checked_add(index.checked_mul(16).context("shape map stride overflow")?)
        .context("shape map entry overflow")?;
    let map_address = usize::try_from(
        b.u32(
            entry
                .checked_add(8)
                .context("shape map body field overflow")?,
        )?,
    )?;
    ensure!(
        b.u32(map_address)? == 0x500,
        "common shape requires map body type0x500"
    );
    let trigger_block = required_relative(
        b,
        map_address
            .checked_add(0x58)
            .context("shape trigger field overflow")?,
    )?;
    let shape_directory = required_relative(
        b,
        trigger_block
            .checked_add(0x10)
            .context("shape directory field overflow")?,
    )?;
    let descriptor_address = shape_directory
        .checked_add(
            selector
                .checked_mul(0x30)
                .context("shape selector stride overflow")?,
        )
        .context("shape descriptor address overflow")?;
    // Only the type byte is consumed. In particular, neither the leading
    // ffffffff word nor the unused remainder of the 48-byte row is decoded.
    let type_address = descriptor_address
        .checked_add(6)
        .context("shape type address overflow")?;
    let type_raw = b.range(type_address, 1)?[0];
    let locator = Locator {
        requested_map_reference: reference,
        canonical_map_reference: 0x8500_0000 | u32::try_from(index)?,
        map_directory_index: index,
        map_hash: b.u32(entry)?,
        map_address,
        trigger_block,
        shape_directory,
        descriptor_address,
    };
    if type_raw != 0 {
        return Err(Unsupported::Type { type_raw, locator }.into());
    }
    Ok(Some(Binding {
        raw_selector,
        type_raw,
        locator,
    }))
}

fn required_relative(bytes: Bytes<'_>, field: usize) -> Result<usize> {
    ensure!(
        bytes.u32(field)? != 0,
        "reached common shape relative pointer is null"
    );
    let target = bytes.relative(field)?;
    ensure!(target != 0, "reached common shape relative target is null");
    Ok(target)
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
