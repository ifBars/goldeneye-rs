//! Bounded native PCM output for decoded frontend media. Device conversion and
//! linear resampling are host presentation, not reconstructed Wii mixing.
//! Callback counters describe submission, not measured speaker playback latency.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const QUEUE_SECONDS: usize = 8;
const ONE_SHOT_VOICES: usize = 8;
const ONE_SHOT_SECONDS: usize = 8;
// Combined retained sample budget, in addition to the primary PCM queue.
const ONE_SHOT_SAMPLES: usize = 192_000 * 2 * ONE_SHOT_SECONDS;
const ONE_SHOT_GAIN_MAX: f32 = 16.;

#[derive(Debug, Clone, Serialize)]
pub struct AudioEvidence {
    pub backend: &'static str,
    pub device: String,
    pub output_rate: u32,
    pub output_channels: u16,
    pub source_rate: u32,
    pub source_channels: u16,
    pub queued_frames: usize,
    pub source_frames_consumed: u64,
    pub output_frames_submitted: u64,
    pub underrun_frames: u64,
    pub callback_frames: u64,
    pub lock_misses: u64,
    pub device_errors: u64,
    pub paused: bool,
    pub oneshot_started: u64,
    pub oneshot_completed: u64,
    pub oneshot_active: usize,
    pub oneshot_output_frames: u64,
    pub mixed_clipped_frames: u64,
    pub oneshot_retained_samples: usize,
    pub oneshot_voice_limit: usize,
    pub oneshot_sample_limit: usize,
    pub primary_gain: f32,
    pub sound_gain: f32,
    pub scope: &'static str,
}

#[derive(Default)]
struct Buffer {
    samples: VecDeque<i16>,
    rate: u32,
    channels: u16,
    maximum_samples: usize,
    phase: u64,
    consumed: u64,
    submitted: u64,
    underrun: u64,
}
impl Buffer {
    fn configured(rate: u32, channels: u16) -> Result<Self> {
        ensure!(
            (8_000..=192_000).contains(&rate),
            "frontend PCM rate outside 8..192 kHz"
        );
        ensure!(
            matches!(channels, 1 | 2),
            "frontend PCM must be mono or stereo"
        );
        let maximum_samples = (rate as usize)
            .checked_mul(usize::from(channels))
            .and_then(|n| n.checked_mul(QUEUE_SECONDS))
            .context("frontend PCM capacity overflow")?;
        let mut samples = VecDeque::new();
        samples
            .try_reserve_exact(maximum_samples)
            .context("allocating bounded frontend PCM queue")?;
        Ok(Self {
            samples,
            rate,
            channels,
            maximum_samples,
            ..Self::default()
        })
    }
    fn queued_frames(&self) -> usize {
        self.samples
            .len()
            .checked_div(usize::from(self.channels))
            .unwrap_or(0)
    }
    fn queue(&mut self, samples: &[i16]) -> Result<()> {
        ensure!(
            self.channels != 0,
            "reset frontend PCM format before queueing"
        );
        ensure!(
            samples.len().is_multiple_of(usize::from(self.channels)),
            "partial frontend PCM frame"
        );
        ensure!(
            samples.len() <= self.maximum_samples.saturating_sub(self.samples.len()),
            "frontend PCM queue capacity exceeded"
        );
        self.samples.extend(samples.iter().copied());
        Ok(())
    }
    fn clear(&mut self) {
        self.samples.clear();
        self.phase = 0;
        self.consumed = 0;
        self.submitted = 0;
        self.underrun = 0;
    }
    /// Called only with a negotiated nonzero output rate. Empty input emits no
    /// PCM and leaves source time unchanged; silence must not skip future audio.
    #[cfg(any(windows, test))]
    fn next_frame(&mut self, output_rate: u32) -> Option<[f32; 2]> {
        let channels = usize::from(self.channels);
        if output_rate == 0 || channels == 0 || self.samples.len() < channels {
            self.underrun = self.underrun.saturating_add(1);
            return None;
        }
        let fraction = self.phase as f32 / output_rate as f32;
        let mut frame = [0.; 2];
        for (channel, value) in frame.iter_mut().take(channels).enumerate() {
            let a = f32::from(self.samples.get(channel).copied().unwrap_or(0)) / 32768.;
            let b = f32::from(
                self.samples
                    .get(channels + channel)
                    .copied()
                    .unwrap_or_else(|| self.samples.get(channel).copied().unwrap_or(0)),
            ) / 32768.;
            *value = (b - a).mul_add(fraction, a);
        }
        if channels == 1 {
            frame[1] = frame[0];
        }
        self.phase += u64::from(self.rate);
        let advance = self.phase / u64::from(output_rate);
        self.phase %= u64::from(output_rate);
        let consumed = (advance as usize).min(self.queued_frames());
        for _ in 0..consumed * channels {
            self.samples.pop_front();
        }
        self.consumed = self.consumed.saturating_add(consumed as u64);
        self.submitted = self.submitted.saturating_add(1);
        Some(frame)
    }
}

