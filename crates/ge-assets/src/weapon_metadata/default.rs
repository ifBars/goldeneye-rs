//! Selected normal-mode fields reached by fresh default slot selection.
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct DefaultMetadata {
    pub inventory: InventoryMetadata,
    pub native_sorted_index: usize,
    pub byte_8_raw: u8,
    /// Runtime row +54..+78 inclusive; source normal group one only.
    pub normal_values_bits: [u32; 10],
    /// Source conditional order: +7c,+88,+80,+84.
    pub modifier_bits: [u32; 4],
}

pub fn find_default_inventory(
    edb: &Edb,
    key: u32,
    maximum_rows: usize,
) -> Result<Option<DefaultMetadata>> {
    let Some(inventory) = find_inventory(edb, key, maximum_rows)? else {
        return Ok(None);
    };
    let sheet = edb
        .spreadsheet(SHEET)?
        .context("default weapon sheet absent")?;
    let budget = spreadsheet::Budget {
        group_fields: 2,
        rows: maximum_rows,
        row_bytes: 0x84,
    };
    let primary = sheet
        .group(0, 0x44, budget)?
        .context("default weapon primary group absent")?;
    let normal = sheet
        .group(1, 0x84, budget)?
        .context("default weapon normal group absent")?;
    ensure!(
        primary.row_count == normal.row_count,
        "default weapon normal row count differs"
    );
    let mut sorted = BTreeMap::new();
    for index in 0..primary.row_count {
        sorted
            .entry(primary.words::<17>(index)?[0])
            .or_insert(index);
    }
    let native_sorted_index = sorted
        .keys()
        .position(|&value| value == inventory.primary_key)
        .context("selected default weapon primary registration absent")?;
    ensure!(
        sorted[&inventory.primary_key] == inventory.source_index,
        "default weapon source registration differs"
    );
    let normal = normal.row(inventory.source_index)?;
    let word = |runtime_offset: usize| -> Result<u32> {
        // Runtime group one begins at44 and omits its leading four-byte row ID.
        let at = 4 + runtime_offset
            .checked_sub(0x44)
            .context("normal runtime field underflow")?;
        Ok(u32::from_be_bytes(
            normal
                .get(at..at + 4)
                .context("normal weapon field truncated")?
                .try_into()?,
        ))
    };
    let mut normal_values_bits = [0; 10];
    for (index, value) in normal_values_bits.iter_mut().enumerate() {
        *value = word(0x54 + index * 4)?;
    }
    let modifier_bits = [word(0x7c)?, word(0x88)?, word(0x80)?, word(0x84)?];
    Ok(Some(DefaultMetadata {
        inventory,
        native_sorted_index,
        byte_8_raw: primary.row(inventory.source_index)?[8],
        normal_values_bits,
        modifier_bits,
    }))
}

#[cfg(test)]
#[path = "tests/default.rs"]
mod tests;
