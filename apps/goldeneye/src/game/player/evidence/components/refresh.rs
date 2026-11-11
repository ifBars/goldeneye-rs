use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindRefresh {
    pub arena_bytes: usize,
    pub reserved_bytes: usize,
    pub reservation_start: usize,
    pub cursor_after: usize,
    pub live_reservations_after: usize,
    pub refresh_ordinal: u64,
    pub allocation_ordinal: u64,
    pub regions_bound: usize,
    pub flags_after: u32,
    pub scope_released: bool,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<BindRefresh> {
    let receipt = player
        .components
        .preparation
        .bind_return()
        .context("TP bind return is absent")?;
    ensure!(
        receipt.player() == player.base.player(),
        "TP bind return player differs"
    );
    let refresh = receipt.refresh();
    Ok(BindRefresh {
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
        state_sha256: super::super::words(b"ge_player_tp_bind_return_v1\0", |emit| {
            receipt.visit_state_words(emit)
        }),
    })
}
