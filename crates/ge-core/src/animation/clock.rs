//! Numeric prerequisites of the source ordinary animation update. These helpers
//! do not schedule updates, process script events, rebind clips, or sample bones.
//! The position helper supports the observed initial global bounds; callers must
//! not treat them as restored arbitrary native history. Nonfinite reached math
//! is an explicit host limit, not a native validation rule.

use anyhow::{Result, ensure};

/// Source PlayerNew pending-refresh input at 8012a068, not a frame duration.
pub const fn source_prewarm_seconds() -> u32 {
    0x3e4c_cccd
}

/// 80104810 installs seconds * the unsigned engine baseline into the clock.
pub fn scaled_actor_step(seconds_bits: u32, baseline: u32) -> Result<u32> {
    let seconds = finite(seconds_bits, "actor seconds")?;
    output(seconds * baseline as f32, "scaled actor step")
}

/// 80306cbc..cc0: transition progress uses the absolute scaled component step.
pub fn transition_delta(component_speed_bits: u32, scaled_step_bits: u32) -> Result<u32> {
    let speed = finite(component_speed_bits, "component speed")?;
    let step = finite(scaled_step_bits, "scaled actor step")?;
    output((speed * step).abs(), "transition delta")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionState {
    /// Pointee of child+0x30.
    pub scalar_bits: u32,
    /// Pointee of child+0x38; separate from the animation header word+0x38.
    pub reciprocal_bits: u32,
}

/// 80320f30: multiplication and addition are separately rounded. The upper
/// guard compares the computed value; the lower guard compares the OLD scalar.
/// Preserve that asymmetry, including a negative computed result from old zero.
pub fn advance_transition(
    state: TransitionState,
    absolute_delta_bits: u32,
) -> Result<TransitionState> {
    let reciprocal = finite(state.reciprocal_bits, "transition reciprocal")?;
    if reciprocal == 0.0 {
        return Ok(state); // Source does not read scalar/delta on this path.
    }
    let delta = finite(absolute_delta_bits, "absolute transition delta")?;
    ensure!(delta >= 0.0, "transition delta must be absolute");
    let scalar = finite(state.scalar_bits, "transition scalar")?;
    let increment = delta * reciprocal;
    let next = scalar + increment;
    ensure!(next.is_finite(), "unsupported transition arithmetic");
    let (next, reciprocal) = if next > 1.0 {
        (1.0, 0.0)
    } else if scalar < 0.0 {
        (0.0, 0.0)
    } else {
        (next, reciprocal)
    };
    Ok(TransitionState {
        scalar_bits: next.to_bits(),
        reciprocal_bits: reciprocal.to_bits(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionInputs {
    /// OLD class04 child+0x80, before ordinary advancement.
    pub phase_bits: u32,
    pub script_duration_raw: u32,
    pub script_fps_bits: u32,
    /// Start of the already selected opcode02, not the animation clip range.
    pub play_start_frame: i16,
    /// Child+0x70, bound from the first animation header+0xe.
    pub first_animation_frames: u16,
    pub animation_fps: u8,
    pub child_flags_raw: u32,
    /// Reached script+0x40; nonzero event-mask branches are not implemented.
    pub script_flags_raw: u32,
    /// Reached timing bundle flags; curve remapping is not implemented here.
    pub timing_flags_raw: u32,
    /// Reached animation BODY+0x38; not a descriptor word or translation mask.
    pub animation_word38_raw: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedPosition {
    pub normalized_position_bits: u32,
    /// Ordinary initflag0 synchronizes child+0x84 to bounded OLD phase.
    pub previous_phase_bits: u32,
}

/// Numeric part of 803256c0 with a supplied, already selected play block.
/// Supports event-free scripts/no timing curves/no reached root-motion callback.
/// This is not the complete virtual28, and does not validate opcode selection.
pub fn prepare_position(inputs: PositionInputs) -> Result<PreparedPosition> {
    ensure!(
        inputs.script_flags_raw & 0x0001_1cf8 == 0,
        "unsupported reached script events"
    );
    ensure!(inputs.timing_flags_raw == 0, "unsupported timing curves");
    ensure!(
        inputs.child_flags_raw & 2 == 0 || inputs.animation_word38_raw & 4 == 0,
        "unsupported reached animation root-motion callback"
    );
    ensure!(
        inputs.first_animation_frames != 0,
        "unsupported zero-frame position divisor"
    );
    let phase = finite(inputs.phase_bits, "current script phase")?;
    let phase = source_unit_bounds(phase);
    let script_frame = phase * (inputs.script_duration_raw as i32) as f32;
    let mut frame = script_frame - f32::from(inputs.play_start_frame);
    let script_fps = finite(inputs.script_fps_bits, "script FPS")?;
    let animation_fps = f32::from(inputs.animation_fps);
    if script_fps != animation_fps {
        frame /= script_fps;
        frame *= animation_fps;
    }
    ensure!(frame.is_finite(), "unsupported position arithmetic");
    let frames = f32::from(inputs.first_animation_frames);
    let frame = if inputs.child_flags_raw & 1 != 0 {
        // Native double fmod followed by frsp, preserving negative remainder.
        (f64::from(frame) % f64::from(frames)) as f32
    } else if frame >= frames {
        frames
    } else if frame <= 0.0 {
        0.0
    } else {
        frame
    };
    Ok(PreparedPosition {
        normalized_position_bits: output(frame / frames, "normalized position")?,
        previous_phase_bits: phase.to_bits(),
    })
}

/// 803222e0..308 for a selected clip FPS. A multi-clip caller must supply the
/// source helper's selected FPS, not invent a blend using another header.
pub fn clip_rate(animation_fps: u8, child_speed_bits: u32, baseline: u32) -> Result<u32> {
    ensure!(baseline != 0, "unsupported zero clock baseline");
    let speed = finite(child_speed_bits, "child speed")?;
    let ratio = f32::from(animation_fps) / baseline as f32;
    output(speed * ratio, "clip rate")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseInputs {
    pub phase_bits: u32,
    /// Script+4: unsigned comparison, signed conversion for the division.
    pub script_duration_raw: u32,
    pub first_animation_frames: u16,
    /// Actual 80321f50 output span, after ordered selection/fallback/blending.
    pub selected_span_bits: u32,
    /// Actual 80321f50 output rate, including FPS/baseline and child speed.
    pub selected_rate_bits: u32,
    pub child_flags_raw: u32,
    pub component_speed_bits: u32,
    pub scaled_step_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseAdvance {
    pub phase_bits: u32,
    pub child_flags_raw: u32,
    /// Source requests 803256c0 again. The host must execute or reject it.
    pub reprocess_script: bool,
}

/// 803269d0 accumulation only, after the old-phase position has been prepared.
/// Loop endpoints wrap ONCE; this deliberately is not a general modulo clock.
pub fn advance_phase(inputs: PhaseInputs) -> Result<PhaseAdvance> {
    let mut result = PhaseAdvance {
        phase_bits: inputs.phase_bits,
        child_flags_raw: inputs.child_flags_raw,
        reprocess_script: false,
    };
    if inputs.child_flags_raw & 2 == 0 || inputs.first_animation_frames == 0 {
        return Ok(result);
    }
    let mut phase = finite(inputs.phase_bits, "current script phase")?;
    if inputs.script_duration_raw > 1 {
        let speed = finite(inputs.component_speed_bits, "component speed")?;
        let step = finite(inputs.scaled_step_bits, "scaled actor step")?;
        let mut rate = speed * step;
        if inputs.child_flags_raw & 0x1_0000 == 0 {
            rate *= finite(inputs.selected_rate_bits, "selected clip rate")?;
        }
        let span = finite(inputs.selected_span_bits, "selected clip span")?;
        let mut increment = f32::from(inputs.first_animation_frames) / span;
        increment /= (inputs.script_duration_raw as i32) as f32;
        increment *= rate;
        phase += increment;
        ensure!(phase.is_finite(), "unsupported phase arithmetic");
    }
    let crossed = if phase >= 1.0 {
        if result.child_flags_raw & 1 != 0 {
            phase -= 1.0;
            result.child_flags_raw |= 0x400;
        } else {
            if result.child_flags_raw & 0x200 == 0 {
                phase = 1.0;
            }
            result.child_flags_raw |= 0x2_0000;
        }
        true
    } else if phase < 0.0 {
        if result.child_flags_raw & 1 != 0 {
            phase += 1.0;
            result.child_flags_raw |= 0x400;
        } else if result.child_flags_raw & 0x200 == 0 {
            phase = 0.0;
        }
        true
    } else {
        false
    };
    if crossed && result.child_flags_raw & 0x10 != 0 {
        result.child_flags_raw |= 0x4_0000;
        result.reprocess_script = true;
    }
    result.phase_bits = phase.to_bits();
    Ok(result)
}

#[allow(
    clippy::manual_clamp,
    reason = "Source comparisons replace negative zero with the positive-zero lower bound"
)]
fn source_unit_bounds(value: f32) -> f32 {
    if value >= 1.0 {
        1.0
    } else if value <= 0.0 {
        0.0
    } else {
        value
    }
}

fn finite(bits: u32, name: &str) -> Result<f32> {
    let value = f32::from_bits(bits);
    ensure!(value.is_finite(), "unsupported nonfinite {name}");
    Ok(value)
}

fn output(value: f32, name: &str) -> Result<u32> {
    ensure!(value.is_finite(), "unsupported nonfinite {name}");
    Ok(value.to_bits())
}

#[cfg(test)]
#[path = "tests/clock.rs"]
mod tests;
