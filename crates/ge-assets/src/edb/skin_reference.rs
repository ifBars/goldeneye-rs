//! Lazy base-resident skin metadata. Cached file tails establish no residency.
use super::Edb;
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};

#[derive(Debug)]
pub struct Binding {
    directory_index: usize,
    hash: u32,
    section: u16,
    address: u32,
}
impl Binding {
    pub fn directory_index(&self) -> usize {
        self.directory_index
    }
    pub fn hash(&self) -> u32 {
        self.hash
    }
    pub fn raw_section(&self) -> u16 {
        self.section
    }
    pub fn authored_address(&self) -> u32 {
        self.address
    }
}

impl Edb {
    /// Resolve metadata without reading the skin body, topology or runtime pointer.
    pub fn base_skin_reference(&self, reference: u32) -> Result<Option<Binding>> {
        let base = self.base_bytes()?;
        let bytes = Bytes(base);
        ensure!(
            self.header.version == 263 && bytes.u32(8)? == 263,
            "skin metadata requires supported EDB version"
        );
        ensure!(
            bytes.u32(4)? == self.header.hash,
            "skin metadata resource identity differs"
        );
        let table = self.tables.get(4).context("missing skin directory")?;
        ensure!(table.name == "skins", "skin metadata requires table four");
        let Some(directory_index) = crate::reference::resolve(base, table, 36, reference)? else {
            return Ok(None);
        };
        let at = directory_index
            .checked_mul(36)
            .and_then(|offset| table.offset.checked_add(offset))
            .context("skin descriptor overflow")?;
        Ok(Some(Binding {
            directory_index,
            hash: bytes.u32(at)?,
            section: bytes.u16(at.checked_add(4).context("skin section field overflow")?)?,
            address: bytes.u32(at.checked_add(8).context("skin address field overflow")?)?,
        }))
    }

    /// Read a reached base body header without exposing a cached nonbase payload.
    pub fn base_skin_body(&self, reference: u32) -> Result<Option<crate::skin::Body>> {
        let Some(binding) = self.base_skin_reference(reference)? else {
            return Ok(None);
        };
        ensure!(
            binding.raw_section() & 0x7fff == 0,
            "skin body requires nonbase publication provider"
        );
        crate::skin::body_read(
            self.base_bytes()?,
            self.tables.get(4).context("missing skin directory")?,
            self.header.hash,
            reference,
            i16::MAX as usize,
        )
    }
}

#[cfg(test)]
#[path = "tests/skin_reference.rs"]
mod tests;
