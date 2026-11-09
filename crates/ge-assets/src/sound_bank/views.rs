use super::{SOUND_STRIDE, View, WAVE_STRIDE};
use anyhow::{Context, Result, ensure};
use std::fmt;

#[derive(Clone, Copy)]
pub struct Sound<'a> {
    pub(super) bank: View<'a>,
    pub(super) index: usize,
}

impl<'a> Sound<'a> {
    pub fn index(self) -> usize {
        self.index
    }
    pub fn resource_hash(self) -> u32 {
        self.bank.resource_hash
    }
    pub fn descriptor(self) -> &'a [u8] {
        let at = self.bank.sound_start + self.index * SOUND_STRIDE;
        &self.bank.data.0[at..at + SOUND_STRIDE]
    }
    pub fn hash(self) -> u32 {
        u32::from_be_bytes(
            self.descriptor()[..4]
                .try_into()
                .expect("checked sound row"),
        )
    }
    pub fn layer_count(self) -> u8 {
        self.descriptor()[12]
    }
    /// Resolve only a requested single-layer resident reference. External
    /// streams return None before touching their non-resident wave pointer;
    /// composite programs are explicitly unsupported by this bridge.
    pub fn resident_wave(self) -> Result<Option<Wave<'a>>> {
        ensure!(
            self.layer_count() == 1,
            "resident lookup requires a single-layer sound"
        );
        let data = self.bank.data;
        let at = self.bank.sound_start + self.index * SOUND_STRIDE;
        let pair = data.relative(at + 8)?;
        data.range(pair, 8)?;
        let parameters = data.relative(pair)?;
        let parameters = data.range(parameters, 12)?;
        if parameters[11] & 2 != 0 {
            return Ok(None);
        }
        let wave_at = data.relative(pair + 4)?;
        let relative = wave_at
            .checked_sub(self.bank.wave_start)
            .context("resident wave precedes directory")?;
        ensure!(
            relative.is_multiple_of(WAVE_STRIDE),
            "resident wave reference is not a directory row"
        );
        Ok(Some(self.bank.wave(relative / WAVE_STRIDE)?))
    }
}

impl fmt::Debug for Sound<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BankSound")
            .field("resource_hash", &self.resource_hash())
            .field("index", &self.index)
            .field("hash", &self.hash())
            .field("layer_count", &self.layer_count())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
pub struct Wave<'a> {
    pub(super) bank: View<'a>,
    pub(super) index: usize,
}

impl<'a> Wave<'a> {
    pub fn index(self) -> usize {
        self.index
    }
    pub fn resource_hash(self) -> u32 {
        self.bank.resource_hash
    }
    pub fn descriptor(self) -> &'a [u8] {
        let at = self.bank.wave_start + self.index * WAVE_STRIDE;
        &self.bank.data.0[at..at + WAVE_STRIDE]
    }
    fn word(self, at: usize) -> u32 {
        u32::from_be_bytes(
            self.descriptor()[at..at + 4]
                .try_into()
                .expect("checked wave row"),
        )
    }
    pub fn hash(self) -> u32 {
        self.word(0)
    }
    pub fn sample_frames(self) -> u32 {
        self.word(4)
    }
    pub fn sample_rate(self) -> u16 {
        u16::from_be_bytes(
            self.descriptor()[0x18..0x1a]
                .try_into()
                .expect("checked wave row"),
        )
    }
    pub fn codec_raw(self) -> u8 {
        self.descriptor()[0x1a]
    }
    pub fn channels_raw(self) -> u8 {
        self.descriptor()[0x1b]
    }
    pub fn data_relative_raw(self) -> u32 {
        self.word(0x1c)
    }
    pub fn data_bytes(self) -> u32 {
        self.word(0x20)
    }
    pub fn payload(self) -> &'a [u8] {
        let size = self.data_bytes() as usize;
        if size == 0 {
            return &self.bank.data.0[..0];
        }
        let start = self.bank.audio_start + self.data_relative_raw() as usize;
        &self.bank.data.0[start..start + size]
    }
}

impl fmt::Debug for Wave<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BankWave")
            .field("resource_hash", &self.resource_hash())
            .field("index", &self.index)
            .field("hash", &self.hash())
            .field("data_bytes", &self.data_bytes())
            .finish_non_exhaustive()
    }
}
