//! Candidate proximity and ordered face-walk goal test from static SJBE52 evidence.
use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct ActorGoalMesh<'a> {
    pub vertices: &'a [[f32; 3]],
    pub triangles: &'a [[usize; 3]],
    pub centers: &'a [[f32; 3]],
    pub neighbors: &'a [[Option<usize>; 3]],
    pub flags: &'a [Option<u32>],
    pub normals: &'a [Option<[f32; 3]>],
}
/// Raw mesh/context coordinates. Only the scratch center changes during a test.
pub struct ActorGoalContext {
    pub origin: [f32; 3],
    pub point: [f32; 3],
    pub start_triangle: usize,
    pub last_center: Option<[f32; 3]>,
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActorGoalDecision {
    TooFar { squared_distance: f32 },
    NormalY { triangle: usize, value: f32 },
    NoNeighbor { triangle: usize, edge: [usize; 2] },
    Accepted,
}
#[derive(Debug, Serialize)]
pub struct ActorGoalResult {
    pub candidate: usize,
    pub raw_center: [f32; 3],
    pub walked_faces: Vec<usize>,
    pub edge_tests: usize,
    pub decision: ActorGoalDecision,
}
impl ActorGoalResult {
    pub fn accepted(&self) -> bool {
        self.decision == ActorGoalDecision::Accepted
    }
}

/// First require full-3D squared distance <=4, then walk toward the candidate's
/// center from the stored start face. Completion is checked AFTER a neighbor
/// transition, never on loop entry. Missing crossings and host-budget exhaustion
/// are errors; do not turn the source's invalid reads/unbounded work into misses.
pub fn actor_target_goal(
    mesh: ActorGoalMesh<'_>,
    context: &mut ActorGoalContext,
    candidate: usize,
    max_face_steps: usize,
) -> Result<ActorGoalResult> {
    ensure!(
        max_face_steps > 0,
        "actor goal face budget must be positive"
    );
    let center = *mesh
        .centers
        .get(candidate)
        .context("actor goal candidate center out of bounds")?;
    ensure!(
        center.into_iter().chain(context.point).all(f32::is_finite),
        "nonfinite actor goal center/point"
    );
    context.last_center = Some(center);
    let squared = dot(sub(context.point, center), sub(context.point, center))?;
    let mut result = ActorGoalResult {
        candidate,
        raw_center: center,
        walked_faces: Vec::new(),
        edge_tests: 0,
        decision: ActorGoalDecision::TooFar {
            squared_distance: squared,
        },
    };
    if squared > 4. {
        return Ok(result);
    }
    ensure!(
        context.origin.into_iter().all(f32::is_finite),
        "nonfinite actor goal origin"
    );
    let mut direction = sub(center, context.origin);
    direction[1] = 0.;
    let side = cross_up(direction)?;
    let (mut face, mut previous) = (context.start_triangle, None);
    loop {
        ensure!(
            result.walked_faces.len() < max_face_steps,
            "actor goal face budget exhausted"
        );
        result.walked_faces.push(face);
        if let Some(normal) = mesh
            .normals
            .get(face)
            .context("actor goal normal out of bounds")?
        {
            ensure!(normal[1].is_finite(), "nonfinite actor goal normal Y");
            if normal[1] < 0.9 {
                result.decision = ActorGoalDecision::NormalY {
                    triangle: face,
                    value: normal[1],
                };
                return Ok(result);
            }
        }
        let ids = mesh
            .triangles
            .get(face)
            .context("actor goal face out of bounds")?;
        let mut selected = None;
        let mut forward = 0.;
        for slot in 0..3 {
            result.edge_tests += 1;
            let a = *mesh
                .vertices
                .get(ids[slot])
                .context("actor goal vertex out of bounds")?;
            let b = *mesh
                .vertices
                .get(ids[(slot + 1) % 3])
                .context("actor goal vertex out of bounds")?;
            ensure!(
                a.into_iter().chain(b).all(f32::is_finite),
                "nonfinite actor goal vertex"
            );
            let edge_side = cross_up(sub(b, a))?;
            let determinant = dot(direction, edge_side)?;
            if determinant.abs() < 0.001 {
                continue;
            }
            let offset = sub(a, context.origin);
            let along_edge = dot(offset, side)? / determinant;
            ensure!(along_edge.is_finite(), "actor goal edge parameter overflow");
            if (along_edge - 0.5).abs() > 0.5 {
                continue;
            }
            let along_ray = dot(offset, edge_side)? / determinant;
            ensure!(
                along_ray.is_finite(),
                "actor goal forward parameter overflow"
            );
            if along_ray <= forward {
                continue;
            }
            let already_selected = selected.is_some();
            selected = Some(slot);
            // The first face takes its first crossing. Later faces keep their
            // first crossing unless a subsequent greater parameter ends the scan.
            if face == context.start_triangle || already_selected {
                break;
            }
            forward = along_ray;
        }
        let slot = selected.context("actor goal has no supported forward crossing; original would address before face records")?;
        let edge = [ids[slot], ids[(slot + 1) % 3]];
        let mut next = None;
        for neighbor in mesh
            .neighbors
            .get(face)
            .context("actor goal neighbors out of bounds")?
        {
            let Some(neighbor) = *neighbor else {
                continue;
            };
            if Some(neighbor) == previous {
                continue;
            }
            if mesh
                .flags
                .get(neighbor)
                .context("actor goal neighbor flags out of bounds")?
                .unwrap_or(0)
                & 1
                != 0
            {
                continue;
            }
            let neighbor_ids = mesh
                .triangles
                .get(neighbor)
                .context("actor goal neighbor face out of bounds")?;
            // Count vertex occurrences, not a deduplicated set or an edge-slot match.
            if neighbor_ids.iter().filter(|&&v| edge.contains(&v)).count() == 2 {
                next = Some(neighbor);
                break;
            }
        }
        let Some(next) = next else {
            result.decision = ActorGoalDecision::NoNeighbor {
                triangle: face,
                edge,
            };
            return Ok(result);
        };
        previous = Some(face);
        face = next;
        if face == candidate {
            result.walked_faces.push(face);
            result.decision = ActorGoalDecision::Accepted;
            return Ok(result);
        }
    }
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> Result<f32> {
    let result = a[2].mul_add(b[2], a[0] * b[0] + a[1] * b[1]);
    ensure!(result.is_finite(), "actor goal dot arithmetic overflow");
    Ok(result)
}
fn cross_up(v: [f32; 3]) -> Result<[f32; 3]> {
    let result = [
        0.0f32.mul_add(v[1], -(v[2] * 1.)),
        v[2].mul_add(0., -(0. * v[0])),
        v[0].mul_add(1., -(0. * v[1])),
    ];
    ensure!(
        result.into_iter().all(f32::is_finite),
        "actor goal cross arithmetic overflow"
    );
    Ok(result)
}
#[cfg(test)]
#[path = "../tests/actor_goal.rs"]
mod tests;
