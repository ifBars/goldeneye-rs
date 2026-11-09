//! Pure timing adapter for the frontend controller's inclusive intro/outro ranges.
//! Native UI markers provide the ranges. This preclamps static render time; it
//! does not reproduce native overshoot/rewind callbacks or a general script VM.
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Segment {
    pub start: f32,
    pub end: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Intro,
    Hold,
    Outro,
    Complete,
}

#[derive(Debug, Clone)]
pub struct Timeline {
    fps: f32,
    intro: Segment,
    outro: Segment,
    intro_seconds: f32,
    outro_seconds: f32,
    seconds: f32,
    outro_started_at: f32,
    frame: f32,
    phase: Phase,
}

impl Timeline {
    /// Segment endpoints are inclusive frames; the gap between ranges is not
    /// played automatically. Reaching the endpoint clamps the frame, but only
    /// strictly later time changes phase. A single-frame segment is allowed.
    /// Execution is
    /// constant work per call, with no frame catch-up loop or event dispatch.
    pub fn new(fps: f32, intro: Segment, outro: Segment) -> Result<Self> {
        ensure!(
            fps.is_finite() && fps > 0.,
            "frontend segment fps must be finite and positive"
        );
        let intro_seconds = duration(intro, fps)?;
        let outro_seconds = duration(outro, fps)?;
        ensure!(
            intro.end < outro.start,
            "frontend segments must be ordered and nonoverlapping"
        );
        Ok(Self {
            fps,
            intro,
            outro,
            intro_seconds,
            outro_seconds,
            seconds: 0.,
            outro_started_at: 0.,
            frame: intro.start,
            phase: Phase::Intro,
        })
    }

    /// Absolute monotonic elapsed seconds since this timeline was created.
    /// Time spent holding never becomes a head start on the outro. Invalid
    /// inputs leave the entire state unchanged, including the elapsed clock.
    pub fn advance_to(&mut self, seconds: f32) -> Result<()> {
        ensure!(seconds.is_finite(), "frontend segment time must be finite");
        ensure!(
            seconds >= self.seconds,
            "frontend segment time cannot rewind"
        );
        match self.phase {
            Phase::Intro => {
                if seconds > self.intro_seconds {
                    self.frame = self.intro.end;
                    self.phase = Phase::Hold;
                } else {
                    self.frame = (self.intro.start + seconds * self.fps).min(self.intro.end);
                }
            }
            Phase::Hold => {}
            Phase::Outro => {
                let elapsed = seconds - self.outro_started_at;
                if elapsed > self.outro_seconds {
                    self.frame = self.outro.end;
                    self.phase = Phase::Complete;
                } else {
                    self.frame = (self.outro.start + elapsed * self.fps).min(self.outro.end);
                }
            }
            Phase::Complete => {}
        }
        self.seconds = seconds;
        Ok(())
    }

    /// Begin the outro only after time has passed the intro's inclusive endpoint.
    /// Confirmation does not consume a frame or advance the wall clock.
    pub fn confirm(&mut self) -> bool {
        if self.phase != Phase::Hold {
            return false;
        }
        self.frame = self.outro.start;
        self.outro_started_at = self.seconds;
        self.phase = Phase::Outro;
        true
    }

    pub fn frame(&self) -> f32 {
        self.frame
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
}

fn duration(segment: Segment, fps: f32) -> Result<f32> {
    ensure!(
        segment.start.is_finite() && segment.end.is_finite(),
        "frontend segment endpoints must be finite"
    );
    ensure!(
        segment.start <= segment.end,
        "frontend segment endpoints are reversed"
    );
    let span = segment.end - segment.start;
    ensure!(span.is_finite(), "frontend segment span overflow");
    let seconds = span / fps;
    ensure!(
        seconds.is_finite() && (span == 0. || seconds > 0.),
        "frontend segment duration overflow or underflow"
    );
    Ok(seconds)
}

#[cfg(test)]
#[path = "tests/frontend_segments.rs"]
mod tests;
