//! Bridge from the outer evaluator to the actual temporary animator. Slot fields
//! are its real retained bytes; the direct list is its real node links.
use super::{Animator, prepare};
use crate::animation::{
    layers::NodeId,
    nodes::Kind,
    scratch::evaluation::{Layers, Read, Step, Tail},
    script_child::evaluation::{Outcome, Trace},
};
use anyhow::{Context, Result, ensure};

pub(super) struct Bridge<'a, S> {
    pub animator: &'a Animator,
    pub head: Option<NodeId>,
    pub skin: &'a mut S,
    pub prepares: &'a mut Vec<prepare::Prepare>,
    pub guards: &'a mut Vec<Trace>,
}
fn known<T>(value: Option<T>) -> Read<T> {
    value.map_or(Read::Unknown, Read::Known)
}
fn slot_field(slot: u8, at: usize) -> usize {
    0x138 + 0x20 * usize::from(slot) + at
}
impl<S: prepare::SkinSource> Layers for Bridge<'_, S> {
    fn skeleton_present(&mut self) -> Result<bool> {
        let Some(skeleton) = self.animator.skeleton() else {
            return Ok(false);
        };
        ensure!(
            skeleton.animator() == Some(self.animator.identity()),
            "temporary skeleton belongs to another animator"
        );
        Ok(true)
    }
    fn active_count(&mut self) -> Result<Read<u8>> {
        Ok(known(self.animator.byte(0x130)))
    }
    fn weight_bits(&mut self, slot: u8) -> Result<Read<u32>> {
        Ok(known(self.animator.word(slot_field(slot, 0xc))))
    }
    fn tail(&mut self, slot: u8) -> Result<Read<Tail>> {
        let ids = self.animator.node_ids();
        if slot == 0
            && let Some(head) = self.head
        {
            // Slot0 words were superseded by the direct prepend; walk its
            // real links from HEAD to TAIL within the allocated node bound.
            let mut tail = head;
            let mut links = ids.links(head).context("temporary head links absent")?;
            ensure!(
                links.slot == 0 && links.previous.is_none(),
                "temporary head is not the slot0 list head"
            );
            for _ in 0..self.animator.script_children().len() {
                let Some(next) = links.next else {
                    return Ok(Read::Known(Tail::Node(tail)));
                };
                tail = next;
                links = ids.links(next).context("temporary list links absent")?;
                ensure!(links.slot == 0, "temporary list crosses slots");
            }
            anyhow::bail!("temporary list walk exceeds its node allocation");
        }
        Ok(match self.animator.word(slot_field(slot, 4)) {
            Some(0) => Read::Known(Tail::Null),
            _ => Read::Unknown,
        })
    }
    fn prepare(&mut self, _slot: u8) -> Result<Step<()>> {
        let skeleton = self
            .animator
            .skeleton()
            .context("temporary prepare skeleton absent")?;
        let resume = self
            .prepares
            .last()
            .is_some_and(|prepare| prepare.returned().is_none());
        if !resume {
            self.prepares.try_reserve(1)?;
            self.prepares
                .push(prepare::Prepare::new(skeleton.primary()));
        }
        let prepare = self.prepares.last_mut().expect("retained prepare");
        ensure!(
            prepare.primary() == skeleton.primary(),
            "temporary skeletal primary changed during preparation"
        );
        prepare.advance(self.skin, skeleton.auxiliary().len())
    }
    fn invoke(&mut self, node: NodeId, slot: u8, _successes: u32) -> Result<Step<u8>> {
        let ids = self.animator.node_ids();
        ensure!(
            ids.kind(node) == Some(Kind::Script)
                && ids.links(node).is_some_and(|links| links.slot == slot),
            "temporary evaluated node is not a linked script child"
        );
        let animator = self.animator;
        let trace = animator
            .script_children()
            .evaluate_guard(node, || Ok(animator.word(0x20)))?;
        self.guards.try_reserve(1)?;
        self.guards.push(trace);
        Ok(match trace.outcome {
            Outcome::Returned(byte) => Step::Done(byte),
            Outcome::Pending(_) => Step::Pending,
        })
    }
    fn previous(&mut self, node: NodeId) -> Result<Read<Option<NodeId>>> {
        Ok(known(
            self.animator
                .node_ids()
                .links(node)
                .map(|links| links.previous),
        ))
    }
}
