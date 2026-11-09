//! Startup presentation timeline, independent of clocks, assets and input devices.
//!
//! Warning limits and logo order follow the supplied Wii build. Media timing is
//! supplied from checked asset headers. The legal hold is an explicit host choice,
//! not a native asynchronous loading lifetime; title confirmation to the menu
//! is a host adaptation, not the complete original profile/save flow.
use crate::frontend_fade::Fade;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::time::Duration;

pub const WARNING_MINIMUM_SECONDS: f64 = 1.5;
pub const WARNING_TIMEOUT_SECONDS: f64 = 21.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Strap,
    Zapper,
    Activision,
    Eurocom,
    Mgm,
    Legal,
    Title,
    MainMenu,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MediaTiming {
    pub frame_count: u32,
    pub fps: f32,
}

impl MediaTiming {
    pub fn duration_seconds(self) -> f64 {
        f64::from(self.frame_count) / f64::from(self.fps)
    }

    fn validate(self) -> Result<()> {
        ensure!(self.frame_count > 0, "startup media has no frames");
        ensure!(
            self.fps.is_finite() && self.fps > 0.,
            "startup media frame rate must be finite and positive"
        );
        ensure!(
            self.duration_seconds().is_finite(),
            "startup media duration is not finite"
        );
        Ok(())
    }

