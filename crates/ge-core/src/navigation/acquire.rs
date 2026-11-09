//! Ordered navmesh acquisition from translated candidates, not a render model transform.
use super::{NavigationFrame, SurfaceHit, SurfaceRay, SurfaceTriangle, nearest_surface_hit};
use anyhow::{Result, ensure};
use serde::Serialize;

pub struct NavigationInstance<'a> {
    pub frame: NavigationFrame,
    pub bounds: [[f32; 3]; 2],
    pub triangles: &'a [SurfaceTriangle],
    /// Stored centers or the prepared ordered centroid fallback, indexed by face ID.
    pub centers: &'a [[f32; 3]],
}

#[derive(Debug, Serialize)]
pub struct AcquiredNavigation {
    pub instance: usize,
    pub hit: SurfaceHit,
    pub center_height_difference: f32,
}

#[derive(Debug, Serialize)]
pub struct AcquisitionResult {
    pub selected: Option<AcquiredNavigation>,
    pub bounds_passed: usize,
    pub surface_candidates: usize,
}

/// Subtract translation BEFORE raising local Y, unlike ordinary endpoint binding.
/// Broad-phase a downward displacement, then nearest ray with 0<=t<distance.
/// Choose the least absolute WORLD FACE-CENTER height difference, keeping FIFO ties.
pub fn acquire_navigation(
    instances: &[NavigationInstance<'_>],
    position: [f32; 3],
    distance: f32,
) -> Result<AcquisitionResult> {
    ensure!(
        position.into_iter().all(f32::is_finite) && distance.is_finite() && distance > 0.,
        "invalid navigation acquisition request"
    );
    let mut result = AcquisitionResult {
        selected: None,
        bounds_passed: 0,
        surface_candidates: 0,
    };
    let mut best = f32::MAX;
    for (index, instance) in instances.iter().enumerate() {
        ensure!(
            instance.bounds.iter().flatten().all(|v| v.is_finite())
                && (0..3).all(|i| instance.bounds[0][i] <= instance.bounds[1][i]),
            "invalid navigation acquisition bounds"
        );
        ensure!(
            instance.centers.len() == instance.triangles.len()
                && instance
                    .triangles
                    .iter()
                    .enumerate()
                    .all(|(i, t)| i == t.index)
                && instance.centers.iter().flatten().all(|v| v.is_finite()),
            "invalid navigation acquisition face tables"
        );
        let mut origin = instance.frame.position_to_local(position)?;
        origin[1] += 1.;
        ensure!(
            origin.into_iter().all(f32::is_finite),
            "navigation acquisition lift overflow"
        );
        if !vertical_bounds_hit(instance.bounds, origin, distance)? {
            continue;
        }
        result.bounds_passed += 1;
        let Some(hit) = nearest_surface_hit(
            SurfaceRay {
                origin,
                direction: [0., -1., 0.],
                max_parameter: distance,
            },
            instance.triangles.iter().copied(),
        )?
        else {
            continue;
        };
        result.surface_candidates += 1;
        let center_world_y = instance.centers[hit.triangle][1] + instance.frame.translation[1];
        let difference = (center_world_y - position[1]).abs();
        ensure!(
            difference.is_finite(),
            "navigation acquisition center-height overflow"
        );
        if difference < best {
            best = difference;
            result.selected = Some(AcquiredNavigation {
                instance: index,
                hit,
                center_height_difference: difference,
            });
        }
    }
    Ok(result)
}

// Specialization of the observed segment/AABB helper for displacement (0,-distance,0).
// Starting inside is inclusive. Outside, directions below 1e-5 are not tested;
// plane parameters include both ends. This precheck does not replace the ray's strict max.
fn vertical_bounds_hit(bounds: [[f32; 3]; 2], origin: [f32; 3], distance: f32) -> Result<bool> {
    if origin[0] < bounds[0][0]
        || origin[0] > bounds[1][0]
        || origin[2] < bounds[0][2]
        || origin[2] > bounds[1][2]
    {
        return Ok(false);
    }
    if origin[1] >= bounds[0][1] && origin[1] <= bounds[1][1] {
        return Ok(true);
    }
    if distance < 1e-5 {
        return Ok(false);
    }
    for y in [bounds[0][1], bounds[1][1]] {
        let t = (y - origin[1]) / -distance;
        ensure!(
            t.is_finite(),
            "navigation acquisition bounds parameter overflow"
        );
        if (0. ..=1.).contains(&t) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "../tests/acquire.rs"]
mod tests;
