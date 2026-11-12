//! Component-local host node identities shared by every animation node kind.
//! Allocation history is distinct from live layer membership and native addresses.
use super::layers::NodeId;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerId(u64);
impl OwnerId {
    pub fn new(value: u64) -> Result<Self> {
        ensure!(value != 0, "animation node owner is zero");
        Ok(Self(value))
    }
    pub fn get(self) -> u64 {
        self.0
    }
    pub(crate) fn visit_state_words(self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.0 >> 32) as u32);
        emit(self.0 as u32);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Control,
    Script,
}
#[derive(Debug)]
pub struct Ids {
    owner: OwnerId,
    next: u64,
    maximum: usize,
    allocated: BTreeMap<NodeId, Kind>,
}
impl Ids {
    pub fn from_constructor(owner: OwnerId, maximum: usize) -> Self {
        Self {
            owner,
            next: 0,
            maximum,
            allocated: BTreeMap::new(),
        }
    }
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn allocate(&mut self, kind: Kind) -> Result<NodeId> {
        ensure!(
            self.allocated.len() < self.maximum,
            "animation node identity budget exhausted"
        );
        let next = self
            .next
            .checked_add(1)
            .context("animation node identity overflow")?;
        let id = NodeId::new(next)?;
        self.allocated.insert(id, kind);
        self.next = next;
        Ok(id)
    }
    pub fn kind(&self, id: NodeId) -> Option<Kind> {
        self.allocated.get(&id).copied()
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.owner.visit_state_words(&mut emit);
        emit((self.next >> 32) as u32);
        emit(self.next as u32);
        emit(((self.maximum as u64) >> 32) as u32);
        emit(self.maximum as u32);
        emit(self.allocated.len() as u32);
        for (id, kind) in &self.allocated {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
            emit(match kind {
                Kind::Control => 0,
                Kind::Script => 1,
            });
        }
    }
}
