//! Continuation of the same retained phase-zero child, before parent effects.
//! Metadata caching precedes body/timing reads. No pose sampling is implied.
mod resource;
mod state;
use super::*;
pub(super) use state::visit_binding;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub directory_index: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body {
    Absent,
    Resident,
    Pending,
}
pub trait Source {
    fn file(&self) -> u32;
    /// None means the required retained file owner is unavailable.
    fn generation(&mut self, file: u32) -> Result<Option<u32>>;
    fn resolve(&mut self, file: u32, reference: u32) -> Result<Option<Binding>>;
    fn body(&mut self, file: u32, binding: Binding) -> Result<Body>;
    fn timing_flags(&mut self, command_offset: u32) -> Result<u32>;
    fn animation_fps(&mut self, file: u32, binding: Binding) -> Result<u8>;
    fn script_fps(&mut self) -> Result<u32>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub index: u16,
    pub file: Option<u32>,
    pub generation: Option<u32>,
    pub binding: Option<Binding>,
    pub body_bound: Option<bool>,
    pub timing_flags: Option<u32>,
    pub animation_fps: Option<u8>,
    pub script_fps_bits: Option<u32>,
    pub frame_bits: Option<u32>,
    pub position_bits: Option<u32>,
    pub previous_position_bits: Option<u32>,
    pub root_motion_pending: bool,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct Selection {
    pub offset: u32,
    pub frame_bits: u32,
    pub start: i16,
}
impl Trace {
    pub(crate) fn new(index: u16) -> Self {
        Self {
            index,
            file: None,
            generation: None,
            binding: None,
            body_bound: None,
            timing_flags: None,
            animation_fps: None,
            script_fps_bits: None,
            frame_bits: None,
            position_bits: None,
            previous_position_bits: None,
            root_motion_pending: false,
        }
    }
}
impl Store {
    pub fn prepare_phase_zero_position(
        &mut self,
        id: NodeId,
        source: &mut impl Source,
    ) -> Result<()> {
        let h = self
            .headers
            .get_mut(&id)
            .context("script position owner absent")?;
        ensure!(
            h.stage == Stage::AwaitingAnimation && h.position.is_none(),
            "script position continuation differs or replays"
        );
        h.stage = Stage::PreparingPosition;
        let mut t = Trace::new(0);
        let result = h.prepare_selected_position(source, &mut t, None);
        h.position = Some(t);
        if result.is_err() {
            h.stage = Stage::Failed;
        }
        result
    }
}
impl Header {
    pub fn position(&self) -> Option<&Trace> {
        self.position.as_ref()
    }
    pub(in crate::animation::script_child) fn prepare_selected_position(
        &mut self,
        source: &mut impl Source,
        t: &mut Trace,
        selection: Option<Selection>,
    ) -> Result<()> {
        ensure!(
            source.file() == self.file,
            "script position file owner differs"
        );
        t.index = u16::from_be_bytes([
            self.byte(0x6c).context("clip index unknown")?,
            self.byte(0x6d).context("clip index unknown")?,
        ]);
        if !self.resolve_position_animation(source, t)? {
            return Ok(());
        }
        let selection = match selection {
            Some(selection) => selection,
            None => {
                let init = self
                    .initialization
                    .as_ref()
                    .context("script selection absent")?;
                Selection {
                    offset: init.selected_offset.context("selected play absent")?,
                    frame_bits: init.frame_bits.context("script frame absent")?,
                    start: init.selected_start.context("play start absent")?,
                }
            }
        };
        let mut frame = f32::from_bits(selection.frame_bits) - f32::from(selection.start);
        let frames = u16::from_be_bytes([
            self.byte(0x70).context("clip frame count unknown")?,
            self.byte(0x71).context("clip frame count unknown")?,
        ]);
        let flags = source.timing_flags(selection.offset)?;
        t.timing_flags = Some(flags);
        if flags & 3 != 0 {
            self.stage = Stage::AwaitingTimingCurve;
            return Ok(());
        }
        if t.body_bound == Some(true) {
            let binding = t.binding.context("resident animation metadata absent")?;
            let file = t.file.context("animation file identity absent")?;
            let fps = source.animation_fps(file, binding)?;
            t.animation_fps = Some(fps);
            let script = source.script_fps()?;
            t.script_fps_bits = Some(script);
            let script = f32::from_bits(script);
            if script != f32::from(fps) {
                frame = (frame / script) * f32::from(fps);
            }
        }
        t.frame_bits = Some(frame.to_bits());
        let old = self.word(0x28).context("old clip position unknown")?;
        self.w(0x68, old);
        t.previous_position_bits = Some(old);
        let child_flags = self.word(0x20).context("child flags unknown")?;
        let span = f32::from(frames);
        let bounded = if child_flags & 1 != 0 {
            if span == 0.0 {
                0.0
            } else {
                (f64::from(frame) % f64::from(span)) as f32
            }
        } else if frame >= span {
            span
        } else if frame <= 0.0 {
            0.0
        } else {
            frame
        };
        self.w(0x28, bounded.to_bits());
        let position = bounded / span;
        self.w(0x28, position.to_bits());
        t.position_bits = Some(position.to_bits());
        ensure!(
            position.is_finite(),
            "unsupported nonfinite normalized script position; reached writes retained"
        );
        if child_flags & 2 != 0 && position != 0.0 {
            t.root_motion_pending = true;
            self.stage = Stage::AwaitingRootMotion;
        } else {
            // initflag=1 skips previous-phase synchronization and events.
            self.stage = Stage::ReturnedInitialized;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
