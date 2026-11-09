//! Narrow inventory-admission fields from the source guns spreadsheet.
//! Group zero primary registration is separate from group four inventory keys.
use crate::{edb::Edb, spreadsheet};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
mod default;
pub use default::{DefaultMetadata, find_default_inventory};

pub const RESOURCE: u32 = 0x0109_000f;
pub const SHEET: u32 = 0x1400_001c;

#[derive(Debug, Clone, Copy)]
pub struct InventoryMetadata {
    pub inventory_key: u32,
    pub primary_key: u32,
    pub type_key: u32,
    pub category_key: u32,
    pub byte_9_raw: u8,
    pub ammunition_raw: [u32; 2],
    pub source_index: usize,
    pub group_zero_address: usize,
    pub group_four_address: usize,
}

/// Only the groups reached by these fields are read. Other weapon fields and
/// native full-row concatenation belong to their future consumers.
pub fn find_inventory(
    edb: &Edb,
    key: u32,
    maximum_rows: usize,
) -> Result<Option<InventoryMetadata>> {
    ensure!(
        edb.header.hash == RESOURCE && edb.header.version == 263,
        "weapon metadata requires supported guns resource"
    );
    let sheet = edb
        .spreadsheet(SHEET)?
        .context("weapon metadata sheet absent")?;
    let budget = spreadsheet::Budget {
        group_fields: 5,
        rows: maximum_rows,
        row_bytes: 0x44,
    };
    let primary = sheet
        .group(0, 0x44, budget)?
        .context("weapon primary group absent")?;
    let inventory = sheet
        .group(4, 0x24, budget)?
        .context("weapon inventory group absent")?;
    ensure!(
        primary.row_count == inventory.row_count,
        "weapon metadata group row counts differ"
    );
    let mut registered = BTreeMap::new();
    for index in 0..primary.row_count {
        let words = primary.words::<17>(index)?;
        registered.entry(words[0]).or_insert(index);
    }
    for (primary_key, index) in registered {
        let item = inventory.words::<9>(index)?;
        if item[1] != key {
            continue;
        }
        let row = primary.words::<17>(index)?;
        return Ok(Some(InventoryMetadata {
            inventory_key: item[1],
            primary_key,
            type_key: row[1],
            category_key: row[10],
            byte_9_raw: primary.row(index)?[9],
            ammunition_raw: [item[6], item[7]],
            source_index: index,
            group_zero_address: primary.block_address + 4 + index * 0x44,
            group_four_address: inventory.block_address + 4 + index * 0x24,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
