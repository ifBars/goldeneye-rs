//! Checked skin-body topology and lazy tag lookup, without sampled pose inference.
use crate::{
    binary::Bytes,
    edb::{Edb, Table},
};
use anyhow::{Context, Result, ensure};
mod textures;
pub use textures::{EnumeratedTextures, TextureBudget, TextureDependencies, TextureDependency};
pub(crate) use textures::{enumerate_at as textures_enumerate_at, read as textures_read};

const STRIDE: usize = 0x24;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub skins: usize,
    pub bones: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub directory_index: usize,
    pub descriptor_address: usize,
    pub hash: u32,
    pub address: usize,
    /// Full disk descriptor, including opaque words and the unused runtime pointer.
    pub words_raw: [u32; 9],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bone {
    pub parent_raw: u16,
    pub range_raw: u16,
    pub word_4_raw: u16,
    pub word_6_raw: u16,
    pub source_address: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    pub bone_index: u16,
    pub tag_raw: u16,
    pub source_index: usize,
    pub source_address: usize,
}

/// Source body+0x44 vectors. Their proven consumer supplies fallback local
/// translations and root-height inputs; no full skin rest pose is inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationDefaults {
    pub address: Option<usize>,
    pub vectors: Vec<[u32; 4]>,
}

#[derive(Debug)]
pub struct Skin {
    resource_hash: u32,
    resource_bytes: u32,
    descriptor: Descriptor,
    bones: Vec<Bone>,
}

impl Skin {
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }
    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }
    pub fn fallback_translations(
        &self,
        edb: &Edb,
        maximum_bones: usize,
    ) -> Result<TranslationDefaults> {
        ensure!(
            edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes
                && edb.header.version == 263,
            "skin translation lookup resource identity differs"
        );
        edb.skin_fallback_translations(self, maximum_bones)
    }
    /// Address the auxiliary count only when capture needs it. A nonzero count
    /// remains data; this reader does not allocate or invent auxiliary values.
    pub fn auxiliary_count(&self, edb: &Edb) -> Result<u32> {
        ensure!(
            edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes
                && edb.header.version == 263,
            "skin auxiliary lookup resource identity differs"
        );
        edb.skin_auxiliary_count(self)
    }
    /// Enumerate geometry texture dependencies through the same retained EDB.
    /// Directory absence is a distinct outcome, never an empty enumeration.
    pub fn texture_dependencies(
        &self,
        edb: &Edb,
        budget: TextureBudget,
    ) -> Result<TextureDependencies> {
        ensure!(
            edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes
                && edb.header.version == 263,
            "skin texture lookup resource identity differs"
        );
        edb.skin_texture_dependencies_for_skin(self, budget)
    }
    /// Use the same retained EDB owner from which this skin was read. Tags are
    /// addressed only during this lookup; later entries stay unconsumed.
    pub fn find_tag(&self, edb: &Edb, tag: u32, tag_budget: usize) -> Result<Option<Tag>> {
        ensure!(
            edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes
                && edb.header.version == 263,
            "skin tag lookup resource identity differs"
        );
        edb.skin_tag(self, tag, tag_budget)
    }
}

