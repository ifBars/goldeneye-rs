use super::*;

impl Owner {
    pub(crate) fn callback_tail_fixture(
        player: PlayerId,
        item: ItemId,
        state: Option<u32>,
        flags: Option<u32>,
    ) -> Self {
        let mut owner = Self::new(player);
        owner.stage = Stage::VirtualBasicComplete;
        owner.item = Some(item);
        if let Some(state) = state {
            owner.w(0x704, state);
        }
        if let Some(flags) = flags {
            owner.w(0x220, flags);
        }
        owner.w(0x708, 0x1234_5678);
        owner
    }
}
