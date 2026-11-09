//! Scoped title-camera projection reconstructed from static frontend contracts.
//!
//! The source renderer installs a `(0, -10, 0)` root render override and a camera
//! at `(0, 18, 0)`. Its active full-viewport projection selects 4:3 or 16:9 from
//! external display state. The host must choose that state explicitly; it is not
//! recovered from the game assets. Host `tan` and ordinary f32 arithmetic remain
//! provisional substitutes for the Wii SDK math and GPU viewport precision.
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    Wide,
    Standard,
}

impl Aspect {
    fn ratio(self) -> f32 {
        match self {
            Self::Wide => f32::from_bits(0x3fe3_8e39),
            Self::Standard => f32::from_bits(0x3faa_aaab),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Projected {
    /// Position on the existing 640 by 480 frontend canvas.
    pub position: [f32; 2],
    /// Positive camera distance, retained for perspective-correct interpolation.
    pub clip_w: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    aspect: Aspect,
    fov_degrees: f32,
    cotangent: f32,
}

impl Camera {
    pub fn new(aspect: Aspect) -> Result<Self> {
        let fov_radians = f32::from_bits(0x3d0e_fa35);
        let degrees_numerator = 180. * fov_radians;
        // Preserve the engine conversion's actual denominator, not pi.
        let fov_degrees = degrees_numerator / f32::from_bits(0x4049_566d);
        let half_degrees = 0.5 * fov_degrees;
        let half_radians = f32::from_bits(0x3c8e_fa35) * half_degrees;
        // The SDK call returns double and then rounds to single. This host
        // function is not a claim of Wii libm or reciprocal-bit equivalence.
        let tangent = f64::from(half_radians).tan() as f32;
        let cotangent = 1. / tangent;
        ensure!(
            fov_degrees.is_finite() && cotangent.is_finite() && fov_degrees > 0. && cotangent > 0.,
            "invalid frontend camera projection constants"
        );
        Ok(Self {
            aspect,
            fov_degrees,
            cotangent,
        })
    }

    pub fn aspect(&self) -> Aspect {
        self.aspect
    }

    pub fn fov_degrees(&self) -> f32 {
        self.fov_degrees
    }

    pub fn cotangent(&self) -> f32 {
        self.cotangent
    }

    /// Initial source queued-camera clipping distance. Indirect runtime changes
    /// to original global state are outside this fresh-frontend adapter.
    pub fn near(&self) -> f32 {
        f32::from_bits(0x3ca3_d70a)
    }

    pub fn far(&self) -> f32 {
        13_000.
    }

    /// Project a script-world point before the source root render override.
    /// Depth clip boundaries are inclusive. A point outside them is an explicit
    /// unsupported clipping case, not silently clamped or moved onto a boundary.
    /// Off-canvas XY positions remain valid for the GPU's viewport clipping.
    pub fn project(&self, world_point: [f32; 3]) -> Result<Projected> {
        ensure!(
            world_point.iter().all(|v| v.is_finite()),
            "nonfinite frontend projection point"
        );
        let root_y = world_point[1] - 10.;
        let view_z = root_y - 18.;
        let clip_w = -view_z;
        ensure!(
            clip_w.is_finite() && clip_w >= self.near() && clip_w <= self.far(),
            "frontend point lies outside supported camera clip range"
        );
        let horizontal = self.cotangent / self.aspect.ratio();
        let clip_x = horizontal * world_point[0];
        let clip_y = self.cotangent * world_point[2];
        let ndc_x = clip_x / clip_w;
        let ndc_y = clip_y / clip_w;
        let pixel_x = 320. * ndc_x;
        let pixel_y = 240. * ndc_y;
        let position = [320. + pixel_x, 240. - pixel_y];
        ensure!(
            position.iter().all(|v| v.is_finite()),
            "frontend projected position overflow"
        );
        Ok(Projected { position, clip_w })
    }

    /// Specialized view-Z row for the current axis-aligned source title camera,
    /// including its root render override. Input matrices are column-major. Only
    /// the addressed Y row is read; no invented origin offset or normalization is
    /// applied. Native general-matrix FMA/signed-zero behavior remains unverified.
    pub fn depth_row(&self, model: [f32; 16]) -> Result<[f32; 4]> {
        let mut row = [model[1], model[5], model[9], model[13]];
        ensure!(
            row.iter().all(|v| v.is_finite()),
            "nonfinite frontend depth row"
        );
        let root_translation = row[3] - 10.;
        row[3] = root_translation - 18.;
        ensure!(
            row[3].is_finite(),
            "frontend depth-row translation overflow"
        );
        Ok(row)
    }
}

#[cfg(test)]
#[path = "tests/frontend_projection.rs"]
mod tests;
