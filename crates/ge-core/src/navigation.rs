//! Surface-query contract reconstructed from static SJBE52 revision 0 evidence.
//! Wii reciprocal estimates/FP state and original runtime comparisons remain unverified.
use anyhow::{Result, ensure};
use serde::Serialize;
use std::ops::Range;

mod acquire;
mod actor_boundary;
mod actor_edge;
mod actor_goal;
mod manager_edge;
pub use manager_edge::manager_navigation_edge;
mod actor_action;
mod actor_target;
pub use actor_action::{
    ActorTargetActionResult, ActorTargetActionState, ActorTargetGate, ActorTargetMotion,
    ActorTargetOwner, ActorTargetRecord, ActorTargetSelection, actor_target_gate,
    actor_target_motion, run_actor_target_action,
};
pub use actor_boundary::{
    ActorBoundaryBand, ActorBoundaryResult, ActorBoundarySample, ActorBoundaryTarget,
    actor_boundary_gate,
};
pub use actor_edge::{ACTOR_EDGE_DOT_MIN, ActorEdgeDecision, ActorEdgeMesh, actor_target_edge};
pub use actor_goal::{
    ActorGoalContext, ActorGoalDecision, ActorGoalMesh, ActorGoalResult, actor_target_goal,
};
pub use actor_target::{
    ActorTargetBinding, ActorTargetDecision, ActorTargetMesh, ActorTargetPrepared,
    ActorTargetRequest, prepare_actor_target,
};
mod boundary;
pub use boundary::{
    BoundaryCache, BoundaryEdge, BoundaryHit, BoundaryMesh, BoundaryStats, build_boundary_cache,
};
mod path;
mod request;
pub mod runtime;
pub use acquire::{AcquiredNavigation, AcquisitionResult, NavigationInstance, acquire_navigation};
pub use request::{NavigationDispatch, NavigationRequestState};
mod route;
mod route_state;
pub use route::WaypointRoute;
pub use route_state::{NavigationRouteSearch, NavigationRouteState};
mod waypoints;
mod world;
mod world_goal;
pub use waypoints::{WaypointLimits, WaypointMesh, WaypointResult, build_waypoints};
pub use world::{
    NavigationFrame, WorldEndpoint, WorldPathRequest, WorldPathResult, locate_world_endpoint,
    select_world_triangle_path,
};
pub use world_goal::{
    SelectedWorldGoal, WorldGoalPathRequest, WorldGoalPathResult,
    select_world_triangle_path_to_goal_with_point,
    select_world_triangle_path_to_goal_with_point_and_edge_policy,
};
mod traversal;
pub use path::{
    GoalPathResult, GoalPathSearch, PathGraph, PathSearch, PathSearchResult, TrianglePath,
    select_triangle_path, select_triangle_path_to_goal_with_point,
    select_triangle_path_to_goal_with_point_and_edge_policy,
    select_triangle_path_to_goal_with_policy, select_triangle_path_with_policy,
    squared_goal_heuristic, surface_centroid,
};
pub use traversal::{
    SegmentSearch, SegmentSearchResult, TraversalEdge, connected_segment_hit,
    connected_segment_hit_with_fallible_policy, connected_segment_hit_with_policy,
};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SurfaceRay {
    pub origin: [f32; 3],
    /// Not normalized here: max_parameter bounds the ray parameter, not world distance.
    pub direction: [f32; 3],
    pub max_parameter: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceTriangle {
    pub index: usize,
    pub vertices: [[f32; 3]; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SurfaceHit {
    pub triangle: usize,
    pub parameter: f32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// The observed query falls back to all triangles for negative/out-of-range group IDs.
pub fn group_range(total: usize, groups: &[Range<usize>], requested: i32) -> Result<Range<usize>> {
    let selected = usize::try_from(requested).ok().and_then(|i| groups.get(i));
    let range = selected.cloned().unwrap_or(0..total);
    ensure!(
        range.start <= range.end && range.end <= total,
        "invalid navigation group range"
    );
    Ok(range)
}

/// Return the first accepted triangle in iteration/storage order, without nearest-hit sorting.
/// Endpoints are excluded, both windings are accepted, and triangle edges are inclusive.
/// Geometry is mesh-local. No placement transform, character clearance or collision response.
pub fn first_surface_hit(
    ray: SurfaceRay,
    triangles: impl IntoIterator<Item = SurfaceTriangle>,
) -> Result<Option<SurfaceHit>> {
    surface_hit(ray, triangles, Selection::First)
}

/// Return the least nonnegative accepted parameter, strictly below max_parameter.
/// Equal parameters retain the first triangle in storage order. Unlike first_surface_hit,
/// an origin on a triangle is accepted. Direction is used without normalization.
pub fn nearest_surface_hit(
    ray: SurfaceRay,
    triangles: impl IntoIterator<Item = SurfaceTriangle>,
) -> Result<Option<SurfaceHit>> {
    surface_hit(ray, triangles, Selection::Nearest)
}

/// Test one triangle along origin + t * displacement, including both t=0 and t=1.
/// Displacement is a full segment vector, not a normalized direction or endpoint.
pub fn triangle_segment_hit(
    origin: [f32; 3],
    displacement: [f32; 3],
    triangle: SurfaceTriangle,
) -> Result<Option<SurfaceHit>> {
    surface_hit(
        SurfaceRay {
            origin,
            direction: displacement,
            max_parameter: 1.,
        },
        [triangle],
        Selection::Segment,
    )
}

#[derive(Clone, Copy)]
enum Selection {
    First,
    Nearest,
    Segment,
}

fn surface_hit(
    ray: SurfaceRay,
    triangles: impl IntoIterator<Item = SurfaceTriangle>,
    selection: Selection,
) -> Result<Option<SurfaceHit>> {
    ensure!(
        ray.origin
            .iter()
            .chain(&ray.direction)
            .chain([&ray.max_parameter])
            .all(|v| v.is_finite()),
        "non-finite navigation ray"
    );
    if ray.max_parameter <= 0. {
        return Ok(None);
    }
    let mut best = None;
    let mut maximum = ray.max_parameter;
    for triangle in triangles {
        ensure!(
            triangle.vertices.iter().flatten().all(|v| v.is_finite()),
            "non-finite navigation triangle"
        );
        let (normal, offset) = plane(triangle.vertices)?;
        let denominator = plane_dot(normal, offset, ray.direction, 0.);
        ensure!(denominator.is_finite(), "navigation ray-plane overflow");
        if denominator.abs() < 1e-10 {
            continue;
        }
        let parameter = -plane_dot(normal, offset, ray.origin, 1.) / denominator;
        ensure!(parameter.is_finite(), "navigation intersection overflow");
        let accepted_parameter = match selection {
            Selection::First => parameter > 0. && parameter < maximum,
            Selection::Nearest => parameter >= 0. && parameter < maximum,
            Selection::Segment => (0. ..=1.).contains(&parameter),
        };
        if !accepted_parameter {
            continue;
        }
        // Separate multiply/add preserves the observed projection operation boundaries.
        let position = std::array::from_fn(|i| ray.origin[i] + ray.direction[i] * parameter);
        ensure!(
            position.iter().all(|v| v.is_finite()),
            "navigation projection overflow"
        );
        if contains(position, triangle.vertices, normal)? {
            let hit = SurfaceHit {
                triangle: triangle.index,
                parameter,
                position,
                normal,
            };
            if matches!(selection, Selection::Nearest) {
                maximum = parameter;
                best = Some(hit);
            } else {
                return Ok(Some(hit));
            }
        }
    }
    Ok(best)
}

fn difference(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn plane(points: [[f32; 3]; 3]) -> Result<([f32; 3], f32)> {
    let ab = difference(points[1], points[0]);
    let bc = difference(points[2], points[1]);
    let cross = [
        ab[1].mul_add(bc[2], -(ab[2] * bc[1])),
        ab[2].mul_add(bc[0], -(bc[2] * ab[0])),
        ab[0].mul_add(bc[1], -(bc[0] * ab[1])),
    ];
    let squared = cross[2].mul_add(cross[2], cross[0] * cross[0] + cross[1] * cross[1]);
    ensure!(squared.is_finite(), "navigation plane overflow");
    // Preserve the observed small/degenerate-plane fallback instead of silently dropping it.
    if squared <= f32::EPSILON {
        return Ok(([1., 0., 0.], 0.));
    }
    // Ordinary f32 normalization is provisional; Wii estimate instructions are not emulated.
    let inverse_length = 1. / squared.sqrt();
    let normal = cross.map(|v| v * inverse_length);
    let a = points[0];
    let offset = -normal[2].mul_add(a[2], normal[0] * a[0] + normal[1] * a[1]);
    ensure!(
        normal.iter().all(|v| v.is_finite()) && offset.is_finite(),
        "navigation plane overflow"
    );
    Ok((normal, offset))
}

fn plane_dot(normal: [f32; 3], offset: f32, value: [f32; 3], w: f32) -> f32 {
    normal[2].mul_add(value[2], normal[0] * value[0]) + offset.mul_add(w, normal[1] * value[1])
}

fn contains(point: [f32; 3], vertices: [[f32; 3]; 3], normal: [f32; 3]) -> Result<bool> {
    for i in 0..3 {
        let edge = difference(vertices[i], vertices[(i + 2) % 3]);
        let outward = [
            normal[2].mul_add(edge[1], -(normal[1] * edge[2])),
            normal[0].mul_add(edge[2], -(normal[2] * edge[0])),
            normal[1].mul_add(edge[0], -(normal[0] * edge[1])),
        ];
        let relative = difference(point, vertices[i]);
        let side = relative[2].mul_add(
            outward[2],
            relative[1].mul_add(outward[1], relative[0] * outward[0]),
        );
        ensure!(side.is_finite(), "navigation edge test overflow");
        if side > 0. {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "tests/navigation.rs"]
mod tests;
