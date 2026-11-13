//! Actual layer membership driven by owned host node allocations. Execution is
//! incremental like the source; errors latch failure and require discarding the
//! state/host attempt. This is not child ticking, pose sampling or camera output.
mod capture;
pub mod command;
mod execute;
mod types;
pub use types::{Budget, Host, NodeId, Slot, SlotOverride, Snapshot};

use anyhow::{Result, ensure};
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct State {
    slots: [Slot; 4],
    active_count: u8,
    selection_mask: u8,
    clear_mask: u8,
    budget: Budget,
    work: Work,
    identities: BTreeSet<NodeId>,
    created: Vec<NodeId>,
    failed: bool,
}

#[derive(Debug, Default)]
struct Work {
    nodes: usize,
    commands: usize,
    groups: usize,
    children: usize,
}

impl State {
    /// Explicit empty-list snapshot. It cannot restore opaque existing lists.
    pub fn from_empty_snapshot(snapshot: Snapshot, budget: Budget) -> Result<Self> {
        ensure!(
            snapshot.active_count <= 4,
            "animation active slot count exceeds physical slots"
        );
        Ok(Self {
            slots: std::array::from_fn(|i| Slot {
                nodes: Vec::new(),
                weight_bits: snapshot.weight_bits[i],
                flags_raw: snapshot.slot_flags[i],
                bone_mask_words: None,
            }),
            active_count: snapshot.active_count,
            selection_mask: snapshot.selection_mask,
            clear_mask: snapshot.clear_mask,
            budget,
            work: Work::default(),
            identities: BTreeSet::new(),
            created: Vec::new(),
            failed: false,
        })
    }
    pub fn slots(&self) -> &[Slot; 4] {
        &self.slots
    }
    pub(crate) fn set_slot_weight(&mut self, slot: usize, bits: u32) -> Result<()> {
        anyhow::ensure!(slot < self.slots.len(), "weight physical slot exceeds four");
        self.slots[slot].weight_bits = bits;
        Ok(())
    }
    pub(crate) fn set_slot_mask_words(&mut self, slot: usize, words: [u32; 4]) -> Result<()> {
        anyhow::ensure!(
            slot < self.slots.len(),
            "bone mask physical slot exceeds four"
        );
        self.slots[slot].bone_mask_words = Some(words);
        Ok(())
    }
    pub(crate) fn or_slot_flags(&mut self, slot: usize, bits: u16) -> Result<()> {
        anyhow::ensure!(slot < self.slots.len(), "flags physical slot exceeds four");
        self.slots[slot].flags_raw |= bits;
        Ok(())
    }
    pub fn active_count(&self) -> u8 {
        self.active_count
    }
    pub fn selection_mask(&self) -> u8 {
        self.selection_mask
    }
    pub fn clear_mask(&self) -> u8 {
        self.clear_mask
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    /// Historical actual allocations, including detached controls or later
    /// destroyed nodes. This is not a count of current live membership.
    pub fn created_nodes(&self) -> &[NodeId] {
        &self.created
    }
}

#[cfg(test)]
mod tests;
