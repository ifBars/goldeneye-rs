//! Actual nonempty evaluation and cancellation facts; never sampled pose data.
use anyhow::Result;
use ge_core::{
    animation::{
        scratch::evaluation::{Branch, Stage},
        script_child::{ClipPointer, evaluation::Outcome},
    },
    mission::player::components::temporary::Pending,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evaluation {
    pub stage: &'static str,
    pub pending_boundary: Option<String>,
    pub reserved: bool,
    pub released: bool,
    pub lease_live: bool,
    pub aborted: bool,
    pub arena_owner: Option<u64>,
    pub ordinal: Option<u64>,
    pub allocation_ordinal: Option<u64>,
    pub reservation_start: Option<usize>,
    pub cursor_after: Option<usize>,
    pub live_after: Option<usize>,
    pub slots: Vec<Value>,
    pub prepares: Vec<Value>,
    pub guards: Vec<Value>,
    pub node_visits: usize,
    pub nonempty_layers: u32,
    pub successful_samples: u64,
    pub advanced_context: bool,
    pub commit_register: u32,
    pub blended_layers: u32,
    pub committed: bool,
    pub state_sha256: String,
}

pub(super) fn build(pending: &Pending) -> Result<Option<Evaluation>> {
    let Some(trace) = pending.evaluation() else {
        return Ok(None);
    };
    Ok(Some(from_parts(
        trace,
        pending.skin_prepares(),
        pending.node_guards(),
    )?))
}

pub(in crate::game::player::evidence::components::preload) fn from_parts(
    trace: &ge_core::animation::scratch::Evaluation,
    prepares: &[ge_core::mission::player::components::temporary::prepare::Prepare],
    guards: &[ge_core::animation::script_child::evaluation::Trace],
) -> Result<Evaluation> {
    let lease = trace.lease();
    let (stage, boundary) = match trace.stage() {
        Stage::Running => ("running", None),
        Stage::Pending(boundary) => ("pending", Some(format!("{boundary:?}"))),
        Stage::Returned => ("returned", None),
        Stage::Failed => ("failed", None),
        Stage::Aborted => ("aborted", None),
    };
    Ok(Evaluation {
        stage, pending_boundary: boundary, reserved: trace.reserved(), released: trace.released(),
        lease_live: lease.as_ref().map(|l| l.is_live()).transpose()?.unwrap_or(false),
        aborted: lease.as_ref().map(|l| l.is_aborted()).transpose()?.unwrap_or(false),
        arena_owner: lease.as_ref().map(|l| l.owner()), ordinal: trace.ordinal(),
        allocation_ordinal: trace.allocation_ordinal(), reservation_start: trace.reservation_start(),
        cursor_after: trace.cursor_after(), live_after: trace.live_after(),
        slots: trace.slots().iter().map(|s| {
            let (branch, tail) = match s.branch { None => (None, None), Some(Branch::LowWeight)=>(Some("low_weight"),None),Some(Branch::EmptyTail)=>(Some("empty_tail"),None),Some(Branch::Nonempty(id))=>(Some("nonempty"),Some(id.get())) };
            json!({"slot":s.slot,"weight_bits":s.weight_bits,"branch":branch,"tail":tail,"prepared":s.prepared,"masks_cleared":s.masks_cleared,"nodes":s.nodes.iter().map(|(id,result)|json!({"node":id.get(),"result":result})).collect::<Vec<_>>(),"successes":s.successes,"flags_after":s.flags_after})
        }).collect(),
        prepares: prepares.iter().map(|p|json!({"file":p.primary().file,"reference":p.primary().reference,"generation":p.descriptor().map(|d|d.generation),"descriptor_bound":p.descriptor().is_some(),"body_resident":p.body_resident(),"auxiliary_words":p.auxiliary_words(),"lookups":p.lookups(),"body_reads":p.body_reads(),"returned":p.returned().map(|r|format!("{r:?}")),"pause":p.pause().map(|r|format!("{r:?}"))})).collect(),
        guards: guards.iter().map(|g| {
            let (pointer,record) = match g.pointer { ClipPointer::ConstructorInline=>("constructor_inline",None),ClipPointer::Record(index)=>("record",Some(index)),ClipPointer::Null=>("null",None),ClipPointer::Unknown=>("unknown",None) };
            let (result,boundary) = match g.outcome { Outcome::Returned(result)=>(Some(result),None),Outcome::Pending(b)=>(None,Some(format!("{b:?}"))) };
            json!({"node":g.node.get(),"pointer":pointer,"record":record,"clip_reference":g.clip_reference,"animator_reference_read":g.animator_reference_read,"animator_reference":g.animator_reference,"result":result,"pending_boundary":boundary})
        }).collect(),
        node_visits:trace.node_visits(), nonempty_layers:trace.nonempty_layers(),successful_samples:trace.successful_samples(),advanced_context:trace.advanced_context(),commit_register:trace.commit_register(),blended_layers:trace.blended_layers(),committed:trace.committed(),
        state_sha256: super::super::super::super::words(b"ge_temporary_nonempty_evaluation_v1\0", |mut emit| {
            trace.visit_state_words(&mut emit);
            for prepare in prepares { prepare.visit_state_words(&mut emit); }
            for guard in guards { guard.visit_state_words(&mut emit); }
        }),
    })
}
