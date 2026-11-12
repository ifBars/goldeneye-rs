use super::super::super::Owned;
use super::BindRefresh;
use crate::game::world_hooks;
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThirdPersonCapture {
    pub physical_slots: usize,
    pub controls: usize,
    pub bones_per_control: usize,
    pub auxiliary_words_per_control: usize,
    pub native_header_bytes: usize,
    pub native_backing_bytes_per_control: usize,
    pub active_slots: usize,
    pub capture_mask: u8,
    pub preserve_mask: u8,
    pub update_flags_after: u16,
    pub frame_stamp_known: bool,
    pub postprocess_executed: bool,
    pub fallback: u32,
    pub fallback_writes: u64,
    pub refresh: BindRefresh,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned, hook: &world_hooks::Attempt) -> Result<ThirdPersonCapture> {
    let owner = player
        .components
        .preparation
        .animator()
        .context("captured TP animator is absent")?;
    let runtime = owner.skeleton().context("captured TP skeleton is absent")?;
    let layers = owner.layers().context("captured TP slot owner is absent")?;
    let refresh = owner
        .capture_refresh()
        .context("TP control refresh did not return")?;
    let fp = player
        .components
        .preparation
        .first_person()
        .context("FP bind owner is absent")?;
    let action = &player
        .action
        .as_ref()
        .context("TP capture action owner is absent")?
        .state;
    let (fallback, writes) = hook.animation_fallback()?;
    ensure!(
        owner.controls().len() == 4
            && !layers.failed()
            && layers.active_count() == 1
            && owner.byte(0x130) == Some(1)
            && owner.byte(0xc) == Some(0)
            && owner.byte(0xd) == Some(3)
            && owner.byte(0x135) == Some(0)
            && runtime.frame_stamp_bits().is_err()
            && refresh.follows_in_same_arena(fp.refresh().context("FP refresh did not return")?)
            && fallback == 0x0900_06c0
            && action.fallback() == Some(fallback)
            && action.fallback_writes() == Some(writes),
        "TP captured state or actual shared fallback differs"
    );
    let mut ids = std::collections::BTreeSet::new();
    for (slot, start) in layers.slots().iter().zip([0x138, 0x158, 0x178, 0x198]) {
        ensure!(
            slot.nodes_head().len() == 1
                && owner.word(start).is_none()
                && owner.word(start + 4).is_none()
                && slot.flags_raw() == 1
                && owner.word(start + 12) == Some(slot.weight_bits()),
            "TP physical capture membership differs"
        );
        let id = slot.nodes_head()[0];
        ensure!(ids.insert(id), "TP physical slots share a control identity");
        let control = owner
            .controls()
            .get(id)
            .context("TP captured payload owner is absent")?;
        ensure!(
            control.component().get() == owner.identity().id().get()
                && control.transition_bits() == 1f32.to_bits()
                && control.rate_bits() == 0
                && control.flags() == 0
                && control.locals()?.len() == runtime.skeleton().bones().len()
                && control.auxiliary()? == runtime.auxiliary(),
            "TP captured payload differs"
        );
        for (local, bone) in control.locals()?.iter().zip(runtime.skeleton().bones()) {
            ensure!(
                local.translation_bits == bone.local_translation_bits()
                    && local.quaternion_bits == bone.local_quaternion_bits(),
                "TP control did not capture actual primary locals"
            );
        }
    }
    let bones = runtime.skeleton().bones().len();
    let auxiliary = runtime.auxiliary().len();
    Ok(ThirdPersonCapture {
        physical_slots: 4,
        controls: owner.controls().len(),
        bones_per_control: bones,
        auxiliary_words_per_control: auxiliary,
        native_header_bytes: 0x54,
        native_backing_bytes_per_control: bones * 32 + auxiliary * 4,
        active_slots: 1,
        capture_mask: 0xf,
        preserve_mask: 3,
        update_flags_after: 3,
        frame_stamp_known: false,
        postprocess_executed: false,
        fallback,
        fallback_writes: writes,
        refresh: BindRefresh {
            arena_bytes: ge_core::animation::scratch::ARENA_BYTES,
            reserved_bytes: 0x2200,
            reservation_start: refresh.reservation_start(),
            cursor_after: refresh.cursor_after(),
            live_reservations_after: refresh.live_after(),
            refresh_ordinal: refresh.ordinal(),
            allocation_ordinal: refresh.allocation_ordinal(),
            regions_bound: 4,
            flags_after: 0,
            scope_released: true,
            state_sha256: super::super::words(b"ge_player_tp_capture_refresh_v1\0", |emit| {
                refresh.visit_state_words(emit)
            }),
        },
        state_sha256: super::super::words(b"ge_player_tp_control_capture_v2\0", |mut emit| {
            owner.controls().visit_state_words(&mut emit);
            layers.visit_state_words(emit);
        }),
    })
}
