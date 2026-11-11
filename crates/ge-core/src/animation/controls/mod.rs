//! Owned class-control snapshots. Captured locals and auxiliary words are kept
//! separate from sampled poses, node-list membership and native addresses.
//! IDs are per-component host handles, not native allocator or SDK history.
use crate::animation::{layers::NodeId, skeleton::LocalPose};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

/// Nonzero host identity of the animator owning these snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerId(u64);
impl OwnerId {
    pub fn new(value: u64) -> Result<Self> {
        ensure!(value != 0, "control owner identity is zero");
        Ok(Self(value))
    }
    pub fn get(self) -> u64 {
        self.0
    }
    fn visit_state_words(self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.0 >> 32) as u32);
        emit(self.0 as u32);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub nodes: usize,
    pub bones: usize,
    pub auxiliary_words: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            nodes: 4096,
            bones: 128,
            auxiliary_words: 4096,
        }
    }
}
#[derive(Debug)]
pub struct Control {
    component: OwnerId,
    locals: Option<Vec<LocalPose>>,
    auxiliary: Option<Vec<u32>>,
    transition_bits: u32,
    rate_bits: u32,
    flags: u32,
}
#[derive(Debug)]
pub struct Store {
    component: OwnerId,
    next: u64,
    controls: BTreeMap<NodeId, Control>,
    budget: Budget,
}
impl Store {
    pub fn from_constructor(component: OwnerId, budget: Budget) -> Self {
        Self {
            component,
            next: 0,
            controls: BTreeMap::new(),
            budget,
        }
    }
    /// Retain the actual header allocation before the owner's refresh/backing
    /// operations. A later failure must not discard this reached allocation.
    pub fn allocate(&mut self) -> Result<NodeId> {
        ensure!(
            self.controls.len() < self.budget.nodes,
            "control allocation budget exhausted"
        );
        let next = self
            .next
            .checked_add(1)
            .context("control identity overflow")?;
        let id = NodeId::new(next)?;
        self.controls.insert(
            id,
            Control {
                component: self.component,
                locals: None,
                auxiliary: None,
                transition_bits: 1f32.to_bits(),
                rate_bits: 0,
                flags: 0,
            },
        );
        self.next = next;
        Ok(id)
    }
    pub fn capture(
        &mut self,
        id: NodeId,
        locals: Vec<LocalPose>,
        auxiliary: Vec<u32>,
    ) -> Result<()> {
        ensure!(
            locals.len() <= self.budget.bones && auxiliary.len() <= self.budget.auxiliary_words,
            "control backing budget exhausted"
        );
        let control = self
            .controls
            .get_mut(&id)
            .context("control header owner is absent")?;
        ensure!(
            control.locals.is_none() && control.auxiliary.is_none(),
            "control backing is already bound"
        );
        control.locals = Some(locals);
        control.auxiliary = Some(auxiliary);
        Ok(())
    }
    pub fn clone_control(&mut self, source: NodeId) -> Result<NodeId> {
        // Clone uses a fresh base/header constructor; only backing is copied.
        let id = self.allocate()?;
        let source = self
            .controls
            .get(&source)
            .context("cloned control owner is absent")?;
        let mut locals = Vec::new();
        let bones = source
            .locals
            .as_ref()
            .context("cloned control locals are unbound")?;
        locals.try_reserve_exact(bones.len())?;
        locals.extend_from_slice(bones);
        let mut auxiliary = Vec::new();
        let words = source
            .auxiliary
            .as_ref()
            .context("cloned control auxiliary storage is unbound")?;
        auxiliary.try_reserve_exact(words.len())?;
        auxiliary.extend_from_slice(words);
        let target = self.controls.get_mut(&id).expect("retained clone header");
        target.locals = Some(locals);
        target.auxiliary = Some(auxiliary);
        Ok(id)
    }
    pub fn get(&self, id: NodeId) -> Option<&Control> {
        self.controls.get(&id)
    }
    pub fn len(&self) -> usize {
        self.controls.len()
    }
    pub fn is_empty(&self) -> bool {
        self.controls.is_empty()
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.component.visit_state_words(&mut emit);
        emit((self.next >> 32) as u32);
        emit(self.next as u32);
        emit(self.controls.len() as u32);
        for (id, control) in &self.controls {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
            control.visit_state_words(&mut emit);
        }
    }
}
impl Control {
    pub fn component(&self) -> OwnerId {
        self.component
    }
    pub fn locals(&self) -> Result<&[LocalPose]> {
        self.locals.as_deref().context("control locals are unbound")
    }
    pub fn auxiliary(&self) -> Result<&[u32]> {
        self.auxiliary
            .as_deref()
            .context("control auxiliary storage is unbound")
    }
    pub fn transition_bits(&self) -> u32 {
        self.transition_bits
    }
    pub fn rate_bits(&self) -> u32 {
        self.rate_bits
    }
    pub fn flags(&self) -> u32 {
        self.flags
    }
    fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(self.transition_bits);
        emit(self.rate_bits);
        emit(self.flags);
        emit(u32::from(self.locals.is_some()));
        if let Some(locals) = &self.locals {
            emit(locals.len() as u32);
            for bone in locals {
                for word in bone
                    .translation_bits
                    .into_iter()
                    .chain(bone.quaternion_bits)
                {
                    emit(word);
                }
            }
        }
        emit(u32::from(self.auxiliary.is_some()));
        if let Some(words) = &self.auxiliary {
            emit(words.len() as u32);
            for &word in words {
                emit(word);
            }
        }
    }
}

#[cfg(test)]
mod tests;
