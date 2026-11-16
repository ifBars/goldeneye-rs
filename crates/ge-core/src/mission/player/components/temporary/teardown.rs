//! Mutating leaves of the temporary's explicit source destructor (80305450).
//! The ordered driver lives with the preload call. Once started, the owner is
//! partially destroyed: bind/evaluation leaves reject it and only these leaves,
//! an explicit abort or a final consume remain. Nothing here runs in Drop.
use super::{Pending, Stage};
use crate::animation::{
    layers::NodeId,
    nodes::Kind,
    script_child::destroy::{ChildDependency, ChildReceipt, Destructor, Step},
};
use anyhow::{Context, Result, ensure};

/// One slot-list node after 80320e40 read its `next` and unlinked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildCursor {
    slot: u8,
    node: NodeId,
    next: Option<NodeId>,
    /// None: a control node whose virtual destructor is unaudited.
    destructor: Option<Destructor>,
}
impl ChildCursor {
    pub fn slot(&self) -> u8 {
        self.slot
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn next(&self) -> Option<NodeId> {
        self.next
    }
    pub fn destructor(&self) -> Option<Destructor> {
        self.destructor
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(u32::from(self.slot));
        for node in [Some(self.node), self.next] {
            let value = node.map_or(0, NodeId::get);
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        emit(u32::from(self.destructor.is_some()));
        if let Some(destructor) = &self.destructor {
            destructor.visit_state_words(emit);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildTeardown {
    /// The virtual destructor returned; the identity is retired.
    Destroyed {
        receipt: ChildReceipt,
        next: Option<NodeId>,
    },
    /// The unlinked node is a control node (destructor unaudited).
    ControlNode,
    /// Paused inside the class-four destructor; its prefix stays applied.
    Blocked(ChildDependency),
}
/// Native skeletal storage released by the skeleton destructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkeletonReceipt {
    pub bones: usize,
    pub auxiliary_words: usize,
    pub block_bytes: usize,
}
impl Pending {
    pub fn teardown_started(&self) -> bool {
        self.teardown
    }
    /// Pre-destruction owner check: the returned bind of a bound fresh owner.
    pub(crate) fn ensure_teardown_owner(&self) -> Result<()> {
        ensure!(
            self.stage == Stage::BindReturned && !self.teardown,
            "temporary destructor requires the actual returned bind"
        );
        self.animator.ensure_temporary_post_bind_owner()
    }
    pub(crate) fn start_source_teardown(&mut self) -> Result<()> {
        if !self.teardown {
            self.ensure_teardown_owner()?;
            self.teardown = true;
        }
        Ok(())
    }
    fn ensure_tearing_down(&self) -> Result<()> {
        ensure!(self.teardown, "temporary destructor has not started");
        Ok(())
    }
    /// 80320e40 for one node: read `next`, unlink (patching the slot head),
    /// then enter the node's virtual destructor. The unlink is a real effect
    /// even if the destructor later pauses.
    pub(crate) fn begin_slot_child(&mut self, slot: u8, node: NodeId) -> Result<ChildCursor> {
        self.ensure_tearing_down()?;
        let ids = self.animator.node_ids();
        let links = ids.links(node).context("slot child has no links")?;
        ensure!(links.slot == slot, "slot child belongs to another slot");
        let kind = ids
            .kind(node)
            .context("slot child identity is not allocated")?;
        if kind == Kind::Script {
            // Membership is checked before any effect; it is not a gate.
            self.animator.script_children().begin_destructor(node)?;
        }
        let (ids, store) = self.animator.temporary_children_mut();
        let unlinked = ids.unlink(node)?;
        if self.head == Some(node) {
            self.head = unlinked.next;
        }
        let destructor = match kind {
            Kind::Script => Some(store.begin_destructor(node)?),
            Kind::Control => None,
        };
        Ok(ChildCursor {
            slot,
            node,
            next: unlinked.next,
            destructor,
        })
    }
    /// Resumes the node's destructor from its retained phase.
    pub(crate) fn advance_slot_child(&mut self, cursor: &mut ChildCursor) -> Result<ChildTeardown> {
        self.ensure_tearing_down()?;
        let Some(destructor) = cursor.destructor.as_mut() else {
            return Ok(ChildTeardown::ControlNode);
        };
        let (ids, store) = self.animator.temporary_children_mut();
        match store.advance_destructor(destructor)? {
            Step::Blocked(dependency) => Ok(ChildTeardown::Blocked(dependency)),
            Step::Returned(receipt) => {
                ids.retire(cursor.node)?;
                Ok(ChildTeardown::Destroyed {
                    receipt,
                    next: cursor.next,
                })
            }
        }
    }
    pub(crate) fn destroy_skeleton(&mut self) -> Result<SkeletonReceipt> {
        self.ensure_tearing_down()?;
        let runtime = self.animator.take_temporary_skeleton()?;
        Ok(SkeletonReceipt {
            bones: runtime.skeleton().bones().len(),
            auxiliary_words: runtime.auxiliary().len(),
            block_bytes: runtime.native_storage_bytes(),
        })
    }
    pub(crate) fn clear_component_words(&mut self) -> Result<()> {
        self.ensure_tearing_down()?;
        self.animator.clear_temporary_component_words();
        Ok(())
    }
}