    fn frame(self, seconds: f64, looping: bool) -> u32 {
        let seconds = if looping {
            seconds % self.duration_seconds()
        } else {
            seconds
        };
        ((seconds * f64::from(self.fps)).floor() as u32).min(self.frame_count - 1)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Timing {
    pub activision: MediaTiming,
    pub eurocom: MediaTiming,
    pub mgm: MediaTiming,
    pub background: MediaTiming,
    /// Explicit presentation setting; this is not a verified native script timer.
    pub legal_seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sequence {
    timing: Timing,
    stage: Stage,
    elapsed_seconds: f64,
    background_seconds: f64,
    transition_count: u64,
    fade: Fade,
    warning_fading: bool,
    /// Readiness of the host's implemented dependency subset, not the complete
    /// native work queue or frontend-menu lifecycle gate.
    host_dependencies_ready: bool,
}

impl Sequence {
    pub fn new(timing: Timing) -> Result<Self> {
        for media in [
            timing.activision,
            timing.eurocom,
            timing.mgm,
            timing.background,
        ] {
            media.validate()?;
        }
        ensure!(
            timing.legal_seconds.is_finite() && timing.legal_seconds > 0.,
            "legal presentation duration must be finite and positive"
        );
        Ok(Self {
            timing,
            stage: Stage::Strap,
            elapsed_seconds: 0.,
            background_seconds: 0.,
            transition_count: 0,
            fade: Fade::default(),
            warning_fading: false,
            host_dependencies_ready: true,
        })
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn set_host_dependencies_ready(&mut self, ready: bool) {
        self.host_dependencies_ready = ready;
    }

    pub fn fade(&self) -> &Fade {
        &self.fade
    }

    pub fn overlay_rgba(&self) -> [u8; 4] {
        self.fade.color()
    }

    pub fn warning_fading(&self) -> bool {
        self.warning_fading
    }

    /// Selected frame of the current logo or looping frontend background.
    /// Static warning/legal stages have no movie frame.
    pub fn video_frame(&self) -> Option<u32> {
        match self.stage {
            Stage::Activision => Some(self.timing.activision.frame(self.elapsed_seconds, false)),
            Stage::Eurocom => Some(self.timing.eurocom.frame(self.elapsed_seconds, false)),
            Stage::Mgm => Some(self.timing.mgm.frame(self.elapsed_seconds, false)),
            Stage::Title | Stage::MainMenu => {
                Some(self.timing.background.frame(self.background_seconds, true))
            }
            Stage::Strap | Stage::Zapper | Stage::Legal => None,
        }
    }

    pub fn advance(&mut self, elapsed: Duration) -> Result<()> {
        self.advance_seconds(elapsed.as_secs_f64())
    }

    /// Advance by one host update of active playback time. Already-bound movie
    /// stages can carry a remainder, but warning fades and legal entry stop that
    /// synthetic catch-up: first-update skips are never replaced with invented
    /// fixed ticks. Original asynchronous resource scheduling is not emulated.
    /// Invalid time or fade arithmetic is rejected before any state changes.
    pub fn advance_seconds(&mut self, elapsed: f64) -> Result<()> {
        ensure!(
            elapsed.is_finite() && elapsed >= 0.,
            "startup elapsed time must be finite and nonnegative"
        );
        ensure!(
            (self.elapsed_seconds + elapsed).is_finite()
                && (self.background_seconds + elapsed).is_finite(),
            "startup elapsed time overflow"
        );
        let mut next = self.clone();
        next.advance_validated(elapsed)?;
        *self = next;
        Ok(())
    }

    fn advance_validated(&mut self, elapsed: f64) -> Result<()> {
        if matches!(self.stage, Stage::Strap | Stage::Zapper) {
            // The native owner timer and incoming global delta are singles.
            // Retain the widened result only for the shared host-facing getter;
            // repeated updates must round after every addition, before testing
            // the warning's 1.5/21-second gates.
            let delta = component_delta(elapsed)?;
            let warning_elapsed = self.elapsed_seconds as f32 + delta;
            ensure!(
                warning_elapsed.is_finite(),
                "startup warning elapsed overflow"
            );
            self.elapsed_seconds = f64::from(warning_elapsed);
            if self.warning_fading {
                if self.fade.update(delta)? {
                    let next = match self.stage {
                        Stage::Strap => Stage::Zapper,
                        Stage::Zapper => Stage::Activision,
                        _ => unreachable!(),
                    };
                    self.transition(next);
                }
            } else if self.elapsed_seconds >= WARNING_TIMEOUT_SECONDS {
                self.start_warning_fade();
            }
            return Ok(());
        }
        if self.stage == Stage::Legal {
            self.elapsed_seconds += elapsed;
            if self.fade.is_active() {
                self.fade.update(component_delta(elapsed)?)?;
            }
            // This host hold includes the entry fade, but cannot cut it short.
            if !self.fade.is_active()
                && self.host_dependencies_ready
                && self.elapsed_seconds >= self.timing.legal_seconds
            {
                self.transition(Stage::Title);
            }
            return Ok(());
        }
        let mut remaining = elapsed;
        loop {
            let duration = match self.stage {
                Stage::Strap | Stage::Zapper | Stage::Legal => unreachable!(),
                Stage::Activision => self.timing.activision.duration_seconds(),
                Stage::Eurocom => self.timing.eurocom.duration_seconds(),
                Stage::Mgm => self.timing.mgm.duration_seconds(),
                Stage::Title | Stage::MainMenu => {
                    self.elapsed_seconds += remaining;
                    self.background_seconds += remaining;
                    break;
                }
            };
            let left = (duration - self.elapsed_seconds).max(0.);
            if remaining < left {
                self.elapsed_seconds += remaining;
                break;
            }
            remaining -= left;
            let next = match self.stage {
                Stage::Activision => Stage::Eurocom,
                Stage::Eurocom => Stage::Mgm,
                Stage::Mgm => Stage::Legal,
                Stage::Strap | Stage::Zapper | Stage::Legal | Stage::Title | Stage::MainMenu => {
                    unreachable!()
                }
            };
            self.transition(next);
            if next == Stage::Legal {
                break;
            }
        }
        Ok(())
    }

    /// Host confirmation starts a warning's fade only after its native minimum,
    /// retaining the warning until completion. A repeated confirmation cannot
    /// restart or shorten the fade. A logo accepts confirmation only after native
    /// frame 90 (strictly greater). Mapping host confirm
    /// to the native accepted button mask is an adaptation. Legal confirmation
    /// is ignored; title-to-menu is also an explicit host adaptation.
    pub fn confirm(&mut self) -> bool {
        if matches!(self.stage, Stage::Strap | Stage::Zapper) {
            if self.warning_fading || self.elapsed_seconds < WARNING_MINIMUM_SECONDS {
                return false;
            }
            self.start_warning_fade();
            return true;
        }
        let next = match self.stage {
            Stage::Activision if self.video_frame().is_some_and(|frame| frame > 90) => {
                Stage::Eurocom
            }
            Stage::Eurocom if self.video_frame().is_some_and(|frame| frame > 90) => Stage::Mgm,
            Stage::Mgm if self.video_frame().is_some_and(|frame| frame > 90) => Stage::Legal,
            Stage::Title => Stage::MainMenu,
            _ => return false,
        };
        self.transition(next);
        true
    }

    /// Explicit host option for developer smoke checks; not a native skip rule.
    pub fn skip_to_menu(&mut self) -> bool {
        if self.stage == Stage::MainMenu {
            return false;
        }
        self.background_seconds = 0.;
        self.transition(Stage::MainMenu);
        true
    }

    /// Developer stage entry, not elapsed boot simulation or a native skip rule.
    /// Resets diagnostics rather than claiming the skipped stages were played.
    /// Legal entry still starts its real global fade; other stages start clear.
    pub fn seek_stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.elapsed_seconds = 0.;
        self.background_seconds = 0.;
        self.transition_count = 0;
        self.warning_fading = false;
        self.fade = entry_fade(stage);
    }

    fn start_warning_fade(&mut self) {
        self.warning_fading = true;
        self.fade = Fade::black_out();
    }

    fn transition(&mut self, stage: Stage) {
        self.stage = stage;
        self.elapsed_seconds = 0.;
        self.warning_fading = false;
        self.fade = entry_fade(stage);
        // At most seven automatic transitions and one explicit menu transition.
        self.transition_count += 1;
    }
}

fn entry_fade(stage: Stage) -> Fade {
    if stage == Stage::Legal {
        Fade::black_in()
    } else {
        Fade::default()
    }
}

fn component_delta(seconds: f64) -> Result<f32> {
    let seconds = seconds as f32;
    ensure!(
        seconds.is_finite(),
        "startup component delta exceeds single precision"
    );
    Ok(seconds)
}

#[cfg(test)]
#[path = "tests/frontend_sequence.rs"]
mod tests;
