//! Skin geometry texture-dependency enumeration over one retained EDB.
//!
//! Mesh parts are read from skin body +0x6c in source order. Their low-24
//! references index the entity directory directly, and each geometry object
//! contributes the groups its kind produces: a mesh is one group, an expanding
//! split contributes its children in order, and kinds 0x607/0x608 contribute nothing.
//! Each group's +0x54 list selects texture-directory records whose first word is
//! the requested hash. Only absent hashes are appended, preserving first
//! encounter. Missing directories return their own outcome rather than an empty
//! list; other unread or unbounded native inputs are explicit host errors.

use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

const ENTITY_STRIDE: usize = 0x14;
const TEXTURE_STRIDE: usize = 0x1c;
const PART_STRIDE: usize = 0x14;
fn entry(base: usize, index: usize, stride: usize) -> Result<usize> {
    base.checked_add(
        index
            .checked_mul(stride)
            .context("skin texture index overflow")?,
    )
    .context("skin texture address overflow")
}

#[derive(Debug, Clone, Copy)]
pub struct TextureBudget {
    /// Source part total after its wrapping 32-bit sum.
    pub parts: usize,
    /// Geometry objects visited, including split containers.
    pub objects: usize,
    pub depth: usize,
    /// Texture indices read across every group, duplicates included.
    pub indices: usize,
    pub unique: usize,
}

