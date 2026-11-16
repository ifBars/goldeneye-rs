//! Retained evaluator evidence. Payload bytes are never fingerprinted.
use super::super::wide;
use crate::animation::layers::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    ActiveCount,
    Weight,
    Tail,
    /// Skeleton sample preparation reached an unavailable provider.
    Prepare,
    /// The node's virtual +2c reached an unsupported or unknown operation.
    Node,
    Previous,
    /// 80306f20 layer blending is not implemented.
    LayerBlend,
    /// Skeletal commit is not implemented.
    Commit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Running,
    Pending(Boundary),
    Returned,
    /// A provider error latched; any live reservation stays retained.
    Failed,
    /// Explicit host cancellation released the retained reservation.
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Returned,
    Pending(Boundary),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    LowWeight,
    EmptyTail,
    Nonempty(NodeId),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub slot: u8,
    pub weight_bits: Option<u32>,
    pub branch: Option<Branch>,
    pub prepared: bool,
    pub masks_cleared: bool,
    /// Visited nodes in TAIL->previous order with their raw return byte.
    pub nodes: Vec<(NodeId, u8)>,
    pub successes: u32,
    pub flags_after: Option<u32>,
}
impl Slot {
    pub(super) fn new(slot: u8) -> Self {
        Self {
            slot,
            weight_bits: None,
            branch: None,
            prepared: false,
            masks_cleared: false,
            nodes: Vec::new(),
            successes: 0,
            flags_after: None,
        }
    }
    pub(super) fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(u32::from(self.slot));
        emit(u32::from(self.weight_bits.is_some()));
        if let Some(bits) = self.weight_bits {
            emit(bits);
        }
        match self.branch {
            None => emit(0),
            Some(Branch::LowWeight) => emit(1),
            Some(Branch::EmptyTail) => emit(2),
            Some(Branch::Nonempty(node)) => {
                emit(3);
                wide(emit, node.get());
            }
        }
        emit(u32::from(self.prepared));
        emit(u32::from(self.masks_cleared));
        emit(self.nodes.len() as u32);
        for (node, byte) in &self.nodes {
            wide(emit, node.get());
            emit(u32::from(*byte));
        }
        emit(self.successes);
        emit(u32::from(self.flags_after.is_some()));
        if let Some(flags) = self.flags_after {
            emit(flags);
        }
    }
}
pub(super) fn stage_words(stage: Stage, emit: &mut impl FnMut(u32)) {
    match stage {
        Stage::Running => emit(0),
        Stage::Pending(boundary) => {
            emit(1);
            emit(boundary as u32);
        }
        Stage::Returned => emit(2),
        Stage::Failed => emit(3),
        Stage::Aborted => emit(4),
    }
}
