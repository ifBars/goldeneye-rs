//! Source-ordered continuation after actual shared scratch preparation.
use super::{PendingReset, Stage};
use crate::mission::player::{constructor::ScratchReceipt, inventory::Inventory};
use anyhow::{Result, ensure};

/// Semantic binding to the retained character catalog row, never its address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterBinding {
    pub registry_resource: u32,
    pub key: u32,
    pub source_index: usize,
}

impl PendingReset {
    pub fn after_scratch(&mut self, receipt: &ScratchReceipt) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingScratch,
            "scratch continuation is out of order"
        );
        ensure!(
            receipt.capacity_after > 0,
            "scratch reset has no owned capacity"
        );
        // This store precedes the previous-player/character selector branch.
        self.bytes.insert(0xe58, 1);
        self.stage = Stage::SelectingCharacter;
        Ok(())
    }
    pub fn bind_character(&mut self, character: CharacterBinding) -> Result<()> {
        ensure!(
            self.stage == Stage::SelectingCharacter,
            "character binding is out of order"
        );
        ensure!(
            character.registry_resource == 0x0109_0013
                && character.key >> 16 == 0x4d00
                && character.key & 0xffff != 0,
            "unsupported character catalog binding"
        );
        // The native +6e0 pointer is represented by its actual owner/row identity.
        self.character = Some(character);
        self.stage = Stage::CharacterBound;
        Ok(())
    }
    pub fn begin_inventory(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::CharacterBound,
            "inventory reset is out of order"
        );
        self.bytes.insert(0xc56, 0);
        self.bytes.insert(0xc57, 0);
        self.stage = Stage::SelectingInventory;
        Ok(())
    }
    pub fn record_inventory(&mut self, inventory: &Inventory) -> Result<()> {
        ensure!(
            self.stage == Stage::SelectingInventory,
            "inventory selection is out of order"
        );
        let selected = inventory
            .selection()
            .ok_or_else(|| anyhow::anyhow!("inventory selector has not completed"))?;
        let slots = inventory.slots();
        let fields = [
            slots.c96, slots.c97, slots.c98, slots.c99, slots.c55, slots.ca0,
        ];
        ensure!(
            fields.iter().all(Option::is_some) && slots.pose_bits == Some(selected.pose_bits),
            "inventory selector has incomplete retained stores"
        );
        for (offset, value) in [0xc96, 0xc97, 0xc98, 0xc99, 0xc55, 0xca0]
            .into_iter()
            .zip(fields)
        {
            self.bytes
                .insert(offset, value.expect("all fields checked"));
        }
        for (index, bits) in selected.pose_bits.into_iter().enumerate() {
            self.word_store(0xcb4 + index as u16 * 4, bits);
        }
        self.stage = Stage::InventorySelected;
        Ok(())
    }
}
