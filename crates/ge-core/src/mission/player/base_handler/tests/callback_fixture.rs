use super::*;

/// Authored receiver boundary for dispatcher tests. The app integration test
/// covers construction/reset/basic initialization before this boundary.
pub(crate) fn owner(player: PlayerId, item: Option<ItemId>) -> Owner {
    let mut owner = Owner::new(player);
    owner.stage = Stage::VirtualBasicComplete;
    owner.item = item;
    owner.bindings.insert(0x4fc, Binding::EmbeddedInterface);
    owner
}
