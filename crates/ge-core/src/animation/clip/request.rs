use super::blend::finite;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInputs {
    pub start: i16,
    pub end: i16,
    pub animation_frames: u16,
    /// Actual child+0x28 position, not a chosen frame or elapsed clock.
    pub normalized_position_bits: u32,
    pub child_flags_raw: u32,
    /// Component+0x135. Fresh animator bind writes zero at8030589c; later
    /// history must be supplied. Bit2 disables frame interpolation; bit0x80
    /// requires the skeleton+0x40 object's mask, rather than a fabricated mask.
    pub component_flags_raw: u8,
    /// Full BE word at animation header+0x10. Native reads its HIGH halfword
    /// with lhz before masking format bits0x7f00.
    pub animation_word10_raw: u32,
    /// Child+0x72, read only when child flags&0x100. Constructor80321680
    /// writes zero; None explicitly represents an unavailable/unneeded field.
    pub child_format_raw: Option<u16>,
    pub bone_mask: [u32; 4],
    pub skin_bone_mask: Option<[u32; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameConstants {
    pub minimum_position_bits: u32,
    pub maximum_position_bits: u32,
    pub remainder_period_bits: u64,
    pub snap_remainder_low_bits: u32,
    pub snap_remainder_high_bits: u32,
    pub half_step_bits: u32,
    pub pair_threshold_bits: u32,
    pub minimum_cache_frame: i32,
}

impl FrameConstants {
    /// Observed source constants/global initialization; explicit caller choice,
    /// not restored animation/session state or a clock/frame default.
    pub fn source_initial() -> Self {
        Self {
            minimum_position_bits: 0,
            maximum_position_bits: 0x3f80_0000,
            remainder_period_bits: 0x3fe0_0000_0000_0000,
            snap_remainder_low_bits: 0x3c23_d70a,
            snap_remainder_high_bits: 0x3efa_e148,
            half_step_bits: 0x3f00_0000,
            pair_threshold_bits: 0x3a83_126f,
            minimum_cache_frame: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRequest {
    pub requested_frame0: i32,
    pub cache_frame0: u32,
    pub requested_frame1: Option<i32>,
    pub cache_frame1: Option<u32>,
    pub frame_fraction_bits: u32,
    pub range_frames: i32,
    pub range_end: i32,
    pub scaled_position_bits: u32,
    pub format_flags_raw: u32,
    /// Effective sampler mask before the cache's mandatory bone-zero request.
    pub bone_mask: [u32; 4],
    pub cache_mask: [u32; 4],
}

/// Builds requests only. Signed source requests and cache-clamped frames stay
/// distinct; negative/nonpositive ranges are not silently replaced with a
/// generic animation range. Zero span alone uses the native frame-count fallback.
pub fn frame_request(inputs: FrameInputs, constants: FrameConstants) -> Result<FrameRequest> {
    ensure!(
        inputs.animation_frames != 0,
        "unsupported zero-frame animation cache"
    );
    let position = finite(
        inputs.normalized_position_bits,
        "normalized animation position",
    )?;
    let minimum = finite(
        constants.minimum_position_bits,
        "minimum animation position",
    )?;
    let maximum = finite(
        constants.maximum_position_bits,
        "maximum animation position",
    )?;
    let low = finite(
        constants.snap_remainder_low_bits,
        "lower frame remainder threshold",
    )?;
    let high = finite(
        constants.snap_remainder_high_bits,
        "upper frame remainder threshold",
    )?;
    let step = finite(constants.half_step_bits, "frame half step")?;
    let threshold = finite(constants.pair_threshold_bits, "frame pair threshold")?;
    let period = f64::from_bits(constants.remainder_period_bits);
    ensure!(
        (0.0..=1.0).contains(&maximum)
            && (0.0..=maximum).contains(&minimum)
            && period.is_finite()
            && period > 0.0
            && (0.0..=high).contains(&low)
            && f64::from(high) <= period
            && step > 0.0
            && (0.0..=1.0).contains(&threshold)
            && (0..i32::from(inputs.animation_frames)).contains(&constants.minimum_cache_frame),
        "unsupported frame request constants"
    );
    let position = if position >= maximum {
        maximum
    } else if position <= minimum {
        minimum
    } else {
        position
    };
    let start = i32::from(inputs.start);
    let declared_span = i32::from(inputs.end) - start;
    let span = if declared_span == 0 {
        i32::from(inputs.animation_frames)
    } else {
        declared_span
    };
    let end = start
        .checked_add(span)
        .context("signed clip range overflow")?;
    let mut scaled = position * span as f32;
    // Native calls double fmod with period0.5, then rounds its result to f32.
    // A floor/fmod1 substitute would change the half-grid snapping behavior.
    let remainder = (f64::from(scaled) % period) as f32;
    if remainder < low {
        scaled -= remainder;
    } else if remainder > high {
        let base = scaled - remainder;
        scaled = step + base;
    }
    let frame_position = scaled + start as f32;
    ensure!(
        scaled.is_finite()
            && frame_position.is_finite()
            && frame_position >= i32::MIN as f32
            && frame_position < i32::MAX as f32,
        "unsupported frame position outside signed conversion bounds"
    );
    let mut frame0 = frame_position as i32; // source fctiwz: truncation toward zero
    if frame0 >= end {
        frame0 = end.checked_sub(1).context("signed clip end overflow")?;
    }
    let fraction = if inputs.component_flags_raw & 2 != 0 {
        0.0
    } else {
        frame_position - frame0 as f32
    };
    let frame1 = if fraction > threshold && span > 1 {
        let mut next = frame0.checked_add(1).context("adjacent frame overflow")?;
        if next >= end {
            next = if inputs.child_flags_raw & 1 != 0 {
                next.checked_sub(span).context("loop frame overflow")?
            } else {
                end.checked_sub(1).context("signed clip end overflow")?
            };
        }
        Some(next)
    } else {
        None
    };
    let format = if inputs.child_flags_raw & 0x100 != 0 {
        u32::from(
            inputs
                .child_format_raw
                .context("required child+0x72 frame format is unavailable")?,
        )
    } else {
        inputs.animation_word10_raw >> 16
    } & 0x7f00;
    let mut mask = inputs.bone_mask;
    if inputs.component_flags_raw & 0x80 != 0 {
        let skin = inputs
            .skin_bone_mask
            .context("required skeleton+0x40 bone mask is unavailable")?;
        for (word, skin) in mask.iter_mut().zip(skin) {
            *word &= skin;
        }
    }
    let mut cache_mask = mask;
    cache_mask[0] |= 1;
    let cache = |frame: i32| -> u32 {
        frame.clamp(
            constants.minimum_cache_frame,
            i32::from(inputs.animation_frames) - 1,
        ) as u32
    };
    Ok(FrameRequest {
        requested_frame0: frame0,
        cache_frame0: cache(frame0),
        requested_frame1: frame1,
        cache_frame1: frame1.map(cache),
        frame_fraction_bits: fraction.to_bits(),
        range_frames: span,
        range_end: end,
        scaled_position_bits: scaled.to_bits(),
        format_flags_raw: format,
        bone_mask: mask,
        cache_mask,
    })
}
