//! Native ordered coarse sphere-tree overlap for supported fanout at most sixteen.
//! Collected-record flags and absent-tree policy remain in the caller.
use super::world::StoredBounds;
use anyhow::{Result, ensure};
use serde::Serialize;

pub const MAX_SUPPORTED_FANOUT: u16 = 16;
pub trait CoarseSource {
    type Node: Copy;
    fn node_count(&mut self, node: Self::Node) -> Result<u16>;
    fn pool_index(&mut self, node: Self::Node) -> Result<u16>;
    fn sphere(&mut self, index: u16, slot: u16) -> Result<[f32; 4]>;
    fn child_target(&mut self, node: Self::Node, slot: u16) -> Result<Self::Node>;
    fn root_bounds(&mut self) -> Result<StoredBounds>;
    fn leaf_bounds(&mut self, index: u16) -> Result<StoredBounds>;
}
#[derive(Debug, Clone, Copy)]
pub struct CoarseLimits {
    /// Node-count reads, including the internal precheck and scan reread.
    pub node_reads: usize,
    pub sphere_tests: usize,
    /// Greater fanout has asymmetric native u32/u16 mask behavior, unsupported here.
    pub fanout: u16,
}
#[derive(Debug, Default, Serialize)]
pub struct CoarseResult {
    pub overlaps: bool,
    pub node_reads: usize,
    pub sphere_tests: usize,
    pub leaf_tests: usize,
    pub maximum_fanout: u16,
}

/// Sphere W is radius, while origin W is subtracted before query-radius addition.
/// Do not clamp a negative effective radius: its rounded square is still tested.
pub fn sphere_overlaps(sphere: [f32; 4], origin: [f32; 4], radius: f32) -> Result<bool> {
    ensure!(
        sphere
            .into_iter()
            .chain(origin)
            .chain([radius])
            .all(f32::is_finite),
        "nonfinite coarse sphere input"
    );
    let delta = std::array::from_fn::<_, 4, _>(|i| sphere[i] - origin[i]);
    let x = delta[0] * delta[0];
    let y = delta[1] * delta[1];
    let xy = x + y;
    let distance = delta[2].mul_add(delta[2], xy);
    let effective = radius + delta[3];
    let limit = effective * effective;
    ensure!(
        delta
            .into_iter()
            .chain([x, y, xy, distance, effective, limit])
            .all(f32::is_finite),
        "coarse sphere arithmetic overflow"
    );
    Ok(distance <= limit)
}
fn count<S: CoarseSource>(
    source: &mut S,
    node: S::Node,
    result: &mut CoarseResult,
    limit: usize,
) -> Result<u16> {
    ensure!(
        result.node_reads < limit,
        "coarse node-read budget exhausted"
    );
    result.node_reads += 1;
    let count = source.node_count(node)?;
    result.maximum_fanout = result.maximum_fanout.max(count);
    Ok(count)
}
enum Work<N> {
    Scan(N),
    Descend { node: N, mask: u16, next: u16 },
}

/// Scan every sphere in ascending order before visiting selected children in
/// descending order. Shared/cyclic nodes are not globally deduplicated. Missing
/// trees are an adapter decision; this function only accepts an established root.
pub fn query<S: CoarseSource>(
    source: &mut S,
    root: S::Node,
    origin: [f32; 4],
    radius: f32,
    limits: CoarseLimits,
) -> Result<CoarseResult> {
    ensure!(
        [limits.node_reads, limits.sphere_tests]
            .into_iter()
            .all(|v| v > 0 && v <= 1_000_000)
            && limits.fanout > 0
            && limits.fanout <= MAX_SUPPORTED_FANOUT,
        "invalid coarse tree host budget"
    );
    let mut result = CoarseResult::default();
    if count(source, root, &mut result, limits.node_reads)? == 0 {
        result.leaf_tests += 1;
        result.overlaps = source
            .root_bounds()?
            .contains_with_margin([origin[0], origin[1], origin[2]], radius)?;
        return Ok(result);
    }
    let mut work = vec![Work::Scan(root)];
    while let Some(item) = work.pop() {
        match item {
            Work::Scan(node) => {
                // Native entry reads pool index before reloading child count.
                let index = source.pool_index(node)?;
                let children = count(source, node, &mut result, limits.node_reads)?;
                ensure!(
                    children <= limits.fanout,
                    "unsupported coarse fanout {children} exceeds host limit {}",
                    limits.fanout
                );
                let mut mask = 0u16;
                for slot in 0..children {
                    ensure!(
                        result.sphere_tests < limits.sphere_tests,
                        "coarse sphere-test budget exhausted"
                    );
                    result.sphere_tests += 1;
                    if sphere_overlaps(source.sphere(index, slot)?, origin, radius)? {
                        mask |= 1u16 << slot;
                    }
                }
                if mask != 0 {
                    work.push(Work::Descend {
                        node,
                        mask,
                        next: children,
                    });
                }
            }
            Work::Descend {
                node,
                mask,
                mut next,
            } => {
                while next > 0 {
                    next -= 1;
                    if mask & (1u16 << next) == 0 {
                        continue;
                    }
                    let child = source.child_target(node, next)?;
                    if count(source, child, &mut result, limits.node_reads)? == 0 {
                        let index = source.pool_index(child)?;
                        result.leaf_tests += 1;
                        if source
                            .leaf_bounds(index)?
                            .contains_with_margin([origin[0], origin[1], origin[2]], radius)?
                        {
                            result.overlaps = true;
                            return Ok(result);
                        }
                    } else {
                        if next > 0 {
                            work.push(Work::Descend { node, mask, next });
                        }
                        work.push(Work::Scan(child));
                        break;
                    }
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../tests/collision_coarse.rs"]
mod tests;
