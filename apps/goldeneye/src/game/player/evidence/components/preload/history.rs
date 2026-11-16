//! Capture live diagnostics once, before teardown invalidates the owner.
use super::{post_bind, temporary};
use crate::game::{world::post_trigger_resources::ScriptPublication, world_hooks::ScalarFacts};
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    components::temporary::Pending,
    constructor::preload::{
        Call, CallStage,
        bone_outputs::{BoneOutputs, Stage},
        post_bind::PostBind,
    },
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Completion {
    pub history_limit: usize,
    pub completed_temporary_count: usize,
    pub discarded_temporary_count: usize,
    pub found_output_count: usize,
    pub missing_output_count: usize,
    pub destroyed_temporary_count: usize,
    pub entry_commit_count: usize,
    pub history: Vec<Completed>,
    pub pending_snapshot: Option<PreloadSnapshot>,
    pub first_completed_snapshot: Option<PreloadSnapshot>,
    pub destruction: Option<Value>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Completed {
    pub snapshot: PreloadSnapshot,
    pub destruction: Value,
}
pub(super) fn destruction(
    t: &ge_core::mission::player::constructor::preload::teardown::Tombstone,
) -> Value {
    let s = t.snapshot();
    json!({"stage":"destroyed", "pending_dependency":null,
        "temporary_identity":t.temporary().id().get(), "ordinal":t.binding().request.ordinal,
        "children":t.children().len(), "skeleton":t.skeleton().map(|s|json!({"bones":s.bones,"auxiliary_words":s.auxiliary_words,"block_bytes":s.block_bytes})),
        "snapshot_words":s.state_words(), "snapshot_digest":format!("{:016x}",s.state_digest()),
        "state_sha256":super::super::super::words(b"ge_preload_tombstone_v1\0",|emit|t.visit_state_words(emit))})
}
pub(super) fn teardown(
    t: &ge_core::mission::player::constructor::preload::teardown::Teardown,
) -> Value {
    use ge_core::mission::player::constructor::preload::teardown::Stage;
    json!({"stage":match t.stage(){Stage::AttachmentList=>"attachment_list",Stage::ExtraOwner=>"extra_owner",Stage::SlotChildren=>"slot_children",Stage::Skeleton=>"skeleton",Stage::ComponentWords=>"component_words",Stage::AttachmentListAgain=>"attachment_list_again",Stage::BaseDestructor=>"base_destructor",Stage::AwaitingFree=>"awaiting_free",Stage::Destroyed=>"destroyed",Stage::Failed=>"failed",Stage::Aborted=>"aborted"},
        "pending_dependency":t.dependency().map(|d|d.as_str()),
        "temporary_identity":t.binding().temporary.id().get(), "ordinal":t.binding().request.ordinal,
        "children":t.children().len(), "skeleton":t.skeleton().map(|s|json!({"bones":s.bones,"auxiliary_words":s.auxiliary_words,"block_bytes":s.block_bytes})),
        "snapshot_words":t.snapshot().state_words(), "snapshot_digest":format!("{:016x}",t.snapshot().state_digest()),
        "state_sha256":super::super::super::words(b"ge_preload_teardown_v1\0",|emit|t.visit_state_words(emit))})
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreloadSnapshot {
    pub ordinal: usize,
    pub source_ordinal: usize,
    pub player: u64,
    pub receiver: u64,
    pub weapon: u32,
    pub generic: u32,
    pub alternate: bool,
    pub variant: u8,
    pub entry: u8,
    pub mapped: u32,
    pub reference: u32,
    pub temporary_identity: u64,
    pub temporary: temporary::Temporary,
    pub post_bind: post_bind::Caller,
    pub outputs: Outputs,
    pub script_publication: Option<ScriptPublication>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outputs {
    pub stage: &'static str,
    pub pending_dependency: Option<&'static str>,
    pub iteration: u8,
    pub hash_cursor: u8,
    pub hash_reads: u32,
    pub lookups: u32,
    pub body_reads: u32,
    pub tag_walks: u32,
    pub stores: u32,
    pub found: usize,
    pub missing: usize,
    pub outcomes: Vec<Option<&'static str>>,
    pub state_sha256: String,
}
pub(super) fn output_evidence(outputs: &BoneOutputs) -> Outputs {
    use ge_core::mission::player::constructor::preload::bone_outputs::{Missing, Outcome, Step};
    let outcomes: Vec<_> = outputs
        .outcomes()
        .iter()
        .map(|outcome| {
            outcome.map(|outcome| match outcome {
                Outcome::Found { .. } => "found",
                Outcome::Missing(Missing::ZeroHash) => "zero_hash",
                Outcome::Missing(Missing::DescriptorAbsent) => "descriptor_absent",
                Outcome::Missing(Missing::BodyAbsent) => "body_absent",
                Outcome::Missing(Missing::Sentinel) => "sentinel",
            })
        })
        .collect();
    Outputs {
        stage: match outputs.stage() {
            Stage::Hash => "hash",
            Stage::Lookup => "lookup",
            Stage::Body => "body",
            Stage::Tag => "tag",
            Stage::Store(Step::Index) => "store_index",
            Stage::Store(Step::TranslationLow) => "store_translation_low",
            Stage::Store(Step::TranslationHigh) => "store_translation_high",
            Stage::Store(Step::RotationLow) => "store_rotation_low",
            Stage::Store(Step::RotationHigh) => "store_rotation_high",
            Stage::Done => "done",
            Stage::Failed => "failed",
            Stage::Aborted => "aborted",
        },
        pending_dependency: outputs.dependency().map(|d| d.as_str()),
        iteration: outputs.iteration(),
        hash_cursor: outputs.hash_cursor(),
        hash_reads: outputs.hash_reads(),
        lookups: outputs.lookups(),
        body_reads: outputs.body_reads(),
        tag_walks: outputs.tag_walks(),
        stores: outputs.stores(),
        found: outcomes.iter().filter(|o| **o == Some("found")).count(),
        missing: outcomes
            .iter()
            .filter(|o| o.is_some() && **o != Some("found"))
            .count(),
        outcomes,
        state_sha256: super::super::super::words(b"ge_preload_bone_outputs_v1\0", |emit| {
            outputs.visit_state_words(emit)
        }),
    }
}

pub(in crate::game::player) fn capture_preload_snapshot(
    call: &Call,
    temporary: &Pending,
    post: &PostBind,
    outputs: &BoneOutputs,
    process: ScalarFacts,
    script_publication: Option<ScriptPublication>,
) -> Result<PreloadSnapshot> {
    let binding = outputs.binding();
    let request = call.request();
    ensure!(
        outputs.stage() == Stage::Done
            && !temporary.teardown_started()
            && call.stage() == CallStage::AwaitingTemporary
            && binding.request == request
            && post.request() == request
            && binding.temporary == temporary.animator().identity()
            && post.temporary_identity() == binding.temporary
            && call.entry_index() == Some(binding.entry)
            && call
                .mapping()
                .context("snapshot call mapping absent")?
                .output
                == binding.mapped
            && call.reference().unwrap_or(0) == binding.reference,
        "preload snapshot requires matching live call and completed bone outputs"
    );
    Ok(PreloadSnapshot {
        ordinal: request.ordinal,
        source_ordinal: request.source_ordinal,
        player: request.player.get(),
        receiver: request.receiver.id().get(),
        weapon: request.weapon,
        generic: request.generic(),
        alternate: request.alternate,
        variant: request.variant,
        entry: binding.entry,
        mapped: binding.mapped,
        reference: binding.reference,
        temporary_identity: binding.temporary.id().get(),
        temporary: temporary::build(temporary)?,
        post_bind: post_bind::build(post, temporary, process)?,
        outputs: output_evidence(outputs),
        script_publication,
    })
}
