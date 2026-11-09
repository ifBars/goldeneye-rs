//! Checked, lazy spreadsheet rows. The sheet header and row words remain opaque.
//!
//! Native group access addresses a signed field-relative pointer at
//! `body + 4 + 4 * group`. The first body word is preserved, not inferred to be
//! a group count. Row layout and host work budgets are supplied by the caller.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

const DIRECTORY_STRIDE: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Descriptor {
    pub directory_index: usize,
    pub hash: u32,
    pub section_raw: u16,
    pub debug_raw: u16,
    pub address: usize,
    pub runtime_pointer_raw: u32,
    pub word_10_raw: u32,
}

/// These are host access limits, not fields asserted to exist in the asset.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub group_fields: usize,
    pub rows: usize,
    pub row_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct Spreadsheet<'a> {
    data: &'a [u8],
    address: usize,
    descriptor: Option<Descriptor>,
}

#[derive(Debug, Clone, Copy)]
pub struct Rows<'a> {
    data: &'a [u8],
    pub pointer_field: usize,
    pub relative_raw: i32,
    pub block_address: usize,
    pub row_count: usize,
    pub row_stride: usize,
}

/// Resolve the native coded reference in the twenty-byte spreadsheet directory.
/// The raw body offset at +8 is used; the on-disc runtime pointer at +12 is not
/// dereferenced or treated as a replacement address.
pub fn read<'a>(data: &'a [u8], table: &Table, reference: u32) -> Result<Option<Spreadsheet<'a>>> {
    ensure!(
        table.name == "spreadsheets",
        "spreadsheet reader requires table eleven"
    );
    let Some(index) = crate::reference::resolve(data, table, DIRECTORY_STRIDE, reference)? else {
        return Ok(None);
    };
    let at = table
        .offset
        .checked_add(
            index
                .checked_mul(DIRECTORY_STRIDE)
                .context("spreadsheet directory overflow")?,
        )
        .context("spreadsheet directory overflow")?;
    let bytes = Bytes(data);
    bytes.range(at, DIRECTORY_STRIDE)?;
    let address = bytes.u32(at + 8)? as usize;
    let mut sheet = read_at(data, address)?;
    sheet.descriptor = Some(Descriptor {
        directory_index: index,
        hash: bytes.u32(at)?,
        section_raw: bytes.u16(at + 4)?,
        debug_raw: bytes.u16(at + 6)?,
        address,
        runtime_pointer_raw: bytes.u32(at + 12)?,
        word_10_raw: bytes.u32(at + 16)?,
    });
    Ok(Some(sheet))
}

/// Create a view with only the opaque body header addressed. Groups stay lazy.
pub fn read_at(data: &[u8], address: usize) -> Result<Spreadsheet<'_>> {
    Bytes(data).range(address, 4)?;
    Ok(Spreadsheet {
        data,
        address,
        descriptor: None,
    })
}

impl<'a> Spreadsheet<'a> {
    pub fn address(&self) -> usize {
        self.address
    }

    pub fn descriptor(&self) -> Option<&Descriptor> {
        self.descriptor.as_ref()
    }

    pub fn header_word_raw(&self) -> Result<u32> {
        Bytes(self.data).u32(self.address)
    }

    /// Address only the requested group. A zero displacement remains absent,
    /// distinct from a present group with zero rows. Neither neighboring group
    /// pointers nor unrequested row layouts are interpreted.
    pub fn group(
        &self,
        index: usize,
        row_stride: usize,
        budget: Budget,
    ) -> Result<Option<Rows<'a>>> {
        ensure!(
            index < budget.group_fields,
            "spreadsheet group access budget exhausted"
        );
        ensure!(
            row_stride > 0 && row_stride <= budget.row_bytes,
            "spreadsheet row stride outside host budget"
        );
        let pointer_field = self
            .address
            .checked_add(4)
            .and_then(|at| {
                index
                    .checked_mul(4)
                    .and_then(|offset| at.checked_add(offset))
            })
            .context("spreadsheet group field overflow")?;
        let bytes = Bytes(self.data);
        let relative_raw = bytes.u32(pointer_field)? as i32;
        if relative_raw == 0 {
            return Ok(None);
        }
        let block_address = bytes.relative(pointer_field)?;
        let row_count = usize::try_from(bytes.u32(block_address)?)?;
        ensure!(
            row_count <= budget.rows,
            "spreadsheet row count exceeds host budget"
        );
        let rows_address = block_address
            .checked_add(4)
            .context("spreadsheet rows address overflow")?;
        let row_bytes = row_count
            .checked_mul(row_stride)
            .context("spreadsheet row bytes overflow")?;
        bytes.range(rows_address, row_bytes)?;
        Ok(Some(Rows {
            data: self.data,
            pointer_field,
            relative_raw,
            block_address,
            row_count,
            row_stride,
        }))
    }
}

impl<'a> Rows<'a> {
    pub fn row(&self, index: usize) -> Result<&'a [u8]> {
        ensure!(
            index < self.row_count,
            "spreadsheet row index outside group"
        );
        let at = self
            .block_address
            .checked_add(4)
            .and_then(|at| {
                index
                    .checked_mul(self.row_stride)
                    .and_then(|offset| at.checked_add(offset))
            })
            .context("spreadsheet row offset overflow")?;
        Bytes(self.data).range(at, self.row_stride)
    }

    /// Decode a complete fixed-width big-endian word row without interpreting
    /// float, reference or flag meanings that belong to its consuming system.
    pub fn words<const N: usize>(&self, index: usize) -> Result<[u32; N]> {
        ensure!(
            N.checked_mul(4) == Some(self.row_stride),
            "spreadsheet word width does not match row stride"
        );
        let row = self.row(index)?;
        let mut words = [0; N];
        for (index, word) in words.iter_mut().enumerate() {
            *word = Bytes(row).u32(index * 4)?;
        }
        Ok(words)
    }
}

#[cfg(test)]
#[path = "tests/spreadsheet.rs"]
mod tests;
