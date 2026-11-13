//! Same-player repeated invocation. Prior completed evidence is captured before
//! any live heads/history change. No component/reset/capture helper is replayed.
use super::*;
use crate::game::world::post_trigger_resources::FileBinding;
use ge_core::mission::player::actions::{
    mode_call,
    repeat::{self, Stage},
};
mod command;
mod seek;
pub(in crate::game::player) struct CommandOwned {
    pub location: Option<ge_assets::animation_modes::CommandLocation>,
    pub stream: Option<ge_assets::animation_modes::CommandStream>,
}
pub(in crate::game::player) struct Owned {
    pub callback_tail: Option<actions::callback_tail::CallbackTail>,
    pub state: actions::Pending,
    pub mappings: Vec<ge_assets::weapon_static::Remap>,
    pub dispatch: Option<repeat::Dispatch>,
    pub commands: Vec<CommandOwned>,
    pub seek_started: bool,
}
pub(super) fn initialize(
    player: &mut Player,
    world: &World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    ensure!(
        player.repeat_action.is_none(),
        "repeat action entry replays"
    );
    let snapshot = super::super::evidence::capture_initial_action(player, hook)?;
    player
        .action
        .as_mut()
        .context("first action absent")?
        .completed_evidence = Some(snapshot);
    let state = player
        .action
        .as_ref()
        .expect("first action")
        .state
        .begin_repeated(&player.base, &player.reset)?;
    player.repeat_action = Some(Owned {
        callback_tail: None,
        state,
        mappings: Vec::new(),
        dispatch: None,
        commands: Vec::new(),
        seek_started: false,
    });
    let owned = player.repeat_action.as_mut().expect("retained repeat");
    owned.state.bind_components(
        &player.components.preparation,
        world.item(player.base.item().context("repeat item absent")?)?,
    )?;
    owned
        .state
        .bind_inventory(&player.base, player.prelude.inventory())?;
    while let Some(request) = owned.state.next_remap()? {
        hook.ensure_current()?;
        let source = if request.target == Target::FirstPerson {
            player.components.first_person.component_source.as_ref()
        } else {
            player.components.component_source.as_ref()
        }
        .context("repeat mapping wrapper absent")?;
        ensure!(
            player
                .components
                .preparation
                .mode_animator(request.component)?
                .component()
                .is_some_and(|binding| binding.file == source.file()),
            "repeat mapping receiver/wrapper differs"
        );
        let mapped = player
            .loadout
            .map_initial_action(request, |mode| source.mode_available(mode))?;
        let (output, accepted) = (mapped.output, mapped.mapped);
        owned.mappings.push(mapped);
        owned.state.mapped(request, output, accepted)?;
    }
    owned.dispatch = Some(repeat::Dispatch::begin(
        &mut owned.state,
        &player.components.preparation,
        hook.store_animation_fallback(0x0900_000b)?,
    )?);
    run(player, world, hook)
}

fn run(player: &mut Player, world: &World, hook: &world_hooks::Attempt) -> Result<()> {
    loop {
        hook.ensure_current()?;
        let owned = player
            .repeat_action
            .as_mut()
            .context("repeat invocation absent")?;
        let dispatch = owned
            .dispatch
            .as_mut()
            .context("repeat dispatcher absent")?;
        match dispatch.stage() {
            Stage::FirstThirdPerson | Stage::SecondThirdPerson | Stage::FirstPerson => {
                let is_fp = dispatch.stage() == Stage::FirstPerson;
                let ordinal = match dispatch.stage() {
                    Stage::FirstThirdPerson => 0,
                    Stage::SecondThirdPerson => 1,
                    _ => 2,
                };
                let fp_source = player
                    .components
                    .first_person
                    .component_source
                    .as_ref()
                    .context("repeat FP wrapper absent")?;
                let tp_source = player
                    .components
                    .component_source
                    .as_ref()
                    .context("repeat TP wrapper absent")?;
                let source = if is_fp { fp_source } else { tp_source };
                if dispatch.calls().len() == ordinal {
                    let fallback = if is_fp {
                        hook.store_animation_fallback(0x0900_000b)?
                    } else {
                        hook.animation_fallback()?
                    };
                    let tracks = if is_fp {
                        true
                    } else {
                        !fp_source.mode_available(owned.state.resolved()[0].output)?
                    };
                    dispatch.begin_call(
                        tracks,
                        &mut player.base,
                        &player.reset,
                        &player.components.preparation,
                        fallback,
                    )?;
                    owned.commands.push(CommandOwned {
                        location: None,
                        stream: None,
                    });
                }
                let returned = command::run(
                    dispatch.current_call_mut()?,
                    &mut owned.commands[ordinal],
                    &mut player.components.preparation,
                    source,
                    &mut player.reset,
                    hook,
                )?;
                if !returned {
                    return Ok(());
                }
                dispatch.call_returned()?;
            }
            Stage::ThirdPersonCoordinate => {
                dispatch.begin_seek(&player.reset, &mut player.components.preparation)?
            }
            Stage::ThirdPersonSeek => {
                if let Some(node) = dispatch.seek_node() {
                    let source = player
                        .components
                        .component_source
                        .as_ref()
                        .context("repeat seek wrapper absent")?;
                    if !seek::run(
                        &mut player.components.preparation,
                        source,
                        player.base.player(),
                        node,
                        &mut owned.seek_started,
                        hook,
                    )? {
                        return Ok(());
                    }
                }
                dispatch.finish_seek(&player.components.preparation)?;
            }
            Stage::EnablingOverride => {
                hook.store_animation_slot_override(1, 1)?;
                dispatch.override_stored(true)?;
            }
            Stage::DisablingOverride => {
                hook.store_animation_slot_override(0, 0)?;
                dispatch.override_stored(false)?;
            }
            Stage::FirstPersonCoordinate => dispatch.finish_fp_coordinate(
                &player.reset,
                &mut player.components.preparation,
                &hook.animation_parameter_globals()?,
            )?,
            Stage::Returned => {
                owned.state.finish_repeat_dispatch(dispatch)?;
                super::suffix::initialize_repeat(player, world, hook)?;
                return finish_callback(player, world, hook);
            }
        }
    }
}

fn finish_callback(player: &mut Player, world: &World, hook: &world_hooks::Attempt) -> Result<()> {
    let first = &player
        .action
        .as_ref()
        .context("callback first invocation absent")?
        .state;
    let second = player
        .repeat_action
        .as_mut()
        .context("callback repeat invocation absent")?;
    if second.state.stage() != actions::Stage::Returned {
        return Ok(());
    }
    hook.ensure_current()?;
    second.callback_tail = Some(actions::callback_tail::CallbackTail::begin(
        first,
        &second.state,
        second
            .dispatch
            .as_ref()
            .context("callback repeated dispatcher absent")?,
    )?);
    let item = world.item(player.base.item().context("callback item absent")?)?;
    second
        .callback_tail
        .as_mut()
        .expect("retained callbacktail")
        .advance(
            &mut player.base,
            &mut player.reset,
            item,
            player.virtual_init.physics_mut(),
        )?;
    hook.ensure_current()?;
    Ok(())
}