/// One independent host-clock voice. A completed voice retains its Arc until a
/// control-thread call retires it: the callback never frees a final PCM owner.
#[cfg_attr(
    not(any(windows, test)),
    expect(
        dead_code,
        reason = "Playback cursors are consumed by the Windows output callback outside tests."
    )
)]
struct OneShot {
    _id: u64,
    samples: Arc<[i16]>,
    rate: u32,
    channels: u16,
    gain: f32,
    frame: usize,
    phase: u64,
    finished: bool,
}
impl OneShot {
    #[cfg(any(windows, test))]
    fn next_frame(&mut self, output_rate: u32) -> Option<[f32; 2]> {
        if self.finished || output_rate == 0 {
            return None;
        }
        let channels = usize::from(self.channels);
        let frames = self.samples.len() / channels;
        let fraction = self.phase as f32 / output_rate as f32;
        let mut output = [0.; 2];
        // Validated whole nonempty PCM frames and a live frame cursor guarantee
        // these indices. Use checked reads so even a damaged cursor cannot panic.
        for (channel, value) in output.iter_mut().take(channels).enumerate() {
            let a = f32::from(
                self.samples
                    .get(self.frame * channels + channel)
                    .copied()
                    .unwrap_or(0),
            ) / 32768.;
            let next = self.frame.saturating_add(1).min(frames.saturating_sub(1));
            let b = f32::from(
                self.samples
                    .get(next * channels + channel)
                    .copied()
                    .unwrap_or(0),
            ) / 32768.;
            *value = (b - a).mul_add(fraction, a) * self.gain;
        }
        if channels == 1 {
            output[1] = output[0];
        }
        self.phase += u64::from(self.rate);
        let advance = self.phase / u64::from(output_rate);
        self.phase %= u64::from(output_rate);
        self.frame = self.frame.saturating_add(advance as usize).min(frames);
        self.finished = self.frame == frames;
        Some(output)
    }
}

