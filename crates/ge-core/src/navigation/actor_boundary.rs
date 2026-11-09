//! Four observed actor range-band predicates. Action selection and live actor storage are separate.
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ActorBoundaryBand {
    #[serde(rename = "4.5")]
    Radius45,
    #[serde(rename = "3.4")]
    Radius34,
    #[serde(rename = "3.45")]
    Radius345,
    #[serde(rename = "3.6")]
    Radius36,
}
impl ActorBoundaryBand {
    pub fn radius(self) -> f32 {
        match self {
            Self::Radius45 => 4.5,
            Self::Radius34 => 3.4,
            Self::Radius345 => 3.45,
            Self::Radius36 => 3.6,
        }
    }
}

/// Caller-provided target records, corresponding to positions at +0x148 with stride 0x4c.
/// This is a typed host view, not an original object-memory reader.
pub struct ActorBoundaryTarget<'a> {
    pub flags_raw: u32,
    pub active_record: u8,
    pub positions: &'a [[f32; 3]],
}
impl ActorBoundaryTarget<'_> {
    /// Modes zero/one select the active record only when flag bit zero is set;
    /// otherwise they use record zero. Other target-helper modes are not modeled here.
    pub fn position(&self) -> Result<[f32; 3]> {
        let index = if self.flags_raw & 1 != 0 {
            usize::from(self.active_record)
        } else {
            0
        };
        let position = *self
            .positions
            .get(index)
            .context("actor boundary target record out of bounds")?;
        ensure!(
            position.into_iter().all(f32::is_finite),
            "nonfinite actor boundary target"
        );
        Ok(position)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActorBoundarySample {
    pub actor: [f32; 3],
    pub target: [f32; 3],
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActorBoundaryResult {
    UnsupportedMode,
    OutsideBand { distance_squared: f32 },
    HeightMismatch { difference: f32 },
    MissingBinding,
    Boundary { clear: bool },
}
impl ActorBoundaryResult {
    pub fn accepted(&self) -> bool {
        matches!(self, Self::Boundary { clear: true })
    }
}

/// Preserve lazy gate order and the second position resolution before the boundary
/// call. A second sample is not range/height-gated again. `mode` and binding presence
/// are caller snapshots; this does not schedule actions or own live actor objects.
pub fn actor_boundary_gate(
    band: ActorBoundaryBand,
    mode: u32,
    has_binding: bool,
    mut sample: impl FnMut() -> Result<ActorBoundarySample>,
    mut boundary_clear: impl FnMut([f32; 3], [f32; 3]) -> Result<bool>,
) -> Result<ActorBoundaryResult> {
    if mode > 1 {
        return Ok(ActorBoundaryResult::UnsupportedMode);
    }
    let first = checked_sample(sample()?)?;
    let [dx, dy, dz] = std::array::from_fn(|i| first.actor[i] - first.target[i]);
    let distance_squared = dz.mul_add(dz, dx * dx);
    ensure!(
        distance_squared.is_finite() && dy.is_finite(),
        "actor boundary distance overflow"
    );
    let inner = band.radius() - 0.25;
    let outer = band.radius() + 0.25;
    if distance_squared >= outer * outer || distance_squared <= inner * inner {
        return Ok(ActorBoundaryResult::OutsideBand { distance_squared });
    }
    if dy.abs() >= 1. {
        return Ok(ActorBoundaryResult::HeightMismatch {
            difference: dy.abs(),
        });
    }
    if !has_binding {
        return Ok(ActorBoundaryResult::MissingBinding);
    }
    let second = checked_sample(sample()?)?;
    Ok(ActorBoundaryResult::Boundary {
        clear: boundary_clear(second.actor, second.target)?,
    })
}
fn checked_sample(sample: ActorBoundarySample) -> Result<ActorBoundarySample> {
    ensure!(
        sample
            .actor
            .into_iter()
            .chain(sample.target)
            .all(f32::is_finite),
        "nonfinite actor boundary position"
    );
    Ok(sample)
}

#[cfg(test)]
#[path = "../tests/actor_boundary.rs"]
mod tests;
