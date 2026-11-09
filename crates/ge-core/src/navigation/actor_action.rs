//! Target action gate and selected-record update; host math is provisional for Wii estimates/libm.
use super::{ActorTargetRequest, NavigationFrame};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

const PI: f32 = f32::from_bits(0x40490fdb);
const TAU: f32 = f32::from_bits(0x40c90fdb);
#[derive(Clone, Copy, Serialize)]
pub struct ActorTargetOwner {
    pub frame: NavigationFrame,
    pub yaw: f32,
}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActorTargetRecord {
    pub position: [f32; 3],
    pub motion: [f32; 3],
}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActorTargetActionState {
    pub state_raw: u32,
    pub active_index: usize,
    pub mode_raw: u8,
    pub target: [f32; 3],
    pub records: Vec<ActorTargetRecord>,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ActorTargetGate {
    pub normalized_yaw: f32,
    pub bearing: f32,
    pub difference: f32,
    pub lower: f32,
    pub upper: f32,
    pub allowed: bool,
}
#[derive(Clone, Copy, Serialize)]
pub struct ActorTargetSelection {
    pub raw_center: [f32; 3],
    pub frame: NavigationFrame,
}
#[derive(Debug, Serialize)]
pub struct ActorTargetMotion {
    pub world_center: [f32; 3],
    pub displacement: [f32; 3],
    pub length: f32,
    pub exponential_denominator: f32,
    pub scale: f32,
    pub inverse_length: f32,
    pub motion: [f32; 3],
}
#[derive(Serialize)]
pub struct ActorTargetActionResult {
    pub gate: ActorTargetGate,
    /// Original caller return value, distinct from the retained action state.
    pub return_code: u32,
    pub update: Option<ActorTargetMotion>,
}

/// The observed five angle pairs precede the actor policy tables. Unsupported
/// indices are host errors, never unchecked reads into adjacent interface data.
pub fn actor_target_gate(
    owner: ActorTargetOwner,
    target: [f32; 3],
    active_index: usize,
    mode_raw: u8,
) -> Result<ActorTargetGate> {
    let bands = [
        (0x3f490fdb, 0x3f490fdb),
        (0x3edf66f3, 0x3e5f66f3),
        (0x3f490fdb, 0x3f490fdb),
        (0x3f490fdb, 0x3f490fdb),
        (0x3f490fdb, 0x3f490fdb),
    ];
    let &(lower, upper) = bands
        .get(active_index)
        .context("actor target angular index outside observed table")?;
    let (lower, upper) = if mode_raw == 0 {
        (lower, upper)
    } else {
        (upper, lower)
    };
    let (lower, upper) = (f32::from_bits(lower), f32::from_bits(upper));
    ensure!(owner.yaw.is_finite(), "nonfinite actor target owner yaw");
    let yaw = 0.0f32 + owner.yaw;
    let normalized_yaw = if yaw.abs() < PI {
        yaw
    } else {
        let remainder = (f64::from(yaw) % f64::from(TAU)) as f32;
        if remainder.abs() < PI {
            remainder
        } else if yaw >= 0. {
            remainder - TAU
        } else {
            TAU + remainder
        }
    };
    ensure!(
        [
            target[0],
            target[2],
            owner.frame.translation[0],
            owner.frame.translation[2]
        ]
        .into_iter()
        .all(f32::is_finite),
        "nonfinite actor target planar position/frame"
    );
    let x = target[0] - owner.frame.translation[0];
    let z = target[2] - owner.frame.translation[2];
    ensure!(
        x.is_finite() && z.is_finite(),
        "actor target planar translation overflow"
    );
    let bearing = f64::from(x).atan2(f64::from(z)) as f32;
    let difference = angle_difference(normalized_yaw, bearing);
    ensure!(
        difference.is_finite(),
        "actor target angle arithmetic overflow"
    );
    Ok(ActorTargetGate {
        normalized_yaw,
        bearing,
        difference,
        lower,
        upper,
        allowed: in_band(difference, lower, upper),
    })
}
fn in_band(difference: f32, lower: f32, upper: f32) -> bool {
    difference < upper && difference > -lower
}
fn positive_turn(value: f32) -> f32 {
    if (0. ..TAU).contains(&value) {
        return value;
    }
    let remainder = (f64::from(value) % f64::from(TAU)) as f32;
    if remainder >= 0. {
        remainder
    } else {
        TAU + remainder
    }
}
fn angle_difference(from: f32, to: f32) -> f32 {
    let difference = positive_turn(to) - positive_turn(from);
    if difference.abs() < PI {
        difference
    } else if difference < 0. {
        TAU + difference
    } else {
        difference - TAU
    }
}

/// Preserve world-center addition, displacement, scale and reciprocal multiply
/// boundaries. Do not cancel them to delta/2. Zero/underflow length is unsupported:
/// the source would feed zero to reciprocal-estimate refinement and store NaNs.
pub fn actor_target_motion(
    selection: ActorTargetSelection,
    active: [f32; 3],
) -> Result<ActorTargetMotion> {
    let world_center = selection.frame.position_to_world(selection.raw_center)?;
    ensure!(
        active.into_iter().all(f32::is_finite),
        "nonfinite active actor target position"
    );
    let displacement = std::array::from_fn(|i| world_center[i] - active[i]);
    let squared = displacement[2].mul_add(
        displacement[2],
        displacement[0] * displacement[0] + displacement[1] * displacement[1],
    );
    ensure!(squared.is_finite(), "actor target motion length overflow");
    ensure!(
        squared > 0.,
        "unsupported zero/underflow actor target motion length"
    );
    let length = squared.sqrt(); // Wii reciprocal-square-root estimate/refinement remains provisional.
    let exponential = (-25.0f64).exp() as f32;
    let exponential_denominator = 1.0f32 - exponential;
    let factor = 5.0f32 * 0.2f32;
    let scale = (factor * length) / exponential_denominator;
    let inverse_length = 1.0f32 / length; // Provisional substitute for ps_res plus two refinements.
    let mut motion = displacement.map(|v| ((v * scale) * inverse_length) * 0.5);
    motion[1] += 1.;
    ensure!(
        motion.into_iter().all(f32::is_finite),
        "actor target motion arithmetic overflow"
    );
    Ok(ActorTargetMotion {
        world_center,
        displacement,
        length,
        exponential_denominator,
        scale,
        inverse_length,
        motion,
    })
}

/// A failed gate or completed query miss returns 1 and retains all records/state.
/// Only a selected goal sets state 2 and replaces the active record's motion.
/// Checked host errors retain state; original exceptional-input effects are unverified.
pub fn run_actor_target_action(
    state: &mut ActorTargetActionState,
    owner: ActorTargetOwner,
    max_face_tests: usize,
    query: impl FnOnce(ActorTargetRequest) -> Result<Option<ActorTargetSelection>>,
) -> Result<ActorTargetActionResult> {
    ensure!(
        max_face_tests > 0,
        "actor action face-test budget must be positive"
    );
    let gate = actor_target_gate(owner, state.target, state.active_index, state.mode_raw)?;
    let mut result = ActorTargetActionResult {
        gate,
        return_code: 1,
        update: None,
    };
    if !gate.allowed {
        return Ok(result);
    }
    let active = state
        .records
        .get(state.active_index)
        .context("actor target active record out of bounds")?
        .position;
    let selected = query(ActorTargetRequest {
        target: state.target,
        active_position: active,
        owner_frame: owner.frame,
        max_face_tests,
    })?;
    let Some(selected) = selected else {
        return Ok(result);
    };
    let update = actor_target_motion(selected, active)?;
    state.state_raw = 2;
    state.records[state.active_index].motion = update.motion;
    result.return_code = 2;
    result.update = Some(update);
    Ok(result)
}
#[cfg(test)]
#[path = "../tests/actor_action.rs"]
mod tests;
