//! Actual host-owned operation metadata used by the source type query.
//! This is not a scheduler: no elapsed-time or caller success flag retires work.
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationBudget {
    pub entries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationKind(u32);
impl OperationKind {
    pub const TYPE_0B: Self = Self(0x0b);

    /// The source insertion path accepts types below 0x3b. Their callbacks are
    /// not implemented by this metadata owner.
    pub fn new(raw: u32) -> Result<Self> {
        ensure!(
            raw < 0x3b,
            "operation type is outside the source insertion scope"
        );
        Ok(Self(raw))
    }
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Source +0x14 values, deliberately independent of queue membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Constructor,
    Queued,
    Running,
    Complete,
    Result,
}
impl OperationState {
    pub const fn raw(self) -> u32 {
        match self {
            Self::Constructor => 0,
            Self::Queued => 1,
            Self::Running => 2,
            Self::Complete => 3,
            Self::Result => 4,
        }
    }
}

/// Source allocation can return ID zero; this is not a native pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationId(u32);
impl OperationId {
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation {
    id: OperationId,
    kind: OperationKind,
    state: OperationState,
}
impl Operation {
    pub const fn id(self) -> OperationId {
        self.id
    }
    pub const fn kind(self) -> OperationKind {
        self.kind
    }
    pub const fn state(self) -> OperationState {
        self.state
    }
}

#[derive(Debug)]
pub struct OperationOwner {
    budget: OperationBudget,
    next_id: u32,
    entries: Vec<Operation>,
}
impl OperationOwner {
    /// An explicit fresh host lifetime. This does not assert an empty queue in
    /// original runtime history, or adopt the admission/loader work queues.
    pub fn fresh(budget: OperationBudget) -> Result<Self> {
        ensure!(
            budget.entries <= u32::MAX as usize,
            "operation budget exceeds supported indexing"
        );
        Ok(Self {
            budget,
            next_id: 0,
            entries: Vec::new(),
        })
    }

    /// Retain an actual metadata entry, including its supplied source state.
    /// Executing/retiring native operations requires a future scoped provider.
    pub fn enqueue(&mut self, kind: OperationKind, state: OperationState) -> Result<OperationId> {
        ensure!(
            self.entries.len() < self.budget.entries,
            "operation entry budget exhausted"
        );
        let next = self
            .next_id
            .checked_add(1)
            .context("operation identity exhausted")?;
        self.entries
            .try_reserve(1)
            .context("operation allocation failed")?;
        let id = OperationId(self.next_id);
        self.entries.push(Operation { id, kind, state });
        self.next_id = next;
        Ok(id)
    }

    /// 801fd3d0 examines every retained entry's type, regardless of its state.
    /// Completed entries still block until a real scheduler removes them.
    pub fn contains_kind(&self, kind: OperationKind) -> bool {
        self.entries.iter().any(|entry| entry.kind == kind)
    }
    pub fn entries(&self) -> &[Operation] {
        &self.entries
    }
    pub const fn original_history_verified(&self) -> bool {
        false
    }
}
