//! Auto-clearing frontend RGBA fades, independent of presentation and clocks.
//!
//! This is the observed positive-duration path. Setup preserves the exact start
//! bytes; the first component update only clears its pending-update flag. Later
//! updates interpolate normalized lanes with separate single-precision operations
//! and truncate back to bytes. Completion clears the overlay instead of latching
//! its target color. Caller ordering relative to the original boot scheduler is
//! deliberately not inferred here.
use anyhow::{Result, ensure};
use serde::Serialize;

pub const BOOT_FADE_SECONDS: f32 = 0.5;

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct Fade {
    start: [f32; 4],
    target: [f32; 4],
    duration_seconds: f32,
    elapsed_seconds: f32,
    first_update_pending: bool,
    active: bool,
    rgba: [u8; 4],
}

impl Fade {
    /// Create the source auto-clear path for a finite, positive duration. Other
    /// duration coercions and non-clearing native modes remain unsupported.
    pub fn new(start: [u8; 4], target: [u8; 4], duration_seconds: f32) -> Result<Self> {
        ensure!(
            duration_seconds.is_finite() && duration_seconds > 0.,
            "frontend fade duration must be finite and positive"
        );
        Ok(Self {
            start: start.map(|lane| f32::from(lane) / 255.),
            target: target.map(|lane| f32::from(lane) / 255.),
            duration_seconds,
            elapsed_seconds: 0.,
            first_update_pending: true,
            active: true,
            rgba: start,
        })
    }

    /// Proven warning acceptance fade: transparent black to opaque black.
    pub const fn black_out() -> Self {
        Self {
            start: [0.; 4],
            target: [0., 0., 0., 1.],
            duration_seconds: BOOT_FADE_SECONDS,
            elapsed_seconds: 0.,
            first_update_pending: true,
            active: true,
            rgba: [0; 4],
        }
    }

    /// Proven legal-entry fade: opaque black to transparent black.
    pub const fn black_in() -> Self {
        Self {
            start: [0., 0., 0., 1.],
            target: [0.; 4],
            duration_seconds: BOOT_FADE_SECONDS,
            elapsed_seconds: 0.,
            first_update_pending: true,
            active: true,
            rgba: [0, 0, 0, 255],
        }
    }

    /// One component update. Returns true only on the update that completes an
    /// active fade. Invalid input or elapsed overflow leaves all state intact.
    /// A pending first update consumes no supplied time, even when it is large.
    pub fn update(&mut self, seconds: f32) -> Result<bool> {
        ensure!(
            seconds.is_finite() && seconds >= 0.,
            "frontend fade delta must be finite and nonnegative"
        );
        if !self.active {
            return Ok(false);
        }
        if self.first_update_pending {
            self.first_update_pending = false;
            return Ok(false);
        }
        let elapsed = self.elapsed_seconds + seconds;
        ensure!(elapsed.is_finite(), "frontend fade elapsed overflow");
        let ratio = (elapsed / self.duration_seconds).clamp(0., 1.);
        let rgba = std::array::from_fn(|lane| {
            let difference = self.target[lane] - self.start[lane];
            let contribution = ratio * difference;
            let normalized = self.start[lane] + contribution;
            (normalized * 255.).trunc() as u8
        });
        self.elapsed_seconds = elapsed;
        if elapsed >= self.duration_seconds {
            self.rgba = [0; 4];
            self.active = false;
            return Ok(true);
        }
        self.rgba = rgba;
        Ok(false)
    }

    pub fn color(&self) -> [u8; 4] {
        self.rgba
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn first_update_pending(&self) -> bool {
        self.first_update_pending
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    pub fn duration_seconds(&self) -> f32 {
        self.duration_seconds
    }

    /// Mirrors duration minus elapsed; overshooting completion can be negative.
    pub fn remaining_seconds(&self) -> f32 {
        self.duration_seconds - self.elapsed_seconds
    }
}

#[cfg(test)]
#[path = "tests/frontend_fade.rs"]
mod tests;
