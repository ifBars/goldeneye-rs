use std::{
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DiscIdentity {
    pub game_id: String,
    pub title: String,
    pub revision: u8,
    pub format: String,
    pub logical_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscFile {
    pub path: String,
    pub bytes: u64,
    pub partition_offset: u64,
    pub executable: bool,
    #[serde(skip)]
    node: nod::Node,
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub disc: DiscIdentity,
    pub file_count: usize,
    pub file_bytes: u64,
    pub extensions: BTreeMap<String, usize>,
    pub files: Vec<DiscFile>,
}

/// Reads only the game data partition. Never executes or links disc code.
pub struct GameDisc {
    pub identity: DiscIdentity,
    pub files: Vec<DiscFile>,
    partition: Box<dyn nod::PartitionBase>,
}

impl GameDisc {
    pub fn open(path: &Path) -> Result<Self> {
        let disc = nod::Disc::new(path).with_context(|| format!("opening {}", path.display()))?;
        let header = disc.header();
        ensure!(header.is_wii(), "expected the Wii edition of GoldenEye 007");
        ensure!(
            header.game_id.starts_with(b"SJB"),
            "unexpected game ID {}; expected GoldenEye 007 Wii (SJB*)",
            header.game_id_str()
        );
        let identity = DiscIdentity {
            game_id: header.game_id_str().to_owned(),
            title: header.game_title_str().to_owned(),
            revision: header.disc_version,
            format: format!("{:?}", disc.meta().format),
            logical_bytes: disc.disc_size(),
        };
        let mut partition = disc.open_partition_kind(nod::PartitionKind::Data)?;
        let meta = partition.meta()?;
        let fst = meta.fst().map_err(anyhow::Error::msg)?;
        let mut directories: Vec<(usize, String)> = Vec::new();
        let mut files = Vec::new();
        for (index, node, name) in fst.iter() {
            while directories.last().is_some_and(|(end, _)| index >= *end) {
                directories.pop();
            }
            let name = name.map_err(anyhow::Error::msg)?;
            validate_component(&name)?;
            let path = match directories.last() {
                Some((_, parent)) => format!("{parent}/{name}"),
                None => name.into_owned(),
            };
            if node.is_dir() {
                let end = node.length() as usize;
                ensure!(
                    end > index && end <= fst.nodes.len(),
                    "invalid directory range for {path}"
                );
                ensure!(
                    directories
                        .last()
                        .is_none_or(|(parent_end, _)| end <= *parent_end),
                    "directory escapes parent: {path}"
                );
                directories.push((end, path));
            } else if node.is_file() {
                files.push(DiscFile {
                    executable: is_executable(&path),
                    path,
                    bytes: node.length(),
                    partition_offset: node.offset(true),
                    node,
                });
            } else {
                bail!("invalid FST node {index}");
            }
        }
        Ok(Self {
            identity,
            files,
            partition,
        })
    }

    pub fn inventory(self) -> Inventory {
        let mut extensions = BTreeMap::new();
        for file in &self.files {
            let extension = Path::new(&file.path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("[none]")
                .to_ascii_lowercase();
            *extensions.entry(extension).or_default() += 1;
        }
        Inventory {
            disc: self.identity,
            file_count: self.files.len(),
            file_bytes: self.files.iter().map(|f| f.bytes).sum(),
            extensions,
            files: self.files,
        }
    }

    /// Explicit typed static-data channel. Keeps the general executable-read
    /// block intact and reuses this owned decoded data partition. The existing
    /// opener's nod metadata allocation remains a separate boundedness caveat.
    pub fn weapon_tables(
        &mut self,
        budget: crate::weapon_static::LoadBudget,
    ) -> Result<crate::weapon_static::WeaponTables> {
        crate::weapon_static::load(&mut *self.partition, &self.identity, budget)
    }

    /// Verify and retain only DATA5, exposing domain-specific typed queries.
    pub fn supplied_data(
        &mut self,
        budget: crate::supplied_data::LoadBudget,
    ) -> Result<crate::supplied_data::SuppliedData> {
        crate::supplied_data::load(&mut *self.partition, &self.identity, budget)
    }

    /// Bounded reads permit archive inspection without materializing multi-GB files.
    pub fn read(&mut self, path: &str, offset: u64, length: usize) -> Result<Vec<u8>> {
        ensure!(length <= 64 * 1024 * 1024, "one read is limited to 64 MiB");
        let file = self
            .files
            .iter()
            .find(|f| f.path.eq_ignore_ascii_case(path.trim_start_matches('/')))
            .with_context(|| format!("file not found: {path}"))?;
        ensure!(
            !file.executable,
            "executable reads are outside the asset API: {path}"
        );
        ensure!(offset <= file.bytes, "offset beyond end of {path}");
        let length = (file.bytes - offset).min(length as u64) as usize;
        let mut stream = self.partition.open_file(file.node)?;
        stream.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0; length];
        stream.read_exact(&mut data)?;
        Ok(data)
    }
}

fn validate_component(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0']),
        "invalid disc path component: {name:?}"
    );
    Ok(())
}

fn is_executable(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| {
            ["dol", "rel", "elf", "rso", "exe", "dll"]
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
}

#[cfg(test)]
#[path = "tests/disc.rs"]
mod tests;
