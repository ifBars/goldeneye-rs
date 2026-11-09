//! Alternate world binding: start-only lookup and raw selected-center waypoint destination.
use super::{
    GoalPathResult, GoalPathSearch, NavigationFrame, PathGraph, SurfaceTriangle, TraversalEdge,
    WorldEndpoint, locate_world_endpoint, select_triangle_path_to_goal_with_point_and_edge_policy,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorldGoalPathRequest {
    pub frame: NavigationFrame,
    pub start: [f32; 3],
    pub max_expansions: usize,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SelectedWorldGoal {
    pub triangle: usize,
    /// Raw mesh center is treated as the wrapper's goal value, without adding translation first.
    pub raw_center: [f32; 3],
    pub local_waypoint_goal: [f32; 3],
    /// Subtract/add round trip can differ from raw_center under f32 cancellation.
    pub returned_world_goal: [f32; 3],
}
#[derive(Debug, Serialize)]
pub struct WorldGoalPathResult {
    pub start_endpoint: Option<WorldEndpoint>,
    pub selected_goal: Option<SelectedWorldGoal>,
    /// Absent on a missing start; a completed search may separately find no acceptable face.
    pub path_search: Option<GoalPathResult>,
}

/// Start uses the ordinary raised downward nearest lookup. There is NO goal ray.
/// The explicit point/predicate and default edge/cost policies do not recreate a
/// full actor policy. Preserve the raw-center coordinate rule in later waypoints.
pub fn select_world_triangle_path_to_goal_with_point(
    request: WorldGoalPathRequest,
    triangles: &[SurfaceTriangle],
    graph: PathGraph<'_>,
    point: [f32; 3],
    goal: impl FnMut(usize) -> Result<bool>,
) -> Result<WorldGoalPathResult> {
    select_world_triangle_path_to_goal_with_point_and_edge_policy(
        request,
        triangles,
        graph,
        point,
        goal,
        |edge| Ok(Some(edge.to_triangle) != edge.previous_triangle),
    )
}

pub fn select_world_triangle_path_to_goal_with_point_and_edge_policy(
    request: WorldGoalPathRequest,
    triangles: &[SurfaceTriangle],
    graph: PathGraph<'_>,
    point: [f32; 3],
    goal: impl FnMut(usize) -> Result<bool>,
    allow_edge: impl FnMut(TraversalEdge) -> Result<bool>,
) -> Result<WorldGoalPathResult> {
    ensure!(
        request.max_expansions > 0,
        "world predicate path requires a positive expansion budget"
    );
    ensure!(
        triangles.len() == graph.neighbors.len()
            && triangles.len() == graph.centers.len()
            && triangles.iter().enumerate().all(|(i, t)| t.index == i),
        "world predicate geometry/topology IDs do not match"
    );
    let start_endpoint =
        locate_world_endpoint(request.frame, request.start, triangles.iter().copied())?;
    let Some(start) = start_endpoint else {
        return Ok(WorldGoalPathResult {
            start_endpoint,
            selected_goal: None,
            path_search: None,
        });
    };
    let path_search = select_triangle_path_to_goal_with_point_and_edge_policy(
        GoalPathSearch {
            start_triangle: start.local_hit.triangle,
            max_expansions: request.max_expansions,
        },
        graph,
        point,
        goal,
        allow_edge,
    )?;
    let selected_goal = path_search
        .search
        .path
        .as_ref()
        .map(|path| -> Result<SelectedWorldGoal> {
            let triangle = *path
                .triangles
                .last()
                .context("empty successful world predicate path")?;
            let raw_center = *graph
                .centers
                .get(triangle)
                .context("selected world goal out of bounds")?;
            let local_waypoint_goal = request.frame.position_to_local(raw_center)?;
            let returned_world_goal = request.frame.position_to_world(local_waypoint_goal)?;
            Ok(SelectedWorldGoal {
                triangle,
                raw_center,
                local_waypoint_goal,
                returned_world_goal,
            })
        })
        .transpose()?;
    Ok(WorldGoalPathResult {
        start_endpoint,
        selected_goal,
        path_search: Some(path_search),
    })
}

#[cfg(test)]
#[path = "../tests/world_goal.rs"]
mod tests;
