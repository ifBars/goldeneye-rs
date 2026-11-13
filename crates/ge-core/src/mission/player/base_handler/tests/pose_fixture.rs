use super::*;
impl Owner {
    pub(crate) fn pose_fixture(player: PlayerId, item: ItemId, entry: u8) -> Self {
        let mut owner = super::callback_fixture::owner(player, Some(item));
        owner.b(0x702, entry);
        owner.b(0x618, 1);
        owner.w(0x98c, 0);
        // Deliberately stale numeric storage must never replace CharacterBinding.
        owner.w(0x6e0, 0);
        owner
    }
}
