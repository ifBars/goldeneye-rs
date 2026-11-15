//! Lazy metadata for script publication. Serialized pointers and available
//! bytes do not establish a live job, body publication or residency.
use super::{AnimationScriptReference, Edb, Table, script_reference};
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};
use std::ops::Range;

/// A metadata reader tied to this retained EDB and its logically resident base.
#[derive(Clone, Copy)]
pub struct View<'a> {
    owner: &'a Edb,
    base: &'a [u8],
}

/// A resolved table-five descriptor, without a body read or residency claim.
pub struct Script<'a> {
    view: View<'a>,
    descriptor: AnimationScriptReference,
}

/// A section-directory locator. Individual fields are read only on demand.
#[derive(Clone, Copy)]
pub struct Section<'a> {
    view: View<'a>,
    directory_index: usize,
    at: usize,
}

impl Edb {
    pub fn script_publication(&self) -> Result<View<'_>> {
        let base = self.base_bytes()?;
        let b = Bytes(base);
        ensure!(
            self.header.version == 263 && b.u32(8)? == 263,
            "script publication requires supported EDB version"
        );
        ensure!(
            b.u32(4)? == self.header.hash,
            "script publication resource identity differs"
        );
        ensure!(
            b.u32(0x18)? == self.header.base_bytes && b.u32(0x14)? == self.header.payload_bytes,
            "script publication size metadata differs"
        );
        Ok(View { owner: self, base })
    }
}

impl<'a> View<'a> {
    pub fn resource_hash(self) -> u32 {
        self.owner.header.hash
    }

    /// The completion callback reads section zero's own header, not the EDB
    /// resource hash or flags. End/job fields and other sections are untouched.
    pub fn base_callback_mask(self) -> Result<u32> {
        let section = self.section(0)?;
        let start = Bytes(self.base).u32(section.field(4)?)? as usize;
        let mask = start
            .checked_add(4)
            .context("section callback mask overflow")?;
        Bytes(self.base).u32(mask)
    }

    pub fn script(self, reference: u32) -> Result<Option<Script<'a>>> {
        let table = self
            .owner
            .tables
            .get(5)
            .context("missing script directory")?;
        ensure!(
            table.name == "animation_scripts",
            "script directory kind differs"
        );
        let descriptor = script_reference::metadata(self.base, table, reference)?;
        Ok(descriptor.map(|descriptor| Script {
            view: self,
            descriptor,
        }))
    }

    fn section(self, token: u32) -> Result<Section<'a>> {
        let table = self
            .owner
            .tables
            .first()
            .context("missing section directory")?;
        ensure!(table.name == "sections", "section directory kind differs");
        let directory_index = crate::reference::resolve(self.base, table, 16, token)?
            .context("script section is absent")?;
        Ok(Section {
            view: self,
            directory_index,
            at: entry(table, directory_index)?,
        })
    }
}

impl<'a> Script<'a> {
    pub fn descriptor(&self) -> &AnimationScriptReference {
        &self.descriptor
    }

    /// The high initialized bit is not part of the section token. Resolving a
    /// row does not read its job or calculate a body address.
    pub fn section(&self) -> Result<Section<'a>> {
        self.view
            .section(u32::from(self.descriptor.section & 0x7fff))
    }
}

impl Section<'_> {
    pub fn directory_index(self) -> usize {
        self.directory_index
    }

    pub fn key(self) -> Result<u32> {
        Bytes(self.view.base).u32(self.field(0)?)
    }

    /// Serialized raw data only; zero is not itself a live NULL-body proof.
    pub fn runtime_job_raw(self) -> Result<u32> {
        Bytes(self.view.base).u32(self.field(12)?)
    }

    /// The declared file span, without reading or granting access to its bytes.
    /// Callers proving a NULL job need not request this field.
    pub fn span(self) -> Result<Range<usize>> {
        let b = Bytes(self.view.base);
        let start = b.u32(self.field(4)?)? as usize;
        let end = b.u32(self.field(8)?)? as usize;
        ensure!(start <= end, "script section span is reversed");
        ensure!(
            end <= self.view.owner.header.payload_bytes as usize,
            "script section span exceeds resource"
        );
        Ok(start..end)
    }

    fn field(self, offset: usize) -> Result<usize> {
        self.at
            .checked_add(offset)
            .context("script section field overflow")
    }
}

fn entry(table: &Table, index: usize) -> Result<usize> {
    table
        .offset
        .checked_add(
            index
                .checked_mul(16)
                .context("script section stride overflow")?,
        )
        .context("script section offset overflow")
}

#[cfg(test)]
#[path = "tests/script_publication.rs"]
mod tests;
