//! Ordered optional geometry sphere-tree gate for the independent line query.
use super::{
    coarse::{CoarseLimits, CoarseResult, CoarseSource, MAX_SUPPORTED_FANOUT},
    line_bounds,
};
use anyhow::{Result, ensure};

fn dot(a: [f32; 3], b: [f32; 3]) -> Result<f32> {
    let x = a[0] * b[0];
    let y = a[1] * b[1];
    let xy = x + y;
    let value = a[2].mul_add(b[2], xy);
    ensure!(
        [x, y, xy, value].into_iter().all(f32::is_finite),
        "line coarse dot-product overflow"
    );
    Ok(value)
}

/// Squared distance to the CLOSED segment: endpoint projections do not divide.
/// Interior projection products and subtractions round separately, before the
/// X/Y squares, rounded sum and fused Z-square addition. No request maximum is
/// applied here; callers supply the original full local displacement.
pub fn point_segment_squared_distance(
    point: [f32; 3],
    origin: [f32; 3],
    displacement: [f32; 3],
) -> Result<f32> {
    ensure!(
        point
            .into_iter()
            .chain(origin)
            .chain(displacement)
            .all(f32::is_finite),
        "nonfinite line coarse segment input"
    );
    let mut delta = std::array::from_fn(|axis| point[axis] - origin[axis]);
    ensure!(
        delta.into_iter().all(f32::is_finite),
        "line coarse point subtraction overflow"
    );
    let projection = dot(delta, displacement)?;
    if projection > 0. {
        let length_squared = dot(displacement, displacement)?;
        if projection >= length_squared {
            for axis in 0..3 {
                delta[axis] -= displacement[axis];
            }
        } else {
            let parameter = projection / length_squared;
            ensure!(parameter.is_finite(), "line coarse projection overflow");
            for axis in 0..3 {
                let product = displacement[axis] * parameter;
                ensure!(
                    product.is_finite(),
                    "line coarse projection product overflow"
                );
                delta[axis] -= product;
            }
        }
    }
    ensure!(
        delta.into_iter().all(f32::is_finite),
        "line coarse residual overflow"
    );
    dot(delta, delta)
}

/// Strict sphere gate: tangency rejects. Radius is squared unchanged, including
/// a negative stored radius; the origin has no W component in this predicate.
pub fn sphere_overlaps(sphere: [f32; 4], origin: [f32; 3], displacement: [f32; 3]) -> Result<bool> {
    let distance =
        point_segment_squared_distance([sphere[0], sphere[1], sphere[2]], origin, displacement)?;
    let squared_radius = sphere[3] * sphere[3];
    ensure!(
        sphere[3].is_finite() && squared_radius.is_finite(),
        "nonfinite line coarse radius or square"
    );
    Ok(distance < squared_radius)
}

fn count<S: CoarseSource>(
    source: &mut S,
    node: S::Node,
    result: &mut CoarseResult,
    limit: usize,
) -> Result<u16> {
    ensure!(
        result.node_reads < limit,
        "line coarse node-read budget exhausted"
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

/// Scan all spheres ascending before selected child visits descending. A leaf
/// uses the separate line/AABB SAT gate. Root leaves ignore their stored index.
/// Fanout above sixteen is an explicit unsupported error: native stack masks
/// truncate to u16 while active masks and PPC shifts have different widths.
/// Repeated/cyclic nodes are preserved; exhaustion is an error, never a miss.
pub fn query<S: CoarseSource>(
    source: &mut S,
    root: S::Node,
    origin: [f32; 3],
    displacement: [f32; 3],
    limits: CoarseLimits,
) -> Result<CoarseResult> {
    ensure!(
        [limits.node_reads, limits.sphere_tests]
            .into_iter()
            .all(|limit| limit > 0 && limit <= 1_000_000)
            && limits.fanout > 0
            && limits.fanout <= MAX_SUPPORTED_FANOUT,
        "invalid line coarse tree host budget"
    );
    let mut result = CoarseResult::default();
    if count(source, root, &mut result, limits.node_reads)? == 0 {
        result.leaf_tests += 1;
        result.overlaps =
            line_bounds::segment_overlaps(source.root_bounds()?, origin, displacement)?;
        return Ok(result);
    }
    let mut work = vec![Work::Scan(root)];
    while let Some(item) = work.pop() {
        match item {
            Work::Scan(node) => {
                // Pool index precedes the count reread, including a count that
                // changed to zero since its parent's internal-node precheck.
                let index = source.pool_index(node)?;
                let children = count(source, node, &mut result, limits.node_reads)?;
                ensure!(
                    children <= limits.fanout,
                    "unsupported line coarse fanout {children} exceeds host limit {}",
                    limits.fanout
                );
                let mut mask = 0u16;
                for slot in 0..children {
                    ensure!(
                        result.sphere_tests < limits.sphere_tests,
                        "line coarse sphere-test budget exhausted"
                    );
                    result.sphere_tests += 1;
                    if sphere_overlaps(source.sphere(index, slot)?, origin, displacement)? {
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
                        if line_bounds::segment_overlaps(
                            source.leaf_bounds(index)?,
                            origin,
                            displacement,
                        )? {
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
#[path = "../tests/collision_line_coarse.rs"]
mod tests;
