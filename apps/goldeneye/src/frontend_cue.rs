//! Immutable resident frontend cues. Decoding is explicit, bounded and silent;
//! callers submit the returned PCM to an independent host-clock one-shot voice.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    frontend_stream::{self, NgcaInfo},
    resource::GameAssets,
    sound_info,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

const BANK: u32 = 0x2d500001;
const SOUND: u32 = 0x2d7011ad;
const SOUND_INFO: u32 = 0x2d200000;
const BANK_BYTE_LIMIT: u64 = 8 * 1024 * 1024;
const INFO_BYTE_LIMIT: u64 = 2 * 1024 * 1024;
const INFO_ROW_LIMIT: usize = 65_536;

#[derive(Debug, Clone, Serialize)]
pub struct ResourceOrigin {
    pub index: &'static str,
    pub archive: String,
    pub resource_hash: u32,
    pub offset: u64,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct SourceOrigin {
    pub bank: ResourceOrigin,
    pub sound_info: ResourceOrigin,
}

/// Narrow resource reader: archive locations come from the checked file index,
/// never an embedded resource path or an assumed hardcoded byte offset.
struct Resource {
    assets: GameAssets,
    origin: ResourceOrigin,
}
impl Resource {
    fn open(disc: &Path, index: &'static str, hash: u32, limit: u64) -> Result<Self> {
        let assets = GameAssets::open(disc, index)?;
        let mut matches = assets
            .index
            .entries
            .iter()
            .filter(|entry| entry.hash == hash);
        let entry = matches
            .next()
            .context("frontend cue resource absent from index")?;
        ensure!(
            matches.next().is_none(),
            "ambiguous frontend cue resource hash"
        );
        ensure!(
            entry.version == 10,
            "unsupported frontend cue resource version"
        );
        let bytes = u64::from(entry.base_bytes);
        ensure!(
            (0x2c..=limit).contains(&bytes),
            "frontend cue resource exceeds byte budget"
        );
        let location = entry
            .locations
            .first()
            .context("frontend cue resource has no location")?;
        let archive = format!(
            "{}.{:03}",
            Path::new(index).with_extension("").display(),
            location.archive
        );
        let offset = u64::from(location.offset);
        let container = assets
            .disc
            .files
            .iter()
            .find(|file| file.path.eq_ignore_ascii_case(&archive))
            .context("frontend cue resource container missing")?;
        ensure!(
            offset
                .checked_add(bytes)
                .is_some_and(|end| end <= container.bytes),
            "frontend cue resource exceeds container"
        );
        let origin = ResourceOrigin {
            index,
            archive,
            resource_hash: hash,
            offset,
            bytes,
        };
        let mut resource = Self { assets, origin };
        let header = resource.read(0, 0x14)?;
        ensure!(
            &header[..4] == b"MUSX"
                && u32::from_le_bytes(header[4..8].try_into()?) == hash
                && u32::from_le_bytes(header[8..12].try_into()?) == 10
                && u64::from(u32::from_le_bytes(header[12..16].try_into()?)) == bytes
                && &header[0x10..0x14] == b"WII_",
            "frontend cue resource differs from indexed Wii MUSX v10 identity"
        );
        Ok(resource)
    }
    fn read(&mut self, at: u64, count: usize) -> Result<Vec<u8>> {
        ensure!(
            at.checked_add(u64::try_from(count)?)
                .is_some_and(|end| end <= self.origin.bytes),
            "frontend cue read outside indexed resource"
        );
        let offset = self
            .origin
            .offset
            .checked_add(at)
            .context("frontend cue read offset overflow")?;
        let bytes = self.assets.disc.read(&self.origin.archive, offset, count)?;
        ensure!(bytes.len() == count, "truncated frontend cue resource read");
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub info: NgcaInfo,
    pub decoded_frames: u64,
    pub decoded_chunks: u64,
    pub interleaved_samples: u64,
    pub nonzero_samples: u64,
    pub peak_absolute_sample: u32,
    pub pcm_s16le_sha256: String,
    pub source_flags_raw: u32,
    pub stop_on_owner_release: bool,
    pub source_origin: SourceOrigin,
    pub reference_compared: bool,
    pub scope: &'static str,
}

pub struct Cue {
    info: NgcaInfo,
    pcm: Arc<[i16]>,
    evidence: Evidence,
}
/// Confirmed identities for the supported finite resident cues. A caller must
/// establish these from the local source before adding another binding.
#[derive(Clone, Copy)]
pub(crate) struct CueSpec {
    pub sound: u32,
    pub wave: u32,
    pub rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub flags: u32,
    pub scope: &'static str,
}
const INTRO: CueSpec = CueSpec {
    sound: SOUND,
    wave: 0x2da01cd0,
    rate: 22_050,
    channels: 2,
    frames: 11_256,
    flags: 0x1fc,
    scope: "Source title-intro sound 2d7011ad, bank 2d500001, wave 2da01cd0; full nonlooping resident NGCA decode and runtime class-3 metadata. PCM remains memory-only. Host voice resampling/mixing, bus gain and hardware output equivalence are not Wii parity claims.",
};

impl Cue {
    pub fn load(disc: &Path) -> Result<Self> {
        Self::load_many(disc, &[INTRO])?
            .pop()
            .context("missing intro cue")
    }

