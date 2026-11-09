//! State-one grenade arc preparation and two-stage clearance scheduling.
//! Collision queries require a backend; these requests never imply an empty world.
use crate::{
    grenade,
    navigation::{
        ActorTargetActionResult, ActorTargetActionState, ActorTargetGate, ActorTargetOwner,
        ActorTargetRequest, ActorTargetSelection, actor_target_gate, run_actor_target_action,
    },
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
mod world_box_request;
pub use world_box_request::GrenadeWorldBoxRequest;
mod world_line_request;
pub use world_line_request::GrenadeWorldLineRequest;

const ANGLES: [u32; 5] = [0x3f490fdb, 0x3f75be0b, 0x3f1c61aa, 0x3f91361e, 0x3edf66f3];

#[derive(Debug, Clone, Serialize)]
pub struct GrenadeArc {
    pub angle_index: u32,
    pub angle: f32,
    pub tangent: f32,
    pub start: [f32; 3],
    pub raised_displacement: [f32; 3],
    pub motion: [f32; 3],
    pub horizontal_distance: f32,
    pub horizontal_velocity: f32,
    pub apex_time: f32,
    pub flight_time_squared: f32,
    pub apex_displacement: [f32; 3],
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArcRejection {
    Elevation,
    ApexAfterTarget,
}
#[derive(Debug, Serialize)]
pub struct GrenadeArcCandidate {
    pub motion: [f32; 3],
    pub rejection: Option<ArcRejection>,
    pub arc: Option<GrenadeArc>,
}

/// Finite host reconstruction; sqrt/tan/sin/cos are provisional for Wii math.
/// A normal rejection still carries the intermediate motion stored by the source.
pub fn prepare_grenade_arc(
    start: [f32; 3],
    target: [f32; 3],
    angle_index: u32,
    small_lift_raw: u8,
) -> Result<GrenadeArcCandidate> {
    let angle = f32::from_bits(
        *ANGLES
            .get(angle_index as usize)
            .context("grenade arc angle index outside observed table")?,
    );
    ensure!(
        start.into_iter().chain(target).all(f32::is_finite),
        "nonfinite grenade arc input"
    );
    let mut displacement = std::array::from_fn(|i| target[i] - start[i]);
    displacement[1] += if small_lift_raw != 0 { 0.1 } else { 1. };
    ensure!(
        displacement.into_iter().all(f32::is_finite),
        "grenade arc displacement overflow"
    );
    let tangent = f64::from(angle).tan() as f32;
    let horizontal = grenade::length([displacement[0], 0., displacement[2]])?;
    ensure!(
        horizontal > 0.,
        "unsupported zero horizontal grenade arc distance"
    );
    let denominator = tangent - displacement[1] / horizontal;
    if denominator <= 0.001 {
        return Ok(GrenadeArcCandidate {
            motion: displacement,
            rejection: Some(ArcRejection::Elevation),
            arc: None,
        });
    }
    let gravity_distance = 4.905f32 * horizontal;
    let speed_squared = (gravity_distance * (1. + tangent * tangent)) / denominator;
    ensure!(speed_squared.is_finite(), "grenade arc speed overflow");
    let speed = if speed_squared > f32::EPSILON {
        speed_squared.sqrt()
    } else {
        0.
    };
    let horizontal_velocity = speed * (f64::from(angle).cos() as f32);
    ensure!(
        horizontal_velocity > 0.,
        "unsupported zero horizontal grenade velocity"
    );
    let scale = horizontal_velocity / horizontal;
    let mut motion = displacement.map(|v| v * scale);
    motion[1] = speed * (f64::from(angle).sin() as f32);
    let apex_time = motion[1] / 9.81f32;
    let flight_time = horizontal / horizontal_velocity;
    let flight_time_squared = flight_time * flight_time;
    let apex_time_squared = apex_time * apex_time;
    ensure!(
        motion
            .into_iter()
            .chain([apex_time, flight_time_squared, apex_time_squared])
            .all(f32::is_finite),
        "grenade arc motion/time overflow"
    );
    if apex_time_squared >= flight_time_squared {
        return Ok(GrenadeArcCandidate {
            motion,
            rejection: Some(ArcRejection::ApexAfterTarget),
            arc: None,
        });
    }
    let mut apex_displacement = motion.map(|v| v * apex_time);
    apex_displacement[1] -= 4.905f32 * apex_time_squared;
    ensure!(
        apex_displacement.into_iter().all(f32::is_finite),
        "grenade arc apex overflow"
    );
    let arc = GrenadeArc {
        angle_index,
        angle,
        tangent,
        start,
        raised_displacement: displacement,
        motion,
        horizontal_distance: horizontal,
        horizontal_velocity,
        apex_time,
        flight_time_squared,
        apex_displacement,
    };
    Ok(GrenadeArcCandidate {
        motion,
        rejection: None,
        arc: Some(arc),
    })
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrenadeArcState {
    pub action: ActorTargetActionState,
    pub angle_index: u32,
    pub prepare_arc_raw: u8,
    pub small_lift_raw: u8,
    pub cached_apex: [f32; 3],
}
/// First ascent uses a padded oriented box, not a straight ray or curved primitive.
/// The world/contact backend is not implemented; unavailable queries must error.
pub enum GrenadeClearance<'a> {
    WorldBox {
        arc: &'a GrenadeArc,
        volume: &'a GrenadeClearanceVolume,
    },
    /// Source constructor stores cached apex as origin, then raises delta Y.
    WorldLine {
        origin: [f32; 3],
        constructor_displacement: [f32; 3],
        displacement: [f32; 3],
    },
    /// Issued only after a clear world line. The source passes no excluded actor.
    ActorLine {
        origin: [f32; 3],
        constructor_displacement: [f32; 3],
        displacement: [f32; 3],
    },
}

#[derive(Debug, Serialize)]
pub struct GrenadeClearanceVolume {
    pub shape: crate::collision::OrientedBox,
    /// The original manager selection uses start plus apex distance, not a fitted
    /// sphere around the padded box. This does not prove loaded-world membership.
    pub selection_origin: [f32; 3],
    pub selection_radius: f32,
}
impl GrenadeArc {
    /// First-ascent oriented box from the source's type-three volume construction.
    /// Host sqrt and reciprocal remain provisional for Wii estimate instructions.
    pub fn clearance_volume(&self) -> Result<GrenadeClearanceVolume> {
        ensure!(
            self.start
                .into_iter()
                .chain(self.apex_displacement)
                .chain([self.tangent, self.horizontal_velocity, self.apex_time])
                .all(f32::is_finite),
            "nonfinite grenade clearance arc"
        );
        let horizontal_apex = self.horizontal_velocity * self.apex_time;
        ensure!(
            horizontal_apex.is_finite() && horizontal_apex > 0.,
            "unsupported horizontal grenade apex distance"
        );
        let slope = self.apex_displacement[1] / horizontal_apex;
        let chord_scale = (1. + slope * slope).sqrt();
        let curvature_velocity = self.horizontal_velocity * (self.tangent - slope);
        let padding = (curvature_velocity * curvature_velocity) / (19.62f32 * chord_scale);
        let extent_y = (padding + 0.3f32) * 0.5;
        let mut axis_a = self.apex_displacement;
        // This normalization has a different accumulation order from length3:
        // the paired path fuses Z into X before adding Y, with the fourth lane zero.
        let square = axis_a[2].mul_add(axis_a[2], axis_a[0] * axis_a[0]) + axis_a[1] * axis_a[1];
        ensure!(square.is_finite(), "grenade clearance axis overflow");
        if square > 1e-7 {
            let inverse = 1. / square.sqrt();
            axis_a = axis_a.map(|v| v * inverse);
        }
        let apex_length = grenade::length(self.apex_displacement)?;
        let extent_x = 0.3f32 + apex_length * 0.5;
        let negative_y = -axis_a[1];
        let inverse_horizontal = 1. / horizontal_apex;
        let mut axis_b = self
            .apex_displacement
            .map(|v| (v * negative_y) * inverse_horizontal);
        axis_b[1] = grenade::length([axis_a[0], 0., axis_a[2]])?;
        let center = std::array::from_fn(|i| {
            let first = self.start[i] + axis_a[i] * extent_x;
            first + axis_b[i] * extent_y
        });
        let shape = crate::collision::OrientedBox {
            center,
            axes: [axis_a, axis_b],
            half_extents: [extent_x, extent_y, 0.3],
        };
        shape.validate()?;
        shape.axis(2)?;
        Ok(GrenadeClearanceVolume {
            shape,
            selection_origin: self.start,
            selection_radius: apex_length,
        })
    }
}
#[derive(Serialize)]
pub struct GrenadeArcAction {
    pub gate: ActorTargetGate,
    pub return_code: u32,
    pub rejection: Option<ArcRejection>,
    pub arc: Option<GrenadeArc>,
    pub volume: Option<GrenadeClearanceVolume>,
}
#[derive(Serialize)]
pub struct GrenadeActionDispatch {
    pub return_code: u32,
    pub target: Option<ActorTargetActionResult>,
    pub analytic: Option<GrenadeArcAction>,
}
impl GrenadeArcState {
    /// Owning dispatcher: state zero uses the navigation target action, state one
    /// uses analytic clearance, all other states return four without inspecting inputs.
    pub fn dispatch(
        &mut self,
        owner: ActorTargetOwner,
        max_face_tests: usize,
        target: impl FnMut(ActorTargetRequest) -> Result<Option<ActorTargetSelection>>,
        blocked: impl FnMut(GrenadeClearance<'_>) -> Result<bool>,
    ) -> Result<GrenadeActionDispatch> {
        let mut result = GrenadeActionDispatch {
            return_code: 4,
            target: None,
            analytic: None,
        };
        match self.action.state_raw {
            0 => {
                let action =
                    run_actor_target_action(&mut self.action, owner, max_face_tests, target)?;
                result.return_code = action.return_code;
                result.target = Some(action);
            }
            1 => {
                let action = self.query(owner, blocked)?;
                result.return_code = action.return_code;
                result.analytic = Some(action);
            }
            _ => {}
        }
        Ok(result)
    }
    fn reject(&mut self) -> u32 {
        self.angle_index = self.angle_index.wrapping_add(1);
        if self.angle_index >= 5 {
            if self.small_lift_raw == 0 {
                return 1;
            }
            self.small_lift_raw = 0;
            self.angle_index = 0;
            // Preserve prepare_arc_raw on this wrap, including the line stage.
        } else {
            self.prepare_arc_raw = 1;
        }
        0
    }
    /// Direct state-one handler. The owning dispatcher must choose it; a rejected
    /// angular gate does not address records or invoke the collision backend.
    /// Callback errors preserve prior writes, rather than claiming a clear world.
    pub fn query(
        &mut self,
        owner: ActorTargetOwner,
        mut blocked: impl FnMut(GrenadeClearance<'_>) -> Result<bool>,
    ) -> Result<GrenadeArcAction> {
        let gate = actor_target_gate(
            owner,
            self.action.target,
            self.action.active_index,
            self.action.mode_raw,
        )?;
        let mut result = GrenadeArcAction {
            gate,
            return_code: 1,
            rejection: None,
            arc: None,
            volume: None,
        };
        if !gate.allowed {
            return Ok(result);
        }
        if self.prepare_arc_raw != 0 {
            let selected = self
                .action
                .records
                .get_mut(self.action.active_index)
                .context("grenade arc selected record out of bounds")?;
            let candidate = prepare_grenade_arc(
                selected.position,
                self.action.target,
                self.angle_index,
                self.small_lift_raw,
            )?;
            selected.motion = candidate.motion;
            result.rejection = candidate.rejection;
            if let Some(arc) = candidate.arc {
                let volume = arc.clearance_volume()?;
                let hit = blocked(GrenadeClearance::WorldBox {
                    arc: &arc,
                    volume: &volume,
                })?;
                result.volume = Some(volume);
                if !hit {
                    let apex = std::array::from_fn(|i| arc.start[i] + arc.apex_displacement[i]);
                    ensure!(
                        apex.into_iter().all(f32::is_finite),
                        "grenade cached apex overflow"
                    );
                    self.prepare_arc_raw = 0;
                    self.cached_apex = apex;
                    result.return_code = 0;
                    result.arc = Some(arc);
                    return Ok(result);
                }
                result.arc = Some(arc);
            }
        } else {
            let request = GrenadeWorldLineRequest::from_target(
                self.cached_apex,
                self.action.target,
                self.small_lift_raw,
            )?;
            let world_hit = blocked(GrenadeClearance::WorldLine {
                origin: request.origin,
                constructor_displacement: request.constructor_delta,
                displacement: request.delta,
            })?;
            if !world_hit
                && !blocked(GrenadeClearance::ActorLine {
                    origin: request.origin,
                    constructor_displacement: request.constructor_delta,
                    displacement: request.delta,
                })?
            {
                self.action.state_raw = 3;
                result.return_code = 3;
                return Ok(result);
            }
        }
        result.return_code = self.reject();
        Ok(result)
    }
}
#[cfg(test)]
#[path = "tests/grenade_arc.rs"]
mod tests;
