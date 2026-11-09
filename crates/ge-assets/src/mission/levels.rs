//! Group-zero level rows, preserving all words without inventing field types.
use super::{CATALOG_RESOURCE, PRIMARY_SHEET, SECONDARY_SHEET};
use crate::{edb::Edb, spreadsheet};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub primary: usize,
    pub secondary: usize,
}

/// Source offsets identify the retained input rows without exporting their data.
#[derive(Debug, Clone, Copy)]
pub struct RowLocations {
    pub primary_first_row: usize,
    pub secondary_first_row: usize,
}

#[derive(Debug)]
pub struct LevelCatalog {
    primary: Vec<[u32; 45]>,
    secondary: Vec<[u32; 29]>,
    locations: RowLocations,
}

impl LevelCatalog {
    pub fn read(edb: &Edb, budget: Budget) -> Result<Self> {
        ensure!(
            edb.header.hash == CATALOG_RESOURCE && edb.header.version == 263,
            "mission catalog requires the supported o_leveld resource"
        );
        ensure!(
            edb.header.payload_bytes <= 1024 * 1024,
            "mission catalog exceeds host byte budget"
        );
        let (primary, primary_first_row) = rows::<45>(edb, PRIMARY_SHEET, budget.primary)?;
        let (secondary, secondary_first_row) = rows::<29>(edb, SECONDARY_SHEET, budget.secondary)?;
        Ok(Self {
            primary,
            secondary,
            locations: RowLocations {
                primary_first_row,
                secondary_first_row,
            },
        })
    }

    pub fn primary(&self) -> &[[u32; 45]] {
        &self.primary
    }

    pub fn secondary(&self) -> &[[u32; 29]] {
        &self.secondary
    }

    pub fn locations(&self) -> RowLocations {
        self.locations
    }

    pub fn into_rows(self) -> (Vec<[u32; 45]>, Vec<[u32; 29]>) {
        (self.primary, self.secondary)
    }
}

fn rows<const N: usize>(edb: &Edb, hash: u32, maximum: usize) -> Result<(Vec<[u32; N]>, usize)> {
    let sheet = edb
        .spreadsheet(hash)?
        .with_context(|| format!("mission spreadsheet {hash:#010x} is absent"))?;
    let stride = N.checked_mul(4).context("mission row stride overflow")?;
    let group = sheet
        .group(
            0,
            stride,
            spreadsheet::Budget {
                group_fields: 1,
                rows: maximum,
                row_bytes: stride,
            },
        )?
        .with_context(|| format!("mission spreadsheet {hash:#010x} group zero is absent"))?;
    let first_row = group
        .block_address
        .checked_add(4)
        .context("mission first-row address overflow")?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(group.row_count)
        .context("allocating bounded mission rows")?;
    for index in 0..group.row_count {
        result.push(group.words::<N>(index)?);
    }
    Ok((result, first_row))
}

#[cfg(test)]
#[path = "tests/levels.rs"]
mod tests;
