//! Checked English text lookup through the local EDB page mapping.
//!
//! Text stays in user-owned resources. Page order and the native biased binary
//! search are retained; no full-payload scan, sorting, or fallback translation is
//! used. Other language selection and live section residency remain separate.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub pages: usize,
    pub rows_per_page: usize,
    pub code_units: usize,
}

#[derive(Debug)]
pub struct Text {
    pub hash: u32,
    pub section_hash: u32,
    pub entry_offset: usize,
    pub flags_raw: u32,
    pub text: String,
}

pub fn read(
    data: &[u8],
    sections: &Table,
    references: &Table,
    spreadsheets: &Table,
    hash: u32,
    budget: Budget,
) -> Result<Option<Text>> {
    ensure!(
        sections.name == "sections"
            && references.name == "references"
            && spreadsheets.name == "spreadsheets",
        "frontend text requires section, reference and spreadsheet directories"
    );
    let Some(index) = crate::reference::resolve(data, spreadsheets, 20, 0x1400_0018)? else {
        return Ok(None);
    };
    let b = Bytes(data);
    let mapping = b.u32(spreadsheets.offset + index * 20 + 8)? as usize;
    let pages = b.u32(mapping)? as usize;
    ensure!(pages <= budget.pages, "frontend text page budget exhausted");
    b.range(
        mapping + 4,
        pages.checked_mul(8).context("text page size overflow")?,
    )?;
    b.range(references.offset, usize::from(references.count) * 16)?;
    b.range(sections.offset, usize::from(sections.count) * 16)?;
    for page in 0..pages {
        let reference = b.u32(mapping + 8 + page * 8)? as usize;
        ensure!(
            reference < usize::from(references.count),
            "text page reference outside directory"
        );
        let descriptor = references.offset + reference * 16;
        let body = b.u32(descriptor + 8)? as usize;
        if body == 0 {
            continue;
        }
        let section = usize::from(b.u16(descriptor + 4)?);
        ensure!(
            section < usize::from(sections.count),
            "text page section outside directory"
        );
        let section = sections.offset + section * 16;
        let start = b.u32(section + 4)? as usize;
        let end = b.u32(section + 8)? as usize;
        ensure!(
            start <= end && end <= data.len(),
            "invalid text section bounds"
        );
        ensure!(
            body == start.checked_add(16).context("text body overflow")?,
            "text page body differs from section payload"
        );
        ensure!(
            body.checked_add(8).is_some_and(|v| v <= end),
            "truncated text page header"
        );
        let count = b.u32(body + 4)? as usize;
        ensure!(
            count <= budget.rows_per_page,
            "frontend text row budget exhausted"
        );
        let entries = body + 8;
        let bytes = count.checked_mul(16).context("text row size overflow")?;
        ensure!(
            entries.checked_add(bytes).is_some_and(|v| v <= end),
            "text rows cross section boundary"
        );
        let Some(row) = lookup(b, entries, count, hash)? else {
            continue;
        };
        let entry = entries + row * 16;
        if b.u32(entry + 4)? == 0 {
            return Ok(None);
        }
        let address = b.relative(entry + 4)?;
        ensure!(
            address >= entries + bytes && address < end && address % 2 == 0,
            "English text pointer outside page strings"
        );
        let mut units = Vec::new();
        for i in 0..=budget.code_units {
            let position = address
                .checked_add(i.checked_mul(2).context("text code unit offset overflow")?)
                .context("text position overflow")?;
            ensure!(
                position.checked_add(2).is_some_and(|v| v <= end),
                "unterminated English text in section"
            );
            let unit = b.u16(position)?;
            if unit == 0 {
                return Ok(Some(Text {
                    hash,
                    section_hash: b.u32(section)?,
                    entry_offset: entry,
                    flags_raw: b.u32(entry + 12)?,
                    text: String::from_utf16(&units).context("invalid English UTF-16")?,
                }));
            }
            ensure!(
                i < budget.code_units,
                "frontend text code unit budget exhausted"
            );
            units.push(unit);
        }
        unreachable!("bounded text loop returns at terminator or budget error");
    }
    Ok(None)
}

fn lookup(b: Bytes<'_>, entries: usize, count: usize, hash: u32) -> Result<Option<usize>> {
    if count == 0 {
        return Ok(None);
    }
    let (mut low, mut high, mut bias) = (0, count - 1, 1);
    loop {
        let middle = (high + low + bias) / 2;
        let value = b.u32(entries + middle * 16)?;
        if value == hash {
            return Ok(Some(middle));
        }
        if middle == low {
            return Ok(None);
        }
        if value > hash {
            high = middle;
            bias = 0;
        } else {
            low = middle;
            bias = 1;
        }
    }
}

#[cfg(test)]
#[path = "tests/frontend_text.rs"]
mod tests;
