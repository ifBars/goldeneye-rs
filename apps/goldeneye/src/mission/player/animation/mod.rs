//! Retained animation planning against the actual first-person component owner.
mod evidence;
mod provider;

use anyhow::Result;
pub(crate) use evidence::Evidence;
use ge_assets::edb::Edb;
use ge_core::animation::mode::{self, Inputs, Selection};

pub(super) struct PreparedAnimation {
    selection: Selection,
    evidence: Evidence,
}

impl PreparedAnimation {
    pub fn prepare(component: &Edb, requested: u32, previous: u32, fallback: u32) -> Result<Self> {
        let budget = mode::Budget {
            lookups: 3,
            transition_rows: 65_536,
            commands: 4096,
            groups: 4096,
            children: 16_384,
        };
        let mut source = provider::Source::new(component.animation_modes()?, budget);
        let selection = mode::select(
            &mut source,
            Inputs {
                requested,
                previous,
                fallback,
                resource_available: true,
            },
            budget,
        )?;
        let evidence = Evidence::from_selection(&selection);
        Ok(Self {
            selection,
            evidence,
        })
    }
    pub fn evidence(&self) -> &Evidence {
        debug_assert_eq!(self.selection.return_word, self.evidence.return_word);
        debug_assert_eq!(self.selection.attempts.len(), self.evidence.attempts);
        &self.evidence
    }
    pub(super) fn selection(&self) -> &Selection {
        &self.selection
    }
}

#[cfg(test)]
mod tests;
