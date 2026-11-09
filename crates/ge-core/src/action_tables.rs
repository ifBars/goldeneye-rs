//! Ordered action-table initialization from caller-supplied spreadsheet rows.
//! This binds the observed references; it does not execute actions, initialize
//! other common managers, or claim that the complete boot work queue is ready.
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub actions: usize,
    pub composites: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRow {
    /// All five source words in their original order. Only word zero is used
    /// as the action lookup hash by this initializer.
    pub words_raw: [u32; 5],
}

impl ActionRow {
    pub fn hash(&self) -> u32 {
        self.words_raw[0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingSlot {
    pub action_hash: u32,
    /// First matching source action row; absent matches stay native-null.
    pub action_index: Option<usize>,
    pub word1_raw: u32,
    /// Preserve unused upper bits instead of discarding them during binding.
    pub word2_raw: u32,
}

impl BindingSlot {
    pub fn low_byte(&self) -> u8 {
        self.word2_raw as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeRow {
    pub id: u32,
    pub words_raw: [u32; 13],
    pub slots: [BindingSlot; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionTables {
    actions: Vec<ActionRow>,
    composites: Vec<CompositeRow>,
}

impl ActionTables {
    /// Copy complete records in storage order, then bind each of the four
    /// references by a storage-first linear search. Duplicate hashes, duplicate
    /// composite IDs, zero hashes and unresolved references are all preserved.
    /// Budgets are explicit host limits, not asserted native content limits.
    pub fn bind(
        actions: Vec<[u32; 5]>,
        composites: Vec<[u32; 13]>,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            actions.len() <= budget.actions,
            "action row host budget exhausted"
        );
        ensure!(
            composites.len() <= budget.composites,
            "action composite host budget exhausted"
        );
        let actions: Vec<_> = actions
            .into_iter()
            .map(|words_raw| ActionRow { words_raw })
            .collect();
        let composites = composites
            .into_iter()
            .map(|words_raw| {
                let slots = std::array::from_fn(|slot| {
                    let start = 1 + slot * 3;
                    let action_hash = words_raw[start];
                    BindingSlot {
                        action_hash,
                        action_index: actions
                            .iter()
                            .position(|action| action.hash() == action_hash),
                        word1_raw: words_raw[start + 1],
                        word2_raw: words_raw[start + 2],
                    }
                });
                CompositeRow {
                    id: words_raw[0],
                    words_raw,
                    slots,
                }
            })
            .collect();
        Ok(Self {
            actions,
            composites,
        })
    }

    pub fn actions(&self) -> &[ActionRow] {
        &self.actions
    }

    pub fn composites(&self) -> &[CompositeRow] {
        &self.composites
    }
}

#[cfg(test)]
#[path = "tests/action_tables.rs"]
mod tests;
