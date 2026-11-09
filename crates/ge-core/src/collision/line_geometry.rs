//! Ordered line virtual dispatch over mesh, split and linked geometry.
//!
//! One external mutable line request remains shared by the callbacks. Bounds in
//! this traversal are the working local bounds prepared for the current root;
//! mesh hits shrink world bounds, without refreshing these local bounds.
use super::{
    dispatch::{DispatchLimits, GeometrySource, Node},
    line_bounds::overlap,
    world::StoredBounds,
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct LineGeometryRequest {
    pub origin: [f32; 3],
    pub delta: [f32; 3],
    pub bounds: StoredBounds,
    pub optional_context_key: u32,
    pub dispatch_flags: u32,
    pub stop_first: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct LineGeometryOutcome {
    pub returned: bool,
    pub work_used: usize,
    pub nodes_dispatched: usize,
    pub bounds_tests: usize,
    pub bounds_rejections: usize,
    pub coarse_checks: usize,
    pub coarse_tests: usize,
    pub coarse_rejections: usize,
    pub mesh_calls: usize,
    pub link_misses: usize,
}

struct State {
    limits: DispatchLimits,
    outcome: LineGeometryOutcome,
}
impl State {
    fn work(&mut self) -> Result<()> {
        ensure!(
            self.outcome.work_used < self.limits.work_budget,
            "line geometry work budget exhausted"
        );
        self.outcome.work_used += 1;
        Ok(())
    }

    fn overlaps(
        &mut self,
        source: &mut impl GeometrySource,
        node: Node,
        request: &LineGeometryRequest,
    ) -> Result<bool> {
        self.outcome.bounds_tests += 1;
        let stored = source.bounds(node)?;
        let node_bounds = StoredBounds {
            minimum: [stored.minimum[0], stored.minimum[1], stored.minimum[2]],
            maximum: [stored.maximum[0], stored.maximum[1], stored.maximum[2]],
        };
        let passes = overlap(request.bounds, node_bounds)?;
        self.outcome.bounds_rejections += usize::from(!passes);
        Ok(passes)
    }
}

/// Traverse recognized line implementations. `coarse` addresses only a split
/// child's optional metadata: None means absent, Some(false) rejects, and errors
/// remain errors. It runs after child bounds and before optional-context binding.
///
/// Every 0x601 invokes `mesh`, including when incoming flags could bypass mesh
/// gates. That callback must prepare its mode-two resident batch cache BEFORE
/// applying the mesh's own bounds/coarse gates. Mesh success is ORed in storage
/// order; only `stop_first` permits an early return after a success.
pub fn query(
    source: &mut impl GeometrySource,
    root: Node,
    request: &LineGeometryRequest,
    limits: DispatchLimits,
    mut coarse: impl FnMut(Node) -> Result<Option<bool>>,
    mut mesh: impl FnMut(Node, u32, u32) -> Result<bool>,
) -> Result<LineGeometryOutcome> {
    ensure!(
        limits.work_budget > 0
            && limits.work_budget <= 1_000_000
            && limits.depth_budget > 0
            && limits.depth_budget <= 128,
        "invalid line geometry budgets"
    );
    let mut state = State {
        limits,
        outcome: LineGeometryOutcome::default(),
    };
    state.outcome.returned =
        dispatch(source, root, request, &mut state, &mut coarse, &mut mesh, 0)?;
    Ok(state.outcome)
}

fn child_request(
    source: &mut impl GeometrySource,
    child: Node,
    request: &LineGeometryRequest,
    dispatch_flags: u32,
) -> Result<LineGeometryRequest> {
    let optional_context_key = if request.optional_context_key == 0 {
        0
    } else {
        request
            .optional_context_key
            .wrapping_add(u32::from(source.optional_context_index(child)?) * 0x1c)
    };
    Ok(LineGeometryRequest {
        optional_context_key,
        dispatch_flags,
        ..*request
    })
}

fn dispatch(
    source: &mut impl GeometrySource,
    node: Node,
    request: &LineGeometryRequest,
    state: &mut State,
    coarse: &mut impl FnMut(Node) -> Result<Option<bool>>,
    mesh: &mut impl FnMut(Node, u32, u32) -> Result<bool>,
    depth: usize,
) -> Result<bool> {
    ensure!(
        depth < state.limits.depth_budget,
        "line geometry depth budget exhausted"
    );
    state.work()?;
    state.outcome.nodes_dispatched += 1;
    match source.node_type(node)? {
        0x601 => {
            state.outcome.mesh_calls += 1;
            mesh(node, request.optional_context_key, request.dispatch_flags)
        }
        0x603 => {
            // This gate is unconditional, including incoming dispatch bit one.
            if !state.overlaps(source, node, request)? {
                return Ok(false);
            }
            let mut returned = false;
            let mut index = 0u32;
            // Reload at every condition, including the terminating condition.
            while index < source.split_child_count(node)? {
                state.work()?;
                let child = source.split_child(node, index)?;
                if state.overlaps(source, child, request)? {
                    state.outcome.coarse_checks += 1;
                    let passes = match coarse(child)? {
                        None => true,
                        Some(passes) => {
                            state.outcome.coarse_tests += 1;
                            state.outcome.coarse_rejections += usize::from(!passes);
                            passes
                        }
                    };
                    if passes {
                        let next = child_request(source, child, request, 1)?;
                        returned |= dispatch(source, child, &next, state, coarse, mesh, depth + 1)?;
                        if returned && request.stop_first {
                            return Ok(true);
                        }
                    }
                }
                index += 1;
            }
            Ok(returned)
        }
        0x608 => {
            let mut returned = false;
            let mut index = 0u32;
            while index < u32::from(source.link_count(node)?) {
                state.work()?;
                if let Some(child) = source.link_target(node, index as u8)? {
                    let next = child_request(source, child, request, 0)?;
                    returned |= dispatch(source, child, &next, state, coarse, mesh, depth + 1)?;
                    if returned && request.stop_first {
                        return Ok(true);
                    }
                } else {
                    state.outcome.link_misses += 1;
                }
                index += 1;
            }
            Ok(returned)
        }
        kind => bail!("unsupported line geometry type {kind:#x}"),
    }
}

#[cfg(test)]
#[path = "../tests/collision_line_geometry.rs"]
mod tests;
