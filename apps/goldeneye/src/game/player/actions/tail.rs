use super::*;
use ge_core::animation::script_child::update::Stage;
pub(super) fn initialize(player: &mut Player, hook: &world_hooks::Attempt) -> Result<()> {
    let action = player
        .action
        .as_mut()
        .context("TP dispatch action absent")?;
    action
        .state
        .begin_third_person_dispatch_tail(&player.reset, &mut player.components.preparation)?;
    let source = player
        .components
        .component_source
        .as_ref()
        .context("TP dispatch wrapper absent")?;
    let id = player.base.player();
    loop {
        hook.ensure_current()?;
        let node = action
            .state
            .begin_third_person_dispatch_update(&mut player.components.preparation)?;
        if let Some(node) = node {
            let mut script = source.shared_script_source();
            player
                .components
                .preparation
                .begin_third_person_script_update(id, node, &mut script)?;
            hook.ensure_current()?;
            let header = player
                .components
                .preparation
                .animator()
                .and_then(|a| a.script_children().get(node))
                .context("TP update script absent")?;
            let mut stage = header
                .updates()
                .last()
                .context("TP script invocation absent")?
                .stage;
            if stage == Stage::AwaitingRate {
                let binding = header.cached().context("TP update script binding absent")?;
                let mut rate = source.script_position_source(binding)?;
                player
                    .components
                    .preparation
                    .rate_third_person_script_update(
                        id,
                        node,
                        &mut rate,
                        hook.animation_rate_inputs(true)?,
                    )?;
                hook.ensure_current()?;
                stage = player
                    .components
                    .preparation
                    .animator()
                    .and_then(|a| a.script_children().get(node))
                    .and_then(|h| h.updates().last())
                    .context("TP updated rate absent")?
                    .stage;
                if stage == Stage::AwaitingSelection {
                    let mut select = source.phase_zero_source(binding)?;
                    player
                        .components
                        .preparation
                        .select_third_person_script_update(
                            id,
                            node,
                            &mut select,
                            hook.animation_step()?,
                        )?;
                    hook.ensure_current()?;
                    stage = player
                        .components
                        .preparation
                        .animator()
                        .and_then(|a| a.script_children().get(node))
                        .and_then(|h| h.updates().last())
                        .context("TP update selection absent")?
                        .stage;
                }
                if stage == Stage::AwaitingPosition {
                    let mut position = source.script_position_source(binding)?;
                    player
                        .components
                        .preparation
                        .position_third_person_script_update(id, node, &mut position)?;
                    hook.ensure_current()?;
                    stage = player
                        .components
                        .preparation
                        .animator()
                        .and_then(|a| a.script_children().get(node))
                        .and_then(|h| h.updates().last())
                        .context("TP updated position absent")?
                        .stage;
                }
            }
            if stage != Stage::Returned {
                return Ok(());
            }
        }
        action
            .state
            .complete_third_person_dispatch_update(&player.components.preparation)?;
        hook.ensure_current()?;
        if action.state.stage() == actions::Stage::AwaitingFirstPersonDispatch {
            return Ok(());
        }
    }
}
