//! Character group-one rows; visual-variant group zero has a different layout.
use crate::{edb::Edb, spreadsheet};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, btree_map::Entry};

pub const CATALOG_RESOURCE: u32 = 0x0109_0013;
pub const CHARACTER_SHEET: u32 = 0x1400_0021;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub rows: usize,
}

#[derive(Debug, Clone)]
pub struct CharacterRow {
    words: [u32; 6],
    source_index: usize,
    source_address: usize,
}

impl CharacterRow {
    pub fn key(&self) -> u32 {
        self.words[0]
    }
    pub fn fp_skin_file(&self) -> u32 {
        self.words[1]
    }
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn source_address(&self) -> usize {
        self.source_address
    }
    pub fn words_raw(&self) -> &[u32; 6] {
        &self.words
    }
}

#[derive(Debug)]
pub struct CharacterCatalog {
    rows: Vec<CharacterRow>,
    by_key: BTreeMap<u32, usize>,
    duplicate_count: usize,
}

impl CharacterCatalog {
    pub fn read(edb: &Edb, budget: Budget) -> Result<Self> {
        ensure!(
            edb.header.hash == CATALOG_RESOURCE && edb.header.version == 263,
            "character catalog requires supported o_chrctr resource"
        );
        let sheet = edb
            .spreadsheet(CHARACTER_SHEET)?
            .context("character spreadsheet is absent")?;
        let group = sheet
            .group(
                1,
                24,
                spreadsheet::Budget {
                    group_fields: 2,
                    rows: budget.rows,
                    row_bytes: 24,
                },
            )?
            .context("character spreadsheet group one is absent")?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(group.row_count)
            .context("allocating bounded character rows")?;
        let mut by_key = BTreeMap::new();
        let mut duplicate_count = 0;
        for source_index in 0..group.row_count {
            let words = group.words::<6>(source_index)?;
            let entry = match by_key.entry(words[0]) {
                Entry::Occupied(_) => {
                    duplicate_count += 1;
                    continue;
                }
                Entry::Vacant(entry) => entry,
            };
            let source_address = source_index
                .checked_mul(24)
                .and_then(|offset| group.block_address.checked_add(4)?.checked_add(offset))
                .context("character source row address overflow")?;
            entry.insert(rows.len());
            rows.push(CharacterRow {
                words,
                source_index,
                source_address,
            });
        }
        Ok(Self {
            rows,
            by_key,
            duplicate_count,
        })
    }

    /// Unique rows in first-arrival source order. Duplicate keys retain their
    /// first row, independently of the unsigned-key lookup index.
    pub fn rows(&self) -> &[CharacterRow] {
        &self.rows
    }
    /// Missing native-default fallback is deliberately not fabricated here.
    pub fn find(&self, key: u32) -> Option<&CharacterRow> {
        self.by_key.get(&key).map(|&index| &self.rows[index])
    }
    pub fn duplicate_count(&self) -> usize {
        self.duplicate_count
    }
}

#[cfg(test)]
#[path = "tests/catalog.rs"]
mod tests;
