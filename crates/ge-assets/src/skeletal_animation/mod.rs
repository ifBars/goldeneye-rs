//! Checked table-three/type-0x102 metadata and compressed payload spans.
//! This does not decode frames, construct a live child or evaluate a pose.
use crate::{
    binary::Bytes,
    edb::{Edb, Table},
};
use anyhow::{Context, Result, ensure};

const STRIDE: usize = 0x20;
const HEADER_BYTES: usize = 0x98;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub directory_entries: usize,
    pub rotation_channels: usize,
    pub translation_channels: usize,
    pub auxiliary_channels: usize,
    pub payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub reference: u32,
    pub directory_index: usize,
    pub source_address: usize,
    pub hash: u32,
    pub section_raw: u16,
    pub debug_raw: u16,
    pub address: usize,
    /// Includes unused serialized runtime pointers and an optional same-wrapper
    /// skin reference at +0x1c, distinct from the body's owning-wrapper field.
    /// None of these words are host pointers or forced external skin bindings.
    pub words_raw: [u32; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadSpan {
    pub address: usize,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub address: usize,
    /// Native initialization overwrites this key with a payload-relative offset.
    pub stored_key_raw: u32,
    pub word_08_raw: u32,
    pub frames_per_second: u8,
    pub cache_chunk_frames: u8,
    pub frame_count: u16,
    pub word_10_raw: u32,
    pub rotation_channels: u8,
    pub translation_channels: u8,
    pub auxiliary_channels: u16,
    pub word_18_raw: u32,
    pub runtime_wrapper_raw: u32,
    /// Authored metadata; this alone does not force another resident skin.
    pub model_pair_raw: [u32; 2],
    pub words_28_raw: [u32; 2],
    pub translation_mask_raw: [u32; 4],
}

/// Owned metadata with a span into the same retained immutable EDB owner.
/// Original header arrays and frame bytes deliberately do not implement Serialize.
#[derive(Debug, Clone)]
pub struct Animation {
    resource_hash: u32,
    resource_bytes: u32,
    descriptor: Descriptor,
    header: Header,
    payload: PayloadSpan,
    translation_bones: Vec<u16>,
}

impl Animation {
    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn payload_span(&self) -> PayloadSpan {
        self.payload
    }

    pub fn translation_bones(&self) -> &[u16] {
        &self.translation_bones
    }

    /// Lazy raw XYZ root bias. This reads source metadata, never mutates the
    /// shared raw frame cache and never substitutes the authored model skin.
    pub fn root_bias_bits(&self, edb: &Edb) -> Result<[u32; 3]> {
        ensure!(
            edb.header.version == 263
                && edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes,
            "skeletal root bias resource identity differs"
        );
        edb.skeletal_animation_root_bias(self)
    }

    /// Packed source body+0x38 consumer flags/relative data word. Interpret only
    /// reached proven flag bits; this value is not an on-disc host pointer.
    pub fn consumer_flags_bits(&self, edb: &Edb) -> Result<u32> {
        ensure!(
            edb.header.version == 263
                && edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes,
            "skeletal consumer flags resource identity differs"
        );
        edb.skeletal_animation_consumer_flags(self)
    }

    /// Borrow compressed bytes from the retained owner. No decoding or copy.
    pub fn compressed_payload<'a>(&self, edb: &'a Edb) -> Result<&'a [u8]> {
        ensure!(
            edb.header.version == 263
                && edb.header.hash == self.resource_hash
                && edb.header.payload_bytes == self.resource_bytes,
            "skeletal animation resource identity differs"
        );
        edb.skeletal_animation_payload(self)
    }
}

