//! Source-selected linked map lookup through a map's ordered trigger directory.
//!
//! This is a resource reader, not trigger activation or world registration.
//! SJBE52 r0's native type index 62 identifies class 0x1e000004/subtype
//! 0x1f0000a1. No original runtime type table is embedded in the adapter.
use super::triggers;
pub use super::triggers::Budget;
use crate::edb::Table;
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Locator {
    pub map_reference: u32,
    pub map_directory_index: usize,
    pub map_hash: u32,
    pub map_address: usize,
    pub trigger_block: usize,
    pub trigger_ordinal: usize,
    pub record_address: usize,
    pub schema_index: u16,
    pub schema_address: usize,
    pub native_type_index: u32,
    pub flags_raw: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LinkedMap {
    /// Missing property flags produce the native 0xffffffff sentinel. Preserve
    /// it even for a matched record; it is distinct from no matching record.
    pub file: u32,
    pub map: u32,
    pub locator: Locator,
}

pub fn read(
    data: &[u8],
    maps: &Table,
    reference: u32,
    budget: Budget,
) -> Result<Option<LinkedMap>> {
    read_with_classifier(data, maps, reference, budget, |native_type_index| {
        Ok(native_type_index == 62)
    })
}

/// The classifier maps the addressed local schema's native type index to the
/// specific linked-map class/subtype pair. It is called in storage order and
/// never for rows after the first matching record. This keeps unsupported or
/// later classification policies separate from the checked serialized reader.
pub fn read_with_classifier(
    data: &[u8],
    maps: &Table,
    reference: u32,
    budget: Budget,
    mut classify: impl FnMut(u32) -> Result<bool>,
) -> Result<Option<LinkedMap>> {
    let Some(mut records) = triggers::records(data, maps, reference, budget)? else {
        return Ok(None);
    };
    while let Some(record) = records.next_record()? {
        let locator = record.locator;
        if !classify(locator.native_type_index)? {
            continue;
        }
        let flags = record.word(8)?;
        let property = |bit, offset| -> Result<u32> {
            if flags & bit == 0 {
                return Ok(u32::MAX);
            }
            record.word(offset)
        };
        return Ok(Some(LinkedMap {
            file: property(1, 0x30)?,
            map: property(2, 0x34)?,
            locator: Locator {
                map_reference: locator.map_reference,
                map_directory_index: locator.map_directory_index,
                map_hash: locator.map_hash,
                map_address: locator.map_address,
                trigger_block: locator.trigger_block,
                trigger_ordinal: locator.trigger_ordinal,
                record_address: locator.record_address,
                schema_index: locator.schema_index,
                schema_address: locator.schema_address,
                native_type_index: locator.native_type_index,
                flags_raw: flags,
            },
        }));
    }
    Ok(None)
}

#[cfg(test)]
#[path = "tests/maps.rs"]
mod tests;
