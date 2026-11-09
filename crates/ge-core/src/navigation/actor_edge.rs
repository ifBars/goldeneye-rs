//! Actor target-directed edge predicate, independently authored from static behavior.
use super::TraversalEdge;
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Fixed 110-degree engine cosine, statically evaluated under round-to-nearest.
/// This differs by one ULP from standard cosine. Original FP mode remains unverified.
pub const ACTOR_EDGE_DOT_MIN: f32 = f32::from_bits(0xbeaf1d42);
pub struct ActorEdgeMesh<'a> {
    pub neighbors: &'a [[Option<usize>; 3]],
    pub centers: &'a [[f32; 3]],
    pub flags: &'a [Option<u32>],
    pub normals: &'a [Option<[f32; 3]>],
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActorEdgeDecision {
    ImmediateBacktrack,
    FaceFlags { flags_raw: u32 },
    NormalY { value: f32 },
    Direction { dot: f32, allowed: bool },
}
impl ActorEdgeDecision {
    pub fn allowed(&self) -> bool {
        matches!(self, Self::Direction { allowed: true, .. })
    }
}

/// Preserve rejection order. Only the destination's low two flags and stored
/// normal Y are consulted; missing normal data skips that gate. Positions are raw
/// mesh/context values: there is no frame transform or horizontal flattening.
pub fn actor_target_edge(
    mesh: ActorEdgeMesh<'_>,
    edge: TraversalEdge,
    point: [f32; 3],
) -> Result<ActorEdgeDecision> {
    let actual = mesh
        .neighbors
        .get(edge.from_triangle)
        .and_then(|row| row.get(edge.slot))
        .context("actor edge source/slot out of bounds")?;
    ensure!(
        *actual == Some(edge.to_triangle),
        "actor edge destination disagrees with neighbor slot"
    );
    if Some(edge.to_triangle) == edge.previous_triangle {
        return Ok(ActorEdgeDecision::ImmediateBacktrack);
    }
    let flags = mesh
        .flags
        .get(edge.to_triangle)
        .context("actor edge flags out of bounds")?
        .unwrap_or(0);
    if flags & 3 != 0 {
        return Ok(ActorEdgeDecision::FaceFlags { flags_raw: flags });
    }
    if let Some(normal) = mesh
        .normals
        .get(edge.to_triangle)
        .context("actor edge normal out of bounds")?
    {
        ensure!(normal[1].is_finite(), "nonfinite actor edge normal Y");
        if normal[1] < 0.9 {
            return Ok(ActorEdgeDecision::NormalY { value: normal[1] });
        }
    }
    let from = *mesh
        .centers
        .get(edge.from_triangle)
        .context("actor edge source center out of bounds")?;
    let to = *mesh
        .centers
        .get(edge.to_triangle)
        .context("actor edge destination center out of bounds")?;
    ensure!(
        from.into_iter().chain(to).chain(point).all(f32::is_finite),
        "nonfinite actor edge position"
    );
    let outgoing = normalize(std::array::from_fn(|i| to[i] - from[i]))?;
    let target = normalize(std::array::from_fn(|i| point[i] - from[i]))?;
    let dot = dot(outgoing, target)?;
    Ok(direction_decision(dot))
}
fn direction_decision(dot: f32) -> ActorEdgeDecision {
    ActorEdgeDecision::Direction {
        dot,
        allowed: dot > ACTOR_EDGE_DOT_MIN,
    }
}
fn dot(a: [f32; 3], b: [f32; 3]) -> Result<f32> {
    let value = a[2].mul_add(b[2], a[0] * b[0] + a[1] * b[1]);
    ensure!(
        value.is_finite(),
        "actor edge direction arithmetic overflow"
    );
    Ok(value)
}
fn normalize(mut v: [f32; 3]) -> Result<[f32; 3]> {
    let squared = dot(v, v)?;
    let length = if squared == 0. { 0. } else { squared.sqrt() }; // Wii estimate/refinement precision is provisional.
    if length > 1e-5 {
        let scale = 1. / length;
        v = v.map(|x| x * scale);
    }
    ensure!(
        v.into_iter().all(f32::is_finite),
        "actor edge normalization overflow"
    );
    Ok(v)
}
#[cfg(test)]
#[path = "../tests/actor_edge.rs"]
mod tests;
