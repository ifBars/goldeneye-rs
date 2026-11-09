use crate::animation::mode::ChildPlan;
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(u64);
impl NodeId {
    pub fn new(value: u64) -> Result<Self> {
        ensure!(value != 0, "animation node identity must be nonzero");
        Ok(Self(value))
    }
    pub fn get(self) -> u64 {
        self.0
    }
}

/// The host owns real captured controls/animation instances behind these IDs.
/// None means the source factory produced no child, never dummy allocation.
pub trait Host {
    fn create_control(&mut self) -> Result<Option<NodeId>>;
    fn clone_control(&mut self, _source: NodeId) -> Result<Option<NodeId>> {
        anyhow::bail!("unsupported animation control clone")
    }
    fn create_child(&mut self, slot: u8, child: &ChildPlan) -> Result<Option<NodeId>>;
    /// Source existing-head virtual +10 -> new-child virtual +14, before insert.
    fn copy_head(&mut self, new: NodeId, old: NodeId) -> Result<()>;
    /// Source scalar zero, reciprocal divisor and word8 writes, after insert.
    fn initialize_child(&mut self, node: NodeId, child: &ChildPlan) -> Result<()>;
    fn destroy_node(&mut self, node: NodeId) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub active_count: u8,
    pub selection_mask: u8,
    pub clear_mask: u8,
    pub slot_flags: [u16; 4],
    pub weight_bits: [u32; 4],
}
impl Snapshot {
    /// Explicit fresh FP callback boundary after source masks are changed to FB.
    pub fn fresh_first_person() -> Self {
        Self {
            active_count: 1,
            selection_mask: 0xfb,
            clear_mask: 0xfb,
            slot_flags: [0; 4],
            weight_bits: [1.0f32.to_bits(); 4],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SlotOverride {
    pub enabled_raw: u8,
    pub slot_raw: u32,
}

/// Aggregate work across successive execute calls; node budget bounds factory
/// requests, including requests which produce None.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub nodes: usize,
    pub commands: usize,
    pub groups: usize,
    pub children: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub(super) nodes: Vec<NodeId>,
    pub(super) weight_bits: u32,
    pub(super) flags_raw: u16,
}
impl Slot {
    /// Tick order. Maintenance/sampling traverse this slice in reverse, subject
    /// to their own weight gates; this module performs neither operation.
    pub fn nodes_head(&self) -> &[NodeId] {
        &self.nodes
    }
    pub fn weight_bits(&self) -> u32 {
        self.weight_bits
    }
    pub fn flags_raw(&self) -> u16 {
        self.flags_raw
    }
}
