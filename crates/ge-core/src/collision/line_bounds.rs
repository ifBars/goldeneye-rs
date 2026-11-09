//! Ordered line/AABB gates and endpoint bounds from the independent line path.
use super::world::StoredBounds;
use anyhow::{Result, ensure};

/// 80293110: maximum XYZ against the other minimum, then minimum XYZ against
/// the other maximum. Ties overlap; later components remain unused on rejection.
pub fn overlap(a: StoredBounds, b: StoredBounds) -> Result<bool> {
    for (maximum, minimum) in a.maximum.into_iter().zip(b.minimum) {
        ensure!(
            maximum.is_finite() && minimum.is_finite(),
            "nonfinite line bound overlap"
        );
        if maximum < minimum {
            return Ok(false);
        }
    }
    for (minimum, maximum) in a.minimum.into_iter().zip(b.maximum) {
        ensure!(
            minimum.is_finite() && maximum.is_finite(),
            "nonfinite line bound overlap"
        );
        if minimum > maximum {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Packed hierarchy helper 802cb6a0. This is a point/face test with a direction
/// epsilon, deliberately distinct from the placement/leaf segment SAT below.
pub fn packed_segment_overlaps(
    bounds: StoredBounds,
    origin: [f32; 3],
    delta: [f32; 3],
) -> Result<bool> {
    if bounds.contains(origin)? {
        return Ok(true);
    }
    let epsilon = f32::from_bits(0x3727_c5ac);
    for (axis, others) in [[2, 1], [2, 0], [1, 0]].into_iter().enumerate() {
        ensure!(delta[axis].is_finite(), "nonfinite packed line direction");
        if delta[axis].abs() < epsilon {
            continue;
        }
        for face in [bounds.minimum[axis], bounds.maximum[axis]] {
            ensure!(
                face.is_finite() && origin[axis].is_finite(),
                "nonfinite packed line face"
            );
            let numerator = -origin[axis] + face;
            let parameter = numerator / delta[axis];
            ensure!(
                [numerator, parameter].into_iter().all(f32::is_finite),
                "packed line face parameter overflow"
            );
            if !(0. ..=1.).contains(&parameter) {
                continue;
            }
            let [a, b] = others;
            let product_a = parameter * delta[a];
            let product_b = parameter * delta[b];
            let point_a = origin[a] + product_a;
            let point_b = origin[b] + product_b;
            ensure!(
                [product_a, product_b, point_a, point_b]
                    .into_iter()
                    .all(f32::is_finite),
                "packed line face point overflow"
            );
            ensure!(
                bounds.minimum[a].is_finite() && bounds.maximum[a].is_finite(),
                "nonfinite packed line face bounds"
            );
            if point_a < bounds.minimum[a] || point_a > bounds.maximum[a] {
                continue;
            }
            ensure!(
                bounds.minimum[b].is_finite() && bounds.maximum[b].is_finite(),
                "nonfinite packed line face bounds"
            );
            if point_b >= bounds.minimum[b] && point_b <= bounds.maximum[b] {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Constructor/world and local refresh helpers multiply delta by the maximum,
/// then add origin. Preserve separate operations and endpoint-min/origin-max ties.
/// Callers retain the constructor's world bounds across later delta/max changes.
pub fn endpoint_bounds(origin: [f32; 3], delta: [f32; 3], maximum: f32) -> Result<StoredBounds> {
    ensure!(
        origin
            .into_iter()
            .chain(delta)
            .chain([maximum])
            .all(f32::is_finite),
        "nonfinite line endpoint input"
    );
    let mut bounds = StoredBounds {
        minimum: [0.; 3],
        maximum: [0.; 3],
    };
    for axis in 0..3 {
        let product = delta[axis] * maximum;
        let endpoint = origin[axis] + product;
        let difference = origin[axis] - endpoint;
        ensure!(
            [product, endpoint, difference]
                .into_iter()
                .all(f32::is_finite),
            "line endpoint bound overflow"
        );
        if difference >= 0. {
            bounds.minimum[axis] = endpoint;
            bounds.maximum[axis] = origin[axis];
        } else {
            bounds.minimum[axis] = origin[axis];
            bounds.maximum[axis] = endpoint;
        }
    }
    Ok(bounds)
}

/// 802931b0 segment SAT, distinct from the packed-hierarchy face-test helper.
/// Uses the full supplied displacement; no request maximum or radius is inferred.
pub fn segment_overlaps(bounds: StoredBounds, origin: [f32; 3], delta: [f32; 3]) -> Result<bool> {
    ensure!(
        bounds
            .minimum
            .into_iter()
            .chain(bounds.maximum)
            .chain(origin)
            .chain(delta)
            .all(f32::is_finite),
        "nonfinite segment/AABB input"
    );
    let extent = std::array::from_fn::<_, 3, _>(|i| (bounds.maximum[i] - bounds.minimum[i]) * 0.5);
    let center = std::array::from_fn::<_, 3, _>(|i| extent[i] + bounds.minimum[i]);
    let half = delta.map(|d| d * 0.5);
    // Source first doubles origin with a fused delta add, then uses a fused
    // half/subtract. Replacing this with origin+half-center changes rounding.
    let relative = std::array::from_fn::<_, 3, _>(|i| {
        origin[i].mul_add(2., delta[i]).mul_add(0.5, -center[i])
    });
    ensure!(
        extent
            .into_iter()
            .chain(center)
            .chain(half)
            .chain(relative)
            .all(f32::is_finite),
        "segment/AABB midpoint overflow"
    );
    let absolute = half.map(f32::abs);
    for axis in 0..3 {
        let limit = extent[axis] + absolute[axis];
        ensure!(limit.is_finite(), "segment/AABB axis bound overflow");
        if relative[axis].abs() > limit {
            return Ok(false);
        }
    }
    let x_a = relative[2] * half[1];
    let x_b = half[2] * relative[1];
    let x = x_a - x_b;
    let x_base = absolute[2] * extent[1];
    let x_limit = extent[2].mul_add(absolute[1], x_base);
    ensure!(
        [x_a, x_b, x, x_base, x_limit]
            .into_iter()
            .all(f32::is_finite),
        "segment/AABB cross-X overflow"
    );
    if x.abs() > x_limit {
        return Ok(false);
    }
    let y_base = half[0] * relative[2];
    let y = half[2].mul_add(relative[0], -y_base);
    let y_bound_base = extent[0] * absolute[2];
    let y_limit = extent[2].mul_add(absolute[0], y_bound_base);
    ensure!(
        [y_base, y, y_bound_base, y_limit]
            .into_iter()
            .all(f32::is_finite),
        "segment/AABB cross-Y overflow"
    );
    if y.abs() > y_limit {
        return Ok(false);
    }
    let z_a = half[0] * relative[1];
    let z_b = relative[0] * half[1];
    let z = z_a - z_b;
    let z_base = extent[0] * absolute[1];
    let z_limit = absolute[0].mul_add(extent[1], z_base);
    ensure!(
        [z_a, z_b, z, z_base, z_limit]
            .into_iter()
            .all(f32::is_finite),
        "segment/AABB cross-Z overflow"
    );
    Ok(z.abs() <= z_limit)
}

#[cfg(test)]
#[path = "../tests/collision_line_bounds.rs"]
mod tests;
