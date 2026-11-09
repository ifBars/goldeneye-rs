//! Presentation clock, decoded media and native audio binding for the frontend.
use crate::frontend_music::Music;
use crate::{
    frontend_audio::{AudioEvidence, FrontendAudio},
    frontend_media::{Catalog, Movie, Player},
};
use anyhow::{Context, Result, ensure};
use ge_assets::tpl::RgbaImage;
use ge_core::frontend_sequence::{MediaTiming, Sequence, Stage, Timing};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

#[derive(Serialize)]
pub struct Evidence {
    pub stage: Stage,
    pub movie: Option<Movie>,
    pub video_frame: Option<u32>,
    pub decoded_frames: u64,
    pub transitions: u64,
    pub clips_decoded: BTreeMap<Movie, u64>,
    pub audio: Option<AudioEvidence>,
    pub muted: bool,
    pub session_suspended: bool,
    pub music: Option<crate::frontend_music::Evidence>,
    pub fade: FadeEvidence,
    pub warning_fading: bool,
    pub startup_resources: crate::startup_resources::Evidence,
    pub intro_cue: Option<crate::frontend_cue::Evidence>,
    pub intro_cue_requests: u64,
    pub intro_cue_voices: u64,
    pub intro_cue_gain: Option<f32>,
    pub menu_audio: Option<crate::menu::audio::MenuAudioEvidence>,
    pub menu_effect_requests: BTreeMap<&'static str, u64>,
    pub scope: &'static str,
}
#[derive(Serialize)]
pub struct FadeEvidence {
    pub color: [u8; 4],
    pub active: bool,
    pub first_update_pending: bool,
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
    pub remaining_seconds: f32,
}
pub struct Playback {
    pub sequence: Sequence,
    player: Player,
    audio: Option<FrontendAudio>,
    audio_clip: Option<Movie>,
    shown: Option<Movie>,
    shown_frame: Option<u32>,
    decoded_frames: u64,
    clips_decoded: BTreeMap<Movie, u64>,
    paused: bool,
    session_suspended: bool,
    audio_totals: BTreeMap<Movie, u64>,
    durations: BTreeMap<Movie, f64>,
    music: Option<Music>,
    startup_resources: crate::startup_resources::StartupResources,
    intro_cue: Option<crate::frontend_cue::Cue>,
    intro_cue_requests: u64,
    intro_cue_voices: u64,
    intro_cue_gain: Option<f32>,
    menu_audio: Option<crate::menu::audio::MenuAudio>,
    menu_effect_requests: BTreeMap<&'static str, u64>,
    settings: ge_core::frontend::Settings,
}
impl Playback {
    pub fn new(
        disc: PathBuf,
        catalog: Catalog,
        skip_startup: bool,
        muted: bool,
        start_at: Option<crate::frontend::StartupStage>,
    ) -> Result<Self> {
        let timing = |movie| -> Result<MediaTiming> {
            let clip = catalog
                .clips
                .get(&movie)
                .context("frontend timing missing")?;
            Ok(MediaTiming {
                frame_count: clip.header.frame_count,
                fps: clip.header.fps,
            })
        };
        let timing = Timing {
            activision: timing(Movie::Activision)?,
            eurocom: timing(Movie::Eurocom)?,
            mgm: timing(Movie::Mgm)?,
            background: timing(Movie::Frontend)?,
            legal_seconds: 301.0 / 30.0,
        };
        let mut sequence = Sequence::new(timing)?;
        sequence.set_host_dependencies_ready(false);
        if skip_startup {
            sequence.skip_to_menu();
        }
        if let Some(stage) = start_at {
            use crate::frontend::StartupStage;
            let target = match stage {
                StartupStage::Strap | StartupStage::StrapFade => Stage::Strap,
                StartupStage::Zapper | StartupStage::ZapperFade => Stage::Zapper,
                StartupStage::Activision => Stage::Activision,
                StartupStage::Eurocom => Stage::Eurocom,
                StartupStage::Mgm => Stage::Mgm,
                StartupStage::Legal => Stage::Legal,
                StartupStage::Title => Stage::Title,
                StartupStage::MainMenu => Stage::MainMenu,
            };
            sequence.seek_stage(target);
            if matches!(stage, StartupStage::StrapFade | StartupStage::ZapperFade) {
                sequence.advance_seconds(ge_core::frontend_sequence::WARNING_MINIMUM_SECONDS)?;
                ensure!(sequence.confirm(), "bounded warning-fade entry rejected");
            }
        }
        let audio = if muted {
            None
        } else {
            Some(FrontendAudio::open()?)
        };
        let music = if muted {
            None
        } else {
            Some(Music::new(disc.clone())?)
        };
        let audio_totals = catalog
            .clips
            .iter()
            .filter_map(|(&movie, clip)| {
                clip.header
                    .audio
                    .as_ref()
                    .map(|a| (movie, u64::from(a.total_samples)))
            })
            .collect();
        let durations = catalog
            .clips
            .iter()
            .map(|(&movie, clip)| {
                (
                    movie,
                    f64::from(clip.header.frame_count) / f64::from(clip.header.fps),
                )
            })
            .collect();
        let startup_resources = crate::startup_resources::StartupResources::new(disc.clone());
        let intro_cue = if muted {
            None
        } else {
            Some(crate::frontend_cue::Cue::load(&disc)?)
        };
        let menu_audio = if muted {
            None
        } else {
            Some(crate::menu::audio::MenuAudio::load(&disc)?)
        };
        Ok(Self {
            sequence,
            player: Player::new(disc, catalog.clips)?,
            audio,
            audio_clip: None,
            shown: None,
            shown_frame: None,
            decoded_frames: 0,
            clips_decoded: BTreeMap::new(),
            paused: false,
            session_suspended: false,
            audio_totals,
            durations,
            music,
            startup_resources,
            intro_cue,
            intro_cue_requests: 0,
            intro_cue_voices: 0,
            intro_cue_gain: None,
            menu_audio,
            menu_effect_requests: BTreeMap::new(),
            settings: ge_core::frontend::Settings::default(),
        })
    }
    pub fn set_settings(&mut self, settings: ge_core::frontend::Settings) -> Result<()> {
        self.settings = settings;
        self.apply_gains()
    }
    fn apply_gains(&mut self) -> Result<()> {
        use ge_core::frontend::Setting;
        let music = matches!(self.stage(), Stage::Title | Stage::MainMenu);
        if let Some(audio) = &mut self.audio {
            audio.set_gains(
                if self.session_suspended {
                    0.
                } else if music {
                    self.settings.gain(Setting::MusicVolume)
                } else {
                    1.
                },
                self.settings.gain(Setting::SoundVolume),
            )?;
        }
        Ok(())
    }
    pub fn menu_effect(&mut self, effect: ge_core::frontend::Effect) -> Result<()> {
        use ge_core::frontend::Effect;
        let name = match effect {
            Effect::None => return Ok(()),
            Effect::Navigate => "navigate",
            Effect::Accept => "accept",
            Effect::Back => "back",
            Effect::Adjust => "adjust",
        };
        *self.menu_effect_requests.entry(name).or_default() += 1;
        if let (Some(menu), Some(audio)) = (&mut self.menu_audio, &mut self.audio) {
            menu.play(effect, audio)?;
            audio.set_paused(self.paused)?;
        }
        Ok(())
    }
    pub fn stage(&self) -> Stage {
        self.sequence.stage()
    }
    pub fn elapsed(&self) -> f64 {
        self.sequence.elapsed_seconds()
    }
    fn movie(&self) -> Option<Movie> {
        match self.stage() {
            Stage::Activision => Some(Movie::Activision),
            Stage::Eurocom => Some(Movie::Eurocom),
            Stage::Mgm => Some(Movie::Mgm),
            Stage::Title | Stage::MainMenu => Some(Movie::Frontend),
            _ => None,
        }
    }
    pub fn set_paused(&mut self, paused: bool) -> Result<()> {
        self.paused = paused;
        if let Some(audio) = &mut self.audio {
            let silent = self.audio_clip.is_none() && audio.evidence()?.oneshot_active == 0;
            audio.set_paused(paused || silent)?;
        }
        Ok(())
    }
    /// Host session transition: stop frontend advancement/primary music while
    /// allowing the accepted menu effect to finish. Focus pause remains separate.
    pub fn set_session_suspended(&mut self, suspended: bool) -> Result<()> {
        self.session_suspended = suspended;
        self.apply_gains()
    }
    pub fn confirm(&mut self) -> Result<bool> {
        let before = self.movie();
        let changed = self.sequence.confirm();
        if before != self.movie() {
            self.transition_media()?;
        }
        Ok(changed)
    }
    pub fn play_intro_cue(&mut self, request: &crate::frontend_title::CueRequest) -> Result<()> {
        ensure!(
            request.sound_hash == 0x2d7011ad,
            "unsupported title cue sound"
        );
        self.intro_cue_requests += 1;
        self.intro_cue_gain = Some(request.gain);
        if let (Some(cue), Some(audio)) = (&self.intro_cue, &mut self.audio) {
            audio.play_one_shot(
                cue.info().sample_rate,
                cue.info().channels,
                cue.samples(),
                request.gain,
            )?;
            audio.set_paused(self.paused)?;
            self.intro_cue_voices += 1;
        }
        Ok(())
    }
    fn transition_media(&mut self) -> Result<()> {
        self.shown = None;
        self.shown_frame = None;
        self.audio_clip = None;
        if let Some(audio) = &mut self.audio {
            audio.clear()?;
            audio.set_paused(true)?;
        }
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.movie().is_none() || self.shown == self.movie()
    }
    pub fn tick(&mut self, elapsed: Duration) -> Result<Option<RgbaImage>> {
        self.apply_gains()?;
        if self.session_suspended {
            if let Some(audio) = &self.audio {
                ensure!(
                    audio.evidence()?.device_errors == 0,
                    "native frontend audio device failed"
                );
            }
            return Ok(None);
        }
        self.startup_resources
            .tick(self.stage(), self.sequence.fade().is_active())?;
        self.sequence
            .set_host_dependencies_ready(self.startup_resources.actions_ready());
        let before = self.movie();
        let mut image = None;
        for mut decoded in self.player.poll()? {
            if Some(decoded.movie) != self.movie() {
                continue;
            }
            if let Some(pcm) = decoded.audio.take()
                && let Some(audio) = &mut self.audio
            {
                if self.audio_clip != Some(decoded.movie) {
                    audio.reset(pcm.sample_rate, pcm.channels.try_into()?)?;
                    self.audio_clip = Some(decoded.movie);
                }
                audio.queue(&pcm.samples)?;
                audio.set_paused(self.paused)?;
            }
            self.shown = Some(decoded.movie);
            self.shown_frame = Some(decoded.frame);
            self.decoded_frames += 1;
            *self.clips_decoded.entry(decoded.movie).or_default() += 1;
            image = Some(decoded.image);
        }
        if before == Some(Movie::Frontend)
            && let (Some(music), Some(audio)) = (&mut self.music, &mut self.audio)
        {
            if let Some(pcm) = music.poll()? {
                if self.audio_clip != Some(Movie::Frontend) {
                    audio.reset(pcm.sample_rate, pcm.channels)?;
                    self.audio_clip = Some(Movie::Frontend);
                }
                audio.queue(&pcm.samples)?;
                audio.set_paused(self.paused)?;
            }
            let state = audio.evidence()?;
            ensure!(
                state.device_errors == 0,
                "native frontend audio device failed"
            );
            if needs_music_chunk(
                self.paused,
                self.audio_clip == Some(Movie::Frontend),
                state.source_rate,
                state.queued_frames,
            ) {
                music.request()?;
            }
        }
        if !self.paused && self.ready() {
            if let (Some(movie), Some(audio)) = (
                self.audio_clip.filter(|m| *m != Movie::Frontend),
                self.audio.as_ref(),
            ) {
                let clock = audio.evidence()?;
                ensure!(
                    clock.device_errors == 0,
                    "native frontend audio device failed"
                );
                let total = *self
                    .audio_totals
                    .get(&movie)
                    .context("logo audio clock metadata absent")?;
                let duration = *self
                    .durations
                    .get(&movie)
                    .context("logo video clock metadata absent")?;
                let step = sample_clock_delta(
                    self.elapsed(),
                    clock.source_frames_consumed,
                    clock.source_rate,
                    total,
                    duration,
                );
                self.sequence.advance_seconds(step)?;
            } else {
                self.sequence.advance(elapsed)?;
            }
        }
        if before != self.movie() {
            self.transition_media()?;
            image = None;
        }
        if let (Some(movie), Some(frame)) = (self.movie(), self.sequence.video_frame()) {
            self.player.request(movie, frame)?;
        }
        Ok(image)
    }
    pub fn evidence(&self) -> Result<Evidence> {
        Ok(Evidence {
            stage: self.stage(),
            movie: self.shown,
            video_frame: self.shown_frame,
            decoded_frames: self.decoded_frames,
            transitions: self.sequence.transition_count(),
            clips_decoded: self.clips_decoded.clone(),
            audio: self.audio.as_ref().map(|a| a.evidence()).transpose()?,
            muted: self.audio.is_none(),
            session_suspended: self.session_suspended,
            music: self.music.as_ref().map(|m| m.evidence()),
            fade: FadeEvidence {
                color: self.sequence.overlay_rgba(),
                active: self.sequence.fade().is_active(),
                first_update_pending: self.sequence.fade().first_update_pending(),
                elapsed_seconds: self.sequence.fade().elapsed_seconds(),
                duration_seconds: self.sequence.fade().duration_seconds(),
                remaining_seconds: self.sequence.fade().remaining_seconds(),
            },
            warning_fading: self.sequence.warning_fading(),
            startup_resources: self.startup_resources.evidence(),
            intro_cue: self.intro_cue.as_ref().map(|cue| cue.evidence()),
            intro_cue_requests: self.intro_cue_requests,
            intro_cue_voices: self.intro_cue_voices,
            intro_cue_gain: self.intro_cue_gain,
            menu_audio: self.menu_audio.as_ref().map(|a| a.evidence()),
            menu_effect_requests: self.menu_effect_requests.clone(),
            scope: "Native media timeline and PCM submission diagnostics. Audio clock is host output submission, not measured speaker latency; exact original transitions, fades and reference synchronization remain unverified.",
        })
    }
}

fn needs_music_chunk(paused: bool, initialized: bool, rate: u32, queued: usize) -> bool {
    !paused && (!initialized || rate == 0 || queued < rate as usize / 2)
}

fn sample_clock_delta(elapsed: f64, consumed: u64, rate: u32, total: u64, duration: f64) -> f64 {
    // Linear interpolation can retain the final sample as its right endpoint.
    // Close the clip when only that endpoint remains, using header duration.
    let target = if consumed.saturating_add(1) >= total {
        duration + 1e-9
    } else {
        consumed as f64 / f64::from(rate)
    };
    (target - elapsed).max(0.)
}

#[cfg(test)]
#[path = "tests/frontend_playback.rs"]
mod tests;
