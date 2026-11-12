//! Retained-owner native mode/set views. All body access is reached and bounded.
//! This reader plans no initial mode, allocates no animator children and samples
//! no channels. Nonzero sections require a future section-ownership adapter.
mod commands;
mod sets;
mod types;

pub use types::{Budget, Child, Command, CommandLocation, Descriptor, Group, Set};

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
