use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{components::Registry, world::World};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttachmentBinding {
    pub identity_registered: bool,
    pub item_backlink_bound: bool,
    pub item_list_member: bool,
    pub item_list_ordinal: usize,
    pub item_components: usize,
    pub registry_identities: usize,
    pub component_flag_8e: u8,
    pub first_node_links_null: bool,
    pub state_sha256: String,
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    identities: &Registry,
) -> Result<AttachmentBinding> {
    let preparation = &player.components.preparation;
    let animator = preparation
        .animator()
        .context("attached TP animator is absent")?;
    let attachment = preparation
        .attachment()
        .context("TP attachment return is absent")?;
    let item = world.item(attachment.item())?;
    ensure!(
        attachment.component() == animator.identity()
            && attachment.component().player() == player.base.player()
            && identities.resolve(animator.identity().id()) == Some(animator.identity())
            && animator.item() == Some(item.id())
            && player.base.item() == Some(item.id())
            && item.handler() == Some(animator.player())
            && item.components() == [animator.identity().id()]
            && attachment.ordinal() == 0
            && animator.word(8).is_none()
            && animator.byte(0x8e) == Some(0)
            && animator.word(0) == Some(0)
            && animator.word(4) == Some(0),
        "TP attachment owners differ"
    );
    Ok(AttachmentBinding {
        identity_registered: true,
        item_backlink_bound: true,
        item_list_member: true,
        item_list_ordinal: attachment.ordinal(),
        item_components: item.components().len(),
        registry_identities: identities.len(),
        component_flag_8e: 0,
        first_node_links_null: true,
        state_sha256: super::super::words(b"ge_player_tp_attachment_v1\0", |emit| {
            attachment.visit_state_words(emit)
        }),
    })
}