pub(crate) fn read(
    data: &[u8],
    table: &Table,
    resource_hash: u32,
    resource_bytes: u32,
    reference: u32,
    budget: Budget,
) -> Result<Option<Animation>> {
    ensure!(
        table.name == "animations",
        "skeletal animation requires table three"
    );
    ensure!(
        usize::from(table.count) <= budget.directory_entries,
        "skeletal directory budget exhausted"
    );
    let Some(index) = crate::reference::resolve(data, table, STRIDE, reference)? else {
        return Ok(None);
    };
    let at = index
        .checked_mul(STRIDE)
        .and_then(|n| table.offset.checked_add(n))
        .context("skeletal descriptor overflow")?;
    let b = Bytes(data);
    b.range(at, STRIDE)?;
    let mut words_raw = [0; 8];
    for (i, word) in words_raw.iter_mut().enumerate() {
        *word = b.u32(at + 4 * i)?;
    }
    let section_raw = b.u16(at + 4)?;
    ensure!(
        section_raw & 0x7fff == 0,
        "skeletal section ownership is unsupported"
    );
    let address = words_raw[2] as usize;
    b.range(address, HEADER_BYTES)?;
    ensure!(
        b.u32(address)? == 0x102,
        "unsupported skeletal animation body tag"
    );
    let rotation_channels = data[address + 0x14];
    let translation_channels = data[address + 0x15];
    let auxiliary_channels = b.u16(address + 0x16)?;
    ensure!(
        usize::from(rotation_channels) <= budget.rotation_channels,
        "skeletal rotation budget exhausted"
    );
    ensure!(
        usize::from(translation_channels) <= budget.translation_channels,
        "skeletal translation budget exhausted"
    );
    ensure!(
        usize::from(auxiliary_channels) <= budget.auxiliary_channels,
        "skeletal auxiliary budget exhausted"
    );
    // The supported source consumers use four channel-mask words. This is a
    // host representation limit, not a claimed native checked capacity.
    ensure!(
        rotation_channels <= 128,
        "skeletal rotation mask exceeds supported representation"
    );
    let mut mask = [0; 4];
    for (i, word) in mask.iter_mut().enumerate() {
        *word = b.u32(address + 0x60 + 4 * i)?;
    }
    let mask_count = mask.iter().map(|word| word.count_ones()).sum::<u32>();
    ensure!(
        mask_count == u32::from(translation_channels),
        "skeletal translation mask count differs"
    );
    let payload = PayloadSpan {
        address: words_raw[4] as usize,
        bytes: usize::try_from(words_raw[6])?,
    };
    ensure!(
        payload.bytes <= budget.payload_bytes,
        "skeletal payload budget exhausted"
    );
    b.range(payload.address, payload.bytes)
        .context("skeletal compressed payload outside resource")?;
    let mut translation_bones = Vec::new();
    translation_bones
        .try_reserve_exact(usize::from(translation_channels))
        .context("allocating bounded skeletal channel indices")?;
    for bone in 0..128u16 {
        if mask[usize::from(bone / 32)] & (1 << (bone % 32)) != 0 {
            ensure!(
                bone < u16::from(rotation_channels),
                "skeletal translation bone exceeds rotation channels"
            );
            translation_bones.push(bone);
        }
    }
    Ok(Some(Animation {
        resource_hash,
        resource_bytes,
        descriptor: Descriptor {
            reference,
            directory_index: index,
            source_address: at,
            hash: words_raw[0],
            section_raw,
            debug_raw: b.u16(at + 6)?,
            address,
            words_raw,
        },
        header: Header {
            address,
            stored_key_raw: b.u32(address + 4)?,
            word_08_raw: b.u32(address + 8)?,
            frames_per_second: data[address + 0xc],
            cache_chunk_frames: data[address + 0xd],
            frame_count: b.u16(address + 0xe)?,
            word_10_raw: b.u32(address + 0x10)?,
            rotation_channels,
            translation_channels,
            auxiliary_channels,
            word_18_raw: b.u32(address + 0x18)?,
            runtime_wrapper_raw: b.u32(address + 0x1c)?,
            model_pair_raw: [b.u32(address + 0x20)?, b.u32(address + 0x24)?],
            words_28_raw: [b.u32(address + 0x28)?, b.u32(address + 0x2c)?],
            translation_mask_raw: mask,
        },
        payload,
        translation_bones,
    }))
}

fn validate_animation(data: &[u8], animation: &Animation) -> Result<()> {
    let b = Bytes(data);
    for (i, expected) in animation.descriptor.words_raw.iter().enumerate() {
        ensure!(
            b.u32(animation.descriptor.source_address + 4 * i)? == *expected,
            "skeletal payload descriptor differs"
        );
    }
    ensure!(
        b.u32(animation.descriptor.address)? == 0x102,
        "skeletal payload body differs"
    );
    Ok(())
}

pub(crate) fn root_bias(data: &[u8], animation: &Animation) -> Result<[u32; 3]> {
    validate_animation(data, animation)?;
    let at = animation
        .descriptor
        .address
        .checked_add(0x8c)
        .context("skeletal root bias offset overflow")?;
    let b = Bytes(data);
    Ok([b.u32(at)?, b.u32(at + 4)?, b.u32(at + 8)?])
}

pub(crate) fn consumer_flags(data: &[u8], animation: &Animation) -> Result<u32> {
    validate_animation(data, animation)?;
    Bytes(data).u32(
        animation
            .descriptor
            .address
            .checked_add(0x38)
            .context("skeletal consumer flags offset overflow")?,
    )
}

pub(crate) fn payload<'a>(data: &'a [u8], animation: &Animation) -> Result<&'a [u8]> {
    validate_animation(data, animation)?;
    Bytes(data).range(animation.payload.address, animation.payload.bytes)
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
