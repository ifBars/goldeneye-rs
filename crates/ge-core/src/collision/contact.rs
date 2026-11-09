//! Native face clipping / weighted representative points and infinite edge lines.
use super::{BoxTriangleCandidate, BoxTriangleContact, OrientedBox, math::*};
use anyhow::{Result, ensure};

fn plane_distance(point: V, normal: V, offset: f32) -> Result<f32> {
    // Scalar clip helper rounds each product and sums X/Y, then Z, then offset.
    let value = ((normal[0] * point[0] + normal[1] * point[1]) + normal[2] * point[2]) + offset;
    ensure!(value.is_finite(), "collision clip-plane overflow");
    Ok(value)
}
fn clip(points: Vec<V>, normal: V, offset: f32) -> Result<Vec<V>> {
    let Some(&last) = points.last() else {
        return Ok(Vec::new());
    };
    let mut output = Vec::with_capacity(9);
    let mut previous = last;
    let mut previous_distance = plane_distance(previous, normal, offset)?;
    for current in points {
        let current_distance = plane_distance(current, normal, offset)?;
        // Append the previous vertex first. Rotating/canonicalizing the polygon
        // changes the later rounded weighted accumulation order.
        if previous_distance >= 0. {
            output.push(previous);
        }
        if (previous_distance > 0. && current_distance < 0.)
            || (previous_distance < 0. && current_distance > 0.)
        {
            let denominator = previous_distance - current_distance;
            let p = std::array::from_fn(|i| {
                previous[i] - (previous_distance * (previous[i] - current[i])) / denominator
            });
            finite(p)?;
            output.push(p);
        }
        ensure!(
            output.len() <= 9,
            "collision clip polygon exceeds native nine-vertex capacity"
        );
        previous = current;
        previous_distance = current_distance;
    }
    Ok(output)
}
fn weighted(points: &[V], normal: V, offset: f32) -> Result<Option<V>> {
    let mut sum = [0.; 3];
    let mut total = 0f32;
    for &p in points {
        let weight = -(dot(normal, p) - offset);
        ensure!(weight.is_finite(), "collision contact weight overflow");
        if weight >= 0. {
            sum = add(sum, scale(p, weight));
            total += weight;
        }
    }
    finite(sum)?;
    ensure!(total.is_finite(), "collision contact weight sum overflow");
    if total < f32::EPSILON {
        return Ok(None);
    }
    let mean = scale(sum, 1. / total);
    finite(mean)?;
    Ok(Some(mean))
}
fn edge_point(
    shape: &OrientedBox,
    t: &Triangle,
    c: &BoxTriangleCandidate,
    axes: [V; 3],
) -> Result<V> {
    let mut support = shape.center;
    for (i, axis) in axes.iter().enumerate() {
        let sign = if dot(c.normal, *axis) > 0. { 1. } else { -1. };
        let signed_extent = sign * shape.half_extents[i];
        support = add(support, scale(*axis, signed_extent));
    }
    finite(support)?;
    let index = usize::from(c.axis - 5);
    let box_axis = index / 3;
    let points = t.points()?;
    let (anchor, direction) = match index % 3 {
        0 => (points[0], t.edges[0]),
        1 => (points[2], t.edges[1]),
        _ => (points[1], sub(t.edges[1], t.edges[0])),
    };
    let u = normalize(axes[box_axis])?;
    let v = normalize(direction)?;
    let d = sub(support, anchor);
    let a = dot3(u, u);
    let b = -dot3(u, v);
    let z = dot3(v, v);
    let determinant = (a * z - b * b).abs();
    let f = dot3(d, u);
    let parameter = if determinant >= 1e-5 {
        let e = -dot3(d, v);
        let inverse = 1. / determinant;
        inverse * (b * f - a * e)
    } else {
        0.
    };
    ensure!(
        [a, b, z, determinant, f, parameter]
            .into_iter()
            .all(f32::is_finite),
        "collision edge-line parameter overflow"
    );
    let point = add(anchor, scale(v, parameter));
    finite(point)?;
    Ok(point)
}

pub(super) fn build(
    shape: &OrientedBox,
    t: &Triangle,
    c: &BoxTriangleCandidate,
) -> Result<Option<BoxTriangleContact>> {
    if c.axis == 0 {
        return Ok(None);
    }
    ensure!((1..=13).contains(&c.axis), "invalid collision contact axis");
    let axes = [shape.axis(0)?, shape.axis(1)?, shape.axis(2)?];
    let (point, depth) = if c.axis > 4 {
        (edge_point(shape, t, c, axes)?, c.depth)
    } else if c.axis == 1 {
        let reference = neg(c.normal);
        let projections = axes.map(|axis| dot(reference, axis));
        let absolute = projections.map(f32::abs);
        let (u, face, v) = if absolute[1] > absolute[0] {
            if absolute[1] > absolute[2] {
                (0, 1, 2)
            } else {
                (0, 2, 1)
            }
        } else if absolute[0] > absolute[2] {
            (1, 0, 2)
        } else {
            (0, 2, 1)
        };
        let base = sub(shape.center, t.origin);
        let offset = scale(axes[face], shape.half_extents[face]);
        let face_center = if projections[face] > 0. {
            sub(base, offset)
        } else {
            add(base, offset)
        };
        let a = scale(axes[u], shape.half_extents[u]);
        let b = scale(axes[v], shape.half_extents[v]);
        let mut polygon = vec![
            sub(add(face_center, a), b),
            sub(sub(face_center, a), b),
            add(sub(face_center, a), b),
            add(add(face_center, a), b),
        ];
        for &p in &polygon {
            finite(p)?;
        }
        let n = cross(t.edges[0], t.edges[1]);
        let points = t.points()?;
        polygon = clip(polygon, normalize(neg(n))?, 0.)?;
        polygon = clip(polygon, normalize(cross(n, sub(points[1], points[0])))?, 0.)?;
        let edge_normal = normalize(cross(n, sub(points[2], points[1])))?;
        // Source offset uses rebuilt A-C, including its float round trip.
        let offset = dot(sub(points[0], points[2]), edge_normal);
        polygon = clip(polygon, edge_normal, offset)?;
        polygon = clip(polygon, normalize(cross(n, sub(points[0], points[2])))?, 0.)?;
        let Some(mean) = weighted(&polygon, reference, 0.)? else {
            return Ok(None);
        };
        let depth = -dot(reference, mean);
        (add(add(mean, scale(reference, depth)), t.origin), depth)
    } else {
        let face = usize::from(c.axis - 2);
        let (u, v) = match face {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let points = t.points()?;
        let mut polygon = points.map(|p| sub(p, shape.center)).to_vec();
        polygon = clip(polygon, neg(c.normal), shape.half_extents[face])?;
        for i in [u, v] {
            polygon = clip(polygon, axes[i], shape.half_extents[i])?;
            polygon = clip(polygon, neg(axes[i]), shape.half_extents[i])?;
        }
        let Some(mean) = weighted(&polygon, c.normal, shape.half_extents[face])? else {
            return Ok(None);
        };
        let depth = -(dot(c.normal, mean) - shape.half_extents[face]);
        (add(mean, shape.center), depth)
    };
    finite(point)?;
    ensure!(depth.is_finite(), "collision contact depth overflow");
    if depth < 0. {
        return Ok(None);
    }
    let point_minus_normal = sub(point, c.normal);
    finite(point_minus_normal)?;
    Ok(Some(BoxTriangleContact {
        axis: c.axis,
        normal: c.normal,
        point,
        point_minus_normal,
        depth,
    }))
}

#[cfg(test)]
#[path = "../tests/collision_clip.rs"]
mod tests;
