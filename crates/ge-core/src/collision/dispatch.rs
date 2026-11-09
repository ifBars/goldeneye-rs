//! Confirmed static geometry dispatch over explicit lazy node sources.
//! Collection success means a record was appended, not a physical contact or clearance.
use super::{
    bounds::Bounds,
    collector::{CollectedRecord, CollectionContext, RecordBuffer},
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;

pub type Node = u32;

#[derive(Debug, Clone, Copy)]
pub struct LocalSphere {
    pub center: [f32; 3],
    pub radius: f32,
}
#[derive(Debug, Clone, Copy)]
pub struct QuerySphere {
    pub origin: [f32; 3],
    pub radius: f32,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SphereProbe {
    pub distance_squared: f32,
    pub radius_squared: f32,
    pub passes: bool,
}

/// Source sphere pruning uses a point with W=1, rounded X then fused Y/Z/W.
/// Radius is added without matrix scaling. Squared distance rounds X/Y squares
/// and their sum, then fuses Z-square into that sum. Equality passes.
pub fn probe_sphere(
    matrix: &[f32; 16],
    local: LocalSphere,
    query: QuerySphere,
) -> Result<SphereProbe> {
    ensure!(
        local
            .center
            .into_iter()
            .chain(query.origin)
            .chain([local.radius, query.radius])
            .all(f32::is_finite),
        "nonfinite geometry sphere input"
    );
    let mut delta = [0.; 3];
    for row in 0..3 {
        let coefficients = [
            matrix[row],
            matrix[4 + row],
            matrix[8 + row],
            matrix[12 + row],
        ];
        ensure!(
            coefficients.into_iter().all(f32::is_finite),
            "nonfinite geometry sphere matrix coefficient"
        );
        let first = matrix[row] * local.center[0];
        let second = matrix[4 + row].mul_add(local.center[1], first);
        let third = matrix[8 + row].mul_add(local.center[2], second);
        let world = matrix[12 + row].mul_add(1., third);
        delta[row] = world - query.origin[row];
        ensure!(
            [first, second, third, world, delta[row]]
                .into_iter()
                .all(f32::is_finite),
            "geometry sphere transform/delta overflow"
        );
    }
    let x_square = delta[0] * delta[0];
    let y_square = delta[1] * delta[1];
    let xy = x_square + y_square;
    let distance_squared = delta[2].mul_add(delta[2], xy);
    let radius = query.radius + local.radius;
    let radius_squared = radius * radius;
    ensure!(
        [
            x_square,
            y_square,
            xy,
            distance_squared,
            radius,
            radius_squared
        ]
        .into_iter()
        .all(f32::is_finite),
        "geometry sphere distance/radius overflow"
    );
    Ok(SphereProbe {
        distance_squared,
        radius_squared,
        passes: distance_squared <= radius_squared,
    })
}

/// Opaque identities are supplied by a resident asset/runtime adapter. Each
/// callback addresses only the requested field/entry; it must report corrupt
/// addressed data as an error rather than silently omitting geometry.
pub trait GeometrySource {
    /// Confirmed serialized identity corresponding to the selected virtual implementation.
    fn node_type(&mut self, node: Node) -> Result<u32>;
    fn sphere(&mut self, node: Node) -> Result<LocalSphere>;
    fn bounds(&mut self, node: Node) -> Result<Bounds>;
    fn split_child_count(&mut self, node: Node) -> Result<u32>;
    /// Storage-order relative pointer at split +0x5c + index*4. Null/corrupt is an error.
    fn split_child(&mut self, node: Node, index: u32) -> Result<Node>;
    fn link_count(&mut self, node: Node) -> Result<u8>;
    /// Reference word at link +0x58 + index*0x14, resolved through its owning
    /// resource's sixteen-byte reference directory. A legitimate unresolved
    /// reference is None; this is not the entity-directory selector.
    fn link_target(&mut self, node: Node, index: u8) -> Result<Option<Node>>;
    /// Addressed child's unsigned short at +0x52; unused when incoming context is zero.
    fn optional_context_index(&mut self, node: Node) -> Result<u16>;
}

#[derive(Debug, Clone, Copy)]
pub struct DispatchRequest {
    pub query: QuerySphere,
    pub context: CollectionContext,
    pub optional_context_key: u32,
    /// Native r6 controls traversal pruning; it is distinct from record flags.
    pub traversal_flags: u32,
    /// Native r7 propagates unchanged to each appended record's low sixteen bits.
    pub collection_flags: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct DispatchLimits {
    pub work_budget: usize,
    pub depth_budget: usize,
}
impl Default for DispatchLimits {
    fn default() -> Self {
        Self {
            work_budget: 1_000_000,
            depth_budget: 128,
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct DispatchOutcome {
    pub returned: bool,
    pub work_used: usize,
    pub nodes_dispatched: usize,
    pub sphere_tests: usize,
    pub sphere_rejections: usize,
    pub bounds_tests: usize,
    pub bounds_rejections: usize,
    pub link_misses: usize,
    pub appended: usize,
    pub full_rejections: usize,
}
struct State {
    limits: DispatchLimits,
    outcome: DispatchOutcome,
}
impl State {
    fn work(&mut self) -> Result<()> {
        ensure!(
            self.outcome.work_used < self.limits.work_budget,
            "geometry-dispatch work budget exhausted"
        );
        self.outcome.work_used += 1;
        Ok(())
    }
    fn sphere(
        &mut self,
        source: &mut impl GeometrySource,
        node: Node,
        request: &DispatchRequest,
    ) -> Result<bool> {
        self.outcome.sphere_tests += 1;
        let passes =
            probe_sphere(&request.context.matrix, source.sphere(node)?, request.query)?.passes;
        self.outcome.sphere_rejections += usize::from(!passes);
        Ok(passes)
    }
}

/// Dispatch recognized mesh/split/link implementations in storage order. There
/// is no global visited set, deduplication, or early-out on success/full buffers.
/// Host errors retain any already appended prefix and must not become clearance.
pub fn collect(
    source: &mut impl GeometrySource,
    root: Node,
    request: &DispatchRequest,
    records: &mut RecordBuffer,
    limits: DispatchLimits,
) -> Result<DispatchOutcome> {
    ensure!(
        limits.work_budget > 0
            && limits.work_budget <= 1_000_000
            && limits.depth_budget > 0
            && limits.depth_budget <= 128,
        "invalid geometry-dispatch budgets"
    );
    let mut state = State {
        limits,
        outcome: DispatchOutcome::default(),
    };
    state.outcome.returned = dispatch(source, root, request, records, &mut state, 0)?;
    Ok(state.outcome)
}

fn child_request(
    source: &mut impl GeometrySource,
    node: Node,
    request: &DispatchRequest,
    traversal_flags: u32,
) -> Result<DispatchRequest> {
    let optional_context_key = if request.optional_context_key == 0 {
        0
    } else {
        request
            .optional_context_key
            .wrapping_add(u32::from(source.optional_context_index(node)?) * 0x1c)
    };
    Ok(DispatchRequest {
        optional_context_key,
        traversal_flags,
        ..*request
    })
}

fn dispatch(
    source: &mut impl GeometrySource,
    node: Node,
    request: &DispatchRequest,
    records: &mut RecordBuffer,
    state: &mut State,
    depth: usize,
) -> Result<bool> {
    ensure!(
        depth < state.limits.depth_budget,
        "geometry-dispatch depth budget exhausted"
    );
    state.work()?;
    state.outcome.nodes_dispatched += 1;
    match source.node_type(node)? {
        0x601 => {
            if request.traversal_flags & 3 == 0 {
                if request.collection_flags & 2 == 0 && !state.sphere(source, node, request)? {
                    return Ok(false);
                }
                state.outcome.bounds_tests += 1;
                let passes = source
                    .bounds(node)?
                    .transformed(&request.context.matrix)?
                    .contains_with_margin(request.query.origin, request.query.radius)?;
                if !passes {
                    state.outcome.bounds_rejections += 1;
                    return Ok(false);
                }
            }
            let appended = records.append(|| {
                Ok(CollectedRecord::from_dispatch(
                    &request.context,
                    node,
                    request.optional_context_key,
                    request.collection_flags,
                ))
            })?;
            if appended {
                state.outcome.appended += 1;
            } else {
                state.outcome.full_rejections += 1;
            }
            Ok(appended)
        }
        0x603 => {
            if request.traversal_flags & 3 == 0 && !state.sphere(source, node, request)? {
                return Ok(false);
            }
            let mut returned = false;
            let mut index = 0u32;
            while index < source.split_child_count(node)? {
                state.work()?;
                let child = source.split_child(node, index)?;
                if request.traversal_flags & 1 != 0 || state.sphere(source, child, request)? {
                    let next = child_request(source, child, request, request.traversal_flags | 2)?;
                    // Do not short-circuit subsequent calls after an earlier true result.
                    let result = dispatch(source, child, &next, records, state, depth + 1)?;
                    returned |= result;
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
                    let next = child_request(source, child, request, request.traversal_flags)?;
                    let result = dispatch(source, child, &next, records, state, depth + 1)?;
                    returned |= result;
                } else {
                    state.outcome.link_misses += 1;
                }
                index += 1;
            }
            Ok(returned)
        }
        kind => bail!("unsupported geometry-dispatch type {kind:#x}"),
    }
}

#[cfg(test)]
#[path = "../tests/collision_dispatch.rs"]
mod tests;
