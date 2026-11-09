//! Source level-table registration and lazy selection, independent of I/O.
//!
//! Input words remain opaque except for source-proven getters. Registration
//! sorts unsigned hashes and rejects duplicate arrivals without replacement.
//! Selection intentionally retains a prior tuning selection when no primary
//! record exists for the requested hash.
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Difficulty {
    Operative = 0,
    Agent = 1,
    DoubleOSeven = 2,
    Classic = 3,
}

impl Difficulty {
    pub const fn value(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Operative => "Operative",
            Self::Agent => "Agent",
            Self::DoubleOSeven => "007",
            Self::Classic => "007 Classic",
        }
    }
}

impl TryFrom<u8> for Difficulty {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Operative),
            1 => Ok(Self::Agent),
            2 => Ok(Self::DoubleOSeven),
            3 => Ok(Self::Classic),
            _ => anyhow::bail!("unsupported mission difficulty {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Limits input records, including duplicate arrivals.
    pub primary: usize,
    pub secondary: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateRow {
    pub table: TableKind,
    pub hash: u32,
    pub source_index: usize,
    pub retained_source_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelRow {
    source_index: usize,
    words_raw: [u32; 45],
}

impl LevelRow {
    pub fn hash(&self) -> u32 {
        self.words_raw[0]
    }

    pub fn source_index(&self) -> usize {
        self.source_index
    }

    pub fn words_raw(&self) -> &[u32; 45] {
        &self.words_raw
    }

    /// Source nonmultiplayer character-selection input at primary row +0x78.
    /// Invalid keys and previous/default character history remain caller work.
    pub fn character_request(&self) -> u32 {
        self.words_raw[0x78 / 4]
    }

    /// Raw movie selector at +0x2c. Zero skips the observed intro-movie request.
    pub fn intro_movie_hash(&self) -> u32 {
        self.words_raw[0x2c / 4]
    }

    /// Raw movie selector at +0x30, used while leaving the prior selected level.
    pub fn outro_movie_hash(&self) -> u32 {
        self.words_raw[0x30 / 4]
    }

    /// Primary and two optional bank selectors at +0x50/+0x54/+0x58. Raw zero
    /// and 0xffffffff remain distinguishable; no universal null rule is added.
    pub fn bank_hashes(&self) -> [u32; 3] {
        [self.words_raw[20], self.words_raw[21], self.words_raw[22]]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuningRow {
    source_index: usize,
    words_raw: [u32; 29],
}

impl TuningRow {
    pub fn hash(&self) -> u32 {
        self.words_raw[0]
    }

    pub fn source_index(&self) -> usize {
        self.source_index
    }

    pub fn words_raw(&self) -> &[u32; 29] {
        &self.words_raw
    }
}

/// Explicit initial state avoids treating unverified native initialization as
/// a zero/default selection. Selected row hashes must exist in the catalog;
/// they need not match current_hash, allowing independently retained state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitialSelection {
    pub current_hash: u32,
    pub previous_hash: u32,
    pub primary_hash: Option<u32>,
    pub secondary_hash: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOutcome {
    /// The secondary selection was not addressed or changed.
    PrimaryMissing,
    Selected {
        has_tuning: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelRegistry {
    primary: Vec<LevelRow>,
    secondary: Vec<TuningRow>,
    duplicates: Vec<DuplicateRow>,
    current_hash: u32,
    previous_hash: u32,
    selected_primary: Option<usize>,
    selected_secondary: Option<usize>,
}

impl LevelRegistry {
    pub fn new(
        primary: Vec<[u32; 45]>,
        secondary: Vec<[u32; 29]>,
        budget: Budget,
        initial: InitialSelection,
    ) -> Result<Self> {
        ensure!(
            primary.len() <= budget.primary,
            "mission primary row budget exhausted"
        );
        ensure!(
            secondary.len() <= budget.secondary,
            "mission tuning row budget exhausted"
        );
        let mut primary_rows: Vec<LevelRow> = Vec::with_capacity(primary.len());
        let mut secondary_rows: Vec<TuningRow> = Vec::with_capacity(secondary.len());
        let mut duplicates = Vec::new();
        for (source_index, words_raw) in primary.into_iter().enumerate() {
            match primary_rows.binary_search_by_key(&words_raw[0], LevelRow::hash) {
                Ok(index) => duplicates.push(DuplicateRow {
                    table: TableKind::Primary,
                    hash: words_raw[0],
                    source_index,
                    retained_source_index: primary_rows[index].source_index,
                }),
                Err(index) => primary_rows.insert(
                    index,
                    LevelRow {
                        source_index,
                        words_raw,
                    },
                ),
            }
        }
        for (source_index, words_raw) in secondary.into_iter().enumerate() {
            match secondary_rows.binary_search_by_key(&words_raw[0], TuningRow::hash) {
                Ok(index) => duplicates.push(DuplicateRow {
                    table: TableKind::Secondary,
                    hash: words_raw[0],
                    source_index,
                    retained_source_index: secondary_rows[index].source_index,
                }),
                Err(index) => secondary_rows.insert(
                    index,
                    TuningRow {
                        source_index,
                        words_raw,
                    },
                ),
            }
        }
        let selected_primary = initial
            .primary_hash
            .map(|hash| {
                primary_rows
                    .binary_search_by_key(&hash, LevelRow::hash)
                    .ok()
                    .context("initial mission primary row is absent")
            })
            .transpose()?;
        let selected_secondary = initial
            .secondary_hash
            .map(|hash| {
                secondary_rows
                    .binary_search_by_key(&hash, TuningRow::hash)
                    .ok()
                    .context("initial mission tuning row is absent")
            })
            .transpose()?;
        Ok(Self {
            primary: primary_rows,
            secondary: secondary_rows,
            duplicates,
            current_hash: initial.current_hash,
            previous_hash: initial.previous_hash,
            selected_primary,
            selected_secondary,
        })
    }

    pub fn primary(&self) -> &[LevelRow] {
        &self.primary
    }

    pub fn secondary(&self) -> &[TuningRow] {
        &self.secondary
    }

    pub fn duplicates(&self) -> &[DuplicateRow] {
        &self.duplicates
    }

    pub fn current_hash(&self) -> u32 {
        self.current_hash
    }

    pub fn previous_hash(&self) -> u32 {
        self.previous_hash
    }

    pub fn selected_primary(&self) -> Option<&LevelRow> {
        self.selected_primary.map(|index| &self.primary[index])
    }

    pub fn selected_secondary(&self) -> Option<&TuningRow> {
        self.selected_secondary.map(|index| &self.secondary[index])
    }

    pub fn select(&mut self, hash: u32) -> SelectionOutcome {
        if self.current_hash != u32::MAX {
            self.previous_hash = self.current_hash;
        }
        self.current_hash = hash;
        self.selected_primary = self
            .primary
            .binary_search_by_key(&hash, LevelRow::hash)
            .ok();
        if self.selected_primary.is_none() {
            return SelectionOutcome::PrimaryMissing;
        }
        self.selected_secondary = self
            .secondary
            .binary_search_by_key(&hash, TuningRow::hash)
            .ok();
        SelectionOutcome::Selected {
            has_tuning: self.selected_secondary.is_some(),
        }
    }
}

#[cfg(test)]
#[path = "tests/catalog.rs"]
mod tests;
