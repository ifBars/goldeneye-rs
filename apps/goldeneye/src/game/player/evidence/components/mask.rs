use anyhow::{Context, Result, ensure};
use ge_core::{
    animation::{layers::command, nodes::Kind},
    mission::player::components::Animator,
};
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MaskEffects {
    pub mask_raw: u16,
    pub active_slots_before: Option<u8>,
    pub slots_visited: usize,
    pub selected_mask: Option<u16>,
    pub clear_mask: Option<u8>,
    pub controls_allocated: usize,
    pub controls_inserted: usize,
    pub capture_returned: bool,
    pub refresh: Option<super::BindRefresh>,
    pub state_sha256: String,
}
pub(super) fn build(owner: &Animator, command: &command::Pending) -> Result<Option<MaskEffects>> {
    let Some(e) = command.mask_effects() else {
        return Ok(None);
    };
    if let Some(id) = e.first {
        let first = owner
            .controls()
            .get(id)
            .context("mask control allocation absent")?;
        ensure!(
            owner.node_ids().kind(id) == Some(Kind::Control)
                && first.component().get() == owner.identity().id().get()
                && owner.controls().len() >= e.controls_allocated,
            "mask control identity or allocation count differs"
        );
        if e.capture_returned {
            let skeleton = owner.skeleton().context("mask captured skeleton absent")?;
            ensure!(
                first.locals()?.len() == skeleton.skeleton().bones().len()
                    && first.auxiliary()?.len() == skeleton.auxiliary().len(),
                "mask capture backing differs from actual skeletal owner"
            );
        }
    } else {
        ensure!(
            e.controls_allocated == 0,
            "mask allocated control identity disappeared"
        );
    }
    Ok(Some(MaskEffects {
        mask_raw: command
            .mask()
            .context("mask effects lost their raw operand")?,
        active_slots_before: e.active_slots_before,
        slots_visited: e.slots_visited,
        selected_mask: e.selected_mask,
        clear_mask: e.clear_mask,
        controls_allocated: e.controls_allocated,
        controls_inserted: e.controls_inserted,
        capture_returned: e.capture_returned,
        refresh: e
            .refresh
            .as_ref()
            .map(|r| super::refresh::from_refresh(r, b"ge_player_mask_refresh_v1\0")),
        state_sha256: super::super::words(b"ge_player_mask_effects_v1\0", |mut emit| {
            e.visit_state_words(&mut emit);
            owner.controls().visit_state_words(&mut emit);
            owner.node_ids().visit_state_words(&mut emit);
            if let Some(layers) = owner.layers() {
                layers.visit_state_words(&mut emit);
            }
        }),
    }))
}