impl Default for TextureBudget {
    fn default() -> Self {
        Self {
            parts: 4096,
            objects: 65536,
            depth: 64,
            indices: 1 << 20,
            unique: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextureDependency {
    pub hash: u32,
    pub texture_index: u16,
    /// Part ordinal of the first encounter.
    pub first_part: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnumeratedTextures {
    /// Source counts at body +0x68, +0x70 and +0x78.
    pub part_counts_raw: [u32; 3],
    pub dependencies: Vec<TextureDependency>,
    pub groups: usize,
    pub splits: usize,
    pub empty_objects: usize,
    pub indices_read: usize,
    pub duplicate_indices: usize,
}

impl EnumeratedTextures {
    pub fn hashes(&self) -> Vec<u32> {
        self.dependencies.iter().map(|value| value.hash).collect()
    }
}

/// The native routine returns before reading parts when either directory has
/// no first entry. That is retained as data distinct from an enumerated list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TextureDependencies {
    EntityDirectoryAbsent,
    TextureDirectoryAbsent,
    Enumerated(EnumeratedTextures),
}

struct Walker<'a> {
    b: Bytes<'a>,
    textures: &'a Table,
    budget: TextureBudget,
    objects: usize,
    result: EnumeratedTextures,
    seen_hashes: BTreeSet<u32>,
}

impl Walker<'_> {
    /// Emulates the slot-0x44 group producer with the expanding owner flag set.
    fn collect(&mut self, object: usize, depth: usize, groups: &mut Vec<usize>) -> Result<()> {
        ensure!(
            depth < self.budget.depth,
            "skin texture split depth budget exhausted"
        );
        ensure!(
            self.objects < self.budget.objects,
            "skin texture object budget exhausted"
        );
        self.objects += 1;
        match self.b.u32(object)? {
            0x601 => {
                groups
                    .try_reserve(1)
                    .context("allocating skin geometry groups")?;
                groups.push(object);
            }
            0x603 => {
                self.result.splits += 1;
                let field = object.checked_add(0x54).context("split field overflow")?;
                let count = usize::try_from(self.b.u32(field)?)?;
                ensure!(
                    count <= self.budget.objects,
                    "skin texture split child budget exhausted"
                );
                let slots = object.checked_add(0x5c).context("split field overflow")?;
                for index in 0..count {
                    let slot = entry(slots, index, 4)?;
                    // A zero slot is a null child whose virtual call cannot be made.
                    ensure!(self.b.u32(slot)? != 0, "skin texture split child is absent");
                    let child = self.b.relative(slot)?;
                    self.collect(child, depth + 1, groups)?;
                }
            }
            0x607 | 0x608 => {}
            kind => bail!("unsupported skin geometry kind {kind:#x} at {object:#x}"),
        }
        Ok(())
    }

    fn group(&mut self, object: usize, part: usize) -> Result<()> {
        self.result.groups += 1;
        let field = object
            .checked_add(0x54)
            .context("texture list field overflow")?;
        // The native list pointer is dereferenced unconditionally.
        ensure!(
            self.b.u32(field)? != 0,
            "skin texture list is absent at {object:#x}"
        );
        let list = self.b.relative(field)?;
        let count = usize::from(self.b.u16(list)?);
        if count == 0 {
            self.result.empty_objects += 1;
        }
        for lane in 0..count {
            ensure!(
                self.result.indices_read < self.budget.indices,
                "skin texture index budget exhausted"
            );
            self.result.indices_read += 1;
            let texture_index = self.b.u16(entry(list, 1 + lane, 2)?)?;
            ensure!(
                usize::from(texture_index) < usize::from(self.textures.count),
                "skin texture index {texture_index} outside directory"
            );
            let hash = self.b.u32(entry(
                self.textures.offset,
                usize::from(texture_index),
                TEXTURE_STRIDE,
            )?)?;
            if self.seen_hashes.contains(&hash) {
                self.result.duplicate_indices += 1;
            } else {
                ensure!(
                    self.result.dependencies.len() < self.budget.unique,
                    "skin unique texture budget exhausted"
                );
                self.result
                    .dependencies
                    .try_reserve(1)
                    .context("allocating skin texture dependencies")?;
                self.seen_hashes.insert(hash);
                self.result.dependencies.push(TextureDependency {
                    hash,
                    texture_index,
                    first_part: part,
                });
            }
        }
        Ok(())
    }
}

pub(crate) fn enumerate_at(
    data: &[u8],
    entities: &Table,
    textures: &Table,
    body: usize,
    budget: TextureBudget,
) -> Result<TextureDependencies> {
    if crate::reference::resolve(data, entities, ENTITY_STRIDE, 0)?.is_none() {
        return Ok(TextureDependencies::EntityDirectoryAbsent);
    }
    if crate::reference::resolve(data, textures, TEXTURE_STRIDE, 0)?.is_none() {
        return Ok(TextureDependencies::TextureDirectoryAbsent);
    }
    let b = Bytes(data);
    ensure!(b.u32(body)? == 0x200, "unsupported skin texture body tag");
    let field = |offset: usize| body.checked_add(offset).context("skin part field overflow");
    let part_counts_raw = [
        b.u32(field(0x68)?)?,
        b.u32(field(0x70)?)?,
        b.u32(field(0x78)?)?,
    ];
    // Source order: +0x78 plus +0x68, then +0x70, each a wrapping 32-bit add.
    let total = part_counts_raw[2]
        .wrapping_add(part_counts_raw[0])
        .wrapping_add(part_counts_raw[1]);
    let total = usize::try_from(total)?;
    ensure!(total <= budget.parts, "skin texture part budget exhausted");
    let mut walker = Walker {
        b,
        textures,
        budget,
        objects: 0,
        seen_hashes: BTreeSet::new(),
        result: EnumeratedTextures {
            part_counts_raw,
            dependencies: Vec::new(),
            groups: 0,
            splits: 0,
            empty_objects: 0,
            indices_read: 0,
            duplicate_indices: 0,
        },
    };
    if total == 0 {
        return Ok(TextureDependencies::Enumerated(walker.result));
    }
    ensure!(b.u32(field(0x6c)?)? != 0, "skin part list is absent");
    let parts = b.relative(field(0x6c)?)?;
    let mut groups = Vec::new();
    for part in 0..total {
        let part_record = entry(parts, part, PART_STRIDE)?;
        let index = (b.u32(
            part_record
                .checked_add(0xc)
                .context("skin mesh part overflow")?,
        )? & 0x00ff_ffff) as usize;
        ensure!(
            index < usize::from(entities.count),
            "skin part entity index {index} outside directory"
        );
        // The loader-fixed object pointer is the descriptor's body address.
        let descriptor = entry(entities.offset, index, ENTITY_STRIDE)?;
        let object = b.u32(
            descriptor
                .checked_add(8)
                .context("skin geometry descriptor overflow")?,
        )? as usize;
        ensure!(object != 0, "skin part geometry object is absent");
        groups.clear();
        walker.collect(object, 0, &mut groups)?;
        for &group in &groups {
            walker.group(group, part)?;
        }
    }
    Ok(TextureDependencies::Enumerated(walker.result))
}

/// Resolve only the skin descriptor needed by this operation. Bone topology,
/// tags and render payloads are not read by the source texture traversal.
pub(crate) fn read(
    data: &[u8],
    tables: &[Table],
    reference: u32,
    budget: TextureBudget,
) -> Result<Option<TextureDependencies>> {
    let skins = tables.get(4).context("missing skin directory")?;
    let Some(index) = crate::reference::resolve(data, skins, 0x24, reference)? else {
        return Ok(None);
    };
    let descriptor = skins
        .offset
        .checked_add(
            index
                .checked_mul(0x24)
                .context("skin descriptor overflow")?,
        )
        .context("skin descriptor overflow")?;
    let body = Bytes(data).u32(
        descriptor
            .checked_add(8)
            .context("skin body field overflow")?,
    )? as usize;
    enumerate_at(
        data,
        tables.get(2).context("missing entity directory")?,
        tables.get(15).context("missing texture directory")?,
        body,
        budget,
    )
    .map(Some)
}
