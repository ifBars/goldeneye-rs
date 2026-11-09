use super::CommandPlan;
use anyhow::Result;

/// Reads only the requested source. A target resolution must not preload its
/// descriptor/body, and unmatched transitions must not preload command lists.
/// Providers bound their own parsing/allocation before returning owned commands.
pub trait Provider {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>>;
    fn mode(&mut self, index: usize) -> Result<Mode>;
    fn transition_target(&mut self, index: usize, ordinal: usize) -> Result<u32>;
    fn commands(&mut self, index: usize, ordinal: usize) -> Result<Vec<Command>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode {
    pub hash: u32,
    pub transition_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    pub requested: u32,
    pub previous: u32,
    pub fallback: u32,
    pub resource_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub lookups: usize,
    pub transition_rows: usize,
    pub commands: usize,
    pub groups: usize,
    pub children: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work {
    pub lookups: usize,
    pub transition_rows: usize,
    pub commands: usize,
    pub groups: usize,
    pub children: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Raw source operation. Executing it needs current masks, lists and real
    /// allocations; this is not an inferred successful ClearLayer operation.
    MaskOperation {
        source_address: usize,
        mask_raw: u16,
    },
    StartSet(StartSet),
}

/// Parsed opcode 0x0c000002 with its reached set expanded in storage order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSet {
    pub source_address: usize,
    pub set_reference: u32,
    pub set_index: usize,
    pub set_hash: u32,
    pub flags_override: u16,
    pub options_raw: i16,
    /// Effective only when options bit zero is enabled. A provider must leave
    /// the serialized +0xc word unread when that bit is absent.
    pub divisor_override_raw: u32,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub source_address: usize,
    pub slot: u16,
    pub weight_bits: u32,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Child {
    pub source_address: usize,
    pub reference: u32,
    pub flags_raw: u16,
    pub divisor_raw: u16,
    pub word8_raw: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptKind {
    Previous,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub kind: AttemptKind,
    pub reference: u32,
    pub source_index: Option<usize>,
    pub source_hash: Option<u32>,
    pub matched_transition: Option<usize>,
    /// Effects stay here even when a matched source hash zero causes fallback.
    pub commands: Vec<CommandPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    ResourceUnavailable,
    TargetMissing,
    AttemptsComplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub inputs: Inputs,
    pub status: Status,
    pub target_index: Option<usize>,
    pub attempts: Vec<Attempt>,
    /// Native source descriptor hash, not the requested mode or child hash.
    /// Zero can coexist with matched, planned commands.
    pub return_word: u32,
    pub work: Work,
}
