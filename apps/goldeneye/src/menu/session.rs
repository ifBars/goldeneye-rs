//! Menu input application, bounded developer probes and debounced preferences.
use super::preferences::Preferences;
use crate::frontend_playback::Playback;
use anyhow::Result;
use ge_core::frontend::{Effect, Input, Screen, Settings, State};
use serde::Serialize;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, clap::ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeInput {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Reset,
}
impl From<ProbeInput> for Input {
    fn from(value: ProbeInput) -> Self {
        match value {
            ProbeInput::Up => Self::Up,
            ProbeInput::Down => Self::Down,
            ProbeInput::Left => Self::Left,
            ProbeInput::Right => Self::Right,
            ProbeInput::Confirm => Self::Confirm,
            ProbeInput::Back => Self::Back,
            ProbeInput::Reset => Self::Reset,
        }
    }
}
#[derive(Serialize)]
pub struct ProbeStep {
    frame: u32,
    input: ProbeInput,
    effect: Effect,
    screen: Screen,
    selected: usize,
    settings: Settings,
}
#[derive(Serialize)]
pub struct Evidence {
    pub writes: u64,
    pub dirty: bool,
    pub probe: Vec<ProbeStep>,
    pub scope: &'static str,
}
pub struct Session {
    pub state: State,
    preferences: Preferences,
    changed_at: Option<Instant>,
    writes: u64,
    inputs: Vec<ProbeInput>,
    probe: Vec<ProbeStep>,
    screen_started: Instant,
}
impl Session {
    pub fn open(path: Option<PathBuf>, inputs: Vec<ProbeInput>) -> Result<Self> {
        let (preferences, settings) = Preferences::open(path)?;
        Ok(Self {
            state: State::with_settings(false, settings),
            preferences,
            changed_at: None,
            writes: 0,
            inputs,
            probe: Vec::new(),
            screen_started: Instant::now(),
        })
    }
    pub fn elapsed(&self) -> f64 {
        self.screen_started.elapsed().as_secs_f64()
    }
    pub fn input(&mut self, input: Input, playback: &mut Playback) -> Result<Effect> {
        let before = *self.state.settings();
        let screen = self.state.screen();
        let effect = self.state.input(input);
        if self.state.screen() != screen {
            self.screen_started = Instant::now();
        }
        if before != *self.state.settings() {
            self.changed_at = Some(Instant::now());
            playback.set_settings(*self.state.settings())?;
        }
        playback.menu_effect(effect)?;
        Ok(effect)
    }
    pub fn select(&mut self, index: usize, playback: &mut Playback) -> Result<bool> {
        let previous = self.state.selected_index();
        if !self.state.select_index(index) {
            return Ok(false);
        }
        if previous != self.state.selected_index() {
            playback.menu_effect(Effect::Navigate)?;
        }
        Ok(true)
    }
    pub fn tick(&mut self, frames: u32) -> Result<Option<ProbeInput>> {
        if self
            .changed_at
            .is_some_and(|at| at.elapsed() >= Duration::from_millis(350))
        {
            self.flush()?;
        }
        Ok(
            (self.probe.len() < self.inputs.len() && frames >= (self.probe.len() as u32 + 1) * 6)
                .then(|| self.inputs[self.probe.len()]),
        )
    }
    pub fn record_probe(&mut self, frame: u32, input: ProbeInput, effect: Effect) {
        self.probe.push(ProbeStep {
            frame,
            input,
            effect,
            screen: self.state.screen(),
            selected: self.state.selected_index(),
            settings: *self.state.settings(),
        });
    }
    pub fn flush(&mut self) -> Result<()> {
        if self.changed_at.is_some() {
            self.writes += u64::from(self.preferences.save(*self.state.settings())?);
            self.changed_at = None;
        }
        Ok(())
    }
    pub fn evidence(self) -> Evidence {
        Evidence {
            writes: self.writes,
            dirty: self.changed_at.is_some(),
            probe: self.probe,
            scope: "Source-backed menu rows and slider values with host input and versioned Windows preferences. Developer inputs use the same handler; no OS input automation or original runtime comparison. Wii saves/profiles and unavailable leaves remain unimplemented.",
        }
    }
}
