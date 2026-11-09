//! Target-action caller binding from static evidence; external action gate and later mutation are separate.
use super::{
    NavigationFrame, SegmentSearch, SegmentSearchResult, SurfaceHit, SurfaceRay, SurfaceTriangle,
    connected_segment_hit_with_fallible_policy, first_surface_hit, manager_navigation_edge,
};
use anyhow::{Result, ensure};
use serde::Serialize;

pub struct ActorTargetMesh<'a> {
    pub triangles: &'a [SurfaceTriangle],
    pub neighbors: &'a [[Option<usize>; 3]],
    pub flags: &'a [Option<u32>],
}
#[derive(Clone, Copy, Serialize)]
pub struct ActorTargetRequest {
    pub target: [f32; 3],
    pub active_position: [f32; 3],
    pub owner_frame: NavigationFrame,
    pub max_face_tests: usize,
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorTargetDecision {
    MissingBinding,
    NoTargetFace,
    TargetHeight,
    BindingLost,
    NoActiveFace,
    Ready,
}
#[derive(Clone, Copy, Serialize)]
pub struct ActorTargetPrepared {
    pub start_triangle: usize,
    /// Unraised active position minus the SECOND binding's translation.
    pub origin: [f32; 3],
    pub point: [f32; 3],
    pub frame: NavigationFrame,
}
#[derive(Serialize)]
pub struct ActorTargetBinding {
    pub decision: ActorTargetDecision,
    pub local_owner_target: [f32; 3],
    pub target_ray: Option<SurfaceRay>,
    pub target_hit: Option<SurfaceHit>,
    pub active_segment: Option<SegmentSearch>,
    pub active_lookup: Option<SegmentSearchResult>,
    pub prepared: Option<ActorTargetPrepared>,
}

/// Run AFTER the caller's separate external action predicate. Retain the first
/// mesh across a lazy second binding/frame lookup. Do not infer a common frame,
/// replace storage-first with nearest, or snap either policy point to its hit.
pub fn prepare_actor_target(
    request: ActorTargetRequest,
    mesh: Option<ActorTargetMesh<'_>>,
    resolve_frame: impl FnOnce() -> Result<Option<NavigationFrame>>,
) -> Result<ActorTargetBinding> {
    ensure!(
        request.max_face_tests > 0,
        "actor target face-test budget must be positive"
    );
    let local = request.owner_frame.position_to_local(request.target)?;
    let mut result = ActorTargetBinding {
        decision: ActorTargetDecision::MissingBinding,
        local_owner_target: local,
        target_ray: None,
        target_hit: None,
        active_segment: None,
        active_lookup: None,
        prepared: None,
    };
    let Some(mesh) = mesh else {
        return Ok(result);
    };
    let ray = SurfaceRay {
        origin: [local[0] + 0., local[1] + 1., local[2] + 0.],
        direction: [-0., -1., -0.],
        max_parameter: 1.25,
    };
    result.target_ray = Some(ray);
    result.target_hit = first_surface_hit(ray, mesh.triangles.iter().copied())?;
    if result.target_hit.is_none() {
        result.decision = ActorTargetDecision::NoTargetFace;
        return Ok(result);
    }
    if local[1].abs() >= 1. {
        result.decision = ActorTargetDecision::TargetHeight;
        return Ok(result);
    }
    let Some(frame) = resolve_frame()? else {
        result.decision = ActorTargetDecision::BindingLost;
        return Ok(result);
    };
    let origin = frame.position_to_local(request.active_position)?;
    let mut raised = origin;
    raised[1] += 1.;
    let segment = SegmentSearch {
        origin: raised,
        displacement: [0., -2., 0.],
        start_triangle: result.target_hit.as_ref().unwrap().triangle,
        previous_triangle: None,
        max_depth: 5,
        max_face_tests: request.max_face_tests,
    };
    result.active_segment = Some(segment);
    let connected = connected_segment_hit_with_fallible_policy(
        segment,
        mesh.triangles,
        mesh.neighbors,
        |edge| manager_navigation_edge(mesh.neighbors, mesh.flags, edge),
    )?;
    if let Some(hit) = connected.hit {
        let point = frame.position_to_local(request.target)?;
        result.decision = ActorTargetDecision::Ready;
        result.prepared = Some(ActorTargetPrepared {
            start_triangle: hit.triangle,
            origin,
            point,
            frame,
        });
    } else {
        result.decision = ActorTargetDecision::NoActiveFace;
    }
    result.active_lookup = Some(connected);
    Ok(result)
}
#[cfg(test)]
#[path = "../tests/actor_target.rs"]
mod tests;
