//! Bridge for the bind caller's suffix on the SAME temporary animator: its
//! actual slot0 head, the head's parameter/phase stores and a SECOND outer
//! evaluation. Evaluation entry points borrow `&self`, so the first bind's
//! forward, evaluation, prepares, guards and bind return stay immutable.
use super::{Pending, Stage, evaluation::Bridge, prepare};
use crate::animation::{
    layers::NodeId,
    nodes::Kind,
    scratch::{
        self,
        evaluation::{Evaluation, Progress},
    },
    script_child::{
        self,
        phase::{OrderedUpperClamp, PhaseAssignment},
        pointee::ParameterWrite,
    },
};
use anyhow::{Context, Result, ensure};

/// The caller's slot0 HEAD read and its class-four type test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostBindHead {
    Null,
    ClassFour(NodeId),
    /// A real head whose type chain does not reach class four.
    NonMatching(NodeId),
    /// Head membership/type is not established; not a known mismatch.
    Unknown,
}
impl Pending {
    fn ensure_bind_returned(&self) -> Result<()> {
        ensure!(
            self.stage() == Stage::BindReturned && self.bind_return() == Some(1),
            "post-bind suffix precedes the actual temporary bind return"
        );
        Ok(())
    }
    /// Reads the actual slot0 head through this animator's own Ids and Store.
    pub fn post_bind_head(&self) -> Result<PostBindHead> {
        self.ensure_bind_returned()?;
        let animator = &self.animator;
        let Some(head) = self.head else {
            // Direct prepend never ran: the constructor slot0 word is the head.
            return Ok(match animator.word(0x138) {
                Some(0) => PostBindHead::Null,
                _ => PostBindHead::Unknown,
            });
        };
        let ids = animator.node_ids();
        let store = animator.script_children();
        let current = ids
            .links(head)
            .is_some_and(|links| links.slot == 0 && links.previous.is_none());
        if !current {
            return Ok(PostBindHead::Unknown);
        }
        Ok(match ids.kind(head) {
            // Store allocation admits class four only; require the header to
            // be a member of this same owner's store.
            Some(Kind::Script) => match store.get(head) {
                Some(header) if header.owner() == ids.owner() => PostBindHead::ClassFour(head),
                _ => PostBindHead::Unknown,
            },
            Some(Kind::Control) => PostBindHead::NonMatching(head),
            None => PostBindHead::Unknown,
        })
    }
    fn ensure_current_class_four(&self, head: NodeId) -> Result<()> {
        ensure!(
            self.post_bind_head()? == PostBindHead::ClassFour(head),
            "post-bind store target is not the actual current class-four head"
        );
        Ok(())
    }
    /// Stores through the actual head's +60 pointee (never the dispatch setter).
    pub fn store_post_bind_parameter(&mut self, head: NodeId, bits: u32) -> Result<ParameterWrite> {
        self.ensure_current_class_four(head)?;
        self.animator
            .temporary_scripts_mut()
            .store_through_parameter(head, bits)
    }
    /// Assigns the captured ordered clamp to the actual head's +80.
    pub fn assign_post_bind_phase(
        &mut self,
        head: NodeId,
        clamp: OrderedUpperClamp,
    ) -> Result<PhaseAssignment> {
        self.ensure_current_class_four(head)?;
        self.animator
            .temporary_scripts_mut()
            .assign_phase_clamp(head, clamp)
    }
    /// A NEW outer evaluation over the current list in the shared arena.
    pub fn begin_post_bind_evaluation(
        &self,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
        skin: &mut impl prepare::SkinSource,
        prepares: &mut Vec<prepare::Prepare>,
        guards: &mut Vec<script_child::evaluation::Trace>,
    ) -> Result<Evaluation> {
        self.ensure_bind_returned()?;
        self.animator.ensure_temporary_post_bind_owner()?;
        let mut bridge = Bridge {
            animator: &self.animator,
            head: self.head,
            skin,
            prepares,
            guards,
        };
        let budget = self.animator.script_children().len();
        Evaluation::begin(context, arena, &mut bridge, budget)
    }
    pub fn advance_post_bind_evaluation(
        &self,
        evaluation: &mut Evaluation,
        context: &mut scratch::Context,
        arena: &mut scratch::Arena,
        skin: &mut impl prepare::SkinSource,
        prepares: &mut Vec<prepare::Prepare>,
        guards: &mut Vec<script_child::evaluation::Trace>,
    ) -> Result<Progress> {
        self.ensure_bind_returned()?;
        let first = self
            .evaluation
            .as_ref()
            .and_then(|first| first.lease())
            .zip(evaluation.lease());
        ensure!(
            first.is_none_or(|(first, second)| !first.same_scope(&second)),
            "post-bind evaluation cannot resume the first bind scope"
        );
        let mut bridge = Bridge {
            animator: &self.animator,
            head: self.head,
            skin,
            prepares,
            guards,
        };
        evaluation
            .advance(context, arena, &mut bridge)
            .context("post-bind evaluation")
    }
}
