//! Caller work is distinct from the immutable first bind evaluation.
use super::temporary::evaluation::{self, Evaluation};
use crate::game::world_hooks::ScalarFacts;
use anyhow::Result;
use ge_core::mission::player::{
    components::temporary::{Pending, PostBindHead},
    constructor::preload::post_bind::PostBind,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Caller {
    pub stage: &'static str,
    pub pending_dependency: Option<&'static str>,
    pub temporary_identity: u64,
    pub head_reads: u32,
    pub first_scope: Option<Value>,
    pub head: Option<Value>,
    pub parameter: Option<Value>,
    pub process: ScalarFacts,
    pub capture: Option<Value>,
    pub phase: Option<Value>,
    pub evaluation: Option<Evaluation>,
    pub state_sha256: String,
}
pub(super) fn build(post: &PostBind, temporary: &Pending, process: ScalarFacts) -> Result<Caller> {
    let head = post.head().map(|head| match head {
        PostBindHead::Null => json!({"kind":"null","node":null}),
        PostBindHead::Unknown => json!({"kind":"unknown","node":null}),
        PostBindHead::ClassFour(id) => json!({"kind":"class_four","node":id.get()}),
        PostBindHead::NonMatching(id) => json!({"kind":"nonmatching","node":id.get()}),
    });
    let parameter = post.parameter().map(|p|json!({"node":p.node.get(),"pointer":match p.pointer {ge_core::animation::script_child::pointee::ParameterPointer::ConstructorInlinePhase=>"constructor_inline_phase",ge_core::animation::script_child::pointee::ParameterPointer::Null=>"null",ge_core::animation::script_child::pointee::ParameterPointer::Unknown=>"unknown"},"offset":p.offset,"previous":p.previous,"bits":p.bits}));
    let capture = post.capture().map(|r|json!({"owner":r.owner(),"read_sequence":r.read_sequence(),"revision":r.revision(),"current_bits":r.current_bits()}));
    let phase = post.phase().map(|p|json!({"upper_bits":p.upper_bits(),"selected_bits":p.selected_bits(),"assignment":post.assignment().map(|a|match a { ge_core::animation::script_child::phase::PhaseAssignment::Deferred=>"deferred",ge_core::animation::script_child::phase::PhaseAssignment::Known(_)=>"known" }),
        "header_known_bits":temporary.factory_header().and_then(|h|h.phase_bits().ok()),
        "header_expression_bound":temporary.factory_header().is_some_and(|h|h.phase_expression()==Some(p))}));
    let first_scope = temporary.evaluation().and_then(|e|e.lease()).map(|l|json!({"arena_owner":l.owner(),"ordinal":l.ordinal(),"allocation_ordinal":l.allocation_ordinal()}))
        .or_else(||temporary.refresh().map(|r|json!({"arena_owner":r.owner(),"ordinal":r.ordinal(),"allocation_ordinal":r.allocation_ordinal()})));
    let state_sha256 =
        super::super::super::words(b"ge_preload_post_bind_caller_v1\0", |mut emit| {
            post.visit_state_words(&mut emit);
            for value in [process.owner, process.revision, process.reads] {
                emit((value >> 32) as u32);
                emit(value as u32);
            }
            emit(process.initial_bits);
            emit(u32::from(process.current_bits.is_some()));
            if let Some(bits) = process.current_bits {
                emit(bits);
            }
            emit(
                process
                    .capture_issued
                    .map_or(0, |issued| if issued { 2 } else { 1 }),
            );
        });
    Ok(Caller {
        stage:match post.stage() { ge_core::mission::player::constructor::preload::post_bind::Stage::AwaitingHead=>"awaiting_head",ge_core::mission::player::constructor::preload::post_bind::Stage::AwaitingParameter=>"awaiting_parameter",ge_core::mission::player::constructor::preload::post_bind::Stage::ReadingScalar=>"reading_scalar",ge_core::mission::player::constructor::preload::post_bind::Stage::AssigningPhase=>"assigning_phase",ge_core::mission::player::constructor::preload::post_bind::Stage::Evaluating=>"evaluating",ge_core::mission::player::constructor::preload::post_bind::Stage::AwaitingBoneOutputs=>"awaiting_bone_outputs",ge_core::mission::player::constructor::preload::post_bind::Stage::Failed=>"failed",ge_core::mission::player::constructor::preload::post_bind::Stage::Aborted=>"aborted" },pending_dependency:post.dependency().map(|d|d.as_str()),temporary_identity:post.temporary_identity().id().get(),head_reads:post.head_reads(),first_scope,head,parameter,process,capture,phase,
        evaluation:post.evaluation().map(|trace|evaluation::from_parts(trace,post.skin_prepares(),post.node_guards())).transpose()?,
        state_sha256,
    })
}
