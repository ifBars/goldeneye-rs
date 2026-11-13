use super::*;
use ge_core::{
    animation::{layers::command, script_child},
    mission::player::actions::FirstPersonStage,
};
pub(super) fn initialize(player: &mut Player, hook: &world_hooks::Attempt) -> Result<()> {
    let result = run(player, hook);
    if result.is_err()
        && let Some(fp) = player
            .action
            .as_mut()
            .and_then(|a| a.state.first_person_dispatch_mut().ok())
        && fp.stage() != FirstPersonStage::Failed
    {
        fp.fail()?;
    }
    result
}
fn run(player: &mut Player, hook: &world_hooks::Attempt) -> Result<()> {
    hook.ensure_current()?;
    let source = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("FP mode wrapper absent")?;
    let action = player.action.as_mut().context("FP initial action absent")?;
    let owner = player
        .components
        .preparation
        .first_person()
        .context("FP receiver absent")?
        .animator();
    ensure!(
        owner.identity() == action.state.resolved()[0].request.component
            && owner
                .component()
                .is_some_and(|b| b.file == source.file() && b.file == 0x0102_0016),
        "FP mode receiver/source identity differs"
    );
    hook.begin_initial_fp_dispatch(
        &mut action.state,
        &mut player.base,
        &player.reset,
        &mut player.components.preparation,
    )?;
    let fp = action.state.first_person_dispatch_mut()?;
    let mut mode = source.mode_source()?;
    let mut next = fp.advance(&mut mode)?;
    while let Some(invocation) = next {
        hook.ensure_current()?;
        let location = source.mode_command_location(invocation)?;
        action.fp_command = Some(location);
        action.fp_command_stream = Some(ge_assets::animation_modes::CommandStream::new(location));
        let mut body = source.mode_command_source(
            action
                .fp_command_stream
                .as_mut()
                .expect("retained FP stream"),
        )?;
        fp.prepare_command(
            &mut player.components.preparation,
            location.opcode(),
            &mut body,
            hook.animation_slot_override()?,
        )?;
        hook.ensure_current()?;
        if fp.command().is_some_and(|c| {
            matches!(
                c.stage(),
                command::Stage::MaskControlPending | command::Stage::MaskClearPending
            )
        }) {
            hook.finish_initial_fp_mask(fp, &mut player.components.preparation)?;
        }
        while fp
            .command()
            .and_then(|c| c.child())
            .is_some_and(|c| ((c.reference >> 24) & 0x7f) == 4)
        {
            let mut script = source.shared_script_source();
            fp.initialize_child_prefix(&mut player.components.preparation, &mut script)?;
            hook.ensure_current()?;
            let header = player
                .components
                .preparation
                .mode_animator(fp.identity())?
                .script_children()
                .get(
                    fp.command()
                        .and_then(|c| c.factory_node())
                        .context("FP factory node absent")?,
                )
                .context("FP child header absent")?;
            if header.stage() == script_child::Stage::AwaitingInitializer {
                let binding = header.cached().context("FP script binding absent")?;
                let mut phase = source.phase_zero_source(binding)?;
                fp.initialize_child_phase_zero(
                    &mut player.components.preparation,
                    &mut phase,
                    hook.animation_step()?,
                )?;
                hook.ensure_current()?;
                let header = player
                    .components
                    .preparation
                    .mode_animator(fp.identity())?
                    .script_children()
                    .get(
                        fp.command()
                            .and_then(|c| c.factory_node())
                            .context("FP position node absent")?,
                    )
                    .context("FP position header absent")?;
                if header.stage() == script_child::Stage::AwaitingAnimation {
                    let mut position = source.script_position_source(binding)?;
                    fp.prepare_child_position(&mut player.components.preparation, &mut position)?;
                    hook.ensure_current()?;
                }
            }
            let header = player
                .components
                .preparation
                .mode_animator(fp.identity())?
                .script_children()
                .get(
                    fp.command()
                        .and_then(|c| c.factory_node())
                        .context("FP effect node absent")?,
                )
                .context("FP effect header absent")?;
            if !header.initializer_returned() {
                break;
            }
            let mut body = source.mode_command_source(
                action
                    .fp_command_stream
                    .as_mut()
                    .context("FP continued stream absent")?,
            )?;
            fp.finish_child_effects(
                &mut player.components.preparation,
                &mut body,
                hook.animation_slot_override()?,
            )?;
            hook.ensure_current()?;
            if fp
                .command()
                .is_none_or(|c| c.stage() != command::Stage::ChildFactoryPending)
            {
                break;
            }
        }
        if fp
            .command()
            .is_some_and(|c| c.stage() == command::Stage::Returned)
        {
            fp.acknowledge_command()?;
        } else {
            return Ok(());
        }
        hook.ensure_current()?;
        let mut mode = source.mode_source()?;
        next = fp.advance(&mut mode)?;
    }
    if fp.stage() == FirstPersonStage::SetterTail {
        fp.finish_setter(&mut player.reset, &player.components.preparation)?;
        hook.ensure_current()?;
    }
    if fp.stage() == FirstPersonStage::DispatchTail {
        fp.finish_dispatch_tail(&player.reset, &mut player.components.preparation)?;
        hook.ensure_current()?;
    }
    if fp.stage() == FirstPersonStage::Returned {
        action.state.complete_first_person_dispatch()?;
    }
    Ok(())
}
