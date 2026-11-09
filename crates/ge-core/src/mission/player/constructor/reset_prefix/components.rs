//! Writes by the component initializer to the same retained PlayerNew fields.
use super::{PendingReset, Stage};
use crate::mission::player::inventory::Inventory;
use anyhow::{Result, ensure};

impl PendingReset {
    pub(crate) fn clear_component_entry(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::Complete,
            "component entry precedes reset completion"
        );
        for offset in 0xfec..0x1150 {
            self.bytes.insert(offset, 0);
        }
        Ok(())
    }
    pub(crate) fn clear_component_fields(&mut self) {
        for offset in [0xafc, 0xb00, 0xaf8, 0x70c, 0x710, 0x714, 0x718] {
            self.word_store(offset, 0);
        }
    }
    pub(crate) fn retain_component_slots(&mut self, inventory: &Inventory) -> Result<()> {
        let slots = inventory.slots();
        for (offset, value) in [(0xc55, slots.c55), (0xc96, slots.c96), (0xc97, slots.c97)] {
            ensure!(value.is_some(), "component slot transition is incomplete");
            self.bytes.insert(offset, value.expect("checked slot"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/component_entry.rs"]
mod tests;
