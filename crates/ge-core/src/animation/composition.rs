//! Explicit single-layer control/script scratch and local commit. Callers must
//! prepare dense skin/root-adjusted locals first. No scheduler or camera lives
//! here; nontrivial quaternion interpolation is explicitly unsupported.
use super::skeleton::{LocalPose, SUPPORTED_BONES, Skeleton};
use anyhow::{Context, Result, ensure};

const ONE: u32 = 0x3f80_0000;
const COMMIT: u32 = 0x4000;
const CURRENT_LAYER: u32 = 0x8000;

#[derive(Debug, Clone, Copy)]
pub struct CurveConstants {
    pub minimum_index: usize,
    pub minimum_bits: u32,
    pub maximum_bits: u32,
}
impl CurveConstants {
    /// Explicit source initialization, not a default for later mutable history.
    pub fn source_initial() -> Self {
        Self {
            minimum_index: 0,
            minimum_bits: 0,
            maximum_bits: ONE,
        }
    }
}

/// Packed percent knots and source scalar Catmull-Rom ordering. Early returns
/// do not inspect curve constants or saturate progress outside [0,1]. Interior
/// nonfinite/zero-span arithmetic is an explicit supported-host error.
pub fn fade(progress_bits: u32, curve_word: u32, constants: CurveConstants) -> Result<u32> {
    if curve_word == 0 {
        return Ok(progress_bits);
    }
    let progress = finite(progress_bits)?;
    if progress <= 0.0 || progress >= 1.0 {
        return Ok(progress_bits);
    }
    ensure!(
        constants.minimum_index < 4,
        "fade minimum index is out of range"
    );
    let minimum = finite(constants.minimum_bits)?;
    let maximum = finite(constants.maximum_bits)?;
    ensure!(minimum <= maximum, "unsupported inverted fade clamp");
    let bytes = curve_word.to_be_bytes();
    let percent = f32::from_bits(0x3c23_d70a);
    let points = [
        [0.0, 0.0],
        [f32::from(bytes[0]) * percent, f32::from(bytes[1]) * percent],
        [f32::from(bytes[2]) * percent, f32::from(bytes[3]) * percent],
        [1.0, 1.0],
    ];
    let segment = if progress < points[1][0] {
        0usize
    } else if progress < points[2][0] {
        1
    } else {
        2
    };
    let numerator = progress - points[segment][0];
    let denominator = points[segment + 1][0] - points[segment][0];
    ensure!(denominator > 0.0, "unsupported reached fade knot span");
    let t = numerator / denominator;
    let prior = constants.minimum_index.max(segment.saturating_sub(1));
    let following = (segment + 2).min(3);
    let [y0, y1, y2, y3] = [
        points[prior][1],
        points[segment][1],
        points[segment + 1][1],
        points[following][1],
    ];
    let t2 = t * t;
    let t3 = t * t2;
    // Preserve separate single-precision operations; no mul_add/reassociation.
    let cubic = ((3.0 * y1 - 3.0 * y2) + y3) - y0;
    let quadratic = ((2.0 * y0 - 5.0 * y1) + 4.0 * y2) - y3;
    let base = 2.0 * y1 + t * (y2 - y0);
    let value = 0.5 * ((base + t2 * quadratic) + t3 * cubic);
    ensure!(value.is_finite(), "unsupported nonfinite fade polynomial");
    Ok(if value >= maximum {
        constants.maximum_bits
    } else if value <= minimum {
        constants.minimum_bits
    } else {
        value.to_bits()
    })
}

