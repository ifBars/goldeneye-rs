//! A new preload animator and skeleton. Live player receivers are inputs only.
mod evaluation;
pub mod prepare;
mod state;
use super::{Animator, Identity, Registry, ResourceBinding, SkeletonBudget, SkeletonInput};
use crate::animation::{
    layers::NodeId,
    scratch::{
        self,
        evaluation::{Boundary, CancellationLease, Evaluation, Progress},
    },
    script_child,
};
use anyhow::{Context, Result, bail, ensure};
pub use prepare::{SkinBody, SkinDescriptor, SkinLookup, SkinSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AwaitingSkeleton,
    BindingSkeleton,
    AwaitingFactory,
    InitializingFactory,
    AwaitingInitializer,
    AwaitingInsertion,
    AwaitingForwardTick,
    ForwardTick,
    AwaitingEvaluation,
    /// A nonempty evaluation holds its reserved scope at a reached pause.
    EvaluatingBind,
    BindReturned,
    Failed,
    /// Explicit cancellation released the retained scope; no bind return.
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    Skeleton,
    SkinRemap,
    FactoryClass,
    FactoryInitializer,
    ScriptBodyResidency,
    ForwardTick,
    Evaluation,
    EvaluationSkin,
    EvaluationAuxiliary,
    EvaluationSampling,
    EvaluationLayers,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skeleton => "temporary_skeleton",
            Self::SkinRemap => "temporary_skin_remap",
            Self::FactoryClass => "temporary_factory_class",
            Self::FactoryInitializer => "temporary_script_initializer",
            Self::ScriptBodyResidency => "temporary_script_body_residency",
            Self::ForwardTick => "temporary_forward_tick",
            Self::Evaluation => "temporary_animation_evaluation",
            Self::EvaluationSkin => "temporary_evaluation_skin",
            Self::EvaluationAuxiliary => "temporary_evaluation_auxiliary_clear",
            Self::EvaluationSampling => "temporary_evaluation_sampling",
            Self::EvaluationLayers => "temporary_evaluation_layers",
        }
    }
}
#[derive(Debug)]
pub struct Pending {
    animator: Animator,
    source_receiver: Identity,
    component: ResourceBinding,
    skin: ResourceBinding,
    stage: Stage,
    factory_node: Option<NodeId>,
    head: Option<NodeId>,
    factory_returned: bool,
    forward: Option<script_child::forward_tick::Trace>,
    maintenance_skipped: bool,
    reverse_skipped: bool,
    refresh: Option<scratch::Refresh>,
    evaluation: Option<Evaluation>,
    prepares: Vec<prepare::Prepare>,
    guards: Vec<script_child::evaluation::Trace>,
}
impl Pending {
    /// Capture CURRENT FP wrapper and skeletal secondary pair. The supplied
    /// registry is the same retained registry that allocated the live receiver.
    pub fn begin(
        current_fp: &Animator,
        registry: &mut Registry,
        extracted_reference: u32,
    ) -> Result<Self> {
        ensure!(
            registry.resolve(current_fp.identity().id()) == Some(current_fp.identity()),
            "temporary receiver does not belong to retained component registry"
        );
        let identity = registry.allocate(current_fp.player())?;
        let animator = Animator::from_constructor(identity);
        let binding = current_fp
            .component()
            .context("temporary FP wrapper absent")?;
        let skin = current_fp
            .skeleton()
            .context("temporary FP skeleton absent")?
            .secondary()
            .context("temporary FP secondary binding absent")?;
        Ok(Self {
            animator,
            source_receiver: current_fp.identity(),
            component: ResourceBinding {
                file: binding.file,
                reference: extracted_reference,
            },
            skin,
            stage: Stage::AwaitingSkeleton,
            factory_node: None,
            head: None,
            factory_returned: false,
            forward: None,
            maintenance_skipped: false,
            reverse_skipped: false,
            refresh: None,
            evaluation: None,
            prepares: Vec::new(),
            guards: Vec::new(),
        })
    }
    pub fn bind_skeleton(
        &mut self,
        input: SkeletonInput<'_>,
        budget: SkeletonBudget,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingSkeleton && input.primary == self.skin,
            "temporary skeleton source differs or bind replays"
        );
        ensure!(
            self.skin.reference >> 24 == 0x0d,
            "temporary skin remapping provider unavailable"
        );
        self.stage = Stage::BindingSkeleton;
        match self
            .animator
            .bind_temporary_skeleton(input, self.component, budget)
        {
            Ok(()) => {
                self.stage = Stage::AwaitingFactory;
                Ok(())
            }
            Err(error) => {
                self.stage = Stage::Failed;
                Err(error)
            }
        }
    }
    pub fn initialize_factory(&mut self, source: &mut impl script_child::Provider) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingFactory,
            "temporary direct factory is out of order"
        );
        let class = (self.component.reference >> 24) & 0x7f;
        if class != 3 && class != 4 {
            // Source direct factory returns NULL for raw0/other unhandled class.
            self.factory_returned = true;
            self.stage = Stage::AwaitingForwardTick;
            return Ok(());
        }
        if class == 3 {
            return Ok(());
        }
        self.stage = Stage::InitializingFactory;
        let result = (|| {
            let id = self.animator.allocate_temporary_script()?;
            self.factory_node = Some(id);
            self.animator
                .temporary_scripts_mut()
                .initialize_prefix(id, source)
        })();
        self.after_initializer(result)
    }
    pub fn initialize_phase_zero(
        &mut self,
        source: &mut impl script_child::initialization::Source,
        scaled_step_bits: u32,
        maximum_entries: usize,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingInitializer,
            "temporary phase-zero source order differs"
        );
        let id = self.factory_node.context("temporary factory node absent")?;
        let speed = self
            .animator
            .word(0x108)
            .context("temporary speed unknown")?;
        let result = self.animator.temporary_scripts_mut().initialize_phase_zero(
            id,
            source,
            script_child::initialization::Inputs {
                component_speed_bits: speed,
                scaled_step_bits,
                maximum_entries,
            },
        );
        self.after_initializer(result)
    }
    pub fn prepare_position(
        &mut self,
        source: &mut impl script_child::position::Source,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingInitializer,
            "temporary position source order differs"
        );
        let id = self.factory_node.context("temporary factory node absent")?;
        let result = self
            .animator
            .temporary_scripts_mut()
            .prepare_phase_zero_position(id, source);
        self.after_initializer(result)
    }
    fn after_initializer(&mut self, result: Result<()>) -> Result<()> {
        if result.is_err() {
            self.stage = Stage::Failed;
            return result;
        }
        self.stage = if self
            .factory_header()
            .is_some_and(|header| header.initializer_returned())
        {
            Stage::AwaitingInsertion
        } else {
            Stage::AwaitingInitializer
        };
        Ok(())
    }
    pub fn finish_factory(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingInsertion,
            "temporary insertion precedes factory return"
        );
        let id = self
            .factory_node
            .context("temporary insertion node absent")?;
        self.animator.insert_temporary_script(id)?;
        self.head = Some(id);
        self.factory_returned = true;
        self.stage = Stage::AwaitingForwardTick;
        Ok(())
    }
    pub fn forward_zero(
        &mut self,
        source: &mut impl script_child::forward_tick::Source,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingForwardTick,
            "temporary zero-time forward order differs"
        );
        self.stage = Stage::ForwardTick;
        if let Some(id) = self.head {
            let mut trace = script_child::forward_tick::Trace::default();
            let result = self
                .animator
                .temporary_scripts_mut()
                .forward_zero(id, source, &mut trace);
            self.forward = Some(trace);
            if let Err(error) = result {
                self.stage = Stage::Failed;
                return Err(error);
            }
            if self
                .forward
                .as_ref()
                .is_some_and(|t| t.stage() == script_child::forward_tick::Stage::AwaitingBody)
            {
                return Ok(());
            }
        }
        // Actual fresh slot flags0 skip maintenance. Semantic bit8 skips the
        // reverse initflag0 callback/root motion; no live capture cleared it.
        ensure!(
            self.animator.byte(0x140) == Some(0)
                && self.animator.byte(0x141) == Some(0)
                && self.animator.byte(0xc) == Some(0)
                && self.animator.byte(0xd) == Some(0x0b),
            "temporary maintenance/reverse fresh branches differ"
        );
        self.maintenance_skipped = true;
        self.reverse_skipped = self.head.is_some();
        self.stage = Stage::AwaitingEvaluation;
        Ok(())
    }
    /// A populated direct list awaits actual sampling. Empty lists execute the
    /// existing real shared reservation/context stores/release before return1.
    pub fn finish_bind_zero_time(
        &mut self,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingEvaluation,
            "temporary evaluation is out of order"
        );
        if self.head.is_some() {
            return Ok(());
        }
        match self.animator.temporary_empty_refresh(context, arena) {
            Ok(refresh) => {
                self.refresh = Some(refresh);
                self.stage = Stage::BindReturned;
                Ok(())
            }
            Err(error) => {
                self.stage = Stage::Failed;
                Err(error)
            }
        }
    }
    /// Nonempty direct list: runs the actual shared evaluator through the real
    /// node and primary-skin provider. Pauses keep the reserved scope and
    /// resume at the same cursor; only the evaluator's release returns bind1.
    /// After the first call the cloned `evaluation_lease` is available.
    pub fn evaluate_bind_zero_time(
        &mut self,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
        skin: &mut impl SkinSource,
    ) -> Result<()> {
        match self.stage() {
            Stage::AwaitingEvaluation => {
                ensure!(
                    self.head.is_some() && self.evaluation.is_none(),
                    "empty temporary list uses the empty refresh"
                );
                let begun = self
                    .animator
                    .ensure_temporary_evaluation_owner()
                    .and_then(|()| {
                        let mut bridge = evaluation::Bridge {
                            animator: &self.animator,
                            head: self.head,
                            skin: &mut *skin,
                            prepares: &mut self.prepares,
                            guards: &mut self.guards,
                        };
                        let budget = self.animator.script_children().len();
                        Evaluation::begin(context, arena, &mut bridge, budget)
                    });
                match begun {
                    Ok(evaluation) => {
                        self.evaluation = Some(evaluation);
                        self.stage = Stage::EvaluatingBind;
                    }
                    Err(error) => {
                        self.stage = Stage::Failed;
                        return Err(error);
                    }
                }
            }
            Stage::EvaluatingBind => {}
            Stage::Aborted => {
                self.stage = Stage::Aborted;
                bail!("temporary evaluation scope was cancelled");
            }
            _ => bail!("temporary nonempty evaluation is out of order"),
        }
        let evaluation = self
            .evaluation
            .as_mut()
            .context("temporary evaluation scope absent")?;
        let mut bridge = evaluation::Bridge {
            animator: &self.animator,
            head: self.head,
            skin,
            prepares: &mut self.prepares,
            guards: &mut self.guards,
        };
        match evaluation.advance(context, arena, &mut bridge) {
            Ok(Progress::Returned) => {
                self.stage = Stage::BindReturned;
                Ok(())
            }
            Ok(Progress::Pending(_)) => Ok(()),
            Err(error) => {
                self.stage = if evaluation.stage() == scratch::evaluation::Stage::Aborted {
                    Stage::Aborted
                } else {
                    Stage::Failed
                };
                Err(error)
            }
        }
    }
    /// Explicit cancellation of a paused or failed nonempty evaluation. The
    /// retained prefix stays as evidence; nothing is replayed or returned.
    /// A scope already reaped through its lease is observed, not released.
    pub fn abort_evaluation(
        &mut self,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
    ) -> Result<()> {
        ensure!(
            matches!(
                self.stage(),
                Stage::EvaluatingBind | Stage::Failed | Stage::Aborted
            ),
            "temporary evaluation has no retained scope to abort"
        );
        self.evaluation
            .as_mut()
            .context("temporary evaluation scope absent")?
            .abort(context, arena)?;
        self.stage = Stage::Aborted;
        Ok(())
    }
    /// Cloneable host cancellation handle for the reserved scope.
    pub fn evaluation_lease(&self) -> Option<CancellationLease> {
        self.evaluation.as_ref()?.lease()
    }
    pub fn evaluation(&self) -> Option<&Evaluation> {
        self.evaluation.as_ref()
    }
    pub fn skin_prepares(&self) -> &[prepare::Prepare] {
        &self.prepares
    }
    pub fn node_guards(&self) -> &[script_child::evaluation::Trace] {
        &self.guards
    }
    /// Reports Aborted once a host reaped the cloned lease.
    pub fn stage(&self) -> Stage {
        let reaped = matches!(self.stage, Stage::EvaluatingBind | Stage::Failed)
            && self
                .evaluation
                .as_ref()
                .is_some_and(|e| e.stage() == scratch::evaluation::Stage::Aborted);
        if reaped { Stage::Aborted } else { self.stage }
    }
    pub fn animator(&self) -> &Animator {
        &self.animator
    }
    pub fn source_receiver(&self) -> Identity {
        self.source_receiver
    }
    pub fn component(&self) -> ResourceBinding {
        self.component
    }
    pub fn skin(&self) -> ResourceBinding {
        self.skin
    }
    pub fn factory_node(&self) -> Option<NodeId> {
        self.factory_node
    }
    pub fn head(&self) -> Option<NodeId> {
        self.head
    }
    pub fn factory_returned(&self) -> bool {
        self.factory_returned
    }
    pub fn factory_header(&self) -> Option<&script_child::Header> {
        self.factory_node
            .and_then(|id| self.animator.script_children().get(id))
    }
    pub fn forward_tick(&self) -> Option<&script_child::forward_tick::Trace> {
        self.forward.as_ref()
    }
    pub fn maintenance_skipped(&self) -> bool {
        self.maintenance_skipped
    }
    pub fn reverse_skipped(&self) -> bool {
        self.reverse_skipped
    }
    pub fn refresh(&self) -> Option<&scratch::Refresh> {
        self.refresh.as_ref()
    }
    pub fn bind_return(&self) -> Option<u32> {
        (self.stage == Stage::BindReturned).then_some(1)
    }
    pub fn dependency(&self) -> Option<Dependency> {
        if self
            .factory_header()
            .is_some_and(|h| h.stage() == script_child::Stage::AwaitingScriptBody)
            || self
                .forward
                .as_ref()
                .is_some_and(|t| t.stage() == script_child::forward_tick::Stage::AwaitingBody)
        {
            return Some(Dependency::ScriptBodyResidency);
        }
        match self.stage() {
            Stage::AwaitingSkeleton if self.skin.reference >> 24 != 0x0d => {
                Some(Dependency::SkinRemap)
            }
            Stage::AwaitingSkeleton | Stage::BindingSkeleton => Some(Dependency::Skeleton),
            Stage::AwaitingFactory if (self.component.reference >> 24) & 0x7f == 3 => {
                Some(Dependency::FactoryClass)
            }
            Stage::InitializingFactory | Stage::AwaitingInitializer => {
                Some(Dependency::FactoryInitializer)
            }
            Stage::AwaitingForwardTick | Stage::ForwardTick => Some(Dependency::ForwardTick),
            Stage::AwaitingEvaluation => Some(Dependency::Evaluation),
            Stage::EvaluatingBind => self.evaluation_dependency(),
            _ => None,
        }
    }
}

impl Pending {
    fn evaluation_dependency(&self) -> Option<Dependency> {
        let scratch::evaluation::Stage::Pending(boundary) = self.evaluation.as_ref()?.stage()
        else {
            return None;
        };
        Some(match boundary {
            Boundary::Prepare => match self.prepares.last().and_then(|p| p.pause()) {
                Some(prepare::Pause::Auxiliary) => Dependency::EvaluationAuxiliary,
                _ => Dependency::EvaluationSkin,
            },
            Boundary::Node => Dependency::EvaluationSampling,
            Boundary::ActiveCount
            | Boundary::Weight
            | Boundary::Tail
            | Boundary::Previous
            | Boundary::LayerBlend
            | Boundary::Commit => Dependency::EvaluationLayers,
        })
    }
}

#[cfg(test)]
mod tests;
