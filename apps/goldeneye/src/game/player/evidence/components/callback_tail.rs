use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    actions::callback_tail::{PhysicsOutcome, Stage},
    world::World,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CallbackTail {
    pub stage: &'static str,
    pub logging_noop_returned: bool,
    pub e59_raw: Option<u8>,
    pub initial_state_704: Option<u32>,
    pub state_704: Option<u32>,
    pub physics_outcome: Option<&'static str>,
    pub physics_flags_before: Option<u16>,
    pub physics_flags_after: Option<u16>,
    pub word_708: Option<u32>,
    pub flags_220_before: Option<u32>,
    pub flags_220_after: Option<u32>,
    pub callback_returned: bool,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned, world: &World) -> Result<Option<CallbackTail>> {
    let Some(tail) = player
        .repeat_action
        .as_ref()
        .and_then(|r| r.callback_tail.as_ref())
    else {
        return Ok(None);
    };
    ensure!(
        tail.player() == player.base.player(),
        "callback tail player differs"
    );
    let item = world.item(player.base.item().context("callback tail item absent")?)?;
    if let Some(e59) = tail.e59_raw() {
        ensure!(
            player.reset.byte(0xe59) == Some(e59) && player.reset.word(0xe60) == Some(0),
            "callback tail timer clear differs"
        );
    }
    if let Some(state) = tail.state_704() {
        ensure!(
            player.base.word(0x704) == Some(state),
            "callback tail state704 differs"
        );
    }
    let (physics_outcome, physics_flags_before, physics_flags_after) = match tail.physics_outcome()
    {
        None => (None, None, None),
        Some(PhysicsOutcome::Absent) => {
            ensure!(
                item.physics().is_none(),
                "callback tail skipped a bound physics owner"
            );
            (Some("absent"), None, None)
        }
        Some(PhysicsOutcome::Cleared {
            identity,
            flags_before,
            flags_after,
        }) => {
            let physics = player
                .virtual_init
                .physics()
                .context("callback tail retained physics owner absent")?;
            ensure!(
                item.physics() == Some(identity)
                    && identity.get() == player.base.player().get()
                    && physics.item_identity() == Some(item.id())
                    && physics.half(8) == Some(flags_after)
                    && flags_after == flags_before & 0xfcfc
                    && physics.word(0x15c) == Some(0)
                    && physics.half(0xa) == Some(0),
                "callback tail physics motion clear differs"
            );
            (Some("flags_cleared"), Some(flags_before), Some(flags_after))
        }
    };
    if let Some(after) = tail.flags_220_after() {
        ensure!(
            player.base.word(0x220) == Some(after) && player.base.word(0x708) == tail.word_708(),
            "callback tail final flags differ"
        );
    }
    let stage = match tail.stage() {
        Stage::Logging => "logging",
        Stage::DerivedTimer => "derived_timer",
        Stage::State704 => "state_704",
        Stage::Physics => "physics",
        Stage::FinalFlags => "final_flags",
        Stage::Returned => "returned",
    };
    Ok(Some(CallbackTail {
        stage,
        logging_noop_returned: tail.logging_noop_returned(),
        e59_raw: tail.e59_raw(),
        initial_state_704: tail.initial_state_704(),
        state_704: tail.state_704(),
        physics_outcome,
        physics_flags_before,
        physics_flags_after,
        word_708: tail.word_708(),
        flags_220_before: tail.flags_220_before(),
        flags_220_after: tail.flags_220_after(),
        callback_returned: tail.callback_returned(),
        state_sha256: super::super::words(b"ge_player_initial_callback_tail_v1\0", |mut emit| {
            tail.visit_state_words(&mut emit);
            player.base.visit_state_words(&mut emit);
            player.reset.visit_state_words(&mut emit);
            if let Some(physics) = player.virtual_init.physics() {
                physics.visit_state_words(&mut emit);
            }
        }),
    }))
}
