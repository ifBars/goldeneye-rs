//! Asset-only native movie decoding audit. Outputs hashes and metadata, never
//! extracted video/audio. This does not establish presentation or speaker output.
use crate::frontend_media::{Catalog, Movie};
use anyhow::{Context, Result, ensure};
use ge_assets::{resource::GameAssets, thp};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(clap::Args)]
pub struct Options {
    pub disc: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}
#[derive(Serialize)]
struct VideoSample {
    frame: u32,
    video_payload_sha256: String,
    rgba_sha256: String,
}
#[derive(Serialize)]
struct AudioReport {
    frames_decoded: usize,
    sample_rate: u32,
    channels: u32,
    samples_per_channel: u64,
    interleaved_samples: u64,
    pcm_s16le_sha256: String,
    nonzero_samples: u64,
    peak_absolute_sample: u32,
    clipped_samples: u64,
}
#[derive(Default)]
struct AudioHash {
    sha: Sha256,
    frames: usize,
    samples_per_channel: u64,
    values: u64,
    nonzero: u64,
    peak: u32,
    clipped: u64,
}
impl AudioHash {
    fn observe(&mut self, audio: &thp::PcmAudio, expected: &thp::AudioInfo) -> Result<()> {
        ensure!(
            audio.channels == expected.channels && audio.sample_rate == expected.sample_rate,
            "movie audit decoded audio format differs"
        );
        ensure!(
            audio.samples.len() == audio.samples_per_channel as usize * audio.channels as usize,
            "movie audit audio sample shape differs"
        );
        let total = self.samples_per_channel + u64::from(audio.samples_per_channel);
        ensure!(
            total <= u64::from(expected.total_samples),
            "movie audit decoded audio exceeds declared samples"
        );
        for &sample in &audio.samples {
            self.sha.update(sample.to_le_bytes());
            self.nonzero += u64::from(sample != 0);
            self.peak = self.peak.max(i32::from(sample).unsigned_abs());
            self.clipped += u64::from(sample == i16::MIN || sample == i16::MAX);
        }
        self.frames += 1;
        self.samples_per_channel = total;
        self.values += audio.samples.len() as u64;
        Ok(())
    }
    fn finish(self, expected: &thp::AudioInfo) -> Result<AudioReport> {
        ensure!(
            self.samples_per_channel == u64::from(expected.total_samples),
            "movie audit decoded audio total differs from header"
        );
        Ok(AudioReport {
            frames_decoded: self.frames,
            sample_rate: expected.sample_rate,
            channels: expected.channels,
            samples_per_channel: self.samples_per_channel,
            interleaved_samples: self.values,
            pcm_s16le_sha256: format!("{:x}", self.sha.finalize()),
            nonzero_samples: self.nonzero,
            peak_absolute_sample: self.peak,
            clipped_samples: self.clipped,
        })
    }
}
#[derive(Serialize)]
struct ClipReport {
    movie: Movie,
    path: &'static str,
    header: thp::Header,
    frames_indexed: usize,
    video_frames_decoded: usize,
    rgba_bytes_decoded: u64,
    /// SHA256 over each frame's LE u32 index followed by decoded row-major RGBA.
    video_sequence_rgba_sha256: String,
    sampled_video: Vec<VideoSample>,
    audio: Option<AudioReport>,
}
fn sample_indices(count: u32) -> Result<BTreeSet<u32>> {
    ensure!(count > 0, "movie audit cannot sample an empty clip");
    Ok(
        [0, 1.min(count - 1), 30.min(count - 1), count / 2, count - 1]
            .into_iter()
            .collect(),
    )
}
pub fn run(options: &Options) -> Result<Value> {
    let mut assets = GameAssets::open(&options.disc, "Filelist.bin")?;
    let catalog = Catalog::load(&mut assets)?;
    let mut clips = Vec::new();
    let mut video_frames = 0;
    let mut audio_values = 0u64;
    for movie in Movie::ALL {
        let clip = catalog
            .clips
            .get(&movie)
            .context("movie audit catalog clip missing")?;
        ensure!(
            clip.frames.len() == clip.header.frame_count as usize,
            "movie audit index count differs"
        );
        let samples = sample_indices(clip.header.frame_count)?;
        let mut sampled_video = Vec::new();
        let mut video_hash = Sha256::new();
        let mut rgba_bytes_decoded = 0u64;
        let mut audio_decoder = thp::AudioDecoder::default();
        let mut audio_hash = AudioHash::default();
        eprintln!(
            "Decoding movie {}: {} frames",
            movie.path(),
            clip.header.frame_count
        );
        for (index, frame) in clip.frames.iter().enumerate() {
            ensure!(
                frame.index as usize == index,
                "movie audit frame index order differs"
            );
            let video =
                assets
                    .disc
                    .read(movie.path(), frame.video_offset, frame.video_bytes as usize)?;
            let image = thp::decode_frame(&clip.header, frame, &video)
                .with_context(|| format!("movie {} video frame {}", movie.path(), frame.index))?;
            video_hash.update(frame.index.to_le_bytes());
            video_hash.update(&image.pixels);
            rgba_bytes_decoded += image.pixels.len() as u64;
            if samples.contains(&frame.index) {
                sampled_video.push(VideoSample {
                    frame: frame.index,
                    video_payload_sha256: format!("{:x}", Sha256::digest(&video)),
                    rgba_sha256: format!("{:x}", Sha256::digest(&image.pixels)),
                });
            }
            if let Some(expected) = &clip.header.audio {
                let data = assets.disc.read(
                    movie.path(),
                    frame
                        .audio_offset
                        .context("movie audit audio offset absent")?,
                    frame.audio_bytes as usize,
                )?;
                let decoded = audio_decoder
                    .decode(&clip.header, frame, &data)
                    .with_context(|| {
                        format!("movie {} audio frame {}", movie.path(), frame.index)
                    })?;
                audio_hash.observe(&decoded, expected)?;
            } else {
                ensure!(
                    frame.audio_offset.is_none() && frame.audio_bytes == 0,
                    "video-only movie has audio frame metadata"
                );
            }
            if (index + 1) % 120 == 0 {
                eprintln!(
                    "Decoded {}: {}/{}",
                    movie.path(),
                    index + 1,
                    clip.frames.len()
                );
            }
        }
        ensure!(
            sampled_video.len() == samples.len(),
            "movie audit selected sample coverage differs"
        );
        let audio = clip
            .header
            .audio
            .as_ref()
            .map(|expected| audio_hash.finish(expected))
            .transpose()?;
        if let Some(audio) = &audio {
            audio_values += audio.interleaved_samples;
        }
        video_frames += clip.frames.len();
        clips.push(ClipReport {
            movie,
            path: movie.path(),
            header: clip.header.clone(),
            frames_indexed: clip.frames.len(),
            video_frames_decoded: clip.frames.len(),
            rgba_bytes_decoded,
            video_sequence_rgba_sha256: format!("{:x}", video_hash.finalize()),
            sampled_video,
            audio,
        });
    }
    let music = crate::frontend_music::audit(&options.disc)?;
    let intro_cue = crate::frontend_cue::Cue::load(&options.disc)?.evidence();
    let report = json!({"title_music":music,"intro_cue":intro_cue,"schema":1,"disc":assets.disc.identity,"clips":clips,"video_frames_decoded":video_frames,
        "interleaved_audio_samples_decoded":audio_values,"reference_compared":false,
        "scope":"Independent native THP baseline video and sequential DSP ADPCM decoding from the supplied image. All indexed video frames and audio packets, resident title music and nonlooping title-intro cue decoded; canonical RGBA/PCM hashes and metadata only. Cue decoding is silent and opens no voice. No original-runtime comparison, presentation timing, movie-order, speaker-output or full frontend-state parity claim."});
    crate::write_new(&options.output, &serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

#[cfg(test)]
#[path = "tests/frontend_media_audit.rs"]
mod tests;
