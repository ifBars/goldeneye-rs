use super::super::{base_handler::BasicReceipt, identity::PlayerId, inventory::Inventory};
use crate::mission::world_hooks::buffer::Buffer;

#[cfg(test)]
#[path = "tests/continuation_prelude_fixture.rs"]
mod continuation_fixture;

/// The derived constructor prelude after an actual base-constructor receipt.
/// This is neither a completed reset nor an initialized PlayerNew.
#[derive(Debug)]
pub struct PendingPrelude {
    player: PlayerId,
    words: [u32; 32],
    local: Buffer,
    inventory: Inventory,
}
impl PendingPrelude {
    pub fn after_basic(receipt: &BasicReceipt) -> Self {
        // b04..b63 are source memzero. b64..b80 are explicit scalar stores.
        let mut words = [0; 32];
        words[24..].copy_from_slice(&[
            0x3f00_0000,
            0x3f00_0000,
            0x3f00_0000,
            0x3f80_0000,
            0x3f00_0000,
            0x3da3_d70a,
            0,
            0,
        ]);
        Self {
            player: receipt.player(),
            words,
            local: Buffer::empty(0),
            inventory: Inventory::from_constructor(),
        }
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn word(&self, offset: usize) -> Option<u32> {
        let delta = offset.checked_sub(0xb04)?;
        if !delta.is_multiple_of(4) {
            return None;
        }
        self.words.get(delta / 4).copied()
    }
    pub fn local_buffer(&self) -> &Buffer {
        &self.local
    }
    pub(super) fn clear_local_buffer(&mut self) -> crate::mission::world_hooks::buffer::ClearStats {
        self.local.clear_backing()
    }
    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }
    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }
    pub fn visit_state_words(&self, mut visit: impl FnMut(u32)) {
        visit(2);
        visit((self.player.get() >> 32) as u32);
        visit(self.player.get() as u32);
        // Semantic derived/interface kinds, two source-null 9c0/9c4 fields.
        visit(1);
        visit(1);
        visit(1);
        visit(0);
        visit(0);
        visit(self.words.len() as u32);
        for word in self.words {
            visit(word);
        }
        let buffer = &self.local;
        visit(buffer.capacity());
        visit(buffer.growth());
        visit(buffer.count());
        visit(u32::from(buffer.has_backing()));
        for value in [buffer.backing_len(), buffer.backing_capacity()] {
            visit((value as u64 >> 32) as u32);
            visit(value as u32);
        }
        if let Some(bytes) = buffer.backing() {
            for byte in bytes {
                visit(u32::from(*byte));
            }
        }
        self.inventory.visit_state_words(visit);
    }
}
