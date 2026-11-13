//! Continue the enclosing constructor after the actual initial callback returns.
//! Metadata reads use the retained selected row; no script resource is requested
//! on a cleanup branch that returns before the original resource lookup.
use super::{Owned, loadout, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    constructor::continuation::{MetadataField, MetadataSource, Pending},
    inventory::Inventory,
    world::World,
};

struct Metadata<'a> {
    sources: &'a loadout::Sources,
    inventory: &'a Inventory,
}
impl MetadataSource for Metadata<'_> {
    fn word(&mut self, slot: u8, field: MetadataField) -> Result<u32> {
        let row = self
            .sources
            .metadata_at_selected_slot(slot, self.inventory)?;
        Ok(match field {
            MetadataField::Type => row.type_key,
            MetadataField::InventoryKey => row.inventory_key,
            MetadataField::PrimaryKey => row.primary_key,
        })
    }
}

pub(super) fn initialize(
    player: &mut Owned,
    world: &World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let Some(callback) = player
        .repeat_action
        .as_ref()
        .and_then(|r| r.callback_tail.as_ref())
        .filter(|tail| tail.callback_returned())
    else {
        return Ok(());
    };
    ensure!(
        player.constructor_tail.is_none() && player.completed_callback_evidence.is_none(),
        "constructor continuation entry replays"
    );
    // Preserve the callback receipt before the enclosing constructor mutates
    // the live owners. The snapshot does not represent constructor completion.
    player.completed_callback_evidence =
        Some(super::evidence::capture_callback_tail(player, world)?);
    player.constructor_tail = Some(Pending::begin(
        callback,
        player
            .callback_entry
            .context("constructor callback entry was not captured")?,
    )?);
    let mut metadata = Metadata {
        sources: &player.loadout,
        inventory: player.prelude.inventory(),
    };
    player
        .constructor_tail
        .as_mut()
        .expect("retained constructor continuation")
        .advance(
            &mut player.base,
            &mut player.reset,
            &player.prelude,
            &mut metadata,
        )?;
    hook.ensure_current()?;
    Ok(())
}
