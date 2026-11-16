//! The source tag walk (80339ae0): a field-relative table of {bone, tag} u16
//! pairs addressed by the skin body, shared by the parsed-skin and the lazy
//! base-body lookups. Checked byte facts only; no lifecycle or readiness state.
use super::Tag;
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};

/// Walk the tag table of the body at `body`.
///
/// Reads, in order: the table pointer at body+0x5c; then per entry the tag u16
/// (+2) first. The sentinel `0xffff` ends the walk before the bone u16 (+0) is
/// read; only the low 16 bits of `tag` are compared; the first match wins and
/// nothing after it is read. A zero table pointer, an out-of-range read or an
/// exhausted `budget` is an error, never a missing tag. The matched bone is
/// returned unchecked: callers bound it against the bone count they trust.
pub(crate) fn walk(data: &[u8], body: usize, tag: u32, budget: usize) -> Result<Option<Tag>> {
    let b = Bytes(data);
    let field = body.checked_add(0x5c).context("skin tag field overflow")?;
    // A null pointer cannot be walked safely; it is not proof of a sentinel.
    ensure!(b.u32(field)? != 0, "skin tag pointer is absent");
    let start = b.relative(field)?;
    for source_index in 0..budget {
        let source_address = source_index
            .checked_mul(4)
            .and_then(|n| start.checked_add(n))
            .context("skin tag offset overflow")?;
        let tag_raw = b.u16(
            source_address
                .checked_add(2)
                .context("skin tag offset overflow")?,
        )?;
        if tag_raw == u16::MAX {
            return Ok(None);
        }
        if tag_raw == tag as u16 {
            return Ok(Some(Tag {
                bone_index: b.u16(source_address)?,
                tag_raw,
                source_index,
                source_address,
            }));
        }
    }
    anyhow::bail!("skin tag entry budget exhausted")
}
