use super::*;
use ge_core::{
    animation::{layers::NodeId, script_child::update::Stage},
    mission::player::{components::Preparation, identity::PlayerId},
};
pub(super) fn run(
    preparation: &mut Preparation,
    source: &FileBinding,
    player: PlayerId,
    node: NodeId,
    started: &mut bool,
    hook: &world_hooks::Attempt,
) -> Result<bool> {
    if !*started {
        preparation.begin_third_person_script_update(
            player,
            node,
            &mut source.shared_script_source(),
        )?;
        *started = true;
        hook.ensure_current()?;
    }
    let header = preparation
        .animator()
        .and_then(|owner| owner.script_children().get(node))
        .context("repeat seek script absent")?;
    let mut stage = header
        .updates()
        .last()
        .context("repeat seek invocation absent")?
        .stage;
    if stage == Stage::Returned {
        return Ok(true);
    }
    let binding = header
        .cached()
        .context("repeat seek script binding absent")?;
    if stage == Stage::AwaitingRate {
        preparation.rate_third_person_script_update(
            player,
            node,
            &mut source.script_position_source(binding)?,
            hook.animation_rate_inputs(true)?,
        )?;
        hook.ensure_current()?;
        stage = current(preparation, node)?;
    }
    if stage == Stage::AwaitingSelection {
        preparation.select_third_person_script_update(
            player,
            node,
            &mut source.phase_zero_source(binding)?,
            hook.animation_step()?,
        )?;
        hook.ensure_current()?;
        stage = current(preparation, node)?;
    }
    if stage == Stage::AwaitingPosition {
        preparation.position_third_person_script_update(
            player,
            node,
            &mut source.script_position_source(binding)?,
        )?;
        hook.ensure_current()?;
        stage = current(preparation, node)?;
    }
    Ok(stage == Stage::Returned)
}
fn current(preparation: &Preparation, node: NodeId) -> Result<Stage> {
    Ok(preparation
        .animator()
        .and_then(|owner| owner.script_children().get(node))
        .and_then(|header| header.updates().last())
        .context("repeat seek trace absent")?
        .stage)
}
