use super::*;
impl Header {
    pub(super) fn select(&mut self, source: &mut impl Source, trace: &mut Trace) -> Result<()> {
        ensure!(
            source.file() == self.file && trace.inputs.maximum_entries > 0,
            "phase-zero resource owner or scan budget differs"
        );
        // This path is fresh: the inline reference is -1, count is one and the
        // wrapper is NULL. The rate getter therefore returns the source 1.0.
        ensure!(
            self.byte(0x74) == Some(1)
                && self.byte(0x76) == Some(0)
                && self.word(0x40) == Some(0)
                && self.word(0x44) == Some(u32::MAX)
                && self.word(0x50) == Some(u32::MAX)
                && self.word(0x64) == Some(0)
                && self.word(0x88) == Some(u32::MAX),
            "phase-zero entry is not a fresh script child"
        );
        let rate = f32::from_bits(self.word(0x2c).context("child speed unknown")?);
        let speed = f32::from_bits(trace.inputs.component_speed_bits);
        let step = f32::from_bits(trace.inputs.scaled_step_bits);
        ensure!(
            rate.is_finite() && speed.is_finite() && step.is_finite(),
            "phase-zero timing input is not finite"
        );
        let delta = ((1.0f32 * rate) * speed) * step;
        ensure!(delta.is_finite(), "phase-zero timing overflow");
        trace.delta_bits = Some(delta.to_bits());
        if delta != 0.0 {
            let sign = if delta > 0.0 { 1i8 } else { -1i8 };
            if self.byte(0x77).map(|byte| byte as i8) != Some(sign) {
                let previous = self.word(0x28).context("previous child position unknown")?;
                let phase = self.word(0x80).context("previous script phase unknown")?;
                self.bytes[0x77] = Some(sign as u8);
                self.w(0x68, previous);
                self.w(0x84, phase);
            }
        }
        let raw = source.duration()?;
        trace.duration_raw = Some(raw);
        let duration = (raw as i32) as f32;
        let phase = f32::from_bits(self.word(0x80).context("script phase unknown")?);
        let frame = phase.clamp(0.0, 1.0) * duration;
        trace.frame_bits = Some(frame.to_bits());
        // initflag=1 skips the event-mask and callback path entirely.
        let mut offset = 0u32;
        loop {
            ensure!(
                trace.entries_visited < trace.inputs.maximum_entries,
                "phase-zero command scan budget exceeded"
            );
            let entry = source.entry(offset)?;
            trace.entries_visited += 1;
            let stop = match entry.opcode {
                2 if f32::from(entry.start) <= frame => {
                    trace.selected_offset = Some(offset);
                    trace.selected_start = Some(entry.start);
                    false
                }
                2 | 0x12 => true,
                _ => false,
            };
            // The native scan reads next even on END/future-play, but does not
            // dereference the next command when it is stopping.
            if stop {
                break;
            }
            ensure!(entry.next > 0, "phase-zero command scan does not advance");
            offset = offset
                .checked_add(u32::from(entry.next))
                .context("phase-zero command offset overflow")?;
        }
        let Some(selected) = trace.selected_offset else {
            self.stage = Stage::ReturnedWithoutPlay;
            return Ok(());
        };
        // The caller compares the first normal reference before writing +88,
        // even when the clip-count fields have not been read or are zero.
        trace.compared_reference = Some(source.reference(selected, false, 0, 0)?);
        self.w(0x88, selected);
        self.setup_clips(source, trace, selected)
    }
}
