//! Checked frontend font descriptors and their sparse UTF-16 glyph pages.
//! Atlas pixels remain in the ordinary texture directory. Glyph spacing words
//! are retained raw until the text-layout consumer's interpretation is confirmed.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Glyph {
    pub code_unit: u16,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// Native text-layout advance at glyph +0x10, before caller scaling.
    pub width: u16,
    /// Native text-layout height at glyph +0x12, before caller scaling.
    pub height: u16,
    /// Remaining signed metric words; no guessed bearing/advance policy.
    pub spacing_raw: [i16; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct FontBody {
    /// Stored atlas/material selector, not a texture hash or process pointer.
    pub texture_selector_raw: u32,
    /// Ascending UTF-16 code units. Absent pages/glyphs are not synthesized.
    pub glyphs: Vec<Glyph>,
}
impl FontBody {
    pub fn glyph(&self, code_unit: u16) -> Option<&Glyph> {
        self.glyphs
            .binary_search_by_key(&code_unit, |glyph| glyph.code_unit)
            .ok()
            .map(|index| &self.glyphs[index])
    }
}

#[derive(Debug, Serialize)]
pub struct FrontendFont {
    pub directory_index: usize,
    pub hash: u32,
    pub section: u16,
    pub debug: u16,
    pub address: u32,
    pub runtime_pointer_raw: u32,
    pub body: FontBody,
}

/// Resolve a native coded font reference through table twelve's 16-byte rows.
/// Missing references remain None; malformed addressed descriptors/body data error.
pub fn read(data: &[u8], table: &Table, reference: u32) -> Result<Option<FrontendFont>> {
    ensure!(
        table.name == "fonts",
        "frontend font reader requires font directory"
    );
    let Some(directory_index) = crate::reference::resolve(data, table, 16, reference)? else {
        return Ok(None);
    };
    let at = table
        .offset
        .checked_add(
            directory_index
                .checked_mul(16)
                .context("font descriptor multiplication overflow")?,
        )
        .context("font descriptor offset overflow")?;
    let b = Bytes(data);
    b.range(at, 16)?;
    let address = b.u32(at + 8)?;
    Ok(Some(FrontendFont {
        directory_index,
        hash: b.u32(at)?,
        section: b.u16(at + 4)?,
        debug: b.u16(at + 6)?,
        address,
        runtime_pointer_raw: b.u32(at + 12)?,
        body: read_at(data, address as usize)?,
    }))
}

/// The font header contains an atlas selector and 256 signed field-relative page
/// pointers. Each nonnull page holds 256 field-relative glyph pointers. Each
/// addressed glyph is 24 bytes: four UV floats and four metric halfwords.
/// Work is bounded by 256 pages and 65,536 glyph entries, with no recursion.
pub fn read_at(data: &[u8], address: usize) -> Result<FontBody> {
    let b = Bytes(data);
    b.range(address, 4 + 256 * 4)
        .context("font header outside resource")?;
    let mut glyphs = Vec::new();
    for high in 0..256usize {
        let page_field = address + 4 + high * 4;
        if b.u32(page_field)? == 0 {
            continue;
        }
        let page = b.relative(page_field)?;
        b.range(page, 256 * 4)
            .context("font glyph page outside resource")?;
        glyphs
            .try_reserve(256)
            .context("allocating bounded font glyphs")?;
        for low in 0..256usize {
            let glyph_field = page + low * 4;
            if b.u32(glyph_field)? == 0 {
                continue;
            }
            let at = b.relative(glyph_field)?;
            b.range(at, 24).context("font glyph outside resource")?;
            glyphs.push(Glyph {
                code_unit: ((high << 8) | low) as u16,
                uv_min: [b.f32(at)?, b.f32(at + 4)?],
                uv_max: [b.f32(at + 8)?, b.f32(at + 12)?],
                width: b.u16(at + 16)?,
                height: b.u16(at + 18)?,
                spacing_raw: [b.u16(at + 20)? as i16, b.u16(at + 22)? as i16],
            });
        }
    }
    Ok(FontBody {
        texture_selector_raw: b.u32(address)?,
        glyphs,
    })
}

#[cfg(test)]
#[path = "tests/frontend_font.rs"]
mod tests;
