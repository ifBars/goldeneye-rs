//! Centroid-seeded, first-failure greedy waypoint reduction from static SJBE52 evidence.
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct WaypointMesh<'a> {
    pub vertices: &'a [[f32; 3]],
    pub triangles: &'a [[usize; 3]],
    /// Stored centers, or ordered f32 centroid fallback prepared by the caller.
    pub centers: &'a [[f32; 3]],
    pub neighbors: &'a [[Option<usize>; 3]],
    pub flags: &'a [Option<u32>],
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WaypointLimits {
    /// Independent host guards; exhaustion is an error, not a rejected shortcut.
    pub max_shortcut_tests: usize,
    pub max_fan_steps: usize,
}

#[derive(Debug, Serialize)]
pub struct WaypointResult {
    pub points: Vec<[f32; 3]>,
    pub retained_indices: Vec<usize>,
    pub seed_count: usize,
    pub shortcut_tests: usize,
    pub fan_steps: usize,
    pub flag_rejections: usize,
}

/// Seed start, centers of chain faces 1..len-1, then goal. Keep endpoint duplicates.
/// Greedily extend until the FIRST failed shortcut; do not search farther candidates.
/// The shortcut checks portals at chain indices anchor+1..candidate (not anchor).
/// Original runtime/FP parity and actual actor clearance remain unverified.
pub fn build_waypoints(
    mesh: WaypointMesh<'_>,
    chain: &[usize],
    start: [f32; 3],
    goal: [f32; 3],
    limits: WaypointLimits,
) -> Result<WaypointResult> {
    ensure!(
        limits.max_shortcut_tests > 0 && limits.max_fan_steps > 0,
        "waypoint work limits must be positive"
    );
    let count = mesh.triangles.len();
    ensure!(
        mesh.centers.len() == count && mesh.neighbors.len() == count && mesh.flags.len() == count,
        "waypoint table length mismatch"
    );
    ensure!(
        chain.iter().all(|&f| f < count),
        "waypoint chain face out of bounds"
    );
    ensure!(
        mesh.triangles
            .iter()
            .flatten()
            .all(|&v| v < mesh.vertices.len())
            && mesh
                .neighbors
                .iter()
                .flatten()
                .all(|n| n.is_none_or(|n| n < count)),
        "waypoint geometry/topology index out of bounds"
    );
    ensure!(
        mesh.vertices
            .iter()
            .flatten()
            .chain(mesh.centers.iter().flatten())
            .chain(&start)
            .chain(&goal)
            .all(|v| v.is_finite()),
        "non-finite waypoint input"
    );
    let mut seeds = Vec::new();
    seeds.try_reserve_exact(chain.len().max(2))?;
    seeds.push(start);
    for &face in chain.iter().skip(1).take(chain.len().saturating_sub(2)) {
        seeds.push(mesh.centers[face]);
    }
    seeds.push(goal);
    let mut result = WaypointResult {
        points: Vec::new(),
        retained_indices: Vec::new(),
        seed_count: seeds.len(),
        shortcut_tests: 0,
        fan_steps: 0,
        flag_rejections: 0,
    };
    result.points.try_reserve_exact(seeds.len())?;
    result.retained_indices.try_reserve_exact(seeds.len())?;
    result.points.push(start);
    result.retained_indices.push(0);
    let mut anchor = 0;
    while anchor < seeds.len() - 1 {
        let mut retained = anchor + 1;
        for candidate in anchor + 2..seeds.len() {
            ensure!(
                result.shortcut_tests < limits.max_shortcut_tests,
                "waypoint shortcut-test budget exhausted"
            );
            result.shortcut_tests += 1;
            if !shortcut(
                mesh,
                chain,
                seeds[anchor],
                seeds[candidate],
                anchor..candidate,
                &mut result,
                limits,
            )? {
                break;
            }
            retained = candidate;
        }
        result.points.push(seeds[retained]);
        result.retained_indices.push(retained);
        anchor = retained;
    }
    Ok(result)
}