    /// Open the indexed generic bank and runtime metadata once for this batch.
    /// The resulting PCM is shared in memory; subsequent playback performs no IO.
    pub(crate) fn load_many(disc: &Path, specs: &[CueSpec]) -> Result<Vec<Self>> {
        ensure!(
            (1..=8).contains(&specs.len()),
            "frontend cue batch exceeds budget"
        );
        for (index, spec) in specs.iter().enumerate() {
            ensure!(
                !specs[..index]
                    .iter()
                    .any(|earlier| earlier.sound == spec.sound),
                "duplicate frontend cue in batch"
            );
        }
        let mut metadata = Resource::open(disc, "File_USA.bin", SOUND_INFO, INFO_BYTE_LIMIT)?;
        let data = metadata.read(0, usize::try_from(metadata.origin.bytes)?)?;
        let table = sound_info::read(&data, INFO_ROW_LIMIT)?;
        ensure!(
            table.header().resource_hash == SOUND_INFO,
            "frontend cue runtime sound-info resource hash differs"
        );
        let mut bank = Resource::open(disc, "File_COM.bin", BANK, BANK_BYTE_LIMIT)?;
        let origin = SourceOrigin {
            bank: bank.origin.clone(),
            sound_info: metadata.origin,
        };
        specs
            .iter()
            .map(|spec| {
                let row = table
                    .lookup(spec.sound)?
                    .context("frontend cue runtime sound metadata absent")?;
                Self::decode(*spec, row, origin.clone(), |at, n| bank.read(at, n))
            })
            .collect()
    }

    fn decode(
        spec: CueSpec,
        row: sound_info::Entry,
        origin: SourceOrigin,
        mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let source_flags_raw = row.flags_raw;
        let stop_on_owner_release = row.stop_on_owner_release();
        ensure!(
            row.sound_hash == spec.sound
                && source_flags_raw == spec.flags
                && !stop_on_owner_release
                && source_flags_raw & (1 << 26) == 0,
            "frontend cue runtime metadata differs from supported nonspatial cue"
        );
        let info = frontend_stream::read_bank_sound(origin.bank.bytes, spec.sound, &mut read)?;
        ensure!(
            info.resource_hash == BANK
                && info.sound_hash == spec.sound
                && info.wave_hash == spec.wave,
            "frontend cue sound/wave binding differs"
        );
        ensure!(
            info.sample_rate == spec.rate
                && info.channels == spec.channels
                && info.sample_frames == spec.frames
                && info.codec_raw == 0x13
                && info.loop_start_frame.is_none(),
            "frontend cue format differs from confirmed nonlooping NGCA source"
        );
        ensure!(
            info.sample_frames <= u64::from(info.sample_rate) * 8,
            "frontend cue exceeds eight-second host voice budget"
        );
        let expected_samples = usize::try_from(info.sample_frames)?
            .checked_mul(usize::from(info.channels))
            .context("frontend cue sample count overflow")?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(expected_samples)
            .context("allocating frontend cue PCM")?;
        let mut decoder = info.decoder();
        let mut hash = Sha256::new();
        let mut decoded_frames = 0u64;
        let mut decoded_chunks = 0u64;
        let mut nonzero_samples = 0u64;
        let mut peak_absolute_sample = 0u32;
        while decoder.remaining_blocks(&info) > 0 {
            let count = decoder.remaining_blocks(&info).min(1024) as usize;
            let chunk = decoder.decode_blocks(&info, count, &mut read)?;
            ensure!(
                chunk.first_frame == decoded_frames
                    && chunk.frame_count > 0
                    && chunk.sample_rate == info.sample_rate
                    && chunk.channels == info.channels
                    && chunk.samples.len() == chunk.frame_count * usize::from(info.channels)
                    && chunk.samples.len() <= expected_samples.saturating_sub(samples.len()),
                "frontend cue decoded chunk exceeds source frame budget"
            );
            decoded_frames += chunk.frame_count as u64;
            decoded_chunks += 1;
            for &sample in &chunk.samples {
                hash.update(sample.to_le_bytes());
                nonzero_samples += u64::from(sample != 0);
                peak_absolute_sample = peak_absolute_sample.max(i32::from(sample).unsigned_abs());
            }
            samples.extend_from_slice(&chunk.samples);
        }
        ensure!(
            decoded_frames == info.sample_frames && samples.len() == expected_samples,
            "frontend cue decoded PCM count differs from source"
        );
        let evidence = Evidence {
            info: info.clone(),
            decoded_frames,
            decoded_chunks,
            interleaved_samples: samples.len() as u64,
            nonzero_samples,
            peak_absolute_sample,
            pcm_s16le_sha256: format!("{:x}", hash.finalize()),
            source_flags_raw,
            stop_on_owner_release,
            source_origin: origin,
            reference_compared: false,
            scope: spec.scope,
        };
        Ok(Self {
            info,
            pcm: samples.into(),
            evidence,
        })
    }
    pub fn info(&self) -> &NgcaInfo {
        &self.info
    }
    pub fn samples(&self) -> Arc<[i16]> {
        self.pcm.clone()
    }
    pub fn evidence(&self) -> Evidence {
        self.evidence.clone()
    }
}

#[cfg(test)]
#[path = "tests/frontend_cue.rs"]
pub(crate) mod tests;