struct Mixer {
    primary: Buffer,
    voices: [Option<OneShot>; ONE_SHOT_VOICES],
    next_id: u64,
    started: u64,
    completed: u64,
    voice_output_frames: u64,
    clipped_frames: u64,
    primary_gain: f32,
    sound_gain: f32,
}
impl Default for Mixer {
    fn default() -> Self {
        Self {
            primary: Buffer::default(),
            voices: std::array::from_fn(|_| None),
            next_id: 1,
            started: 0,
            completed: 0,
            voice_output_frames: 0,
            clipped_frames: 0,
            primary_gain: 1.,
            sound_gain: 1.,
        }
    }
}
impl Mixer {
    /// Control-thread only. No callback path removes a voice or drops its Arc.
    fn retire_completed(&mut self) {
        for slot in &mut self.voices {
            if slot.as_ref().is_some_and(|voice| voice.finished) {
                *slot = None;
            }
        }
    }
    fn active(&self) -> usize {
        self.voices.iter().flatten().filter(|v| !v.finished).count()
    }
    fn retained_samples(&self) -> usize {
        self.voices.iter().flatten().map(|v| v.samples.len()).sum()
    }
    fn play_one_shot(
        &mut self,
        rate: u32,
        channels: u16,
        samples: Arc<[i16]>,
        gain: f32,
    ) -> Result<u64> {
        ensure!(
            (8_000..=192_000).contains(&rate),
            "one-shot PCM rate outside 8..192 kHz"
        );
        ensure!(
            matches!(channels, 1 | 2),
            "one-shot PCM must be mono or stereo"
        );
        ensure!(
            !samples.is_empty() && samples.len().is_multiple_of(usize::from(channels)),
            "one-shot PCM needs nonempty whole frames"
        );
        ensure!(
            samples.len() / usize::from(channels) <= rate as usize * ONE_SHOT_SECONDS,
            "one-shot PCM duration exceeds eight-second host budget"
        );
        ensure!(
            gain.is_finite() && (0. ..=ONE_SHOT_GAIN_MAX).contains(&gain),
            "one-shot gain outside finite 0..16 host bounds"
        );
        let slot = self
            .voices
            .iter()
            .position(|v| v.as_ref().is_none_or(|v| v.finished))
            .context("one-shot voice capacity exceeded")?;
        let active_samples: usize = self
            .voices
            .iter()
            .flatten()
            .filter(|v| !v.finished)
            .map(|v| v.samples.len())
            .sum();
        ensure!(
            samples.len() <= ONE_SHOT_SAMPLES.saturating_sub(active_samples),
            "one-shot PCM sample budget exceeded"
        );
        let id = self.next_id;
        let next_id = id.checked_add(1).context("one-shot voice ID exhausted")?;
        // Every rejection above leaves primary audio, voices and diagnostics
        // unchanged. Retirement and final-Arc destruction happen here, off callback.
        self.retire_completed();
        self.voices[slot] = Some(OneShot {
            _id: id,
            samples,
            rate,
            channels,
            gain,
            frame: 0,
            phase: 0,
            finished: false,
        });
        self.next_id = next_id;
        self.started = self.started.saturating_add(1);
        Ok(id)
    }
    fn clear(&mut self) {
        self.primary.clear();
        for slot in &mut self.voices {
            *slot = None;
        }
        self.started = 0;
        self.completed = 0;
        self.voice_output_frames = 0;
        self.clipped_frames = 0;
    }
    /// Sum independently resampled source frames before the final device-channel
    /// mapping/limiter. Primary queue timing and counters do not depend on SFX.
    #[cfg(any(windows, test))]
    fn next_frame(&mut self, output_rate: u32) -> [f32; 2] {
        let mut sum = self.primary.next_frame(output_rate).unwrap_or([0.; 2]);
        sum[0] *= self.primary_gain;
        sum[1] *= self.primary_gain;
        for voice in self.voices.iter_mut().flatten() {
            if let Some(frame) = voice.next_frame(output_rate) {
                sum[0] += frame[0] * self.sound_gain;
                sum[1] += frame[1] * self.sound_gain;
                self.voice_output_frames = self.voice_output_frames.saturating_add(1);
                if voice.finished {
                    self.completed = self.completed.saturating_add(1);
                }
            }
        }
        sum
    }
}

struct Shared {
    buffer: Mutex<Mixer>,
    paused: AtomicBool,
    callback_frames: AtomicU64,
    lock_misses: AtomicU64,
    device_errors: AtomicU64,
}
impl Shared {
    #[cfg(any(windows, test))]
    fn new() -> Self {
        Self {
            buffer: Mutex::new(Mixer::default()),
            paused: AtomicBool::new(true),
            callback_frames: AtomicU64::new(0),
            lock_misses: AtomicU64::new(0),
            device_errors: AtomicU64::new(0),
        }
    }
}

