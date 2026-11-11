//! Initial caller on the same retained PlayerNew. Target availability queries
//! use actual component wrappers; mapping itself creates no animation controls.
use super::{Owned as Player, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    actions::{self, Target},
    world::World,
};
pub(super) struct Owned {
    pub state: actions::Pending,
    pub mappings: Vec<ge_assets::weapon_static::Remap>,
}
pub(super) fn initialize(
    player: &mut Player,
    world: &World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    if player.flag.is_none() {
        return Ok(());
    }
    hook.ensure_current()?;
    ensure!(
        player.action.is_none(),
        "initial action callback was already entered"
    );
    player.action = Some(Owned {
        state: actions::Pending::begin(&player.base, &mut player.reset)?,
        mappings: Vec::new(),
    });
    let owner = player
        .action
        .as_mut()
        .context("initial action owner is absent")?;
    owner.state.bind_components(
        &player.components.preparation,
        world.item(
            player
                .base
                .item()
                .context("initial action item is absent")?,
        )?,
    )?;
    owner
        .state
        .bind_inventory(&player.base, player.prelude.inventory())?;
    while let Some(request) = owner.state.next_remap()? {
        hook.ensure_current()?;
        let (animator, source) = if request.target == Target::FirstPerson {
            (
                player
                    .components
                    .preparation
                    .first_person()
                    .context("initial action FP is absent")?
                    .animator(),
                player
                    .components
                    .first_person
                    .component_source
                    .as_ref()
                    .context("initial action FP wrapper is absent")?,
            )
        } else {
            (
                player
                    .components
                    .preparation
                    .animator()
                    .context("initial action TP is absent")?,
                player
                    .components
                    .component_source
                    .as_ref()
                    .context("initial action TP wrapper is absent")?,
            )
        };
        ensure!(
            animator.identity() == request.component
                && animator
                    .component()
                    .is_some_and(|binding| binding.file == source.file()),
            "initial action map availability receiver differs"
        );
        let mapped = player
            .loadout
            .map_initial_action(request, |mode| source.mode_available(mode))?;
        let (output, accepted) = (mapped.output, mapped.mapped);
        owner.mappings.push(mapped);
        owner.state.mapped(request, output, accepted)?;
        hook.ensure_current()?;
    }
    hook.capture_initial_tp(&mut owner.state, &mut player.components.preparation)?;
    // Actual captured control lists precede TP mode selection. Mode histories,
    // callback return, FP dispatch and the second generic request still follow.
    Ok(())
}
