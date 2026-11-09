//! Position-to-face binding from the explicit-face branch of the original navigation wrapper.
use super::{
    PathGraph, PathSearch, PathSearchResult, SurfaceHit, SurfaceRay, SurfaceTriangle,
    nearest_surface_hit, select_triangle_path,
};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct NavigationFrame {
    /// Observed wrapper origin. This binding subtracts translation only, not a model matrix.
    pub translation: [f32; 3],
}
impl NavigationFrame {
    /// Convert an unraised request position for later waypoint processing.
    pub fn position_to_local(self, world: [f32; 3]) -> Result<[f32; 3]> {
        ensure!(
            world.iter().chain(&self.translation).all(|v| v.is_finite()),
            "non-finite navigation position/frame"
        );
        let local: [f32; 3] = std::array::from_fn(|i| world[i] - self.translation[i]);
        ensure!(
            local.iter().all(|v| v.is_finite()),
            "navigation position translation overflow"
        );
        Ok(local)
    }
    pub fn position_to_world(self, local: [f32; 3]) -> Result<[f32; 3]> {
        ensure!(
            local.iter().chain(&self.translation).all(|v| v.is_finite()),
            "non-finite navigation position/frame"
        );
        let world: [f32; 3] = std::array::from_fn(|i| local[i] + self.translation[i]);
        ensure!(
            world.iter().all(|v| v.is_finite()),
            "navigation position translation overflow"
        );
        Ok(world)
    }
    /// Preserve operation order: raise world Y by one, then subtract the frame origin.
    /// The original wrapper uses a downward nearest query, 0 <= t < 3, over all faces.
    pub fn endpoint_ray(self, world: [f32; 3]) -> Result<SurfaceRay> {
        ensure!(
            world.iter().chain(&self.translation).all(|v| v.is_finite()),
            "non-finite navigation frame/endpoint"
        );
        let mut raised = world;
        raised[1] += 1.;
        let origin: [f32; 3] = std::array::from_fn(|i| raised[i] - self.translation[i]);
        ensure!(
            origin.iter().all(|v| v.is_finite()),
            "navigation endpoint translation overflow"
        );
        Ok(SurfaceRay {
            origin,
            direction: [0., -1., 0.],
            max_parameter: 3.,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorldEndpoint {
    pub local_hit: SurfaceHit,
    /// Diagnostic surface point. The original world request is not replaced with this point.
    pub world_surface_position: [f32; 3],
}

pub fn locate_world_endpoint(
    frame: NavigationFrame,
    world: [f32; 3],
    triangles: impl IntoIterator<Item = SurfaceTriangle>,
) -> Result<Option<WorldEndpoint>> {
    let hit = nearest_surface_hit(frame.endpoint_ray(world)?, triangles)?;
    hit.map(|local_hit| {
        let world_surface_position: [f32; 3] =
            std::array::from_fn(|i| local_hit.position[i] + frame.translation[i]);
        ensure!(
            world_surface_position.iter().all(|v| v.is_finite()),
            "navigation surface translation overflow"
        );
        Ok(WorldEndpoint {
            local_hit,
            world_surface_position,
        })
    })
    .transpose()
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorldPathRequest {
    pub frame: NavigationFrame,
    pub start: [f32; 3],
    pub goal: [f32; 3],
    pub max_expansions: usize,
}

#[derive(Debug, Serialize)]
pub struct WorldPathResult {
    pub start_endpoint: Option<WorldEndpoint>,
    pub goal_endpoint: Option<WorldEndpoint>,
    /// None means an endpoint was missing. Some with no path means a completed search miss.
    pub path_search: Option<PathSearchResult>,
}

/// Bind original world positions to faces before default explicit-face path selection.
/// Both endpoint lookups run even when the first misses, as in the observed wrapper.
/// Does not generate waypoints, snap actor positions, apply rotation/scale or move actors.
pub fn select_world_triangle_path(
    request: WorldPathRequest,
    triangles: &[SurfaceTriangle],
    graph: PathGraph<'_>,
) -> Result<WorldPathResult> {
    ensure!(
        request.max_expansions > 0,
        "navigation world path requires a positive expansion budget"
    );
    ensure!(
        triangles.len() == graph.neighbors.len()
            && triangles.iter().enumerate().all(|(i, t)| t.index == i),
        "world navigation geometry/topology IDs do not match"
    );
    let start_endpoint =
        locate_world_endpoint(request.frame, request.start, triangles.iter().copied())?;
    let goal_endpoint =
        locate_world_endpoint(request.frame, request.goal, triangles.iter().copied())?;
    let path_search = match (start_endpoint, goal_endpoint) {
        (Some(start), Some(goal)) => Some(select_triangle_path(
            PathSearch {
                start_triangle: start.local_hit.triangle,
                goal_triangle: goal.local_hit.triangle,
                max_expansions: request.max_expansions,
            },
            graph,
        )?),
        _ => None,
    };
    Ok(WorldPathResult {
        start_endpoint,
        goal_endpoint,
        path_search,
    })
}

#[cfg(test)]
#[path = "../tests/world.rs"]
mod tests;
