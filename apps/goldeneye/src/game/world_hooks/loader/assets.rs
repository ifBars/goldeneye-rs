//! Bounded archive reads through the worker's actual partition. Index and
//! payload owners stay local; metadata supplied by a receipt cannot read bytes.
use anyhow::{Context, Result, ensure};
use ge_assets::{filelist::Filelist, resource::GameAssets};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(super) struct Origin {
    pub index: &'static str,
    pub hash: u32,
    pub version: u32,
    pub bytes: usize,
    pub archive: String,
    pub offset: u64,
}

pub(super) struct Sources<'a> {
    assets: &'a mut GameAssets,
    indexes: BTreeMap<&'static str, Filelist>,
}

impl<'a> Sources<'a> {
    pub fn new(assets: &'a mut GameAssets) -> Self {
        Self {
            assets,
            indexes: BTreeMap::new(),
        }
    }

    pub fn locate(
        &mut self,
        index: &'static str,
        hash: u32,
        limit: usize,
    ) -> Result<Option<Origin>> {
        if !self.indexes.contains_key(index) {
            let bytes = self
                .assets
                .disc
                .files
                .iter()
                .find(|file| file.path.eq_ignore_ascii_case(index))
                .context("loader archive index is absent")?
                .bytes;
            ensure!(
                bytes <= 8 * 1024 * 1024,
                "loader archive index byte budget exceeded"
            );
            let data = self.assets.disc.read(index, 0, usize::try_from(bytes)?)?;
            ensure!(
                data.len() as u64 == bytes,
                "loader archive index read was truncated"
            );
            let parsed = Filelist::parse(&data)?;
            parsed.validate_locations(index, &self.assets.disc.files)?;
            self.indexes.insert(index, parsed);
        }
        let table = self
            .indexes
            .get(index)
            .context("loader archive index owner is absent")?;
        let mut matches = table.entries.iter().filter(|entry| entry.hash == hash);
        let Some(entry) = matches.next() else {
            return Ok(None);
        };
        ensure!(
            matches.next().is_none(),
            "loader archive resource hash is ambiguous"
        );
        ensure!(
            entry.version == 10,
            "loader audio resource version is unsupported"
        );
        let bytes = usize::try_from(entry.base_bytes)?;
        ensure!(
            (0x2c..=limit).contains(&bytes),
            "loader audio resource exceeds byte budget"
        );
        let location = entry
            .locations
            .first()
            .context("loader audio resource has no location")?;
        let archive = format!(
            "{}.{:03}",
            index
                .strip_suffix(".bin")
                .context("loader index suffix differs")?,
            location.archive
        );
        let offset = u64::from(location.offset);
        let container = self
            .assets
            .disc
            .files
            .iter()
            .find(|file| file.path.eq_ignore_ascii_case(&archive))
            .context("loader audio container is absent")?;
        ensure!(
            offset
                .checked_add(bytes as u64)
                .is_some_and(|end| end <= container.bytes),
            "loader audio resource exceeds its container"
        );
        Ok(Some(Origin {
            index,
            hash,
            version: entry.version,
            bytes,
            archive,
            offset,
        }))
    }

    pub fn read(&mut self, origin: &Origin) -> Result<Vec<u8>> {
        // Origin has a private constructor and is bound to a checked index in
        // this loader; it is never deserialized from external metadata.
        ensure!(
            self.indexes.contains_key(origin.index),
            "loader resource index is not owned"
        );
        let data = self
            .assets
            .disc
            .read(&origin.archive, origin.offset, origin.bytes)?;
        ensure!(
            data.len() == origin.bytes,
            "loader audio payload read was truncated"
        );
        Ok(data)
    }
}
