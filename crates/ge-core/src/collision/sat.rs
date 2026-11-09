//! Ordered native candidate selection. A candidate is not a completed contact.
use super::{BoxTriangleCandidate, OrientedBox, math::*};
use anyhow::{Result, ensure};

pub(super) enum Outcome {
    Separated(u8),
    Candidate(BoxTriangleCandidate),
}
fn finite_scalars(values: &[f32]) -> Result<()> {
    ensure!(
        values.iter().all(|v| v.is_finite()),
        "collision projection/depth overflow"
    );
    Ok(())
}
fn interval(
    axis: V,
    lo: f32,
    hi: f32,
    radius: f32,
    id: u8,
    normalized: bool,
    best: &mut BoxTriangleCandidate,
) -> Result<bool> {
    let left = radius - lo;
    let right = hi + radius;
    finite_scalars(&[lo, hi, radius, left, right])?;
    finite(axis)?;
    if left < 0. || right < 0. {
        return Ok(false);
    }
    let (normal, depth) = if left > right {
        (neg(axis), right)
    } else {
        (axis, left)
    };
    if normalized {
        let length = length(axis)?;
        if length > 0. {
            let inverse = 1. / length;
            let depth = depth * inverse;
            finite_scalars(&[inverse, depth])?;
            let biased = 1.5f32 * depth;
            ensure!(biased.is_finite(), "collision edge-axis bias overflow");
            if biased < best.depth {
                best.normal = scale(normal, inverse);
                best.depth = depth;
                best.axis = id;
            }
        }
    } else if depth < best.depth {
        best.normal = normal;
        best.depth = depth;
        best.axis = id;
    }
    Ok(true)
}

pub(super) fn select(shape: &OrientedBox, t: &Triangle) -> Result<Outcome> {
    shape.validate()?;
    let axes = [shape.axis(0)?, shape.axis(1)?, shape.axis(2)?];
    let extent = shape.half_extents;
    let [e0, e1] = t.edges;
    let e2 = sub(e1, e0);
    finite(e2)?;
    let n = cross(e0, e1);
    finite(n)?;
    let n_length = length(n)?;
    if n_length == 0. {
        return Ok(Outcome::Separated(99));
    }
    let relative = sub(t.origin, shape.center);
    finite(relative)?;
    let projection = dot(n, relative);
    let first = extent[1] * dot(n, axes[1]).abs();
    let radius = (extent[0] * dot(n, axes[0]).abs() + first) + extent[2] * dot(n, axes[2]).abs();
    let gap = radius + projection;
    finite_scalars(&[projection, radius, gap])?;
    if gap < 0. {
        return Ok(Outcome::Separated(1));
    }
    let mut best = BoxTriangleCandidate {
        axis: 0,
        normal: [0.; 3],
        depth: f32::MAX,
    };
    let inverse = 1. / n_length;
    let plane_depth = gap * inverse;
    finite_scalars(&[inverse, plane_depth])?;
    if plane_depth < best.depth {
        best = BoxTriangleCandidate {
            axis: 1,
            normal: scale(neg(n), inverse),
            depth: plane_depth,
        };
    }
    for i in 0..3 {
        let p0 = dot(axes[i], relative);
        let p1 = p0 + dot(axes[i], e0);
        let p2 = p0 + dot(axes[i], e1);
        finite_scalars(&[p0, p1, p2])?;
        let lo = if p0 < p1 {
            if p0 < p2 { p0 } else { p2 }
        } else if p1 < p2 {
            p1
        } else {
            p2
        };
        let hi = if p0 > p1 {
            if p0 > p2 { p0 } else { p2 }
        } else if p1 > p2 {
            p1
        } else {
            p2
        };
        let id = i as u8 + 2;
        if !interval(axes[i], lo, hi, extent[i], id, false, &mut best)? {
            return Ok(Outcome::Separated(id));
        }
    }
    for i in 0..3 {
        // Native optimized projection pairs/radii use triple-product identities,
        // not independently projecting all three vertices on every cross axis.
        let (j, k) = match i {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        for (edge_index, edge) in [e0, e1, e2].into_iter().enumerate() {
            let axis = cross(axes[i], edge);
            let p0 = dot(axis, relative);
            let term = dot(axes[i], n);
            let p1 = if edge_index == 0 {
                p0 + term
            } else {
                p0 - term
            };
            finite_scalars(&[p0, term, p1])?;
            let radius =
                extent[k] * dot(axes[j], edge).abs() + extent[j] * dot(axes[k], edge).abs();
            let id = 5 + (i * 3 + edge_index) as u8;
            let (lo, hi) = if p0 < p1 { (p0, p1) } else { (p1, p0) };
            if !interval(axis, lo, hi, radius, id, true, &mut best)? {
                return Ok(Outcome::Separated(id));
            }
        }
    }
    finite(best.normal)?;
    Ok(Outcome::Candidate(best))
}
