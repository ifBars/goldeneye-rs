use super::super::super::Owned;
use crate::game::world_hooks;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    components::{PoseCallback, callbacks::Handler},
    world::World,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CallbackBinding {
    pub interface_bound: bool,
    pub table_shared: bool,
    pub callback_slots: usize,
    pub noop_slots: usize,
    pub pose_target_assigned: bool,
    pub pose_postprocess_implemented: bool,
    pub tp_helper_returned: bool,
    pub events_executed: bool,
    pub state_sha256: String,
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    hook: &world_hooks::Attempt,
) -> Result<CallbackBinding> {
    let preparation = &player.components.preparation;
    let returned = preparation
        .third_person_return()
        .context("TP helper has not returned")?;
    let animator = preparation
        .animator()
        .context("TP callback animator is absent")?;
    let table = animator
        .callback_table()
        .context("TP callback table is unbound")?;
    let shared = hook.animation_callbacks()?;
    let interface = player.base.animation_interface()?;
    ensure!(
        Arc::ptr_eq(table, &shared)
            && table.handlers()
                == &[
                    Handler::Event,
                    Handler::Noop,
                    Handler::Noop,
                    Handler::Acknowledge
                ]
            && returned.component() == animator.identity()
            && returned.item() == player.base.item().context("TP callback item is absent")?
            && animator.item() == Some(returned.item())
            && returned.interface() == interface
            && returned.table_owner() == table.owner()
            && world.item(returned.item())?.animation_interface() == Some(interface)
            && animator.word(0x118).is_none()
            && animator
                .skeleton()
                .context("TP callback skeleton is absent")?
                .pose_callback()
                == Some(PoseCallback::PlayerNewPostprocess),
        "TP callback routing or actual shared owner differs"
    );
    Ok(CallbackBinding {
        interface_bound: true,
        table_shared: true,
        callback_slots: 4,
        noop_slots: 2,
        pose_target_assigned: true,
        pose_postprocess_implemented: false,
        tp_helper_returned: true,
        events_executed: false,
        state_sha256: super::super::words(b"ge_player_tp_callbacks_v1\0", |emit| {
            returned.visit_state_words(emit)
        }),
    })
}
