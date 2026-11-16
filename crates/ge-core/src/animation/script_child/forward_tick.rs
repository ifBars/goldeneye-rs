//! Class-four virtual +18 at zero time. This is distinct from initflag1 re-entry.
use super::*;

pub trait Source: Provider {
    fn duration(&mut self, binding: Binding) -> Result<u32>;
    fn fps_bits(&mut self, binding: Binding) -> Result<u32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Entering,
    Resolving,
    AwaitingBody,
    Duration,
    Fps,
    Returned,
    Failed,
}

#[derive(Debug)]
pub struct Trace {
    stage: Stage,
    phase_cleared: bool,
    body_bound: Option<bool>,
    duration_raw: Option<u32>,
    fps_bits: Option<u32>,
    result_bits: Option<u32>,
}
impl Default for Trace {
    fn default() -> Self {
        Self {
            stage: Stage::Entering,
            phase_cleared: false,
            body_bound: None,
            duration_raw: None,
            fps_bits: None,
            result_bits: None,
        }
    }
}
impl Trace {
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn phase_cleared(&self) -> bool {
        self.phase_cleared
    }
    pub fn body_bound(&self) -> Option<bool> {
        self.body_bound
    }
    pub fn duration_raw(&self) -> Option<u32> {
        self.duration_raw
    }
    pub fn fps_bits(&self) -> Option<u32> {
        self.fps_bits
    }
    pub fn result_bits(&self) -> Option<u32> {
        self.result_bits
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.stage as u32);
        emit(u32::from(self.phase_cleared));
        emit(match self.body_bound {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
        for value in [self.duration_raw, self.fps_bits, self.result_bits] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
    }
}
impl Store {
    pub fn forward_zero(
        &mut self,
        id: NodeId,
        source: &mut impl Source,
        trace: &mut Trace,
    ) -> Result<()> {
        let header = self
            .headers
            .get_mut(&id)
            .context("forward tick script owner absent")?;
        ensure!(
            header.initializer_returned() && trace.stage == Stage::Entering,
            "forward tick precedes initializer return or replays"
        );
        header.store_phase(0);
        header.w(0x84, 0);
        trace.phase_cleared = true;
        trace.stage = Stage::Resolving;
        // Resource caching effects survive; the factory's returned stage remains
        // historical rather than being replaced with an initializer invocation.
        let old_stage = header.stage;
        let resolved = header.resolve(source);
        let pending_body = header.stage == super::Stage::AwaitingScriptBody;
        header.stage = old_stage;
        if let Err(error) = resolved {
            trace.stage = Stage::Failed;
            return Err(error);
        }
        trace.body_bound = header.body_bound;
        if pending_body {
            trace.stage = Stage::AwaitingBody;
            return Ok(());
        }
        if header.body_bound != Some(true) {
            trace.stage = Stage::Returned;
            return Ok(());
        }
        let result = (|| {
            let binding = header.cached.context("forward tick body binding absent")?;
            trace.stage = Stage::Duration;
            let duration = source.duration(binding)?;
            trace.duration_raw = Some(duration);
            trace.stage = Stage::Fps;
            let fps = source.fps_bits(binding)?;
            trace.fps_bits = Some(fps);
            // Source unsigned integer-to-double conversion followed by fsubs,
            // then separately rounded fmuls and fdivs. Zero/nonfinite inputs
            // are preserved, not silently changed into a skipped body read.
            let numerator = 0.0f32 * f32::from_bits(fps);
            let value = numerator / duration as f32;
            let bits = value.to_bits();
            header.store_phase(bits);
            header.w(0x84, bits);
            trace.result_bits = Some(bits);
            trace.stage = Stage::Returned;
            Ok(())
        })();
        if result.is_err() {
            trace.stage = Stage::Failed;
        }
        result
    }
}

#[cfg(test)]
#[path = "forward_tick/tests.rs"]
mod tests;
