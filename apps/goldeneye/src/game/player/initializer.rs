//! Enter the real initializer's shared cache section only after component
//! helper success. The preload flag is observed, never cleared to skip work.
use super::{Owned, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    constructor::initializer_prefix::{Bindings, Pending},
    transform_cache,
    world::World,
};

pub(super) fn initialize(
    player: &mut Owned,
    world: &mut World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let Some(component) = player
        .component_tail
        .as_ref()
        .filter(|tail| tail.helper_returned())
    else {
        return Ok(());
    };
    ensure!(
        player.initializer_prefix.is_none() && player.completed_component_evidence.is_none(),
        "initializer prefix entry replays"
    );
    player.completed_component_evidence =
        Some(super::evidence::capture_component_tail(player, world)?);
    player.initializer_prefix = Some(Pending::begin(component)?);
    let item = world.item_mut(player.base.item().context("initializer item absent")?)?;
    hook.with_player_transform_caches(|caches, shared| {
        player
            .initializer_prefix
            .as_mut()
            .expect("retained initializer prefix")
            .advance(
                Bindings {
                    base: &mut player.base,
                    reset: &player.reset,
                    preparation: &player.components.preparation,
                    item,
                },
                caches,
                shared,
                transform_cache::Budget::default(),
            )
    })?;
    hook.ensure_current()?;
    Ok(())
}
