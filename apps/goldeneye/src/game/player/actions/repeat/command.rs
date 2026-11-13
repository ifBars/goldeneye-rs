use super::*;
use ge_core::animation::{layers::command, script_child};
pub(super) fn run(
    call: &mut mode_call::Call,
    owned: &mut CommandOwned,
    preparation: &mut ge_core::mission::player::components::Preparation,
    source: &FileBinding,
    reset: &mut ge_core::mission::player::constructor::PendingReset,
    hook: &world_hooks::Attempt,
) -> Result<bool> {
    ensure!(
        preparation
            .mode_animator(call.inputs().receiver)?
            .component()
            .is_some_and(|binding| binding.file == source.file()),
        "repeat command receiver file differs"
    );
    let result = execute(call, owned, preparation, source, reset, hook);
    if result.is_err() && call.stage() != mode_call::Stage::Failed {
        call.fail()?;
    }
    result
}
fn execute(
    call: &mut mode_call::Call,
    owned: &mut CommandOwned,
    preparation: &mut ge_core::mission::player::components::Preparation,
    source: &FileBinding,
    reset: &mut ge_core::mission::player::constructor::PendingReset,
    hook: &world_hooks::Attempt,
) -> Result<bool> {
    let mut next = if call.stage() == mode_call::Stage::Selecting {
        call.advance(&mut source.mode_source()?)?
    } else {
        call.selector().pending()
    };
    while let Some(invocation) = next {
        hook.ensure_current()?;
        if !call.has_pending_command() {
            let location = source.mode_command_location(invocation)?;
            owned.location = Some(location);
            owned.stream = Some(ge_assets::animation_modes::CommandStream::new(location));
            call.prepare_command(
                preparation,
                location.opcode(),
                &mut source
                    .mode_command_source(owned.stream.as_mut().expect("retained repeat stream"))?,
                hook.animation_slot_override()?,
            )?;
        }
        if call.command().is_some_and(|body| {
            matches!(
                body.stage(),
                command::Stage::MaskControlPending
                    | command::Stage::MaskClearPending
                    | command::Stage::MaskRefreshPending
                    | command::Stage::MaskBackingPending
                    | command::Stage::MaskEffectsPending
                    | command::Stage::MaskDestroyPending
            )
        }) {
            hook.finish_repeat_mask(call, preparation)?;
        }
        while call
            .command()
            .and_then(|body| body.child())
            .is_some_and(|child| ((child.reference >> 24) & 0x7f) == 4)
        {
            call.initialize_child_prefix(preparation, &mut source.shared_script_source())?;
            hook.ensure_current()?;
            let node = call
                .command()
                .and_then(|body| body.factory_node())
                .context("repeat factory node absent")?;
            let header = preparation
                .mode_animator(call.inputs().receiver)?
                .script_children()
                .get(node)
                .context("repeat script header absent")?;
            if header.stage() == script_child::Stage::AwaitingInitializer {
                let binding = header
                    .cached()
                    .context("repeat cached script binding absent")?;
                call.initialize_child_phase_zero(
                    preparation,
                    &mut source.phase_zero_source(binding)?,
                    hook.animation_step()?,
                )?;
                hook.ensure_current()?;
                if preparation
                    .mode_animator(call.inputs().receiver)?
                    .script_children()
                    .get(node)
                    .is_some_and(|header| header.stage() == script_child::Stage::AwaitingAnimation)
                {
                    call.prepare_child_position(
                        preparation,
                        &mut source.script_position_source(binding)?,
                    )?;
                }
            }
            if !preparation
                .mode_animator(call.inputs().receiver)?
                .script_children()
                .get(node)
                .is_some_and(|header| header.initializer_returned())
            {
                return Ok(false);
            }
            call.finish_child_effects(
                preparation,
                &mut source.mode_command_source(
                    owned
                        .stream
                        .as_mut()
                        .context("repeat continuation stream absent")?,
                )?,
                hook.animation_slot_override()?,
            )?;
            if call
                .command()
                .is_none_or(|body| body.stage() != command::Stage::ChildFactoryPending)
            {
                break;
            }
        }
        if call
            .command()
            .is_some_and(|body| body.stage() == command::Stage::Returned)
        {
            call.acknowledge()?;
        } else {
            return Ok(false);
        }
        hook.ensure_current()?;
        next = call.advance(&mut source.mode_source()?)?;
    }
    if call.stage() == mode_call::Stage::SetterTail {
        call.finish(reset)?;
    }
    Ok(call.stage() == mode_call::Stage::Returned)
}
