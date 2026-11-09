//! Checked borrowed MUSX v10/WII_ SBNK 0x1e residency and directory views.
//! Program interpretation, decoding and original SDK scheduling are separate.
use crate::binary::Bytes;
use anyhow::{Context, Result, ensure};
use std::fmt;

mod views;
pub use views::{Sound, Wave};

const HEADER_END: usize = 0x854;
const SOUND_STRIDE: usize = 16;
const WAVE_STRIDE: usize = 40;

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub file_bytes: usize,
    pub sounds: usize,
    pub waves: usize,
    pub directory_bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            file_bytes: 256 * 1024 * 1024,
            sounds: 65536,
            waves: 65536,
            directory_bytes: 4 * 1024 * 1024,
        }
    }
}

/// Namespace for creating a view from its actual retained byte owner.
pub struct Bank;

/// All descriptors and compressed bytes remain borrowed from this exact owner.
/// Directory metadata alone cannot construct a view or authorize another read.
#[derive(Clone, Copy)]
pub struct View<'a> {
    data: Bytes<'a>,
    resource_hash: u32,
    sound_start: usize,
    sound_count: usize,
    wave_start: usize,
    wave_count: usize,
    audio_start: usize,
    audio_bytes: usize,
}

impl Bank {
    /// Validate residency/indexing without interpreting every sound program.
    /// Both directories retain storage order, including duplicate hashes.
    /// A requested ambiguous sound is rejected by `find_sound`, independently
    /// of whether the remainder of the bank can be retained.
    pub fn read(bytes: &[u8], expected_hash: u32, budget: Budget) -> Result<View<'_>> {
        ensure!(
            bytes.len() <= budget.file_bytes,
            "sound bank byte budget exceeded"
        );
        let data = Bytes(bytes);
        data.range(0, HEADER_END)?;
        ensure!(
            data.range(0, 4)? == b"MUSX" && le32(data, 8)? == 10 && data.range(0x10, 4)? == b"WII_",
            "unsupported sound bank container"
        );
        ensure!(
            usize::try_from(le32(data, 0xc)?)? == bytes.len(),
            "sound bank total byte count mismatch"
        );
        ensure!(
            le32(data, 4)? == expected_hash && data.u32(0x80c)? == expected_hash,
            "sound bank resource hash mismatch"
        );
        ensure!(
            data.range(0x800, 4)? == b"SBNK" && data.u32(0x804)? == 0x1e,
            "unsupported SBNK version"
        );
        let sound_count = usize::try_from(data.u32(0x814)?)?;
        let wave_count = usize::try_from(data.u32(0x834)?)?;
        ensure!(
            sound_count <= budget.sounds,
            "sound directory count budget exceeded"
        );
        ensure!(
            wave_count <= budget.waves,
            "wave directory count budget exceeded"
        );
        let sound_bytes = sound_count
            .checked_mul(SOUND_STRIDE)
            .context("sound directory size overflow")?;
        let wave_bytes = wave_count
            .checked_mul(WAVE_STRIDE)
            .context("wave directory size overflow")?;
        ensure!(
            sound_bytes
                .checked_add(wave_bytes)
                .context("bank directory size overflow")?
                <= budget.directory_bytes,
            "sound bank directory byte budget exceeded"
        );
        let sound_start = directory(data, 0x818, sound_bytes)?;
        let wave_start = directory(data, 0x838, wave_bytes)?;
        let audio_bytes = usize::try_from(data.u32(0x84c)?)?;
        let audio_start = if audio_bytes == 0 {
            0
        } else {
            let start = usize::try_from(data.u32(0x850)?)?;
            data.range(start, audio_bytes)?;
            start
        };
        // Payload spans are checked independently of codec/channel support. No
        // NGCA headers, program parameters or sample data are interpreted here.
        for index in 0..wave_count {
            let at = wave_start + index * WAVE_STRIDE;
            let size = usize::try_from(data.u32(at + 0x20)?)?;
            if size != 0 {
                let offset = usize::try_from(data.u32(at + 0x1c)?)?;
                ensure!(
                    offset
                        .checked_add(size)
                        .context("wave payload range overflow")?
                        <= audio_bytes,
                    "wave outside bank audio payload"
                );
            }
        }
        Ok(View {
            data,
            resource_hash: expected_hash,
            sound_start,
            sound_count,
            wave_start,
            wave_count,
            audio_start,
            audio_bytes,
        })
    }
}

fn le32(data: Bytes<'_>, at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(data.range(at, 4)?.try_into()?))
}

fn directory(data: Bytes<'_>, field: usize, bytes: usize) -> Result<usize> {
    if bytes == 0 {
        return Ok(0);
    }
    let start = data.relative(field)?;
    data.range(start, bytes)?;
    Ok(start)
}

impl fmt::Debug for View<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SoundBank")
            .field("resource_hash", &self.resource_hash)
            .field("file_bytes", &self.file_bytes())
            .field("sounds", &self.sound_count)
            .field("waves", &self.wave_count)
            .field("audio_bytes", &self.audio_bytes)
            .finish_non_exhaustive()
    }
}

impl<'a> View<'a> {
    pub fn resource_hash(self) -> u32 {
        self.resource_hash
    }
    pub fn file_bytes(self) -> usize {
        self.data.0.len()
    }
    /// Retained compressed container, usable by a separate bounded decoder.
    pub fn bytes(self) -> &'a [u8] {
        self.data.0
    }
    pub fn sound_count(self) -> usize {
        self.sound_count
    }
    pub fn wave_count(self) -> usize {
        self.wave_count
    }
    pub fn audio_payload(self) -> &'a [u8] {
        &self.data.0[self.audio_start..self.audio_start + self.audio_bytes]
    }
    pub fn sound(self, index: usize) -> Result<Sound<'a>> {
        ensure!(
            index < self.sound_count,
            "sound directory index out of bounds"
        );
        Ok(Sound { bank: self, index })
    }
    pub fn wave(self, index: usize) -> Result<Wave<'a>> {
        ensure!(
            index < self.wave_count,
            "wave directory index out of bounds"
        );
        Ok(Wave { bank: self, index })
    }
    pub fn find_sound(self, hash: u32) -> Result<Option<Sound<'a>>> {
        let mut selected = None;
        for index in 0..self.sound_count {
            let sound = Sound { bank: self, index };
            if sound.hash() == hash {
                ensure!(selected.is_none(), "ambiguous duplicate sound hash");
                selected = Some(sound);
            }
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests;
