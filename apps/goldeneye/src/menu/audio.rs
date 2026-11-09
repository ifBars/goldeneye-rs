//! Cached, source-selected menu effects. The four finite resident waves share
//! the existing independent voice transport and never interrupt title music.
use crate::{
    frontend_audio::FrontendAudio,
    frontend_cue::{Cue, CueSpec, Evidence},
};
use anyhow::{Context, Result, ensure};
use ge_core::frontend::Effect;
use serde::Serialize;
use std::{path::Path, sync::Arc};

const SCOPE: &str = "Four source menu effects from generic bank 2d500001: navigation 2d70044a, accepted action 2d70044d, back 2d700449, changed value 2d70044e. Predecoded local PCM, host-clock independent voices and source request gain1.0; native bus mixing, Wii controls and reference audibility are unverified.";
const SPECS: [CueSpec; 4] = [
    CueSpec {
        sound: 0x2d70044a,
        wave: 0x2da01128,
        rate: 32_000,
        channels: 2,
        frames: 4_480,
        flags: 0x8b,
        scope: SCOPE,
    },
    CueSpec {
        sound: 0x2d70044d,
        wave: 0x2da01125,
        rate: 32_000,
        channels: 2,
        frames: 23_184,
        flags: 0x2d3,
        scope: SCOPE,
    },
    CueSpec {
        sound: 0x2d700449,
        wave: 0x2da01127,
        rate: 32_000,
        channels: 2,
        frames: 9_016,
        flags: 0x118,
        scope: SCOPE,
    },
    CueSpec {
        sound: 0x2d70044e,
        wave: 0x2da01126,
        rate: 32_000,
        channels: 2,
        frames: 2_688,
        flags: 0x52,
        scope: SCOPE,
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct MenuAudioEvidence {
    pub cues: Vec<Evidence>,
    pub voices_started: u64,
    pub capacity_dropped: u64,
    pub source_request_gain: f32,
    pub scope: &'static str,
}

pub struct MenuAudio {
    cues: [Cue; 4],
    voices_started: u64,
    capacity_dropped: u64,
}
impl MenuAudio {
    pub fn load(disc: &Path) -> Result<Self> {
        let cues: [Cue; 4] = Cue::load_many(disc, &SPECS)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("menu cue batch length differs"))?;
        Ok(Self {
            cues,
            voices_started: 0,
            capacity_dropped: 0,
        })
    }

    pub fn play(&mut self, effect: Effect, audio: &mut FrontendAudio) -> Result<Option<u64>> {
        if matches!(effect, Effect::None) {
            return Ok(None);
        }
        let transport = audio.evidence()?;
        ensure!(
            transport.device_errors == 0,
            "frontend audio device has failed"
        );
        let available = transport
            .oneshot_voice_limit
            .saturating_sub(transport.oneshot_active);
        self.submit(effect, available, |rate, channels, pcm, gain| {
            audio.play_one_shot(rate, channels, pcm, gain)
        })
    }

    fn submit(
        &mut self,
        effect: Effect,
        available: usize,
        queue: impl FnOnce(u32, u16, Arc<[i16]>, f32) -> Result<u64>,
    ) -> Result<Option<u64>> {
        let index = match effect {
            Effect::None => return Ok(None),
            Effect::Navigate => 0,
            Effect::Accept => 1,
            Effect::Back => 2,
            Effect::Adjust => 3,
        };
        // Canonical cues fit the transport sample budget even at eight voices.
        // Preserve existing tails when a burst fills all slots. The callback
        // only completes voices, so availability cannot shrink between the
        // control-thread observation and this control-thread submission.
        if available == 0 {
            self.capacity_dropped = self.capacity_dropped.saturating_add(1);
            return Ok(None);
        }
        let cue = self.cues.get(index).context("menu cue absent")?;
        let id = queue(
            cue.info().sample_rate,
            cue.info().channels,
            cue.samples(),
            1.0,
        )?;
        self.voices_started = self.voices_started.saturating_add(1);
        Ok(Some(id))
    }

    pub fn evidence(&self) -> MenuAudioEvidence {
        MenuAudioEvidence {
            cues: self.cues.iter().map(Cue::evidence).collect(),
            voices_started: self.voices_started,
            capacity_dropped: self.capacity_dropped,
            source_request_gain: 1.0,
            scope: SCOPE,
        }
    }
}

#[cfg(test)]
#[path = "tests/audio.rs"]
mod tests;
