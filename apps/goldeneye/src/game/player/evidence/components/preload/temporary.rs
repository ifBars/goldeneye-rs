//! A direct factory has no mode-command parent or copied live head.
use super::super::child::phase_zero;
use anyhow::Result;
use ge_core::{
    animation::script_child::{self, initialization::Storage},
    mission::player::components::temporary::{Pending, Stage},
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clip {
    pub file: Option<u32>,
    pub reference: Option<u32>,
    pub cache_generation: Option<u32>,
    pub start_raw: Option<i16>,
    pub end_raw: Option<i16>,
    pub blend_bits: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Script {
    pub stage: String,
    pub slot: Option<u8>,
    pub factory_flags: Option<u32>,
    pub reference: u32,
    pub descriptor_index: Option<usize>,
    pub body_bound: Option<bool>,
    pub initializer_returned: bool,
    pub phase_zero: Option<phase_zero::PhaseZero>,
    pub clips: Vec<Clip>,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Forward {
    pub stage: String,
    pub phase_cleared: bool,
    pub body_bound: Option<bool>,
    pub duration_raw: Option<u32>,
    pub fps_bits: Option<u32>,
    pub result_bits: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Temporary {
    pub stage: &'static str,
    pub pending_dependency: Option<&'static str>,
    pub identity: u64,
    pub source_receiver: u64,
    pub player: u64,
    pub component_file: u32,
    pub reference: u32,
    pub skin_file: u32,
    pub skin_reference: u32,
    pub attached: bool,
    pub callbacks_bound: bool,
    pub skeleton_bound: bool,
    pub bones: Option<usize>,
    pub auxiliary_words: Option<usize>,
    pub skeleton_backlink_bound: bool,
    pub pose_callback_assigned: bool,
    pub frame_stamp_known: bool,
    pub semantic_flags: [Option<u8>; 2],
    pub component_flags: Option<u8>,
    pub active_slots: Option<u8>,
    pub factory_returned: bool,
    pub head_inserted: bool,
    pub script: Option<Script>,
    pub forward: Option<Forward>,
    pub maintenance_skipped: bool,
    pub reverse_skipped: bool,
    pub bind_return: Option<u32>,
    pub refresh_returned: bool,
    pub state_sha256: String,
}
fn script(header: &script_child::Header) -> Result<Script> {
    let count = match header.clip_storage() {
        None => 0,
        Some(Storage::Inline { .. }) => 1,
        Some(Storage::Heap(records)) => records.len(),
    };
    Ok(Script {
        stage: format!("{:?}", header.stage()),
        slot: header.slot(),
        factory_flags: header.word(0x20),
        reference: header.reference(),
        descriptor_index: header.cached().map(|b| b.directory_index),
        body_bound: header.body_bound(),
        initializer_returned: header.initializer_returned(),
        phase_zero: phase_zero::build(header)?,
        clips: (0..count)
            .map(|index| Clip {
                file: header.clip_file(index),
                reference: header.clip_word(index, 4),
                cache_generation: header.clip_word(index, 8),
                start_raw: header.clip_word(index, 0x18).map(|v| (v >> 16) as i16),
                end_raw: header.clip_word(index, 0x18).map(|v| v as i16),
                blend_bits: header.clip_word(index, 0x14),
            })
            .collect(),
        state_sha256: super::super::super::words(
            b"ge_player_preload_temporary_script_v1\0",
            |emit| header.visit_state_words(emit),
        ),
    })
}
pub(super) fn build(pending: &Pending) -> Result<Temporary> {
    let owner = pending.animator();
    let skeleton = owner.skeleton();
    Ok(Temporary {
        stage: match pending.stage() {
            Stage::AwaitingSkeleton => "awaiting_skeleton",
            Stage::BindingSkeleton => "binding_skeleton",
            Stage::AwaitingFactory => "awaiting_factory",
            Stage::InitializingFactory => "initializing_factory",
            Stage::AwaitingInitializer => "awaiting_initializer",
            Stage::AwaitingInsertion => "awaiting_insertion",
            Stage::AwaitingForwardTick => "awaiting_forward_tick",
            Stage::ForwardTick => "forward_tick",
            Stage::AwaitingEvaluation => "awaiting_evaluation",
            Stage::BindReturned => "bind_returned",
            Stage::Failed => "failed",
        },
        pending_dependency: pending.dependency().map(|d| d.as_str()),
        identity: owner.identity().id().get(),
        source_receiver: pending.source_receiver().id().get(),
        player: owner.player().get(),
        component_file: pending.component().file,
        reference: pending.component().reference,
        skin_file: pending.skin().file,
        skin_reference: pending.skin().reference,
        attached: owner.item().is_some(),
        callbacks_bound: owner.callback_table().is_some(),
        skeleton_bound: skeleton.is_some(),
        bones: skeleton.map(|s| s.skeleton().bones().len()),
        auxiliary_words: skeleton.map(|s| s.auxiliary().len()),
        skeleton_backlink_bound: skeleton.is_some_and(|s| s.animator() == Some(owner.identity())),
        pose_callback_assigned: skeleton.is_some_and(|s| s.pose_callback().is_some()),
        frame_stamp_known: skeleton.is_some_and(|s| s.frame_stamp_bits().is_ok()),
        semantic_flags: [owner.byte(0xc), owner.byte(0xd)],
        component_flags: owner.byte(0x135),
        active_slots: owner.byte(0x130),
        factory_returned: pending.factory_returned(),
        head_inserted: pending.head().is_some(),
        script: pending.factory_header().map(script).transpose()?,
        forward: pending.forward_tick().map(|t| Forward {
            stage: format!("{:?}", t.stage()),
            phase_cleared: t.phase_cleared(),
            body_bound: t.body_bound(),
            duration_raw: t.duration_raw(),
            fps_bits: t.fps_bits(),
            result_bits: t.result_bits(),
        }),
        maintenance_skipped: pending.maintenance_skipped(),
        reverse_skipped: pending.reverse_skipped(),
        bind_return: pending.bind_return(),
        refresh_returned: pending.refresh().is_some(),
        state_sha256: super::super::super::words(b"ge_player_preload_temporary_v1\0", |emit| {
            pending.visit_state_words(emit)
        }),
    })
}