/// One sampled slot's scratch. None marks genuinely unwritten bone storage;
/// constructor identity is never invented to fill it. Inter-layer blending,
/// auxiliary channels and linked skeleton owners are separate prerequisites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scratch {
    locals: Vec<Option<LocalPose>>,
    written_mask: [u32; 4],
    flags_raw: u32,
    successful_samples: usize,
    layer_finished: bool,
}
impl Scratch {
    pub fn new(bones: usize, auxiliary_count: u32, bone_budget: usize) -> Result<Self> {
        ensure!(
            bones > 0 && bones <= SUPPORTED_BONES,
            "unsupported scratch bone count"
        );
        ensure!(
            bones <= bone_budget,
            "animation scratch bone budget exhausted"
        );
        ensure!(
            auxiliary_count == 0,
            "unsupported scratch auxiliary channels"
        );
        Ok(Self {
            locals: vec![None; bones],
            written_mask: [0; 4],
            flags_raw: 0,
            successful_samples: 0,
            layer_finished: false,
        })
    }
    pub fn sample_control(&mut self, captured: &[LocalPose]) -> Result<()> {
        self.validate(captured)?;
        let successes = self
            .successful_samples
            .checked_add(1)
            .context("sample count overflow")?;
        for (destination, local) in self.locals.iter_mut().zip(captured) {
            *destination = Some(*local);
        }
        self.written_mask = [u32::MAX; 4];
        self.successful_samples = successes;
        // Source control success alone does not set the script commit flag.
        Ok(())
    }
    pub fn script_fade(
        &self,
        progress_bits: u32,
        curve_word: u32,
        constants: CurveConstants,
    ) -> Result<u32> {
        if self.successful_samples == 0 {
            Ok(ONE)
        } else {
            fade(progress_bits, curve_word, constants)
        }
    }
    /// Actual fresh prewarm reaches fade1/inversefade0 and performs no child
    /// quaternion interpolation. Inputs must already include source skin/root
    /// and sparse-channel fallback processing. General fade/partial-mask paths
    /// fail before mutation; no approximate quaternion result is substituted.
    pub fn sample_script(
        &mut self,
        prepared: &[LocalPose],
        mask: [u32; 4],
        fade_bits: u32,
    ) -> Result<()> {
        self.validate(prepared)?;
        let inverse = 1.0 - finite(fade_bits)?;
        ensure!(
            inverse == 0.0,
            "unsupported child quaternion interpolation precision"
        );
        ensure!(
            (0..self.locals.len()).all(|i| mask[i / 32] & (1 << (i % 32)) != 0),
            "unsupported partial script composition mask"
        );
        let successes = self
            .successful_samples
            .checked_add(1)
            .context("sample count overflow")?;
        for (destination, local) in self.locals.iter_mut().zip(prepared) {
            *destination = Some(*local);
        }
        for (written, supplied) in self.written_mask.iter_mut().zip(mask) {
            *written |= supplied;
        }
        self.flags_raw |= COMMIT | CURRENT_LAYER;
        self.successful_samples = successes;
        Ok(())
    }
    fn validate(&self, locals: &[LocalPose]) -> Result<()> {
        ensure!(!self.layer_finished, "sampled layer is already finished");
        ensure!(
            locals.len() == self.locals.len(),
            "sampled local count differs from scratch"
        );
        ensure!(
            locals.iter().all(|local| local.finite()),
            "unsupported nonfinite sampled locals"
        );
        Ok(())
    }
    pub fn finish_layer(&mut self) {
        self.flags_raw &= !CURRENT_LAYER;
        self.layer_finished = true;
    }
    /// Source accumulated4000 gate. Control-only scratch stays uncommitted.
    /// Supported target has zero auxiliary channels and no linked runtimes.
    pub fn commit(
        &self,
        skeleton: &mut Skeleton,
        lod_mask: Option<[u32; 4]>,
        bone_budget: usize,
    ) -> Result<bool> {
        if self.flags_raw & COMMIT == 0 {
            return Ok(false);
        }
        ensure!(
            self.layer_finished,
            "animation layer must finish before local commit"
        );
        let locals = self
            .locals
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .context("animation scratch contains unwritten locals")?;
        skeleton.commit_local_pose(&locals, lod_mask, bone_budget)?;
        Ok(true)
    }
    pub fn locals(&self) -> &[Option<LocalPose>] {
        &self.locals
    }
    pub fn written_mask(&self) -> [u32; 4] {
        self.written_mask
    }
    pub fn flags_raw(&self) -> u32 {
        self.flags_raw
    }
    pub fn successful_samples(&self) -> usize {
        self.successful_samples
    }
}

fn finite(bits: u32) -> Result<f32> {
    let value = f32::from_bits(bits);
    ensure!(value.is_finite(), "unsupported nonfinite composition input");
    Ok(value)
}

#[cfg(test)]
#[path = "tests/composition.rs"]
mod tests;
