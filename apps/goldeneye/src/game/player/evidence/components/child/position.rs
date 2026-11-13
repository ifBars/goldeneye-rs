use ge_core::animation::script_child::Header;
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Position {
    pub selected_index: u16,
    pub file_bound: bool,
    pub generation_observed: Option<u32>,
    pub metadata_bound: bool,
    pub cache_generation: Option<u32>,
    pub descriptor_index: Option<usize>,
    pub body_bound: Option<bool>,
    pub timing_flags: Option<u32>,
    pub animation_fps: Option<u8>,
    pub script_fps_bits: Option<u32>,
    pub frame_bits: Option<u32>,
    pub previous_position_bits: Option<u32>,
    pub normalized_position_bits: Option<u32>,
    pub root_motion_pending: bool,
}
pub(super) fn build(header: &Header) -> Option<Position> {
    let t = header.position()?;
    Some(Position {
        selected_index: t.index,
        file_bound: t.file.is_some(),
        generation_observed: t.generation,
        metadata_bound: t.binding.is_some(),
        cache_generation: header.clip_word(usize::from(t.index), 8),
        descriptor_index: t.binding.map(|b| b.directory_index),
        body_bound: t.body_bound,
        timing_flags: t.timing_flags,
        animation_fps: t.animation_fps,
        script_fps_bits: t.script_fps_bits,
        frame_bits: t.frame_bits,
        previous_position_bits: t.previous_position_bits,
        normalized_position_bits: t.position_bits,
        root_motion_pending: t.root_motion_pending,
    })
}
