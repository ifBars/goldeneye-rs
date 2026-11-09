//! Independent grenade launch/spin preparation from static SJBE52 evidence.
//! This does not instantiate a physics body or integrate translation/collision.
use crate::navigation::ActorTargetRecord;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

pub const GRENADE_RESOURCE: u32 = 0x0109000f;
pub const GRENADE_SCRIPT_REFERENCE: u32 = 0x04000d8e;
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrenadeSource {
    pub entity_kind_raw: u32,
    pub location: usize,
}
pub fn grenade_record(source: GrenadeSource) -> Result<usize> {
    if source.entity_kind_raw == 0x52000014 {
        return Ok(4);
    }
    [0, 0, 1, 1, 2, 2, 3, 0]
        .get(source.location)
        .copied()
        .context("grenade source location outside observed table")
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GrenadeOrientation {
    AxisAngle { axis: [f32; 3], angle: f32 },
    EulerYxz { angles: [f32; 3] },
}
#[derive(Debug, Serialize)]
pub struct GrenadeLaunch {
    pub record: usize,
    pub position: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub orientation: GrenadeOrientation,
    pub quaternion_xyzw: [f32; 4],
    pub angular_velocity: [f32; 3],
    pub angular_damping: f32,
    pub random_draws: usize,
}
struct Stream {
    word: u32,
    draws: usize,
}
impl Stream {
    fn unit(&mut self) -> f32 {
        self.word = self
            .word
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.draws += 1;
        ((self.word >> 1) as f32) * f32::from_bits(0x30000000)
    }
    fn axis(&mut self) -> Result<[f32; 3]> {
        // Source draws Z, then Y, then X, before sign bias and normalization.
        let raw = [self.unit(), self.unit(), self.unit()];
        let mut axis = [raw[2], raw[1], raw[0]].map(|u| {
            let scaled = 1.8f32 * u;
            let v = -0.9f32 + scaled;
            v + if v >= 0. { 0.1f32 } else { -0.1f32 }
        });
        let length = length(axis)?;
        if length > 1e-5 {
            let scale = 1.0 / length;
            axis = axis.map(|v| v * scale);
        }
        Ok(axis)
    }
}
/// Prepare after runtime resource/type/component resolution succeeds. The source
/// uses a separate record selector, not the target action's current active index.
/// Host errors leave the caller's RNG word unchanged. Wii exceptional inputs and
/// sin/cos/estimate precision remain unverified.
pub fn prepare_grenade_launch(
    source: GrenadeSource,
    records: &[ActorTargetRecord],
    rng_word: &mut u32,
) -> Result<GrenadeLaunch> {
    let record = grenade_record(source)?;
    let selected = records
        .get(record)
        .context("grenade selected record out of bounds")?;
    ensure!(
        selected
            .position
            .into_iter()
            .chain(selected.motion)
            .all(f32::is_finite),
        "nonfinite grenade record"
    );
    let mut stream = Stream {
        word: *rng_word,
        draws: 0,
    };
    let (orientation, quaternion_xyzw, angular_velocity, angular_damping) = if record == 1 {
        let bearing = f64::from(selected.motion[0]).atan2(f64::from(selected.motion[2])) as f32;
        let angles = [0., bearing, -f32::from_bits(0x3fc90fdb)];
        let q = euler_yxz(angles);
        (
            GrenadeOrientation::EulerYxz { angles },
            q,
            [0., length(selected.motion)? / 0.2f32, 0.],
            5.,
        )
    } else {
        let axis = stream.axis()?;
        let angle = f32::from_bits(0x40c90fdb) * stream.unit();
        let (s, c) = sin_cos(angle * 0.5);
        let q = [axis[0] * s, axis[1] * s, axis[2] * s, c];
        let spin = stream.axis()?;
        let scaled = f32::from_bits(0x4096cbe4) * stream.unit();
        let rate = f32::from_bits(0x3fc90fdb) + scaled;
        (
            GrenadeOrientation::AxisAngle { axis, angle },
            q,
            spin.map(|v| v * rate),
            20.,
        )
    };
    ensure!(
        quaternion_xyzw
            .into_iter()
            .chain(angular_velocity)
            .all(f32::is_finite),
        "grenade orientation/spin overflow"
    );
    *rng_word = stream.word;
    Ok(GrenadeLaunch {
        record,
        position: selected.position,
        linear_velocity: selected.motion,
        orientation,
        quaternion_xyzw,
        angular_velocity,
        angular_damping,
        random_draws: stream.draws,
    })
}
pub(crate) fn length(v: [f32; 3]) -> Result<f32> {
    let squared = v[2].mul_add(v[2], v[0] * v[0] + v[1] * v[1]);
    ensure!(squared.is_finite(), "grenade vector length overflow");
    Ok(if squared == 0. { 0. } else { squared.sqrt() })
}
fn sin_cos(angle: f32) -> (f32, f32) {
    let (s, c) = f64::from(angle).sin_cos();
    (s as f32, c as f32)
}
fn euler_yxz(angles: [f32; 3]) -> [f32; 4] {
    let (sx, cx) = sin_cos(angles[0] * 0.5);
    let (sy, cy) = sin_cos(angles[1] * 0.5);
    let (sz, cz) = sin_cos(angles[2] * 0.5);
    // Y-X-Z quaternion composition; retain products/additions even for X=0.
    let xy = sx * sy;
    let yc = cy * sx;
    let xc = cx * sy;
    let cc = cx * cy;
    [
        yc * cz + xc * sz,
        xc * cz - yc * sz,
        -(cz * xy) + cc * sz,
        cc * cz + xy * sz,
    ]
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrenadeSpin {
    pub flags_raw: u32,
    pub angular_velocity: [f32; 3],
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrenadeTime {
    pub clock_delta: f32,
    pub time_scale: f32,
}
#[derive(Debug, Serialize)]
pub struct GrenadeSpinStep {
    pub enabled: bool,
    pub scaled_delta: Option<f32>,
    pub exponent: Option<f32>,
    pub factor: Option<f32>,
}
impl GrenadeSpin {
    /// Angular-only stage of the projectile update. Bit 0x08 gates damping;
    /// clock_delta*time_scale is rounded before multiplication by negative rate.
    pub fn damp(&mut self, rate: f32, time: GrenadeTime) -> Result<GrenadeSpinStep> {
        let mut result = GrenadeSpinStep {
            enabled: false,
            scaled_delta: None,
            exponent: None,
            factor: None,
        };
        if self.flags_raw & 8 == 0 {
            return Ok(result);
        }
        ensure!(
            self.angular_velocity
                .into_iter()
                .chain([rate, time.clock_delta, time.time_scale])
                .all(f32::is_finite),
            "nonfinite grenade spin/time/rate"
        );
        let scaled_delta = time.time_scale * time.clock_delta;
        ensure!(scaled_delta.is_finite(), "grenade scaled time overflow");
        let exponent = (-rate) * scaled_delta;
        ensure!(exponent.is_finite(), "grenade damping exponent overflow");
        let factor = f64::from(exponent).exp() as f32;
        let velocity = self.angular_velocity.map(|v| v * factor);
        ensure!(
            factor.is_finite() && velocity.into_iter().all(f32::is_finite),
            "grenade angular damping overflow"
        );
        self.angular_velocity = velocity;
        result.enabled = true;
        result.scaled_delta = Some(scaled_delta);
        result.exponent = Some(exponent);
        result.factor = Some(factor);
        Ok(result)
    }
}
#[cfg(test)]
#[path = "tests/grenade.rs"]
mod tests;
