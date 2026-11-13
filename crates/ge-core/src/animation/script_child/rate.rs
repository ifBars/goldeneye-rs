//! Clip-rate query over retained script storage. Selection and metadata-cache
//! writes stay separate from phase initialization and pose evaluation.
use super::{Header, position};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    /// Whether the actual current-file owner exposes its required data body.
    /// Unavailable providers must stop the caller before supplying this value.
    pub current_file_data_available: bool,
    pub baseline: u32,
    pub epsilon_bits: u32,
    pub lower_bits: u32,
    pub upper_bits: u32,
    pub snap_bits: u32,
    pub index_floor: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub inputs: Inputs,
    pub started: bool,
    pub coordinate_bits: Option<u32>,
    pub normal_count: Option<u8>,
    /// Thresholds visited by the ordered scan, excluding its endpoint rereads.
    pub thresholds_read: usize,
    pub selected_index: Option<i32>,
    pub fraction_bits: Option<u32>,
    pub first: Option<position::Trace>,
    pub second: Option<position::Trace>,
    pub first_fps: Option<u8>,
    pub second_fps: Option<u8>,
    pub unscaled_rate_bits: Option<u32>,
    /// Some only after a real return. Missing animations return the source's
    /// default one without baseline scaling; pending providers leave None.
    pub rate_bits: Option<u32>,
}

impl Trace {
    pub fn new(inputs: Inputs) -> Self {
        Self {
            inputs,
            started: false,
            coordinate_bits: None,
            normal_count: None,
            thresholds_read: 0,
            selected_index: None,
            fraction_bits: None,
            first: None,
            second: None,
            first_fps: None,
            second_fps: None,
            unscaled_rate_bits: None,
            rate_bits: None,
        }
    }

    pub(crate) fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(u32::from(self.inputs.current_file_data_available));
        for word in [
            self.inputs.baseline,
            self.inputs.epsilon_bits,
            self.inputs.lower_bits,
            self.inputs.upper_bits,
            self.inputs.snap_bits,
            self.inputs.index_floor as u32,
        ] {
            emit(word);
        }
        emit(u32::from(self.started));
        for value in [self.coordinate_bits, self.normal_count.map(u32::from)] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        emit(((self.thresholds_read as u64) >> 32) as u32);
        emit(self.thresholds_read as u32);
        for value in [
            self.selected_index.map(|index| index as u32),
            self.fraction_bits,
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        for lookup in [&self.first, &self.second] {
            emit(u32::from(lookup.is_some()));
            if let Some(lookup) = lookup {
                lookup.visit_state_words(&mut emit);
            }
        }
        for value in [
            self.first_fps.map(u32::from),
            self.second_fps.map(u32::from),
            self.unscaled_rate_bits,
            self.rate_bits,
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
    }
}

pub(super) fn evaluate(
    header: &mut Header,
    source: &mut impl position::Source,
    trace: &mut Trace,
) -> Result<()> {
    ensure!(
        !trace.started,
        "clip-rate query cannot replay a reached prefix"
    );
    trace.started = true;
    ensure!(
        source.file() == header.file(),
        "clip-rate file owner differs"
    );
    if !trace.inputs.current_file_data_available {
        trace.rate_bits = Some(1f32.to_bits());
        return Ok(());
    }
    select(header, trace)?;
    let index = trace.selected_index.context("clip-rate selection absent")?;
    trace.first = Some(lookup_trace(index)?);
    let first = trace.first.as_mut().expect("retained first clip lookup");
    if !header.resolve_position_animation(source, first)? {
        return Ok(());
    }
    if first.body_bound != Some(true) {
        trace.rate_bits = Some(1f32.to_bits());
        return Ok(());
    }
    let fps = source.animation_fps(
        first.file.context("rate animation file absent")?,
        first.binding.context("rate animation metadata absent")?,
    )?;
    first.animation_fps = Some(fps);
    trace.first_fps = Some(fps);
    let mut rate = f32::from(fps);
    trace.unscaled_rate_bits = Some(rate.to_bits());
    let fraction = f32::from_bits(trace.fraction_bits.context("clip-rate blend absent")?);
    if fraction != 0.0 {
        let next = index
            .checked_add(1)
            .context("clip-rate next index overflow")?;
        trace.second = Some(lookup_trace(next)?);
        let second = trace.second.as_mut().expect("retained second clip lookup");
        if !header.resolve_position_animation(source, second)? {
            return Ok(());
        }
        if second.body_bound == Some(true) {
            let fps = source.animation_fps(
                second.file.context("blended rate animation file absent")?,
                second
                    .binding
                    .context("blended rate animation metadata absent")?,
            )?;
            second.animation_fps = Some(fps);
            trace.second_fps = Some(fps);
            let first_term = rate * (1.0f32 - fraction);
            let second_term = fraction * f32::from(fps);
            rate = first_term + second_term;
            trace.unscaled_rate_bits = Some(rate.to_bits());
        }
    }
    let reciprocal = 1.0f32 / (trace.inputs.baseline as f32);
    trace.rate_bits = Some((rate * reciprocal).to_bits());
    Ok(())
}

fn lookup_trace(index: i32) -> Result<position::Trace> {
    Ok(position::Trace {
        index: u16::try_from(index).context("unsupported clip-rate index")?,
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
    })
}

fn select(header: &Header, trace: &mut Trace) -> Result<()> {
    let coordinate = header.word(0x64).context("clip-rate coordinate unknown")?;
    trace.coordinate_bits = Some(coordinate);
    let coordinate = f32::from_bits(coordinate);
    let count = header.byte(0x74).context("normal clip count unknown")?;
    trace.normal_count = Some(count);
    let mut previous = 0i32;
    if count > 1 {
        for index in 0..usize::from(count) {
            let threshold = f32::from_bits(
                header
                    .clip_word(index, 0x14)
                    .context("clip-rate threshold unknown")?,
            );
            trace.thresholds_read += 1;
            // Native BLE skips the interval when GT is clear, including an
            // unordered comparison. Do not replace this with the inverse <=.
            if threshold > coordinate {
                let previous = usize::try_from(previous)?;
                let low = f32::from_bits(
                    header
                        .clip_word(previous, 0x14)
                        .context("lower clip threshold unknown")?,
                );
                let high = f32::from_bits(
                    header
                        .clip_word(previous + 1, 0x14)
                        .context("upper clip threshold unknown")?,
                );
                let span = high - low;
                let epsilon = f32::from_bits(trace.inputs.epsilon_bits);
                let denominator = if epsilon >= span { epsilon } else { span };
                let raw = (coordinate - low) / denominator;
                let upper = f32::from_bits(trace.inputs.upper_bits);
                let lower = f32::from_bits(trace.inputs.lower_bits);
                let mut fraction = if raw >= upper {
                    upper
                } else if raw <= lower {
                    lower
                } else {
                    raw
                };
                let mut selected = previous as i32;
                if fraction >= f32::from_bits(trace.inputs.snap_bits) {
                    selected += 1;
                    fraction = 0.0;
                }
                if header.word(0x20).context("clip-rate flags unknown")? & 0x80 != 0 {
                    fraction = 0.0;
                }
                let last = i32::from(count) - 1;
                selected = if selected >= last {
                    last
                } else {
                    selected.max(trace.inputs.index_floor)
                };
                trace.selected_index = Some(selected);
                trace.fraction_bits = Some(fraction.to_bits());
                return Ok(());
            }
            previous = index as i32;
        }
    }
    trace.selected_index = Some(previous);
    trace.fraction_bits = Some(0f32.to_bits());
    Ok(())
}

#[cfg(test)]
mod tests;
