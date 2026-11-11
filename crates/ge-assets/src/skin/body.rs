//! Lazy body binding for a visual callback. This reads no bone topology, tags,
//! geometry, or on-disk runtime-cache pointer as evidence of native residency.
use super::STRIDE;
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

#[derive(Debug)]
pub struct Body {
    file: u32,
    reference: u32,
    directory_index: usize,
    address: usize,
    stored_hash: u32,
}
impl Body {
    pub fn file(&self) -> u32 {
        self.file
    }
    pub fn reference(&self) -> u32 {
        self.reference
    }
    pub fn directory_index(&self) -> usize {
        self.directory_index
    }
    pub fn address(&self) -> usize {
        self.address
    }
    pub fn stored_hash(&self) -> u32 {
        self.stored_hash
    }
}
pub(crate) fn read(
    data: &[u8],
    table: &Table,
    file: u32,
    reference: u32,
    maximum: usize,
) -> Result<Option<Body>> {
    ensure!(
        table.name == "skins" && usize::from(table.count) <= maximum,
        "visual skin directory or budget invalid"
    );
    let Some(index) = crate::reference::resolve(data, table, STRIDE, reference)? else {
        return Ok(None);
    };
    let descriptor = index
        .checked_mul(STRIDE)
        .and_then(|n| table.offset.checked_add(n))
        .context("visual skin descriptor overflow")?;
    let bytes = Bytes(data);
    let address = bytes.u32(
        descriptor
            .checked_add(8)
            .context("visual skin body field overflow")?,
    )? as usize;
    ensure!(
        bytes.u32(address)? == 0x200,
        "unsupported visual skin body tag"
    );
    Ok(Some(Body {
        file,
        reference,
        directory_index: index,
        address,
        stored_hash: bytes.u32(descriptor)?,
    }))
}
