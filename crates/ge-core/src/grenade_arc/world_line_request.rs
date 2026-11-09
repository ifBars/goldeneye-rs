//! Source line-constructor settings and the grenade caller's later Y mutation.
use crate::collision::{line_bounds, world::StoredBounds};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GrenadeWorldLineRequest {
    pub origin: [f32; 3],
    /// Original constructor displacement, before the caller adds its lift.
    pub constructor_delta: [f32; 3],
    /// Mutable line displacement after the separate rounded Y-lift addition.
    pub delta: [f32; 3],
    /// Constructor bounds persist until an actual mesh hit refreshes them.
    pub initial_world_bounds: StoredBounds,
    pub maximum_parameter: f32,
    pub filter_slot: u32,
    pub filter_flags: u32,
    pub required_any: u32,
    pub excluded_any: u32,
    pub stop_first_raw: u8,
}

impl GrenadeWorldLineRequest {
    /// Preserve the source's constructor-before-lift sequence. This prepares
    /// values only; loaded-world and actor backends remain explicit dependencies.
    pub fn from_target(origin: [f32; 3], target: [f32; 3], small_lift_raw: u8) -> Result<Self> {
        ensure!(
            origin.into_iter().chain(target).all(f32::is_finite),
            "nonfinite grenade clearance input"
        );
        let constructor_delta = std::array::from_fn(|axis| target[axis] - origin[axis]);
        ensure!(
            constructor_delta.into_iter().all(f32::is_finite),
            "grenade clearance displacement overflow"
        );
        let initial_world_bounds = line_bounds::endpoint_bounds(origin, constructor_delta, 1.)?;
        let mut delta = constructor_delta;
        delta[1] += if small_lift_raw != 0 { 0.1 } else { 1. };
        ensure!(
            delta.into_iter().all(f32::is_finite),
            "grenade clearance displacement overflow"
        );
        Ok(Self {
            origin,
            constructor_delta,
            delta,
            initial_world_bounds,
            maximum_parameter: 1.,
            filter_slot: 1,
            filter_flags: 0,
            required_any: 0,
            excluded_any: 0,
            stop_first_raw: 0,
        })
    }
}

#[cfg(test)]
#[path = "../tests/grenade_world_line_request.rs"]
mod tests;
