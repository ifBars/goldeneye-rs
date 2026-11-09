use super::{Budget, Child, Command, StartSet, Work};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandPlan {
    MaskOperation {
        source_address: usize,
        mask_raw: u16,
    },
    StartSet(StartSetPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSetPlan {
    pub source: StartSet,
    /// Source request order, not the eventual HEAD-inserted traversal order.
    pub groups: Vec<GroupPlan>,
    pub effective_flags_or: u16,
    pub cleanup: CleanupIntent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupIntent {
    SuppressedByFlag20,
    /// Future execution still needs existing slot flags, count and clear mask.
    /// Membership uses original serialized slots, including duplicate groups,
    /// independently of any effective slot override.
    Conditional {
        source_slots: Vec<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupPlan {
    pub source_ordinal: usize,
    pub source_slot: u16,
    pub weight_bits: u32,
    pub children: Vec<ChildPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insertion {
    Head,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildPlan {
    pub source_ordinal: usize,
    pub source: Child,
    /// Preserve all bits. On actual successful creation, flags&0x4 can copy the
    /// existing head's virtual value, flags&0x8 can insert a wrapper and rebuild
    /// its slot, and flags&0x20 contributes to conditional cleanup suppression.
    pub effective_flags: u16,
    pub divisor_u32: u32,
    /// Source scalar setter rounds unsigned divisor to f32 before division.
    /// This host preparation is not a Wii floating-point parity claim.
    pub reciprocal_divisor_bits: u32,
    pub scalar_zero_bits: u32,
    /// (reference >> 24)&0x7f. Native classes three/four have factories; other
    /// requests stay raw here, including reference zero. No allocation implied.
    pub factory_class: u8,
    pub insertion: Insertion,
}

/// Explicit current inputs, never inferred from constructor defaults. These
/// are the four physical slots; an override outside them is unsupported here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotInputs {
    pub override_enabled_raw: u8,
    pub override_slot_raw: u32,
    pub current_weight_bits: [u32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightRequest {
    Assign(u32),
    Preserve(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerRequest {
    pub effective_slot: u32,
    pub weight: WeightRequest,
}

impl GroupPlan {
    /// One group's weight intent against an explicit current snapshot. A future
    /// executor must supply updated inputs for subsequent groups and apply real
    /// allocation/list outcomes; this method performs no runtime mutation.
    pub fn layer_request(&self, inputs: SlotInputs) -> Result<LayerRequest> {
        let slot = if inputs.override_enabled_raw != 0 {
            inputs.override_slot_raw
        } else {
            u32::from(self.source_slot)
        };
        let ordinal = usize::try_from(slot)?;
        let existing = *inputs
            .current_weight_bits
            .get(ordinal)
            .context("unsupported effective animation layer slot")?;
        let weight = if slot == 0 {
            WeightRequest::Assign(1.0f32.to_bits())
        } else if f32::from_bits(self.weight_bits) == 0.0 {
            WeightRequest::Assign(0)
        } else {
            WeightRequest::Preserve(existing)
        };
        Ok(LayerRequest {
            effective_slot: slot,
            weight,
        })
    }
}

pub(super) fn charge(used: &mut usize, amount: usize, limit: usize, label: &str) -> Result<()> {
    let next = used
        .checked_add(amount)
        .context("mode work count overflow")?;
    ensure!(next <= limit, "mode {label} budget exhausted");
    *used = next;
    Ok(())
}

pub(super) fn command_plan(
    command: Command,
    budget: Budget,
    work: &mut Work,
) -> Result<CommandPlan> {
    match command {
        Command::MaskOperation {
            source_address,
            mask_raw,
        } => Ok(CommandPlan::MaskOperation {
            source_address,
            mask_raw,
        }),
        Command::StartSet(source) => {
            charge(
                &mut work.groups,
                source.groups.len(),
                budget.groups,
                "group",
            )?;
            // Preflight all child ownership before allocating derived plans.
            let child_count = source.groups.iter().try_fold(0usize, |count, group| {
                count
                    .checked_add(group.children.len())
                    .context("mode child count overflow")
            })?;
            charge(&mut work.children, child_count, budget.children, "child")?;
            let mut groups = Vec::with_capacity(source.groups.len());
            let mut flags_or = 0;
            let divisor_override = if source.options_raw & 1 != 0 {
                source.divisor_override_raw
            } else {
                0
            };
            for (source_ordinal, group) in source.groups.iter().enumerate() {
                let mut children = Vec::with_capacity(group.children.len());
                for (ordinal, &child) in group.children.iter().enumerate() {
                    let effective_flags = if source.flags_override != 0 {
                        source.flags_override
                    } else {
                        child.flags_raw
                    };
                    flags_or |= effective_flags;
                    let divisor = if divisor_override != 0 {
                        divisor_override
                    } else {
                        u32::from(child.divisor_raw)
                    };
                    let reciprocal = if divisor == 0 {
                        1.0
                    } else {
                        1.0 / divisor as f32
                    };
                    children.push(ChildPlan {
                        source_ordinal: ordinal,
                        source: child,
                        effective_flags,
                        divisor_u32: divisor,
                        reciprocal_divisor_bits: reciprocal.to_bits(),
                        scalar_zero_bits: 0,
                        factory_class: ((child.reference >> 24) & 0x7f) as u8,
                        insertion: Insertion::Head,
                    });
                }
                groups.push(GroupPlan {
                    source_ordinal,
                    source_slot: group.slot,
                    weight_bits: group.weight_bits,
                    children,
                });
            }
            let cleanup = if flags_or & 0x20 != 0 {
                CleanupIntent::SuppressedByFlag20
            } else {
                CleanupIntent::Conditional {
                    source_slots: source.groups.iter().map(|group| group.slot).collect(),
                }
            };
            Ok(CommandPlan::StartSet(StartSetPlan {
                source,
                groups,
                effective_flags_or: flags_or,
                cleanup,
            }))
        }
    }
}