pub struct FrontendAudio {
    #[cfg(windows)]
    stream: Option<cpal::Stream>,
    shared: Arc<Shared>,
    output_rate: u32,
    output_channels: u16,
    device: String,
}
impl FrontendAudio {
    /// Open the default WASAPI device paused. No device/unsupported format is an
    /// error for the caller to expose; this never fabricates substitute audio.
    #[cfg(windows)]
    pub fn open() -> Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::host_from_id(cpal::HostId::Wasapi).context("opening WASAPI host")?;
        let device = host
            .default_output_device()
            .context("no default audio output device")?;
        let description = device
            .description()
            .context("audio output description")?
            .name()
            .to_owned();
        let supported = device
            .default_output_config()
            .context("default audio output configuration")?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        ensure!(
            (8_000..=192_000).contains(&config.sample_rate) && (1..=8).contains(&config.channels),
            "audio output configuration outside host bounds"
        );
        let shared = Arc::new(Shared::new());
        let stream = match format {
            cpal::SampleFormat::F32 => build::<f32>(&device, config, shared.clone()),
            cpal::SampleFormat::F64 => build::<f64>(&device, config, shared.clone()),
            cpal::SampleFormat::I16 => build::<i16>(&device, config, shared.clone()),
            cpal::SampleFormat::U16 => build::<u16>(&device, config, shared.clone()),
            cpal::SampleFormat::I32 => build::<i32>(&device, config, shared.clone()),
            other => anyhow::bail!("unsupported default audio sample format {other:?}"),
        }?;
        Ok(Self {
            stream: Some(stream),
            shared,
            output_rate: config.sample_rate,
            output_channels: config.channels,
            device: description,
        })
    }
    #[cfg(not(windows))]
    pub fn open() -> Result<Self> {
        anyhow::bail!("native frontend audio currently requires Windows WASAPI")
    }

    /// Primary format transition: pause first and reset only the primary queue,
    /// resampler and clip counters. Independent one-shots survive late music
    /// priming. Use clear() when a transport/clip reset must discard all audio.
    pub fn reset(&mut self, rate: u32, channels: u16) -> Result<()> {
        let buffer = Buffer::configured(rate, channels)?;
        self.set_paused(true)?;
        let mut mixer = self
            .shared
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("frontend audio buffer poisoned"))?;
        mixer.retire_completed();
        mixer.primary = buffer;
        Ok(())
    }
    pub fn queue(&mut self, pcm16: &[i16]) -> Result<()> {
        let mut mixer = self
            .shared
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("frontend audio buffer poisoned"))?;
        mixer.retire_completed();
        mixer.primary.queue(pcm16)
    }
    /// Live host bus gains affect queued music and already playing cues without
    /// replacing PCM or restarting either source clock. Muted buses still run.
    pub fn set_gains(&mut self, primary: f32, sound: f32) -> Result<()> {
        ensure!(
            primary.is_finite()
                && sound.is_finite()
                && (0. ..=1.).contains(&primary)
                && (0. ..=1.).contains(&sound),
            "frontend bus gains must be finite and within 0..1"
        );
        let mut mixer = self
            .shared
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("frontend audio buffer poisoned"))?;
        mixer.primary_gain = primary;
        mixer.sound_gain = sound;
        Ok(())
    }
    /// Start bounded nonlooping PCM independently of the movie/music queue.
    /// Rate/channel conversion, gain summation and final limiting are host
    /// presentation adaptations, not Wii mixer numerical or scheduling parity.
    /// Completion follows output-device frames; pausing freezes all audio clocks.
    pub fn play_one_shot(
        &mut self,
        rate: u32,
        channels: u16,
        samples: Arc<[i16]>,
        gain: f32,
    ) -> Result<u64> {
        ensure!(
            self.shared.device_errors.load(Ordering::Acquire) == 0,
            "frontend audio device has failed"
        );
        self.shared
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("frontend audio buffer poisoned"))?
            .play_one_shot(rate, channels, samples, gain)
    }
    /// Focus/transport pause. The atomic gate silences callbacks even if the
    /// backend rejects pause; that backend failure is still returned to caller.
    pub fn set_paused(&mut self, paused: bool) -> Result<()> {
        ensure!(
            paused || self.shared.device_errors.load(Ordering::Acquire) == 0,
            "frontend audio device has failed"
        );
        if self.shared.paused.load(Ordering::Acquire) == paused {
            return Ok(());
        }
        self.shared.paused.store(true, Ordering::Release);
        #[cfg(windows)]
        {
            use cpal::traits::StreamTrait;
            let stream = self
                .stream
                .as_ref()
                .context("frontend audio stream closed")?;
            if paused {
                stream.pause().context("pausing frontend audio")?;
            } else {
                stream.play().context("starting frontend audio")?;
                self.shared.paused.store(false, Ordering::Release);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = paused;
            anyhow::bail!("native frontend audio currently requires Windows WASAPI");
        }
        #[cfg(windows)]
        Ok(())
    }
    /// Explicit transport reset: pause and discard primary PCM and all SFX,
    /// resetting clip submission/mixing counters while retaining unique voice IDs.
    pub fn clear(&mut self) -> Result<()> {
        self.set_paused(true)?;
        self.shared
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("frontend audio buffer poisoned"))?
            .clear();
        Ok(())
    }
    pub fn evidence(&self) -> Result<AudioEvidence> {
        let mut mixer = self
            .shared
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("frontend audio buffer poisoned"))?;
        mixer.retire_completed();
        let buffer = &mixer.primary;
        Ok(AudioEvidence {
            backend: "WASAPI",
            device: self.device.clone(),
            output_rate: self.output_rate,
            output_channels: self.output_channels,
            source_rate: buffer.rate,
            source_channels: buffer.channels,
            queued_frames: buffer.queued_frames(),
            source_frames_consumed: buffer.consumed,
            output_frames_submitted: buffer.submitted,
            underrun_frames: buffer.underrun,
            callback_frames: self.shared.callback_frames.load(Ordering::Relaxed),
            lock_misses: self.shared.lock_misses.load(Ordering::Relaxed),
            device_errors: self.shared.device_errors.load(Ordering::Relaxed),
            paused: self.shared.paused.load(Ordering::Acquire),
            oneshot_started: mixer.started,
            oneshot_completed: mixer.completed,
            oneshot_active: mixer.active(),
            oneshot_output_frames: mixer.voice_output_frames,
            mixed_clipped_frames: mixer.clipped_frames,
            oneshot_retained_samples: mixer.retained_samples(),
            oneshot_voice_limit: ONE_SHOT_VOICES,
            oneshot_sample_limit: ONE_SHOT_SAMPLES,
            primary_gain: mixer.primary_gain,
            sound_gain: mixer.sound_gain,
            scope: "Decoded PCM host rate/channel conversion and independent nonlooping SFX summation with final [-1,1] limiting. One-shot output frames sum all voices; clipped frames count device frames with any limited channel. Callback/source consumption is not measured speaker playback or original Wii mixing/timing parity.",
        })
    }
}
impl Drop for FrontendAudio {
    fn drop(&mut self) {
        self.shared.paused.store(true, Ordering::Release);
        #[cfg(windows)]
        {
            use cpal::traits::StreamTrait;
            if let Some(stream) = self.stream.take() {
                let _ = stream.pause();
                drop(stream);
            }
        }
    }
}

