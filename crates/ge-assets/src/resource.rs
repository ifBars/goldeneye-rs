//! Resource selection and bounded EDB payload loading from a user-owned image.
use crate::tpl::RgbaImage;
use crate::{
    disc::GameDisc,
    edb::Edb,
    filelist::{Entry, Filelist},
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, path::Path};

pub struct GameAssets {
    pub disc: GameDisc,
    pub index: Filelist,
    index_path: String,
}

impl GameAssets {
    pub fn open(disc: &Path, index_path: &str) -> Result<Self> {
        let mut disc = GameDisc::open(disc)?;
        let bytes = disc
            .files
            .iter()
            .find(|f| f.path.eq_ignore_ascii_case(index_path))
            .context("resource index not found")?
            .bytes;
        let index = Filelist::parse(&disc.read(index_path, 0, bytes.try_into()?)?)?;
        index.validate_locations(index_path, &disc.files)?;
        Ok(Self {
            disc,
            index,
            index_path: index_path.into(),
        })
    }

    pub fn load_edb(&mut self, selector: &str) -> Result<Edb> {
        let entry = select(&self.index.entries, selector)?;
        ensure!(
            entry.path.to_ascii_lowercase().ends_with(".edb"),
            "selected resource is not an EDB"
        );
        let location = entry
            .locations
            .first()
            .context("resource has no location")?;
        let archive = format!(
            "{}.{:03}",
            Path::new(&self.index_path).with_extension("").display(),
            location.archive
        );
        let offset = u64::from(location.offset);
        let prefix = self.disc.read(&archive, offset, Edb::HEADER_BYTES)?;
        let header = Edb::read_header(&prefix)?;
        ensure!(
            header.base_bytes == entry.base_bytes,
            "EDB base size differs from index"
        );
        ensure!(
            header.hash == entry.hash && header.version == entry.version,
            "EDB identity differs from index"
        );
        let container = self
            .disc
            .files
            .iter()
            .find(|f| f.path.eq_ignore_ascii_case(&archive))
            .context("missing resource container")?;
        let next_offset = self
            .index
            .entries
            .iter()
            .flat_map(|e| &e.locations)
            .filter(|l| l.archive == location.archive && u64::from(l.offset) > offset)
            .map(|l| u64::from(l.offset))
            .min()
            .unwrap_or(container.bytes);
        ensure!(
            offset + u64::from(header.payload_bytes) <= next_offset,
            "EDB payload overlaps next resource or container end"
        );
        let mut data = Vec::new();
        data.try_reserve_exact(header.payload_bytes as usize)
            .context("allocating EDB payload")?;
        while data.len() < header.payload_bytes as usize {
            let remaining = header.payload_bytes as usize - data.len();
            let chunk = self.disc.read(
                &archive,
                offset + data.len() as u64,
                remaining.min(16 * 1024 * 1024),
            )?;
            ensure!(!chunk.is_empty(), "truncated EDB payload");
            data.extend_from_slice(&chunk);
        }
        ensure!(
            data.len() == header.payload_bytes as usize,
            "truncated EDB payload"
        );
        Edb::parse(data)
    }

    /// Resolve cross-EDB texture references without executing original resource code.
    pub fn decode_texture(
        &mut self,
        selector: &str,
        hash: u32,
        frame: usize,
        mip: u8,
    ) -> Result<RgbaImage> {
        resolve_texture(selector, hash, frame, mip, |name| self.load_edb(name))
    }
}

fn resolve_texture(
    selector: &str,
    mut hash: u32,
    frame: usize,
    mip: u8,
    mut load: impl FnMut(&str) -> Result<Edb>,
) -> Result<RgbaImage> {
    let mut selector = selector.to_owned();
    let mut seen = BTreeSet::new();
    let mut dimensions = None;
    for _ in 0..16 {
        let edb = load(&selector)?;
        ensure!(
            seen.insert((edb.header.hash, hash)),
            "cyclic external texture reference"
        );
        let info = edb.texture_info(hash)?;
        let size = (info.width, info.height, info.depth);
        ensure!(
            dimensions.is_none_or(|expected| expected == size),
            "external texture dimensions differ"
        );
        dimensions = Some(size);
        if let Some(file) = info.external_file {
            hash = *info
                .frames
                .first()
                .context("external texture has no target hash")?;
            selector = format!("0x{file:08x}");
        } else {
            return edb.decode_texture(hash, frame, mip);
        }
    }
    anyhow::bail!("external texture reference depth exceeded")
}

fn select<'a>(entries: &'a [Entry], selector: &str) -> Result<&'a Entry> {
    let hash = selector
        .strip_prefix("0x")
        .and_then(|s| u32::from_str_radix(s, 16).ok());
    let mut matches = entries.iter().filter(|e| {
        hash == Some(e.hash)
            || e.path.eq_ignore_ascii_case(selector)
            || e.path
                .rsplit(['/', '\\'])
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case(selector))
    });
    let result = matches
        .next()
        .with_context(|| format!("resource not found: {selector}"))?;
    ensure!(
        matches.next().is_none(),
        "ambiguous resource selector: {selector}; use a full virtual path or hash"
    );
    Ok(result)
}

#[cfg(test)]
#[path = "tests/resource.rs"]
mod tests;
