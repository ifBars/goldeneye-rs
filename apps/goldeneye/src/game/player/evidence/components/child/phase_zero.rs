use anyhow::{Result, bail};
use ge_core::animation::script_child::{Header, Stage, initialization::Storage};
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhaseZero {
    pub stage: &'static str,
    pub delta_bits: u32,
    pub duration_raw: u32,
    pub frame_bits: u32,
    pub entries_visited: usize,
    pub selected_offset: Option<u32>,
    pub normal_requested: Option<u8>,
    pub secondary_requested: Option<u8>,
    pub normal_bound: u8,
    pub secondary_bound: u8,
    pub clip_records: usize,
    pub heap_allocated: bool,
    pub initial_index_raw: Option<u8>,
    pub initial_index_clamped: Option<u8>,
    pub frame_range: Option<i32>,
    pub setup_returned: bool,
    pub flags_applied: bool,
    pub pending_file: Option<u32>,
}
pub(super) fn build(header: &Header) -> Result<Option<PhaseZero>> {
    use anyhow::Context;
    let Some(t) = header.initialization() else {
        return Ok(None);
    };
    let stage = match header.stage() {
        Stage::AwaitingClipFile => "clip_file_pending",
        Stage::AwaitingClipRange => "clip_range_pending",
        Stage::AwaitingFrameCount => "frame_count_pending",
        Stage::AwaitingAnimation => "animation_pending",
        Stage::ReturnedWithoutPlay => "returned_without_play",
        _ => bail!("script phase-zero evidence has no reportable boundary"),
    };
    let (clip_records, heap_allocated) = match header.clip_storage() {
        None => (0, false),
        Some(Storage::Inline { .. }) => (1, false),
        Some(Storage::Heap(records)) => (records.len(), true),
    };
    Ok(Some(PhaseZero {
        stage,
        delta_bits: t.delta_bits.context("script delta absent")?,
        duration_raw: t.duration_raw.context("script duration absent")?,
        frame_bits: t.frame_bits.context("script selection frame absent")?,
        entries_visited: t.entries_visited,
        selected_offset: t.selected_offset,
        normal_requested: t.requested.map(|v| v[0]),
        secondary_requested: t.requested.map(|v| v[1]),
        normal_bound: t.bound[0],
        secondary_bound: t.bound[1],
        clip_records,
        heap_allocated,
        initial_index_raw: t.initial_index_raw,
        initial_index_clamped: t.initial_index_clamped,
        frame_range: t.frame_range,
        setup_returned: t.setup_returned,
        flags_applied: t.flags_raw.is_some(),
        pending_file: t.pending_file,
    }))
}