#[cfg(any(windows, test))]
fn fill_output<T: Copy>(
    output: &mut [T],
    channels: usize,
    rate: u32,
    shared: &Shared,
    convert: impl Fn(f32) -> T,
) {
    output.fill(convert(0.));
    if channels == 0 || rate == 0 {
        return;
    }
    shared
        .callback_frames
        .fetch_add((output.len() / channels) as u64, Ordering::Relaxed);
    if shared.paused.load(Ordering::Acquire) {
        return;
    }
    // Never block, allocate, log, unwrap a lock or panic in our callback.
    let Ok(mut buffer) = shared.buffer.try_lock() else {
        shared.lock_misses.fetch_add(1, Ordering::Relaxed);
        return;
    };
    for frame in output.chunks_exact_mut(channels) {
        if shared.paused.load(Ordering::Acquire) {
            break;
        }
        let pcm = buffer.next_frame(rate);
        let mut clipped = false;
        for (channel, sample) in frame.iter_mut().enumerate() {
            let value = if channels == 1 {
                (pcm[0] + pcm[1]) * 0.5
            } else {
                pcm.get(channel).copied().unwrap_or(0.)
            };
            clipped |= !(-1. ..=1.).contains(&value);
            *sample = convert(value.clamp(-1., 1.));
        }
        if clipped {
            buffer.clipped_frames = buffer.clipped_frames.saturating_add(1);
        }
    }
}

#[cfg(windows)]
fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    shared: Arc<Shared>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;
    let errors = shared.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                fill_output(
                    output,
                    usize::from(config.channels),
                    config.sample_rate,
                    &shared,
                    T::from_sample,
                );
            },
            move |_| {
                errors.device_errors.fetch_add(1, Ordering::Relaxed);
                errors.paused.store(true, Ordering::Release);
            },
            None,
        )
        .context("building native audio output stream")
}

#[cfg(test)]
#[path = "tests/frontend_audio.rs"]
mod tests;
