//! Bind caller suffix after the temporary's actual bind returned 1: current
//! head type test, 0.5 through the head's parameter pointee, the unconditional
//! process-scalar read, the ordered clamp into +80 and a SECOND evaluation.
//! It stops at the six-bone output provider; nothing later is inferred.
mod scalar;
mod state;
use super::{Call, CallStage, Request};
use crate::{
    animation::{
        scratch::{self, CancellationLease, Evaluation, evaluation::Progress},
        script_child::{
            self,
            phase::{OrderedUpperClamp, PhaseAssignment, ScalarCapture},
            pointee::{ParameterStore, ParameterWrite},
        },
    },
    mission::player::components::{
        Identity,
        temporary::{self, PostBindHead, SkinSource, prepare::Prepare},
    },
};
use anyhow::{Context, Result, bail, ensure};
pub use scalar::{ProcessScalar, STARTUP_BITS};

/// Literal stored through the head's parameter pointee.
pub const PARAMETER_BITS: u32 = 0x3f00_0000;
/// Literal upper operand of the ordered clamp (+0).
pub const UPPER_BITS: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AwaitingHead,
    AwaitingParameter,
    ReadingScalar,
    AssigningPhase,
    Evaluating,
    AwaitingBoneOutputs,
    Failed,
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    Head,
    ParameterPointer,
    EvaluationSkin,
    EvaluationAuxiliary,
    EvaluationSampling,
    EvaluationLayers,
    BoneOutputs,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Head => "preload_post_bind_head",
            Self::ParameterPointer => "preload_post_bind_parameter_pointer",
            Self::EvaluationSkin => "preload_post_bind_evaluation_skin",
            Self::EvaluationAuxiliary => "preload_post_bind_evaluation_auxiliary_clear",
            Self::EvaluationSampling => "preload_post_bind_evaluation_sampling",
            Self::EvaluationLayers => "preload_post_bind_evaluation_layers",
            Self::BoneOutputs => "preload_temporary_bone_outputs",
        }
    }
}
#[derive(Debug)]
pub struct PostBind {
    request: Request,
    temporary: Identity,
    stage: Stage,
    head: Option<PostBindHead>,
    head_reads: u32,
    parameter: Option<ParameterStore>,
    capture: Option<ScalarCapture>,
    phase: Option<OrderedUpperClamp>,
    assignment: Option<PhaseAssignment>,
    evaluation: Option<Evaluation>,
    prepares: Vec<Prepare>,
    guards: Vec<script_child::evaluation::Trace>,
}
impl PostBind {
    /// Binds to the call whose extraction actually returned and whose
    /// reference the temporary captured. A returned extraction stores only a
    /// validated nonzero class-four word, so None is the source's known zero.
    pub fn begin(call: &Call, temporary: &temporary::Pending) -> Result<Self> {
        ensure!(
            call.stage() == CallStage::AwaitingTemporary && call.extraction_returned(),
            "post-bind suffix precedes this call's actual reference extraction"
        );
        ensure!(
            temporary.bind_return() == Some(1)
                && temporary.source_receiver() == call.request().receiver
                && temporary.animator().player() == call.request().player
                && temporary.component().reference == call.reference().unwrap_or(0),
            "post-bind suffix requires this call's actual returned temporary bind"
        );
        Ok(Self {
            request: call.request(),
            temporary: temporary.animator().identity(),
            stage: Stage::AwaitingHead,
            head: None,
            head_reads: 0,
            parameter: None,
            capture: None,
            phase: None,
            assignment: None,
            evaluation: None,
            prepares: Vec::new(),
            guards: Vec::new(),
        })
    }
    /// Advances in source order until a typed pause, failure or the six-bone
    /// output provider. Completed steps are never repeated.
    pub fn advance(
        &mut self,
        temporary: &mut temporary::Pending,
        scalar: &mut ProcessScalar,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
        skin: &mut impl SkinSource,
    ) -> Result<()> {
        match self.stage() {
            Stage::Failed => bail!("post-bind suffix failure is latched"),
            Stage::Aborted => {
                self.stage = Stage::Aborted;
                bail!("post-bind suffix was cancelled");
            }
            _ => {}
        }
        ensure!(
            temporary.animator().identity() == self.temporary,
            "post-bind suffix resumed with another temporary"
        );
        loop {
            match self.step(temporary, scalar, context, arena, skin) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(error) => {
                    self.stage = if self.stage() == Stage::Aborted {
                        Stage::Aborted
                    } else {
                        Stage::Failed
                    };
                    return Err(error);
                }
            }
        }
    }
    /// One source step; false means a pause or the final stop point.
    fn step(
        &mut self,
        temporary: &mut temporary::Pending,
        scalar: &mut ProcessScalar,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
        skin: &mut impl SkinSource,
    ) -> Result<bool> {
        match self.stage {
            Stage::AwaitingHead => {
                let head = temporary.post_bind_head()?;
                self.head_reads += 1;
                self.head = Some(head);
                self.stage = match head {
                    PostBindHead::ClassFour(_) => Stage::AwaitingParameter,
                    // NULL or known nonmatching skips both stores and the read.
                    PostBindHead::Null | PostBindHead::NonMatching(_) => Stage::Evaluating,
                    PostBindHead::Unknown => return Ok(false),
                };
            }
            Stage::AwaitingParameter => {
                match temporary.store_post_bind_parameter(self.class_four()?, PARAMETER_BITS)? {
                    ParameterWrite::Stored(receipt) => {
                        self.parameter = Some(receipt);
                        self.stage = Stage::ReadingScalar;
                    }
                    ParameterWrite::Pending(_) => return Ok(false),
                }
            }
            Stage::ReadingScalar => {
                let capture = scalar.read()?;
                ensure!(
                    scalar.issued(&capture),
                    "process scalar capture provenance differs"
                );
                self.capture = Some(capture);
                self.stage = Stage::AssigningPhase;
            }
            Stage::AssigningPhase => {
                let capture = self.capture.clone().context("scalar capture absent")?;
                let clamp = OrderedUpperClamp::new(capture, UPPER_BITS);
                let assignment =
                    temporary.assign_post_bind_phase(self.class_four()?, clamp.clone())?;
                self.phase = Some(clamp);
                self.assignment = Some(assignment);
                self.stage = Stage::Evaluating;
            }
            Stage::Evaluating => {
                if self.evaluation.is_none() {
                    // Registered before any return so a host can reap it.
                    self.evaluation = Some(temporary.begin_post_bind_evaluation(
                        context,
                        arena,
                        skin,
                        &mut self.prepares,
                        &mut self.guards,
                    )?);
                }
                let evaluation = self.evaluation.as_mut().expect("retained evaluation");
                match temporary.advance_post_bind_evaluation(
                    evaluation,
                    context,
                    arena,
                    skin,
                    &mut self.prepares,
                    &mut self.guards,
                )? {
                    Progress::Returned => self.stage = Stage::AwaitingBoneOutputs,
                    Progress::Pending(_) => return Ok(false),
                }
            }
            Stage::AwaitingBoneOutputs | Stage::Failed | Stage::Aborted => return Ok(false),
        }
        Ok(true)
    }
    fn class_four(&self) -> Result<crate::animation::layers::NodeId> {
        match self.head {
            Some(PostBindHead::ClassFour(id)) => Ok(id),
            _ => bail!("post-bind store reached without a class-four head"),
        }
    }
    /// Explicit cancellation. Releases only this suffix's own second scope;
    /// a scope already reaped through its lease is observed, not released.
    pub fn abort(
        &mut self,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
    ) -> Result<()> {
        if let Some(evaluation) = &mut self.evaluation
            && !matches!(evaluation.stage(), scratch::evaluation::Stage::Returned)
        {
            evaluation.abort(context, arena)?;
        }
        self.stage = Stage::Aborted;
        Ok(())
    }
    pub fn evaluation_lease(&self) -> Option<CancellationLease> {
        self.evaluation.as_ref()?.lease()
    }
    /// Reports Aborted once a host reaped the second scope's lease.
    pub fn stage(&self) -> Stage {
        let reaped = matches!(self.stage, Stage::Evaluating | Stage::Failed)
            && self
                .evaluation
                .as_ref()
                .is_some_and(|e| e.stage() == scratch::evaluation::Stage::Aborted);
        if reaped { Stage::Aborted } else { self.stage }
    }
    pub fn dependency(&self) -> Option<Dependency> {
        match self.stage() {
            Stage::AwaitingHead => Some(Dependency::Head),
            Stage::AwaitingParameter => Some(Dependency::ParameterPointer),
            Stage::Evaluating => {
                use temporary::Dependency as T;
                match temporary::evaluation_dependency(self.evaluation.as_ref()?, &self.prepares)? {
                    T::EvaluationSkin => Some(Dependency::EvaluationSkin),
                    T::EvaluationAuxiliary => Some(Dependency::EvaluationAuxiliary),
                    T::EvaluationSampling => Some(Dependency::EvaluationSampling),
                    _ => Some(Dependency::EvaluationLayers),
                }
            }
            Stage::AwaitingBoneOutputs => Some(Dependency::BoneOutputs),
            _ => None,
        }
    }
    pub fn request(&self) -> Request {
        self.request
    }
    pub fn temporary_identity(&self) -> Identity {
        self.temporary
    }
    pub fn head(&self) -> Option<PostBindHead> {
        self.head
    }
    pub fn head_reads(&self) -> u32 {
        self.head_reads
    }
    pub fn parameter(&self) -> Option<ParameterStore> {
        self.parameter
    }
    pub fn capture(&self) -> Option<&ScalarCapture> {
        self.capture.as_ref()
    }
    pub fn phase(&self) -> Option<&OrderedUpperClamp> {
        self.phase.as_ref()
    }
    pub fn assignment(&self) -> Option<PhaseAssignment> {
        self.assignment
    }
    pub fn evaluation(&self) -> Option<&Evaluation> {
        self.evaluation.as_ref()
    }
    pub fn skin_prepares(&self) -> &[Prepare] {
        &self.prepares
    }
    pub fn node_guards(&self) -> &[script_child::evaluation::Trace] {
        &self.guards
    }
}

#[cfg(test)]
mod tests;
