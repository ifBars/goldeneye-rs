//! 800fea58..74: the temporary's virtual destructor (80305450, delete=1) in
//! source order. Every guarded branch is proven a no-op from the owner's
//! actual known words; populated or unknown state pauses at a typed
//! dependency before that step's effects. The finished destructor consumes the
//! owner into a tombstone carrying only a bounded pre-destruction snapshot.
mod snapshot;
mod state;
use super::bone_outputs::{BoneOutputs, CallBinding, Stage as OutputStage};
use crate::{
    animation::{
        layers::NodeId,
        script_child::destroy::{ChildDependency, ChildReceipt},
    },
    mission::player::components::{
        Identity,
        temporary::{self, ChildCursor, ChildTeardown, SkeletonReceipt},
    },
};
use anyhow::{Context, Result, bail, ensure};
pub use snapshot::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AttachmentList,
    ExtraOwner,
    SlotChildren,
    Skeleton,
    ComponentWords,
    AttachmentListAgain,
    BaseDestructor,
    AwaitingFree,
    Destroyed,
    Failed,
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    AttachmentList,
    ExtraOwner,
    ActiveCount,
    SlotList,
    ControlNodeDestructor,
    ChildState,
    ChildReferenceList,
    LinkedBaseChild,
    SkeletonPointer,
    BaseOwner,
    NonTerminatingBase,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AttachmentList => "preload_teardown_attachment_list",
            Self::ExtraOwner => "preload_teardown_extra_owner",
            Self::ActiveCount => "preload_teardown_active_count",
            Self::SlotList => "preload_teardown_slot_list",
            Self::ControlNodeDestructor => "preload_teardown_control_node",
            Self::ChildState => "preload_teardown_child_state",
            Self::ChildReferenceList => "preload_teardown_child_reference_list",
            Self::LinkedBaseChild => "preload_teardown_linked_base_child",
            Self::SkeletonPointer => "preload_teardown_skeleton_pointer",
            Self::BaseOwner => "preload_teardown_base_owner",
            Self::NonTerminatingBase => "preload_teardown_nonterminating_base",
        }
    }
}
impl From<ChildDependency> for Dependency {
    fn from(value: ChildDependency) -> Self {
        match value {
            ChildDependency::ChildState => Self::ChildState,
            ChildDependency::ChildReferenceList => Self::ChildReferenceList,
            ChildDependency::LinkedBaseChild => Self::LinkedBaseChild,
        }
    }
}
/// Resumable position inside the slot-list walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Walk {
    slot: u8,
    /// None: the slot's head is not read yet.
    node: Option<Option<NodeId>>,
    /// Unlinked node whose destructor has not returned yet.
    child: Option<ChildCursor>,
}
#[derive(Debug)]
pub struct Teardown {
    binding: CallBinding,
    snapshot: Snapshot,
    stage: Stage,
    dependency: Option<Dependency>,
    walk: Walk,
    children: Vec<(u8, ChildReceipt)>,
    skeleton: Option<SkeletonReceipt>,
}
impl Teardown {
    /// Requires completed six-bone outputs for this exact temporary. Captures
    /// the only diagnostic snapshot before any destructor effect.
    pub fn begin(outputs: &BoneOutputs, temporary: &temporary::Pending) -> Result<Self> {
        let binding = outputs.binding();
        ensure!(
            outputs.stage() == OutputStage::Done
                && binding.temporary == temporary.animator().identity()
                && temporary.component().reference == binding.reference,
            "temporary destructor precedes this call's six bone outputs"
        );
        temporary.ensure_teardown_owner()?;
        Ok(Self {
            binding,
            snapshot: Snapshot::capture(temporary)?,
            stage: Stage::AttachmentList,
            dependency: None,
            walk: Walk {
                slot: 0,
                node: None,
                child: None,
            },
            children: Vec::new(),
            skeleton: None,
        })
    }
    pub fn advance(&mut self, temporary: &mut temporary::Pending) -> Result<()> {
        match self.stage {
            Stage::Failed => bail!("temporary destructor failure is latched"),
            Stage::Aborted => bail!("temporary destructor was cancelled"),
            Stage::AwaitingFree | Stage::Destroyed => return Ok(()),
            _ => {}
        }
        ensure!(
            temporary.animator().identity() == self.binding.temporary,
            "temporary destructor resumed with another owner"
        );
        self.dependency = None;
        loop {
            match self.step(temporary) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(error) => {
                    self.stage = Stage::Failed;
                    return Err(error);
                }
            }
        }
    }
    fn pause(&mut self, dependency: Dependency) -> Result<bool> {
        self.dependency = Some(dependency);
        Ok(false)
    }
    /// A known zero word passes; nonzero or unknown pauses at `dependency`.
    fn zero(temporary: &temporary::Pending, at: usize) -> bool {
        temporary.animator().word(at) == Some(0)
    }
    fn step(&mut self, temporary: &mut temporary::Pending) -> Result<bool> {
        match self.stage {
            Stage::AttachmentList => {
                // 802dcb10(+0x124): the drain loop runs only on a populated list.
                if !Self::zero(temporary, 0x124) {
                    return self.pause(Dependency::AttachmentList);
                }
                temporary.start_source_teardown()?;
                self.stage = Stage::ExtraOwner;
            }
            Stage::ExtraOwner => {
                if !Self::zero(temporary, 0x120) {
                    return self.pause(Dependency::ExtraOwner);
                }
                self.stage = Stage::SlotChildren;
            }
            Stage::SlotChildren => return self.slot_children(temporary),
            Stage::Skeleton => {
                if temporary.animator().skeleton().is_some() {
                    self.skeleton = Some(temporary.destroy_skeleton()?);
                } else if !Self::zero(temporary, 0x114) {
                    return self.pause(Dependency::SkeletonPointer);
                }
                self.stage = Stage::ComponentWords;
            }
            Stage::ComponentWords => {
                temporary.clear_component_words()?;
                self.stage = Stage::AttachmentListAgain;
            }
            Stage::AttachmentListAgain => {
                // 802dca60(+0x124, -1): the same drain, without frees.
                if !Self::zero(temporary, 0x124) {
                    return self.pause(Dependency::AttachmentList);
                }
                self.stage = Stage::BaseDestructor;
            }
            Stage::BaseDestructor => {
                // 80303af0: a nonzero +0x110 is a non-returning branch.
                if temporary.animator().byte(0x110) != Some(0) {
                    return self.pause(Dependency::NonTerminatingBase);
                }
                // 80300990: owned buffers, children and owner-list unlink.
                let animator = temporary.animator();
                if animator.item().is_some()
                    || !(Self::zero(temporary, 0x5c)
                        && Self::zero(temporary, 0x60)
                        && Self::zero(temporary, 0x10)
                        && Self::zero(temporary, 8))
                {
                    return self.pause(Dependency::BaseOwner);
                }
                self.stage = Stage::AwaitingFree;
                return Ok(false);
            }
            Stage::AwaitingFree | Stage::Destroyed | Stage::Failed | Stage::Aborted => {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// 80320e40(slot, 0) for each slot below the actual active count.
    fn slot_children(&mut self, temporary: &mut temporary::Pending) -> Result<bool> {
        let Some(count) = temporary.animator().byte(0x130) else {
            return self.pause(Dependency::ActiveCount);
        };
        if self.walk.slot >= count {
            self.stage = Stage::Skeleton;
            return Ok(true);
        }
        if let Some(mut cursor) = self.walk.child {
            let result = temporary.advance_slot_child(&mut cursor);
            // The destructor's reached phase is retained even on a pause.
            self.walk.child = Some(cursor);
            return match result? {
                ChildTeardown::Destroyed { receipt, next } => {
                    self.children.try_reserve(1)?;
                    self.children.push((cursor.slot(), receipt));
                    self.walk.child = None;
                    self.walk.node = Some(next);
                    Ok(true)
                }
                ChildTeardown::ControlNode => self.pause(Dependency::ControlNodeDestructor),
                ChildTeardown::Blocked(dependency) => self.pause(dependency.into()),
            };
        }
        let slot = self.walk.slot;
        let node = match self.walk.node {
            Some(node) => node,
            None => {
                let head = if slot == 0 { temporary.head() } else { None };
                if head.is_none() && !Self::zero(temporary, 0x138 + 0x20 * usize::from(slot)) {
                    // A populated slot not retained as this temporary's list.
                    return self.pause(Dependency::SlotList);
                }
                self.walk.node = Some(head);
                head
            }
        };
        let Some(node) = node else {
            self.walk = Walk {
                slot: slot.checked_add(1).context("slot overflow")?,
                node: None,
                child: None,
            };
            return Ok(true);
        };
        // Reads `next` and unlinks before the destructor runs.
        self.walk.child = Some(temporary.begin_slot_child(slot, node)?);
        Ok(true)
    }
    /// The unlinked child whose destructor is in progress, if any.
    pub fn child_in_progress(&self) -> Option<ChildCursor> {
        self.walk.child
    }
    /// Explicit cancellation. No destructor effect is undone or performed;
    /// the partially destroyed owner may only be dropped by the host.
    pub fn abort(&mut self) {
        if !matches!(self.stage, Stage::AwaitingFree | Stage::Destroyed) {
            self.stage = Stage::Aborted;
        }
    }
    /// The final heap free: consumes the owner into its tombstone.
    pub fn finish(mut self, temporary: temporary::Pending) -> Result<Tombstone> {
        ensure!(
            self.stage == Stage::AwaitingFree
                && temporary.animator().identity() == self.binding.temporary
                && temporary.teardown_started(),
            "temporary free precedes its completed destructor"
        );
        drop(temporary);
        self.stage = Stage::Destroyed;
        Ok(Tombstone {
            binding: self.binding,
            snapshot: self.snapshot,
            children: self.children,
            skeleton: self.skeleton,
        })
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn dependency(&self) -> Option<Dependency> {
        self.dependency
    }
    pub fn binding(&self) -> CallBinding {
        self.binding
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn children(&self) -> &[(u8, ChildReceipt)] {
        &self.children
    }
    pub fn skeleton(&self) -> Option<SkeletonReceipt> {
        self.skeleton
    }
}
/// A destroyed temporary: historical evidence only, no live owner.
#[derive(Debug)]
pub struct Tombstone {
    binding: CallBinding,
    snapshot: Snapshot,
    children: Vec<(u8, ChildReceipt)>,
    skeleton: Option<SkeletonReceipt>,
}
impl Tombstone {
    pub fn temporary(&self) -> Identity {
        self.binding.temporary
    }
    pub fn binding(&self) -> CallBinding {
        self.binding
    }
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn children(&self) -> &[(u8, ChildReceipt)] {
        &self.children
    }
    pub fn skeleton(&self) -> Option<SkeletonReceipt> {
        self.skeleton
    }
    /// The bind's historical return, captured before destruction.
    pub fn bind_return(&self) -> Option<u32> {
        self.snapshot.bind_return()
    }
}

#[cfg(test)]
mod tests;
