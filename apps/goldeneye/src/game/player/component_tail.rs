//! Finish only the component helper after the pose helper returns. Full player
//! construction and its later publication remain separate operations.
use super::{Owned as Player, pose::Sources, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    player::{constructor::component_tail::Pending, world::World},
    session::Session,
};

pub(super) fn initialize(
    player: &mut Player,
    session: &Session,
    world: &mut World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let Some(pose) = player
        .pose_setup
        .as_ref()
        .filter(|pose| pose.state.helper_returned())
    else {
        return Ok(());
    };
    ensure!(
        player.component_tail.is_none() && player.completed_pose_evidence.is_none(),
        "component tail entry replays"
    );
    player.completed_pose_evidence = Some(super::evidence::capture_pose_setup(player, world)?);
    player.component_tail = Some(Pending::begin(&pose.state)?);
    let mut source = Sources {
        loadout: &player.loadout,
        session,
        sessionlookups: &mut player
            .pose_setup
            .as_mut()
            .expect("pose owner")
            .sessionlookups,
        hook,
    };
    player
        .component_tail
        .as_mut()
        .expect("retained component tail")
        .advance(
            &player.base,
            &player.reset,
            world.item_mut(player.base.item().context("component tail item absent")?)?,
            &mut source,
        )?;
    hook.ensure_current()?;
    Ok(())
}