pub(crate) fn read(
    data: &[u8],
    table: &Table,
    resource_hash: u32,
    resource_bytes: u32,
    reference: u32,
    budget: Budget,
) -> Result<Option<Skin>> {
    ensure!(table.name == "skins", "skin reader requires table four");
    ensure!(
        usize::from(table.count) <= budget.skins,
        "skin directory budget exhausted"
    );
    let Some(index) = crate::reference::resolve(data, table, STRIDE, reference)? else {
        return Ok(None);
    };
    let at = table
        .offset
        .checked_add(
            index
                .checked_mul(STRIDE)
                .context("skin directory overflow")?,
        )
        .context("skin directory overflow")?;
    let b = Bytes(data);
    b.range(at, STRIDE)?;
    let mut words_raw = [0; 9];
    for (i, word) in words_raw.iter_mut().enumerate() {
        *word = b.u32(at + i * 4)?;
    }
    let address = words_raw[2] as usize;
    ensure!(b.u32(address)? == 0x200, "unsupported skin body tag");
    let count = usize::try_from(b.u32(address.checked_add(4).context("skin body overflow")?)?)?;
    ensure!(count <= budget.bones, "skin bone budget exhausted");
    let mut bones = Vec::new();
    bones
        .try_reserve_exact(count)
        .context("allocating bounded skin topology")?;
    if count > 0 {
        let field = address
            .checked_add(0x48)
            .context("skin hierarchy field overflow")?;
        ensure!(b.u32(field)? != 0, "skin hierarchy pointer is absent");
        let start = b.relative(field)?;
        b.range(
            start,
            count
                .checked_mul(8)
                .context("skin topology size overflow")?,
        )?;
        for index in 0..count {
            let source_address = start + index * 8;
            bones.push(Bone {
                parent_raw: b.u16(source_address)?,
                range_raw: b.u16(source_address + 2)?,
                word_4_raw: b.u16(source_address + 4)?,
                word_6_raw: b.u16(source_address + 6)?,
                source_address,
            });
        }
    }
    Ok(Some(Skin {
        resource_hash,
        resource_bytes,
        descriptor: Descriptor {
            directory_index: index,
            descriptor_address: at,
            hash: words_raw[0],
            address,
            words_raw,
        },
        bones,
    }))
}

pub(crate) fn validate_skin(data: &[u8], skin: &Skin) -> Result<()> {
    let b = Bytes(data);
    let descriptor = &skin.descriptor;
    for (i, expected) in descriptor.words_raw.iter().enumerate() {
        ensure!(
            b.u32(descriptor.descriptor_address + i * 4)? == *expected,
            "skin tag lookup descriptor differs"
        );
    }
    ensure!(
        b.u32(descriptor.address)? == 0x200,
        "skin tag lookup body differs"
    );
    ensure!(
        usize::try_from(b.u32(descriptor.address + 4)?)? == skin.bones.len(),
        "skin tag lookup bone count differs"
    );
    Ok(())
}

pub(crate) fn fallback_translations(
    data: &[u8],
    skin: &Skin,
    maximum_bones: usize,
) -> Result<TranslationDefaults> {
    validate_skin(data, skin)?;
    let count = skin.bones.len();
    ensure!(
        count <= maximum_bones,
        "skin fallback translation bone budget exhausted"
    );
    if count == 0 {
        return Ok(TranslationDefaults {
            address: None,
            vectors: Vec::new(),
        });
    }
    let b = Bytes(data);
    let field = skin
        .descriptor
        .address
        .checked_add(0x44)
        .context("skin translation field overflow")?;
    ensure!(
        b.u32(field)? != 0,
        "skin fallback translation pointer is absent"
    );
    let address = b.relative(field)?;
    b.range(
        address,
        count
            .checked_mul(16)
            .context("skin translation size overflow")?,
    )?;
    let mut vectors = Vec::new();
    vectors
        .try_reserve_exact(count)
        .context("allocating bounded skin translations")?;
    for index in 0..count {
        let at = address + index * 16;
        vectors.push([b.u32(at)?, b.u32(at + 4)?, b.u32(at + 8)?, b.u32(at + 12)?]);
    }
    Ok(TranslationDefaults {
        address: Some(address),
        vectors,
    })
}

pub(crate) fn auxiliary_count(data: &[u8], skin: &Skin) -> Result<u32> {
    validate_skin(data, skin)?;
    Bytes(data).u32(
        skin.descriptor
            .address
            .checked_add(0x80)
            .context("skin auxiliary field overflow")?,
    )
}

pub(crate) fn find_tag(data: &[u8], skin: &Skin, tag: u32, budget: usize) -> Result<Option<Tag>> {
    validate_skin(data, skin)?;
    let b = Bytes(data);
    let descriptor = &skin.descriptor;
    let field = descriptor
        .address
        .checked_add(0x5c)
        .context("skin tag field overflow")?;
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
            let bone_index = b.u16(source_address)?;
            ensure!(
                usize::from(bone_index) < skin.bones.len(),
                "skin tag bone index is out of bounds"
            );
            return Ok(Some(Tag {
                bone_index,
                tag_raw,
                source_index,
                source_address,
            }));
        }
    }
    anyhow::bail!("skin tag entry budget exhausted")
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
#[cfg(test)]
#[path = "tests/textures.rs"]
mod texture_tests;
