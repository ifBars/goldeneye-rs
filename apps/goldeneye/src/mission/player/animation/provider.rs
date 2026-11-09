//! Lazy asset-to-core adapter. Parsing allowances are aggregate across attempts.
use anyhow::{Context, Result};
use ge_assets::animation_modes::{self as asset, View};
use ge_core::animation::mode::{self as core, Provider};

pub(super) struct Source<'a> {
    view: View<'a>,
    remaining: asset::Budget,
}

impl<'a> Source<'a> {
    pub fn new(view: View<'a>, budget: core::Budget) -> Self {
        Self {
            view,
            remaining: asset::Budget {
                commands: budget.commands,
                groups: budget.groups,
                children: budget.children,
            },
        }
    }
}

impl Provider for Source<'_> {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>> {
        self.view.resolve_mode(reference)
    }
    fn mode(&mut self, index: usize) -> Result<core::Mode> {
        let descriptor = self.view.mode(index)?;
        Ok(core::Mode {
            hash: descriptor.hash,
            transition_count: usize::try_from(descriptor.count)?,
        })
    }
    fn transition_target(&mut self, index: usize, ordinal: usize) -> Result<u32> {
        self.view.transition_target(index, ordinal)
    }
    fn commands(&mut self, index: usize, ordinal: usize) -> Result<Vec<core::Command>> {
        let commands = self.view.commands(index, ordinal, self.remaining)?;
        self.remaining.commands -= commands.len();
        let mut result = Vec::new();
        result
            .try_reserve_exact(commands.len())
            .context("retaining bounded mode commands")?;
        for command in commands {
            result.push(match command {
                asset::Command::MaskOperation {
                    source_address,
                    mask_raw,
                } => core::Command::MaskOperation {
                    source_address,
                    mask_raw,
                },
                asset::Command::StartSet {
                    source_address,
                    set_reference,
                    flags_override,
                    options_raw,
                    divisor_override_raw,
                } => {
                    let set = self
                        .view
                        .set(set_reference, self.remaining)?
                        .context("reached animation set is absent")?;
                    self.remaining.groups -= set.groups.len();
                    let mut groups = Vec::new();
                    groups
                        .try_reserve_exact(set.groups.len())
                        .context("retaining bounded mode groups")?;
                    for group in set.groups {
                        self.remaining.children -= group.children.len();
                        groups.push(core::Group {
                            source_address: group.source_address,
                            slot: group.slot,
                            weight_bits: group.weight_bits,
                            children: group
                                .children
                                .into_iter()
                                .map(|child| core::Child {
                                    source_address: child.source_address,
                                    reference: child.reference,
                                    flags_raw: child.flags_raw,
                                    divisor_raw: child.divisor_raw,
                                    word8_raw: child.word8_raw,
                                })
                                .collect(),
                        });
                    }
                    core::Command::StartSet(core::StartSet {
                        source_address,
                        set_reference,
                        set_index: set.descriptor.directory_index,
                        set_hash: set.descriptor.hash,
                        flags_override,
                        options_raw,
                        divisor_override_raw,
                        groups,
                    })
                }
            });
        }
        Ok(result)
    }
}
