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

impl Edb {
    /// The source tag walk (80339ae0) over a resolved base-resident body, with
    /// no topology parse. Only the low 16 bits of `tag` are compared. `Some` is
    /// the first match; `None` is the `0xffff` sentinel reached before any
    /// match, and is the only missing result. A body from another owner or
    /// directory state, a zero table pointer, malformed ranges, an exhausted
    /// `budget` or a matched bone at or beyond the body's bone count are errors.
    pub fn base_skin_tag(
        &self,
        body: &crate::skin::Body,
        tag: u32,
        budget: usize,
    ) -> Result<Option<crate::skin::Tag>> {
        ensure!(
            body.file() == self.header.hash,
            "skin tag body belongs to another resource"
        );
        let current = self
            .base_skin_body(body.reference())?
            .context("skin tag body descriptor disappeared")?;
        ensure!(
            current.directory_index() == body.directory_index()
                && current.address() == body.address()
                && current.stored_hash() == body.stored_hash(),
            "skin tag body binding changed"
        );
        let base = self.base_bytes()?;
        let found = crate::skin::tag_walk(base, body.address(), tag, budget)?;
        if let Some(tag) = &found {
            // The count is read only for a match; a sentinel never needs it.
            let count_at = body
                .address()
                .checked_add(4)
                .context("skin body count overflow")?;
            let count = usize::try_from(Bytes(base).u32(count_at)?)?;
            ensure!(
                usize::from(tag.bone_index) < count,
                "skin tag bone index is out of bounds"
            );
        }
        Ok(found)
    }
}

#[cfg(test)]
#[path = "tests/skin_reference.rs"]
mod tests;
