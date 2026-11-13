use super::*;
pub(super) fn initialize(player: &mut Player, hook: &world_hooks::Attempt) -> Result<()> {
    hook.ensure_current()?;
    let owner = player.action.as_mut().context("TP mode caller is absent")?;
    let tp = player
        .components
        .preparation
        .animator()
        .context("TP selector animator is absent")?;
    let fp = player
        .components
        .preparation
        .first_person()
        .context("TP qualification FP owner is absent")?
        .animator();
    let tp_source = player
        .components
        .component_source
        .as_ref()
        .context("TP selector component wrapper is absent")?;
    let fp_source = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("TP qualification FP wrapper is absent")?;
    ensure!(
        tp.identity() == owner.state.resolved()[1].request.component
            && fp.identity() == owner.state.resolved()[0].request.component
            && tp
                .component()
                .is_some_and(|binding| binding.file == tp_source.file())
            && fp
                .component()
                .is_some_and(|binding| binding.file == fp_source.file()),
        "TP selector or FP qualification target owner differs"
    );
    let global = hook.animation_fallback()?;
    ensure!(
        owner.state.fallback() == Some(global.0) && owner.state.fallback_writes() == Some(global.1),
        "TP selector shared fallback changed before consumption"
    );
    let available = fp_source.mode_available(owner.state.resolved()[0].output)?;
    owner.state.begin_third_person_selector(
        &mut player.base,
        &player.reset,
        &player.components.preparation,
        available,
    )?;
    let mut source = tp_source.mode_source()?;
    let mut next_invocation = owner.state.advance_third_person_selector(&mut source)?;
    hook.ensure_current()?;
    while let Some(invocation) = next_invocation {
        match tp_source.mode_command_location(invocation) {
            Ok(command) => owner.tp_command = Some(command),
            Err(error) => {
                owner.state.third_person_command_failed(invocation)?;
                return Err(error);
            }
        }
        hook.ensure_current()?;
        let location = owner.tp_command.context("TP reached command is absent")?;
        owner.tp_command_stream = Some(ge_assets::animation_modes::CommandStream::new(location));
        let mut source = tp_source.mode_command_source(
            owner
                .tp_command_stream
                .as_mut()
                .expect("retained command stream"),
        )?;
        owner.state.prepare_third_person_command(
            &mut player.components.preparation,
            location.opcode(),
            &mut source,
            hook.animation_slot_override()?,
        )?;
        hook.ensure_current()?;
        while owner
            .state
            .command()
            .and_then(|command| command.child())
            .is_some_and(|child| ((child.reference >> 24) & 0x7f) == 4)
        {
            let mut script_source = tp_source.shared_script_source();
            owner.state.initialize_third_person_child_prefix(
                &mut player.components.preparation,
                &mut script_source,
            )?;
            hook.ensure_current()?;
            let header = owner
                .state
                .command()
                .and_then(|command| command.factory_node())
                .and_then(|id| {
                    player
                        .components
                        .preparation
                        .animator()?
                        .script_children()
                        .get(id)
                })
                .context("phase-zero TP child absent")?;
            if header.stage() == ge_core::animation::script_child::Stage::AwaitingInitializer {
                let binding = header
                    .cached()
                    .context("phase-zero TP script cache absent")?;
                let mut phase_source = tp_source.phase_zero_source(binding)?;
                owner.state.initialize_third_person_child_phase_zero(
                    &mut player.components.preparation,
                    &mut phase_source,
                    hook.animation_step()?,
                    65_536,
                )?;
                hook.ensure_current()?;
                let header = owner
                    .state
                    .command()
                    .and_then(|command| command.factory_node())
                    .and_then(|id| {
                        player
                            .components
                            .preparation
                            .animator()?
                            .script_children()
                            .get(id)
                    })
                    .context("position TP child absent")?;
                if header.stage() == ge_core::animation::script_child::Stage::AwaitingAnimation {
                    let mut source = tp_source.script_position_source(binding)?;
                    owner.state.prepare_third_person_child_position(
                        &mut player.components.preparation,
                        &mut source,
                    )?;
                    hook.ensure_current()?;
                }
            }
            let header = owner
                .state
                .command()
                .and_then(|c| c.factory_node())
                .and_then(|id| {
                    player
                        .components
                        .preparation
                        .animator()?
                        .script_children()
                        .get(id)
                })
                .context("TP child effect header absent")?;
            if !header.initializer_returned() {
                break;
            }
            let mut source = tp_source.mode_command_source(
                owner
                    .tp_command_stream
                    .as_mut()
                    .context("TP continued command stream absent")?,
            )?;
            owner.state.finish_third_person_child_effects(
                &mut player.components.preparation,
                &mut source,
                hook.animation_slot_override()?,
            )?;
            hook.ensure_current()?;
            if owner.state.command().is_none_or(|c| {
                c.stage() != ge_core::animation::layers::command::Stage::ChildFactoryPending
            }) {
                break;
            }
        }
        if owner
            .state
            .command()
            .is_some_and(|c| c.stage() == ge_core::animation::layers::command::Stage::Returned)
        {
            owner.state.acknowledge_third_person_command()?;
            hook.ensure_current()?;
        } else {
            break;
        }
        let mut next_source = tp_source.mode_source()?;
        next_invocation = owner
            .state
            .advance_third_person_selector(&mut next_source)?;
        hook.ensure_current()?;
    }
    // The real operand/weight prefix is retained before its factory/cleanup
    // dependency. No command return or mode-history update is fabricated.
    Ok(())
}
