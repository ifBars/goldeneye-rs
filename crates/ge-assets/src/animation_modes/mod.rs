//! Retained-owner native mode/set views. All body access is reached and bounded.
//! This reader plans no initial mode, allocates no animator children and samples
//! no channels. Nonzero sections require a future section-ownership adapter.
mod sets;
mod types;

pub use types::{Budget, Child, Command, Descriptor, Group, Set};

use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

const STRIDE: usize = 20;

#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    data: &'a [u8],
    modes: &'a Table,
    sets: &'a Table,
}

pub(crate) fn read<'a>(data: &'a [u8], tables: &'a [Table]) -> Result<View<'a>> {
    let modes = tables
        .get(7)
        .context("animation mode table seven is absent")?;
    let sets = tables
        .get(8)
        .context("animation set table eight is absent")?;
    ensure!(
        modes.name == "animation_modes" && sets.name == "animation_sets",
        "animation mode/set table identities differ"
    );
    // Native signed table counts cap resolver work at 32767 entries. Do not
    // preload bodies or validate unrelated descriptor sections here.
    ensure!(
        modes.count <= i16::MAX as u16 && sets.count <= i16::MAX as u16,
        "animation directory count is negative"
    );
    Ok(View { data, modes, sets })
}

impl View<'_> {
    /// Resolves the requested index without consuming its descriptor or body.
    pub fn resolve_mode(&self, reference: u32) -> Result<Option<usize>> {
        crate::reference::resolve(self.data, self.modes, STRIDE, reference)
    }

    pub fn mode(&self, index: usize) -> Result<Descriptor> {
        self.descriptor(self.modes, index)
    }

    /// Read only a transition's target-index key. Its command pointer stays lazy.
    pub fn transition_target(&self, index: usize, row: usize) -> Result<u32> {
        let at = self.transition_address(index, row)?;
        Bytes(self.data).u32(at)
    }

    /// The caller supplies the first matching row. Later duplicate rows and
    /// unmatched pointer words are not read. Unknown reached opcodes are errors.
    pub fn commands(&self, index: usize, row: usize, budget: Budget) -> Result<Vec<Command>> {
        let at = self.transition_address(index, row)?;
        let b = Bytes(self.data);
        let list = self.required_relative(add(at, 4)?, "transition command list")?;
        let count = usize::try_from(b.u32(list)?)?;
        ensure!(
            count <= budget.commands,
            "animation command budget exhausted"
        );
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .context("allocating bounded animation commands")?;
        for ordinal in 0..count {
            let field = add(
                add(list, 4)?,
                ordinal
                    .checked_mul(4)
                    .context("animation command offset overflow")?,
            )?;
            let source_address = self.required_relative(field, "animation command")?;
            let opcode = b.u32(source_address)?;
            let command = match opcode {
                0x0c00_0002 => {
                    let set_reference = b.u32(add(source_address, 4)?)?;
                    let flags_override = b.u16(add(source_address, 8)?)?;
                    let options_raw = b.u16(add(source_address, 10)?)? as i16;
                    let divisor_override_raw = if options_raw & 1 != 0 {
                        b.u32(add(source_address, 12)?)?
                    } else {
                        0
                    };
                    Command::StartSet {
                        source_address,
                        set_reference,
                        flags_override,
                        options_raw,
                        divisor_override_raw,
                    }
                }
                0x0c00_0005 => Command::MaskOperation {
                    source_address,
                    mask_raw: b.u16(add(source_address, 8)?)?,
                },
                _ => anyhow::bail!(
                    "unsupported reached animation opcode {opcode:#010x} at {source_address:#x}"
                ),
            };
            result.push(command);
        }
        Ok(result)
    }

    pub fn set(&self, reference: u32, budget: Budget) -> Result<Option<Set>> {
        sets::read(self, reference, budget)
    }

    fn descriptor(&self, table: &Table, index: usize) -> Result<Descriptor> {
        ensure!(
            index < usize::from(table.count),
            "animation descriptor index is absent"
        );
        let at = add(
            table.offset,
            index
                .checked_mul(STRIDE)
                .context("animation descriptor offset overflow")?,
        )?;
        let b = Bytes(self.data);
        Ok(Descriptor {
            directory_index: index,
            descriptor_address: at,
            hash: b.u32(at)?,
            count: b.u32(add(at, 16)?)?,
        })
    }

    fn body_address(&self, descriptor: Descriptor) -> Result<usize> {
        let b = Bytes(self.data);
        let section = b.u16(add(descriptor.descriptor_address, 4)?)? & 0x7fff;
        ensure!(
            section == 0,
            "unsupported reached animation section {section} at {:#x}",
            descriptor.descriptor_address
        );
        Ok(b.u32(add(descriptor.descriptor_address, 8)?)? as usize)
    }

    fn transition_address(&self, index: usize, row: usize) -> Result<usize> {
        let descriptor = self.mode(index)?;
        ensure!(
            row < usize::try_from(descriptor.count)?,
            "animation transition row is absent"
        );
        add(
            self.body_address(descriptor)?,
            row.checked_mul(8)
                .context("animation transition offset overflow")?,
        )
    }

    fn required_relative(&self, field: usize, label: &str) -> Result<usize> {
        let b = Bytes(self.data);
        ensure!(
            b.u32(field)? != 0,
            "unsupported absent reached {label} pointer at {field:#x}"
        );
        b.relative(field)
    }
}

fn add(address: usize, offset: usize) -> Result<usize> {
    address
        .checked_add(offset)
        .context("animation address overflow")
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
