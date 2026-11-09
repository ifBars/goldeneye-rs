//! EngineX v7 index reader. Layout documented in docs/asset-formats.md.
use crate::disc::DiscFile;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct Location {
    pub offset: u32,
    pub archive: u32,
}

#[derive(Debug, Serialize)]
pub struct Entry {
    /// Some audio entries are unnamed; resolve these by hash rather than inventing a path.
    pub path: String,
    /// EDB entries may describe only their base section, not their entire payload.
    pub base_bytes: u32,
    pub hash: u32,
    pub version: u32,
    pub flags: u32,
    pub locations: Vec<Location>,
}

#[derive(Debug, Serialize)]
pub struct Filelist {
    pub version: u32,
    pub entries: Vec<Entry>,
}

fn word(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset.checked_add(4).context("offset overflow")?)
        .context("truncated filelist field")?;
    Ok(u32::from_be_bytes(bytes.try_into()?))
}

fn relative(data: &[u8], offset: usize) -> Result<usize> {
    let target = (offset as i64)
        .checked_add(word(data, offset)? as i32 as i64)
        .context("relative pointer overflow")?;
    ensure!(
        target >= 0 && target < data.len() as i64,
        "filelist pointer out of bounds at {offset:#x}"
    );
    Ok(target as usize)
}

impl Filelist {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let version = word(data, 0)?;
        ensure!(
            version == 7,
            "only Wii big-endian filelist v7 is supported, got {version}"
        );
        ensure!(
            word(data, 4)? as usize == data.len(),
            "filelist size mismatch"
        );
        let count = word(data, 8)? as usize;
        ensure!(
            count > 0 && count <= data.len() / 28,
            "invalid filelist count"
        );
        ensure!(
            word(data, 12)? >> 16 == 1,
            "loose-file indexes are unsupported"
        );
        let strings = relative(data, 16)?;
        let table_end = strings.checked_add(count * 4).context("table overflow")?;
        ensure!(
            strings >= 20 && table_end <= data.len(),
            "invalid string pointer table"
        );
        let mut cursor = 20;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            ensure!(cursor + 20 <= strings, "entry overlaps string table");
            let base_bytes = word(data, cursor)?;
            let hash = word(data, cursor + 4)?;
            let entry_version = word(data, cursor + 8)?;
            let flags = word(data, cursor + 12)?;
            let copies = word(data, cursor + 16)? as usize;
            cursor += 20;
            ensure!(
                copies > 0 && copies <= (strings.saturating_sub(cursor)) / 8,
                "invalid location count for entry {i}"
            );
            let mut locations = Vec::with_capacity(copies);
            for _ in 0..copies {
                locations.push(Location {
                    offset: word(data, cursor)?,
                    archive: word(data, cursor + 4)?,
                });
                cursor += 8;
            }
            let start = relative(data, strings + i * 4)?;
            ensure!(start >= table_end, "filename overlaps index tables");
            let mut decoded = Vec::new();
            let mut terminated = false;
            for (j, &encoded) in data[start..].iter().take(4096).enumerate() {
                let byte = encoded
                    .wrapping_add(0x16)
                    .wrapping_sub(i as u8)
                    .wrapping_sub(j as u8);
                if byte == 0 {
                    terminated = true;
                    break;
                }
                ensure!(
                    byte.is_ascii_graphic() || byte == b' ',
                    "non-ASCII filename for entry {i}"
                );
                decoded.push(byte);
            }
            ensure!(terminated, "unterminated filename for entry {i}");
            let path = String::from_utf8(decoded)?;
            entries.push(Entry {
                path,
                base_bytes,
                hash,
                version: entry_version,
                flags,
                locations,
            });
        }
        ensure!(
            cursor == strings,
            "unparsed bytes between entries and string table"
        );
        Ok(Self { version, entries })
    }

    pub fn validate_locations(&self, index: &str, files: &[DiscFile]) -> Result<()> {
        let stem = Path::new(index)
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        for entry in &self.entries {
            for location in &entry.locations {
                let archive = format!("{stem}.{:03}", location.archive);
                let file = files
                    .iter()
                    .find(|f| f.path.eq_ignore_ascii_case(&archive))
                    .with_context(|| format!("missing archive {archive}"))?;
                ensure!(
                    u64::from(location.offset) + u64::from(entry.base_bytes) <= file.bytes,
                    "entry {} exceeds {archive}",
                    entry.path
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/filelist.rs"]
mod tests;
