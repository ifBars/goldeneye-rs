//! Source groups used by the actor-definition registry, not its mutable cursors,
//! constructed defaults, supplied-DOL special rows or complete manager state.
use crate::{edb::Edb, spreadsheet};
use anyhow::{Context, Result, ensure};

pub const RESOURCE: u32 = 0x0109_0013;
pub const SHEET: u32 = 0x1400_0021;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub primary_rows: usize,
    pub secondary_rows: usize,
    /// Aggregate row words across both present groups; excludes metadata.
    pub total_words: usize,
    /// Aggregate row bytes across both present groups; excludes metadata.
    pub total_bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            primary_rows: 65_536,
            secondary_rows: 65_536,
            total_words: 1_507_328,
            total_bytes: 6_029_312,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupLocator {
    pub source_group: usize,
    pub pointer_field: usize,
    pub relative_raw: i32,
    pub block_address: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row<const N: usize> {
    words: [u32; N],
    source_index: usize,
    source_address: usize,
    source_group: usize,
}

pub type PrimaryRow = Row<17>;
pub type SecondaryRow = Row<6>;

impl<const N: usize> Row<N> {
    pub fn key(&self) -> u32 {
        self.words[0]
    }
    /// Fixed domain-specific row, with no arbitrary byte/address access.
    pub fn words(&self) -> &[u32; N] {
        &self.words
    }
    pub fn source_index(&self) -> usize {
        self.source_index
    }
    pub fn source_address(&self) -> usize {
        self.source_address
    }
    pub fn source_group(&self) -> usize {
        self.source_group
    }
}

impl PrimaryRow {
    /// Raw word consumed by the source primary runtime-group builder. This is
    /// distinct from spreadsheet group1's opaque six-word rows.
    pub fn group_word(&self) -> u32 {
        self.words[1]
    }
}

#[derive(Debug)]
pub struct Definitions<'a> {
    primary: Option<spreadsheet::Rows<'a>>,
    secondary: Option<spreadsheet::Rows<'a>>,
}

impl<'a> Definitions<'a> {
    pub fn read(edb: &'a Edb, budget: Budget) -> Result<Self> {
        ensure!(
            edb.header.hash == RESOURCE && edb.header.version == 263,
            "actor definitions require supported resource01090013/version263"
        );
        let sheet = edb
            .spreadsheet(SHEET)?
            .context("actor definitions spreadsheet is absent")?;
        let primary = sheet.group(
            0,
            0x44,
            spreadsheet::Budget {
                group_fields: 2,
                rows: budget.primary_rows,
                row_bytes: 0x44,
            },
        )?;
        let secondary = sheet.group(
            1,
            0x18,
            spreadsheet::Budget {
                group_fields: 2,
                rows: budget.secondary_rows,
                row_bytes: 0x18,
            },
        )?;
        let words = primary
            .as_ref()
            .map_or(0, |group| group.row_count)
            .checked_mul(17)
            .and_then(|count| {
                secondary
                    .as_ref()
                    .map_or(0, |group| group.row_count)
                    .checked_mul(6)
                    .and_then(|other| count.checked_add(other))
            })
            .context("actor definition aggregate word overflow")?;
        ensure!(
            words <= budget.total_words,
            "actor definition aggregate word budget exhausted"
        );
        let bytes = words
            .checked_mul(4)
            .context("actor definition aggregate byte overflow")?;
        ensure!(
            bytes <= budget.total_bytes,
            "actor definition aggregate byte budget exhausted"
        );
        Ok(Self { primary, secondary })
    }

    /// None is a missing group pointer; Some(0) is a present empty group. No
    /// default row, safe runtime-group element or complete manager is inferred.
    pub fn primary_count(&self) -> Option<usize> {
        self.primary.map(|group| group.row_count)
    }
    pub fn secondary_count(&self) -> Option<usize> {
        self.secondary.map(|group| group.row_count)
    }
    pub fn primary_locator(&self) -> Option<GroupLocator> {
        self.primary.map(|group| locator(group, 0))
    }
    pub fn secondary_locator(&self) -> Option<GroupLocator> {
        self.secondary.map(|group| locator(group, 1))
    }

    /// Original rows including duplicates, in source order. Registry insertion
    /// and first-duplicate retention are a separate pure owner responsibility.
    pub fn primary(&self, index: usize) -> Result<PrimaryRow> {
        row(
            self.primary
                .context("actor definitions primary group is absent")?,
            index,
            0,
        )
    }
    pub fn secondary(&self, index: usize) -> Result<SecondaryRow> {
        row(
            self.secondary
                .context("actor definitions secondary group is absent")?,
            index,
            1,
        )
    }
}

fn locator(group: spreadsheet::Rows<'_>, source_group: usize) -> GroupLocator {
    GroupLocator {
        source_group,
        pointer_field: group.pointer_field,
        relative_raw: group.relative_raw,
        block_address: group.block_address,
    }
}

fn row<const N: usize>(
    group: spreadsheet::Rows<'_>,
    index: usize,
    source_group: usize,
) -> Result<Row<N>> {
    let words = group.words::<N>(index)?;
    let source_address = index
        .checked_mul(group.row_stride)
        .and_then(|offset| group.block_address.checked_add(4)?.checked_add(offset))
        .context("actor definition source row address overflow")?;
    Ok(Row {
        words,
        source_index: index,
        source_address,
        source_group,
    })
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
