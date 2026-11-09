//! Ordered, depth-bounded surface lookup, not a movement path or shortest-path search.
use super::{SurfaceHit, SurfaceTriangle, triangle_segment_hit};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SegmentSearch {
    pub origin: [f32; 3],
    pub displacement: [f32; 3],
    pub start_triangle: usize,
    pub previous_triangle: Option<usize>,
    /// Number of neighbor transitions allowed. Zero/negative still tests the starting face.
    pub max_depth: i32,
    /// Independent host work guard. Exhaustion is an error, never a geometric miss.
    pub max_face_tests: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SegmentSearchResult {
    pub hit: Option<SurfaceHit>,
    pub face_tests: usize,
    pub policy_checks: usize,
    pub maximum_depth_reached: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraversalEdge {
    pub from_triangle: usize,
    pub to_triangle: usize,
    pub previous_triangle: Option<usize>,
    pub slot: usize,
}

/// Test the current face first, then depth-first neighbors in slots 0, 1, 2,
/// stopping at the first boundary sentinel even if later slots contain IDs.
/// The observed default policy prevents only immediate backtracking. No global visited
/// set, nearest-face reduction, group restriction or movement-path reconstruction.
pub fn connected_segment_hit(
    request: SegmentSearch,
    triangles: &[SurfaceTriangle],
    neighbors: &[[Option<usize>; 3]],
) -> Result<SegmentSearchResult> {
    connected_segment_hit_with_policy(request, triangles, neighbors, |edge| {
        Some(edge.to_triangle) != edge.previous_triangle
    })
}

/// Policy replacement mirrors the original callback boundary: called for each nonboundary
/// edge just before descending. The default backtracking rule is replaced, not added.
/// Triangle IDs must equal their table positions, as in the original navmesh arrays.
pub fn connected_segment_hit_with_policy(
    request: SegmentSearch,
    triangles: &[SurfaceTriangle],
    neighbors: &[[Option<usize>; 3]],
    mut allow_edge: impl FnMut(TraversalEdge) -> bool,
) -> Result<SegmentSearchResult> {
    connected_segment_hit_with_fallible_policy(request, triangles, neighbors, |edge| {
        Ok(allow_edge(edge))
    })
}

/// Checked policy variant; errors propagate only when that edge callback is reached.
pub fn connected_segment_hit_with_fallible_policy(
    request: SegmentSearch,
    triangles: &[SurfaceTriangle],
    neighbors: &[[Option<usize>; 3]],
    mut allow_edge: impl FnMut(TraversalEdge) -> Result<bool>,
) -> Result<SegmentSearchResult> {
    ensure!(
        request.max_face_tests > 0,
        "navigation traversal requires a positive face-test budget"
    );
    ensure!(
        triangles.len() == neighbors.len(),
        "navigation topology length mismatch"
    );
    ensure!(
        request.start_triangle < triangles.len(),
        "navigation start triangle out of bounds"
    );
    ensure!(
        request
            .previous_triangle
            .is_none_or(|p| p < triangles.len()),
        "navigation previous triangle out of bounds"
    );
    let mut stack = Vec::new();
    stack.try_reserve(1)?;
    stack.push(Frame::new(
        request.start_triangle,
        request.previous_triangle,
        request.max_depth,
    ));
    let mut result = SegmentSearchResult {
        hit: None,
        face_tests: 0,
        policy_checks: 0,
        maximum_depth_reached: 0,
    };
    while !stack.is_empty() {
        let depth = stack.len() - 1;
        let frame = &mut stack[depth];
        if !frame.tested {
            ensure!(
                result.face_tests < request.max_face_tests,
                "navigation traversal face-test budget exhausted after {} tests",
                result.face_tests
            );
            ensure!(
                triangles[frame.triangle].index == frame.triangle,
                "navigation triangle IDs do not match topology positions"
            );
            result.face_tests += 1;
            result.maximum_depth_reached = result.maximum_depth_reached.max(depth as u32);
            if let Some(hit) = triangle_segment_hit(
                request.origin,
                request.displacement,
                triangles[frame.triangle],
            )? {
                result.hit = Some(hit);
                return Ok(result);
            }
            frame.tested = true;
            if frame.remaining_depth <= 0 {
                stack.pop();
                continue;
            }
        }
        if frame.next_edge == 3 {
            stack.pop();
            continue;
        }
        let slot = frame.next_edge;
        frame.next_edge += 1;
        let Some(neighbor) = neighbors[frame.triangle][slot] else {
            stack.pop();
            continue;
        };
        ensure!(
            neighbor < triangles.len(),
            "navigation neighbor triangle out of bounds"
        );
        result.policy_checks += 1;
        if allow_edge(TraversalEdge {
            from_triangle: frame.triangle,
            to_triangle: neighbor,
            previous_triangle: frame.previous,
            slot,
        })? {
            let child = Frame::new(neighbor, Some(frame.triangle), frame.remaining_depth - 1);
            stack.try_reserve(1)?;
            stack.push(child);
        }
    }
    Ok(result)
}

struct Frame {
    triangle: usize,
    previous: Option<usize>,
    remaining_depth: i32,
    next_edge: usize,
    tested: bool,
}
impl Frame {
    fn new(triangle: usize, previous: Option<usize>, remaining_depth: i32) -> Self {
        Self {
            triangle,
            previous,
            remaining_depth,
            next_edge: 0,
            tested: false,
        }
    }
}

#[cfg(test)]
#[path = "../tests/traversal.rs"]
mod tests;
