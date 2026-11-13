use super::*;

impl PendingPrelude {
    pub(crate) fn continuation_fixture(player: PlayerId, inventory: Inventory) -> Self {
        Self {
            player,
            words: [0; 32],
            local: Buffer::empty(0),
            inventory,
        }
    }
}
