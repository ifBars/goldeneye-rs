//! Checked views of Wii MUSX v10 runtime sound-info resources (runtime class 3).
//!
//! The generic outer hash/version/size words are little-endian; table metadata
//! and sixteen-byte rows are big-endian. Lookup preserves the native inclusive
//! hash gate and biased binary search. Rows are neither sorted nor deduplicated.
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};

const HEADER_BYTES: usize = 0x2c;
const ROW_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub resource_hash: u32,
    pub version: u32,
    pub file_bytes: u32,
    /// Opaque bytes at +0x14..+0x20. No payload meaning or byte order is inferred.
    pub reserved_raw: [u8; 12],
    pub minimum_hash: u32,
    pub maximum_hash: u32,
    pub record_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub index: usize,
    pub address: usize,
    pub sound_hash: u32,
    /// Preserve exceptional and unknown range values without an eager finite
    /// float check. Lookup addresses hashes; voice setup interprets the ranges.
    pub minimum_range_bits: u32,
    pub maximum_range_bits: u32,
    pub flags_raw: u32,
}

impl Entry {
    pub fn minimum_range(&self) -> f32 {
        f32::from_bits(self.minimum_range_bits)
    }

    pub fn maximum_range(&self) -> f32 {
        f32::from_bits(self.maximum_range_bits)
    }

    /// Conventional integer bit 24, extracted by the native rotate-left-eight
    /// and low-bit mask. Other flags remain uninterpreted here.
    pub fn stop_on_owner_release(&self) -> bool {
        self.flags_raw & (1 << 24) != 0
    }
}

#[derive(Clone, Copy)]
pub struct SoundInfo<'a> {
    data: &'a [u8],
    header: Header,
}

fn le_word(bytes: Bytes<'_>, at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes.range(at, 4)?.try_into()?))
}

/// Read the observed Wii v10 on-disc format with an explicit host row budget.
/// The native loader validates its magic; this checked adapter also limits the
/// supported wrapper version/platform and requires its declared resource size.
/// Neither ranges nor table ordering are reinterpreted as validity constraints.
pub fn read(data: &[u8], max_rows: usize) -> Result<SoundInfo<'_>> {
    let bytes = Bytes(data);
    bytes.range(0, HEADER_BYTES)?;
    ensure!(
        bytes.range(0, 4)? == b"MUSX"
            && le_word(bytes, 8)? == 10
            && bytes.range(0x10, 4)? == b"WII_",
        "unsupported runtime sound-info wrapper"
    );
    let file_bytes = le_word(bytes, 0xc)?;
    ensure!(
        u32::try_from(data.len()).ok() == Some(file_bytes),
        "runtime sound-info declared size differs from resource"
    );
    let record_count = usize::try_from(bytes.u32(0x28)?)?;
    ensure!(
        record_count <= max_rows,
        "runtime sound-info row budget exhausted"
    );
    let row_bytes = record_count
        .checked_mul(ROW_BYTES)
        .context("runtime sound-info row bytes overflow")?;
    bytes.range(HEADER_BYTES, row_bytes)?;
    Ok(SoundInfo {
        data,
        header: Header {
            resource_hash: le_word(bytes, 4)?,
            version: le_word(bytes, 8)?,
            file_bytes,
            reserved_raw: bytes.range(0x14, 12)?.try_into()?,
            minimum_hash: bytes.u32(0x20)?,
            maximum_hash: bytes.u32(0x24)?,
            record_count,
        },
    })
}

impl SoundInfo<'_> {
    pub fn header(&self) -> &Header {
        &self.header
    }

    fn address(&self, index: usize) -> Result<usize> {
        ensure!(
            index < self.header.record_count,
            "runtime sound-info row index outside table"
        );
        HEADER_BYTES
            .checked_add(
                index
                    .checked_mul(ROW_BYTES)
                    .context("sound-info row offset overflow")?,
            )
            .context("sound-info row offset overflow")
    }

    /// Read one raw row, including ranges as bit patterns rather than rejecting
    /// NaNs or assigning meaning to unrelated flag bits.
    pub fn entry(&self, index: usize) -> Result<Entry> {
        let address = self.address(index)?;
        let bytes = Bytes(self.data);
        Ok(Entry {
            index,
            address,
            sound_hash: bytes.u32(address)?,
            minimum_range_bits: bytes.u32(address + 4)?,
            maximum_range_bits: bytes.u32(address + 8)?,
            flags_raw: bytes.u32(address + 12)?,
        })
    }

    /// Select a native resident-Sound class-0x70 row. `None` leaves the caller's
    /// default/fallback sound-info policy explicit rather than fabricating flags.
    /// Duplicate keys select the encountered biased midpoint, not the first row.
    pub fn lookup(&self, sound_hash: u32) -> Result<Option<Entry>> {
        if sound_hash & 0x00f0_0000 != 0x0070_0000
            || sound_hash < self.header.minimum_hash
            || sound_hash > self.header.maximum_hash
            || self.header.record_count == 0
        {
            return Ok(None);
        }
        let (mut low, mut high, mut bias) = (0_usize, self.header.record_count - 1, 1_usize);
        loop {
            let middle = low
                .checked_add(high)
                .and_then(|sum| sum.checked_add(bias))
                .context("sound-info binary-search midpoint overflow")?
                / 2;
            let key = Bytes(self.data).u32(self.address(middle)?)?;
            if key == sound_hash {
                return self.entry(middle).map(Some);
            }
            if middle == low {
                return Ok(None);
            }
            if key > sound_hash {
                high = middle;
                bias = 0;
            } else {
                low = middle;
                bias = 1;
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/sound_info.rs"]
mod tests;
