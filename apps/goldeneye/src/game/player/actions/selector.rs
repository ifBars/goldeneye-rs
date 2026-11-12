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
    let invocation = owner.state.advance_third_person_selector(&mut source)?;
    hook.ensure_current()?;
    if let Some(invocation) = invocation {
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
        if owner
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
        }
    }
    // The real operand/weight prefix is retained before its factory/cleanup
    // dependency. No command return or mode-history update is fabricated.
    Ok(())
}
