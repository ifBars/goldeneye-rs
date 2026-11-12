//! Reached command body up to its next real allocation/cleanup dependency.
//! Reads and weight writes precede child factories. No factory result, later
//! child word or command return is invented by this preparation.
mod factory;
mod state;
use super::{SlotOverride, State, execute::charge};
use anyhow::{Context, Result, ensure};

pub trait Source {
    fn set_reference(&mut self) -> Result<u32>;
    fn resolve_set(&mut self, reference: u32) -> Result<Option<usize>>;
    fn options(&mut self) -> Result<i16>;
    fn divisor_override(&mut self) -> Result<u32>;
    fn flags_override(&mut self) -> Result<u16>;
    /// Reads the reached body binding before its group count, including count zero.
    fn begin_set(&mut self, index: usize) -> Result<usize>;
    fn group_slot(&mut self) -> Result<u16>;
    fn group_weight(&mut self) -> Result<u32>;
    fn child_count(&mut self) -> Result<i16>;
    fn child_divisor(&mut self, ordinal: usize) -> Result<u16>;
    fn child_flags(&mut self, ordinal: usize) -> Result<u16>;
    fn child_reference(&mut self, ordinal: usize) -> Result<u32>;
    fn next_group(&mut self, children: u16) -> Result<()>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Entry,
    ChildFactoryPending,
    CleanupPending,
    MaskControlPending,
    MaskClearPending,
    OpcodePending,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildRequest {
    pub group: usize,
    pub ordinal: usize,
    pub slot: u8,
    pub reference: u32,
    pub effective_flags: u16,
    pub divisor: u32,
}
#[derive(Debug)]
pub struct Group {
    pub source_slot: u16,
    pub effective_slot: u32,
    pub weight_write: Option<u32>,
    pub child_count: Option<i16>,
}
#[derive(Debug)]
pub struct Pending {
    opcode: u32,
    stage: Stage,
    set_reference: Option<u32>,
    set_index: Option<usize>,
    options: Option<i16>,
    divisor: Option<u32>,
    flags: Option<u16>,
    group_count: Option<usize>,
    groups: Vec<Group>,
    child: Option<ChildRequest>,
    mask: Option<u16>,
    flags_or: u16,
    factory_started: bool,
    factory_node: Option<super::NodeId>,
}
impl Pending {
    pub fn new(opcode: u32) -> Self {
        Self {
            opcode,
            stage: Stage::Entry,
            set_reference: None,
            set_index: None,
            options: None,
            divisor: None,
            flags: None,
            group_count: None,
            groups: Vec::new(),
            child: None,
            mask: None,
            flags_or: 0,
            factory_started: false,
            factory_node: None,
        }
    }
    pub fn opcode(&self) -> u32 {
        self.opcode
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn set_reference(&self) -> Option<u32> {
        self.set_reference
    }
    pub fn set_index(&self) -> Option<usize> {
        self.set_index
    }
    pub fn group_count(&self) -> Option<usize> {
        self.group_count
    }
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }
    pub fn child(&self) -> Option<ChildRequest> {
        self.child
    }
    pub fn mask(&self) -> Option<u16> {
        self.mask
    }
    pub fn flags_or(&self) -> u16 {
        self.flags_or
    }
    pub fn weight_writes(&self) -> usize {
        self.groups
            .iter()
            .filter(|group| group.weight_write.is_some())
            .count()
    }
    pub fn prepare(
        &mut self,
        source: &mut impl Source,
        layers: &mut State,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::Entry && !layers.failed,
            "command body cannot replay a reached or failed prefix"
        );
        let result = self.prepare_inner(source, layers, slot_override);
        if result.is_err() {
            self.stage = Stage::Failed;
            layers.failed = true;
        }
        result
    }
    fn prepare_inner(
        &mut self,
        source: &mut impl Source,
        layers: &mut State,
        slot_override: SlotOverride,
    ) -> Result<()> {
        charge(&mut layers.work.commands, layers.budget.commands, "command")?;
        match self.opcode {
            0x0c00_0002 | 0x0c00_0003 => self.start_set(source, layers, slot_override),
            0x0c00_0005 => {
                let mask = source.flags_override()?;
                self.mask = Some(mask);
                self.stage = if mask == 0 {
                    Stage::MaskClearPending
                } else {
                    Stage::MaskControlPending
                };
                Ok(())
            }
            _ => {
                self.stage = Stage::OpcodePending;
                Ok(())
            }
        }
    }
    fn start_set(
        &mut self,
        source: &mut impl Source,
        layers: &mut State,
        slot_override: SlotOverride,
    ) -> Result<()> {
        let reference = source.set_reference()?;
        self.set_reference = Some(reference);
        self.set_index = source.resolve_set(reference)?;
        let options = source.options()?;
        self.options = Some(options);
        let raw_divisor = if options & 1 != 0 {
            source.divisor_override()?
        } else {
            0
        };
        let divisor = if self.opcode == 0x0c00_0003 && (raw_divisor as i32) <= 0 {
            0
        } else {
            raw_divisor
        };
        self.divisor = Some(divisor);
        let flags = source.flags_override()?;
        self.flags = Some(flags);
        let count = source.begin_set(self.set_index.context("reached animation set is absent")?)?;
        self.group_count = Some(count);
        for ordinal in 0..count {
            charge(&mut layers.work.groups, layers.budget.groups, "group")?;
            let source_slot = source.group_slot()?;
            let slot = if slot_override.enabled_raw != 0 {
                slot_override.slot_raw
            } else {
                u32::from(source_slot)
            };
            self.groups.try_reserve(1)?;
            self.groups.push(Group {
                source_slot,
                effective_slot: slot,
                weight_write: None,
                child_count: None,
            });
            let slot = usize::try_from(slot)?;
            ensure!(slot < 4, "unsupported reached effective animation slot");
            let weight = if slot == 0 {
                Some(1f32.to_bits())
            } else if f32::from_bits(source.group_weight()?) == 0.0 {
                Some(0)
            } else {
                None
            };
            if let Some(weight) = weight {
                layers.slots[slot].weight_bits = weight;
                self.groups[ordinal].weight_write = Some(weight);
            }
            let children = source.child_count()?;
            self.groups[ordinal].child_count = Some(children);
            ensure!(
                children >= 0,
                "unsupported negative reached animation child count"
            );
            if children > 0 {
                charge(&mut layers.work.children, layers.budget.children, "child")?;
                let child_divisor = if self.opcode == 0x0c00_0003 {
                    divisor.wrapping_add(u32::from(source.child_divisor(0)?))
                } else if divisor != 0 {
                    divisor
                } else {
                    u32::from(source.child_divisor(0)?)
                };
                let effective_flags = if flags != 0 {
                    flags
                } else {
                    source.child_flags(0)?
                };
                let reference = source.child_reference(0)?;
                self.flags_or |= effective_flags;
                self.child = Some(ChildRequest {
                    group: ordinal,
                    ordinal: 0,
                    slot: slot as u8,
                    reference,
                    effective_flags,
                    divisor: child_divisor,
                });
                self.stage = Stage::ChildFactoryPending;
                return Ok(());
            }
            source.next_group(children as u16)?;
        }
        self.stage = Stage::CleanupPending;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
