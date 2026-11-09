//! Owner-bound target/section locators for the observed skin/texture requests.
//! Available supplied bytes do not establish original streaming completion.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use std::{fmt, ops::Range};

const BASE_KEY: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skin,
    Texture,
}

impl Kind {
    fn from_reference(reference: u32) -> Result<Self> {
        match (reference >> 24) & 0x7f {
            0x0d => Ok(Self::Skin),
            0x06 => Ok(Self::Texture),
            _ => anyhow::bail!("unsupported target section reference kind"),
        }
    }

    fn directory(self) -> (usize, &'static str, usize) {
        match self {
            Self::Skin => (4, "skins", 36),
            Self::Texture => (15, "textures", 28),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub targets: usize,
    pub sections: usize,
    pub section_bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            targets: i16::MAX as usize,
            sections: i16::MAX as usize,
            section_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A locator can only be constructed by reading its retained byte owner. Neither
/// returned metadata nor an external range can authorize another payload read.
pub struct View<'a> {
    resource_hash: u32,
    request: u32,
    kind: Kind,
    directory_index: usize,
    stored_hash: u32,
    raw_section: u16,
    section_index: usize,
    key: u32,
    span: Option<Range<usize>>,
    payload: Option<&'a [u8]>,
}

impl fmt::Debug for View<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TargetSection")
            .field("resource_hash", &self.resource_hash)
            .field("request", &self.request)
            .field("kind", &self.kind)
            .field("directory_index", &self.directory_index)
            .field("stored_hash", &self.stored_hash)
            .field("raw_section", &self.raw_section)
            .field("section_index", &self.section_index)
            .field("key", &self.key)
            .field("span", &self.span)
            .finish_non_exhaustive()
    }
}

impl<'a> View<'a> {
    pub fn resource_hash(&self) -> u32 {
        self.resource_hash
    }
    pub fn request(&self) -> u32 {
        self.request
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn directory_index(&self) -> usize {
        self.directory_index
    }
    pub fn stored_hash(&self) -> u32 {
        self.stored_hash
    }
    pub fn raw_section(&self) -> u16 {
        self.raw_section
    }
    pub fn section_index(&self) -> usize {
        self.section_index
    }
    pub fn key(&self) -> u32 {
        self.key
    }
    pub fn is_base(&self) -> bool {
        self.key == BASE_KEY
    }
    pub fn span(&self) -> Option<Range<usize>> {
        self.span.clone()
    }

    /// The checked range in this same immutable owner. The exact base key has
    /// no streamed payload. Unknown non-base keys remain data, not evidence
    /// that the original resource manager supports or completed their load.
    pub fn payload(&self) -> Option<&'a [u8]> {
        self.payload
    }
}

pub(crate) fn read<'a>(
    data: &'a [u8],
    tables: &[Table],
    resource_hash: u32,
    reference: u32,
    budget: Budget,
) -> Result<Option<View<'a>>> {
    let kind = Kind::from_reference(reference)?;
    let (table_index, name, stride) = kind.directory();
    let targets = tables
        .get(table_index)
        .context("missing target directory")?;
    ensure!(
        targets.name == name,
        "target section directory kind differs"
    );
    ensure!(
        targets.count <= i16::MAX as u16 && usize::from(targets.count) <= budget.targets,
        "target section directory budget or signed count invalid"
    );
    let Some(directory_index) = crate::reference::resolve(data, targets, stride, reference)? else {
        return Ok(None);
    };
    let b = Bytes(data);
    ensure!(
        b.u32(4)? == resource_hash,
        "target section resource identity differs"
    );
    let descriptor = entry(targets.offset, directory_index, stride)?;
    let stored_hash = b.u32(descriptor)?;
    let raw_section = b.u16(
        descriptor
            .checked_add(4)
            .context("target section field overflow")?,
    )?;
    let section_index = usize::from(raw_section & 0x7fff);
    let sections = tables.first().context("missing section directory")?;
    ensure!(
        sections.name == "sections",
        "target section requires section directory zero"
    );
    ensure!(
        sections.count <= i16::MAX as u16 && usize::from(sections.count) <= budget.sections,
        "target section count budget or signed count invalid"
    );
    ensure!(
        section_index < usize::from(sections.count),
        "target section index out of bounds"
    );
    let section = entry(sections.offset, section_index, 16)?;
    let key = b.u32(section)?;
    let (span, payload) = if key == BASE_KEY {
        // The source base-key branch does not address start/end/runtime fields.
        (None, None)
    } else {
        let start = usize::try_from(
            b.u32(
                section
                    .checked_add(4)
                    .context("section start field overflow")?,
            )?,
        )?;
        let end = usize::try_from(
            b.u32(
                section
                    .checked_add(8)
                    .context("section end field overflow")?,
            )?,
        )?;
        let bytes = end
            .checked_sub(start)
            .context("target section span is reversed")?;
        ensure!(
            bytes <= budget.section_bytes,
            "target section byte budget exhausted"
        );
        (Some(start..end), Some(b.range(start, bytes)?))
    };
    Ok(Some(View {
        resource_hash,
        request: reference,
        kind,
        directory_index,
        stored_hash,
        raw_section,
        section_index,
        key,
        span,
        payload,
    }))
}

fn entry(start: usize, index: usize, stride: usize) -> Result<usize> {
    start
        .checked_add(
            index
                .checked_mul(stride)
                .context("target section stride overflow")?,
        )
        .context("target section entry overflow")
}

#[cfg(test)]
mod tests;
