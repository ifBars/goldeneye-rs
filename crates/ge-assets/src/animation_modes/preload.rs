//! A lazy reference extractor for preload, independent of live command execution.
//! It reads no command flags, group weights or child flags/divisor/postword.
use super::{Bytes, View, add};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct ExtractionBudget {
    pub transitions: usize,
    pub commands: usize,
    pub groups: usize,
    pub children: usize,
}

/// Source ordinals and aggregate reached reads. A found reference is not a
/// temporary animator binding, preload return or sampled pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassFourReference {
    pub reference: u32,
    pub requested_index: usize,
    pub baseline_index: usize,
    pub transition_ordinal: usize,
    pub command_ordinal: usize,
    pub set_index: usize,
    pub group_ordinal: usize,
    pub child_ordinal: usize,
    pub transitions_read: usize,
    pub commands_read: usize,
    pub groups_read: usize,
    pub children_read: usize,
}

fn charge(value: &mut usize, maximum: usize, label: &str) -> Result<()> {
    ensure!(
        *value < maximum,
        "preload extraction {label} budget exhausted"
    );
    *value = value
        .checked_add(1)
        .context("preload extraction counter overflow")?;
    Ok(())
}

impl View<'_> {
    /// Read from the actual logically resident base view supplied by the owner.
    /// None is a known exhausted lookup, distinct from malformed/unresident Err.
    /// It is not permission to skip the source caller's later temporary binding.
    pub fn first_preload_class_four(
        &self,
        requested_mode: u32,
        budget: ExtractionBudget,
    ) -> Result<Option<ClassFourReference>> {
        let Some(requested_index) = self.resolve_mode(requested_mode)? else {
            return Ok(None);
        };
        let Some(baseline_index) = self.resolve_mode(0x0900_000b)? else {
            return Ok(None);
        };
        let b = Bytes(self.data);
        let baseline = self.directory_address(self.modes, baseline_index)?;
        let transitions = usize::try_from(b.u32(add(baseline, 16)?)?)?;
        let (mut transitions_read, mut commands_read, mut groups_read, mut children_read) =
            (0, 0, 0, 0);
        // Zero transitions never reaches the descriptor's body/section fields.
        for transition_ordinal in 0..transitions {
            charge(&mut transitions_read, budget.transitions, "transition")?;
            let body = self.body_address_at(baseline)?;
            let transition = add(
                body,
                transition_ordinal
                    .checked_mul(8)
                    .context("preload transition stride overflow")?,
            )?;
            if b.u32(transition)? != u32::try_from(requested_index)? {
                continue;
            }
            let list = self.required_relative(add(transition, 4)?, "preload command list")?;
            let commands = usize::try_from(b.u32(list)?)?;
            for command_ordinal in 0..commands {
                charge(&mut commands_read, budget.commands, "command")?;
                let field = add(
                    add(list, 4)?,
                    command_ordinal
                        .checked_mul(4)
                        .context("preload command stride overflow")?,
                )?;
                let command = self.required_relative(field, "preload command")?;
                if !matches!(b.u32(command)?, 0x0c00_0002 | 0x0c00_0004) {
                    continue;
                }
                let reference = b.u32(add(command, 4)?)?;
                let Some(set_index) =
                    crate::reference::resolve(self.data, self.sets, 20, reference)?
                else {
                    continue;
                };
                let set = self.directory_address(self.sets, set_index)?;
                // Source binds the set body BEFORE reading its group count,
                // even for zero groups. Do not use the eager set reader.
                let mut group_at = self.body_address_at(set)?;
                let groups = usize::try_from(b.u32(add(set, 16)?)?)?;
                for group_ordinal in 0..groups {
                    charge(&mut groups_read, budget.groups, "group")?;
                    let children = b.u16(add(group_at, 2)?)? as i16;
                    ensure!(
                        children >= 0,
                        "unsupported negative preload extraction child count"
                    );
                    let child_base = add(group_at, 8)?;
                    for child_ordinal in 0..usize::try_from(children)? {
                        charge(&mut children_read, budget.children, "child")?;
                        let at = add(
                            child_base,
                            child_ordinal
                                .checked_mul(12)
                                .context("preload child stride overflow")?,
                        )?;
                        let reference = b.u32(at)?;
                        if reference != 0 && ((reference >> 24) & 0x7f) == 4 {
                            return Ok(Some(ClassFourReference {
                                reference,
                                requested_index,
                                baseline_index,
                                transition_ordinal,
                                command_ordinal,
                                set_index,
                                group_ordinal,
                                child_ordinal,
                                transitions_read,
                                commands_read,
                                groups_read,
                                children_read,
                            }));
                        }
                    }
                    group_at = add(
                        child_base,
                        usize::try_from(children)?
                            .checked_mul(12)
                            .context("preload group stride overflow")?,
                    )?;
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "tests/preload.rs"]
mod tests;