fn shortcut(
    mesh: WaypointMesh<'_>,
    chain: &[usize],
    start: [f32; 3],
    goal: [f32; 3],
    span: std::ops::Range<usize>,
    result: &mut WaypointResult,
    limits: WaypointLimits,
) -> Result<bool> {
    for i in span.start + 1..span.end {
        let before = chain[i];
        let after = chain[i + 1];
        if mesh.flags[before].unwrap_or(0) & 1 != 0 {
            result.flag_rejections += 1;
            return Ok(false);
        }
        let Some(edge) = shared_edge(mesh.triangles[before], mesh.triangles[after]) else {
            continue;
        };
        let [a, b] = edge.map(|v| mesh.vertices[v]);
        if !intersects_xz(start, goal, a, b)? {
            return Ok(false);
        }
        let direction = sub(goal, start);
        let squared = dot(direction, direction);
        ensure!(
            squared.is_finite() && squared > 0.,
            "waypoint shortcut length overflow/zero"
        );
        for vertex in edge {
            let p = mesh.vertices[vertex];
            let parameter = dot(sub(p, start), direction) / squared;
            ensure!(
                parameter.is_finite(),
                "waypoint clearance projection overflow"
            );
            if (parameter - 0.5).abs() > 0.5 {
                continue;
            }
            // Original projection uses separate multiply and add operations.
            let projected = std::array::from_fn(|j| start[j] + direction[j] * parameter);
            let distance = sub(p, projected);
            let distance_squared = dot(distance, distance);
            ensure!(
                distance_squared.is_finite(),
                "waypoint clearance distance overflow"
            );
            if distance_squared > 0.25 {
                continue;
            }
            if !closed_vertex_fan(mesh, before, after, vertex, result, limits)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

// Match in the first face's vertex order, stopping after two matches. Do not compare positions.
fn shared_edge(a: [usize; 3], b: [usize; 3]) -> Option<[usize; 2]> {
    let mut out = [0; 2];
    let mut count = 0;
    for vertex in a {
        for other in b {
            if vertex == other {
                out[count] = vertex;
                count += 1;
                if count == 2 {
                    return Some(out);
                }
            }
        }
    }
    None
}

fn intersects_xz(start: [f32; 3], goal: [f32; 3], a: [f32; 3], b: [f32; 3]) -> Result<bool> {
    let d = sub(goal, start);
    let e = sub(b, a);
    let p = sub(start, a);
    let determinant = d[0] * e[2] - d[2] * e[0];
    ensure!(
        determinant.is_finite(),
        "waypoint portal determinant overflow"
    );
    if determinant.abs() < 0.001 {
        return Ok(false);
    }
    let t = (p[2] * e[0] - p[0] * e[2]) / determinant;
    let u = (p[2] * d[0] - p[0] * d[2]) / determinant;
    ensure!(
        t.is_finite() && u.is_finite(),
        "waypoint portal parameter overflow"
    );
    Ok(t > 0. && t <= 1. && u > 0. && u <= 1.)
}

fn closed_vertex_fan(
    mesh: WaypointMesh<'_>,
    before: usize,
    after: usize,
    vertex: usize,
    result: &mut WaypointResult,
    limits: WaypointLimits,
) -> Result<bool> {
    let (mut previous, mut current) = (before, after);
    loop {
        ensure!(
            result.fan_steps < limits.max_fan_steps,
            "waypoint vertex-fan budget exhausted"
        );
        result.fan_steps += 1;
        // Unlike connected-face surface lookup, this scan skips boundary slots.
        let next = mesh.neighbors[current]
            .iter()
            .flatten()
            .copied()
            .find(|&next| next != previous && mesh.triangles[next].contains(&vertex));
        let Some(next) = next else { return Ok(false) };
        previous = current;
        current = next;
        if current == before {
            return Ok(true);
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[2].mul_add(b[2], a[0] * b[0] + a[1] * b[1])
}

#[cfg(test)]
#[path = "../tests/waypoints.rs"]
mod tests;
