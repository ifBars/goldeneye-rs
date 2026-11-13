use super::{Backing, BackingId};
use crate::{
    animation::skeleton::Skeleton,
    mission::player::{components::Identity, identity::PlayerId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    FirstPerson,
    ThirdPerson,
}
impl Slot {
    pub(super) const fn index(self) -> usize {
        match self {
            Self::FirstPerson => 0,
            Self::ThirdPerson => 1,
        }
    }
}

/// The caller has already found the actual component. None retains an unknown
/// skeleton provider; it is not an empty skeleton or a proven native NULL.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
    pub receiver: Identity,
    pub skeleton: Option<&'a Skeleton>,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Total live native allocation bytes across BOTH cache headers and temp.
    pub native_bytes: usize,
    /// Total live native backing count, including reached temporary storage.
    pub allocations: usize,
    /// Descendant nodes visited per individual header build.
    pub descendant_visits: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            native_bytes: 131_072,
            allocations: 10,
            descendant_visits: 16_384,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Entry,
    FirstPerson,
    ThirdPerson,
    Selecting,
    Returned,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStage {
    CleanupGate,
    Skeleton,
    MatrixAllocation,
    ValidityAllocation,
    ValidityClear,
    OffsetsAllocation,
    TemporaryAllocation,
    Descendants,
    CompactAllocation,
    CompactCopy,
    TemporaryRelease,
    Returned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTrace {
    pub slot: Slot,
    pub receiver: Identity,
    pub stage: BuildStage,
    pub bone_count: Option<u32>,
    pub roots_completed: usize,
    pub descendant_visits: usize,
    pub descendant_bytes: usize,
    pub offsets_written: usize,
    pub allocations: Vec<BackingId>,
    pub releases: Vec<BackingId>,
    pub waiting_skeleton: bool,
    pub failed: bool,
    pub returned: bool,
}
impl BuildTrace {
    pub(super) fn new(slot: Slot, receiver: Identity) -> Self {
        Self {
            slot,
            receiver,
            stage: BuildStage::CleanupGate,
            bone_count: None,
            roots_completed: 0,
            descendant_visits: 0,
            descendant_bytes: 0,
            offsets_written: 0,
            allocations: Vec::new(),
            releases: Vec::new(),
            waiting_skeleton: false,
            failed: false,
            returned: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub ordinal: u64,
    pub stage: Stage,
    pub receivers: [Option<Identity>; 2],
    pub selected_before: Option<Slot>,
    pub builds: [Option<BuildTrace>; 2],
    pub missing_receiver_skip: bool,
    pub reused_selected: bool,
    pub selection_returned: bool,
    pub selected_after: Option<Slot>,
    pub failed: bool,
    pub returned: bool,
}

#[derive(Debug)]
pub struct Header {
    pub(super) matrix: Option<Backing>,
    pub(super) validity: Option<Backing>,
    pub(super) offsets: Option<Backing>,
    pub(super) descendants: Option<Backing>,
    // Native stack temporary retained by the pending operation, not a header
    // field. Keeping it here preserves ownership even after a checked failure.
    pub(super) temporary: Option<Backing>,
    pub(super) count: u32,
    pub(super) association: Option<PlayerId>,
    pub(super) invalidation_count_reads: u64,
    pub(super) invalidation_pointer_reads: u64,
}
impl Header {
    pub(super) fn from_boot() -> Self {
        Self {
            matrix: None,
            validity: None,
            offsets: None,
            descendants: None,
            temporary: None,
            count: 0,
            association: None,
            invalidation_count_reads: 0,
            invalidation_pointer_reads: 0,
        }
    }
    pub fn matrix(&self) -> Option<&Backing> {
        self.matrix.as_ref()
    }
    pub fn validity(&self) -> Option<&Backing> {
        self.validity.as_ref()
    }
    pub fn offsets(&self) -> Option<&Backing> {
        self.offsets.as_ref()
    }
    pub fn descendants(&self) -> Option<&Backing> {
        self.descendants.as_ref()
    }
    pub fn temporary(&self) -> Option<&Backing> {
        self.temporary.as_ref()
    }
    pub fn count(&self) -> u32 {
        self.count
    }
    pub fn association(&self) -> Option<PlayerId> {
        self.association
    }
    pub fn invalidation_reads(&self) -> (u64, u64) {
        (
            self.invalidation_count_reads,
            self.invalidation_pointer_reads,
        )
    }
    pub fn native_bytes(&self) -> usize {
        self.backings().flatten().map(Backing::native_bytes).sum()
    }
    pub fn allocations(&self) -> usize {
        self.backings().flatten().count()
    }
    pub(super) fn backings(&self) -> impl Iterator<Item = Option<&Backing>> {
        [
            self.matrix.as_ref(),
            self.validity.as_ref(),
            self.offsets.as_ref(),
            self.descendants.as_ref(),
            self.temporary.as_ref(),
        ]
        .into_iter()
    }
    pub(super) fn release(&mut self, matrix_gate: bool) -> Vec<BackingId> {
        if matrix_gate && self.matrix.is_none() {
            return Vec::new();
        }
        let mut released = Vec::new();
        for field in [
            &mut self.descendants,
            &mut self.offsets,
            &mut self.validity,
            &mut self.matrix,
        ] {
            if let Some(backing) = field.take() {
                released.push(backing.id());
            }
        }
        self.count = 0;
        released
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub selected: Option<Slot>,
    pub counts: [u32; 2],
    pub matrix_backings: [Option<BackingId>; 2],
    pub associations: [Option<PlayerId>; 2],
    pub native_bytes: usize,
    pub allocations: usize,
    pub invocation_count: u64,
}
