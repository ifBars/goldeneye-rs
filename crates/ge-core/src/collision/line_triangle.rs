//! Mesh-local line/triangle primitive and resident plane preparation.
//!
//! The ordered arithmetic follows the confirmed static path. Host reciprocal
//! and reciprocal-square-root seeds substitute for Wii estimates, so original
//! floating-point parity remains unverified. No back-face or area gate is added.
use anyhow::{Result, ensure};

const PARALLEL_THRESHOLD: f32 = 1e-10;

#[derive(Debug, Clone, Copy)]
pub struct LineTriangleQuery {
    pub origin: [f32; 3],
    pub delta: [f32; 3],
    /// Inclusive upper parameter. A hit at this value replaces an earlier hit.
    pub maximum_parameter: f32,
    pub required_any: u32,
    pub excluded_any: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineTriangleHit {
    pub parameter: f32,
    pub local_point: [f32; 4],
    /// Plane coefficients, with all four signs reversed for an odd strip step.
    pub local_plane: [f32; 4],
}

fn finite<const N: usize>(values: [f32; N]) -> Result<()> {
    ensure!(
        values.into_iter().all(f32::is_finite),
        "nonfinite line triangle value"
    );
    Ok(())
}

fn subtract(a: [f32; 3], b: [f32; 3]) -> Result<[f32; 3]> {
    let result = std::array::from_fn(|axis| a[axis] - b[axis]);
    finite(result)?;
    Ok(result)
}

/// Prepare one plane in the resident compact included-batch triangle stream.
/// Call for every included triangle, including triangles whose flag has 0x200.
/// The ordinal resets at each strip. Its rotating source slots matter to plane
/// offset rounding; an odd strip step is not repaired here.
pub fn prepare_line_plane(vertices: [[f32; 3]; 3], triangle_in_strip: usize) -> Result<[f32; 4]> {
    for vertex in vertices {
        finite(vertex)?;
    }
    let first = (3 - triangle_in_strip % 3) % 3;
    let a = vertices[first];
    let b = vertices[(first + 1) % 3];
    let c = vertices[(first + 2) % 3];
    let e = subtract(b, a)?;
    let f = subtract(c, b)?;
    let normal = [
        f[2].mul_add(e[1], -(e[2] * f[1])),
        e[2].mul_add(f[0], -(f[2] * e[0])),
        e[0].mul_add(f[1], -(f[0] * e[1])),
    ];
    finite(normal)?;
    let square = normal[2].mul_add(normal[2], normal[0] * normal[0] + normal[1] * normal[1]);
    finite([square])?;
    if square <= f32::EPSILON {
        return Ok([1., 0., 0., 0.]);
    }
    let estimate = 1. / square.sqrt();
    let correction = (-square).mul_add(estimate * estimate, 1.);
    let refined = correction.mul_add(estimate * 0.5, estimate);
    // The native plane constructor uses a reciprocal estimate without a second
    // Newton refinement. Keeping this separate also preserves its rounding site.
    let inverse_length = 1. / (refined * square);
    let n = normal.map(|value| value * inverse_length);
    let offset = -n[2].mul_add(a[2], n[0] * a[0] + n[1] * a[1]);
    let plane = [n[0], n[1], n[2], offset];
    finite(plane)?;
    Ok(plane)
}

fn edge_accepts(start: [f32; 3], end: [f32; 3], point: [f32; 3], normal: [f32; 3]) -> Result<bool> {
    let edge = subtract(end, start)?;
    let displacement = subtract(point, end)?;
    let side_normal = [
        normal[2].mul_add(edge[1], -(normal[1] * edge[2])),
        normal[0].mul_add(edge[2], -(normal[2] * edge[0])),
        normal[1].mul_add(edge[0], -(normal[0] * edge[1])),
    ];
    finite(side_normal)?;
    let side = displacement[2].mul_add(
        side_normal[2],
        displacement[1].mul_add(side_normal[1], displacement[0] * side_normal[0]),
    );
    finite([side])?;
    Ok(side <= 0.)
}

/// Query a precomputed plane, fetching XYZ only after plane, interval and flag
/// gates pass. Endpoints and triangle edges are included; both faces can hit.
/// The caller owns metadata, replacing the shared maximum, and stop-first state.
pub fn intersect_line_triangle(
    query: &LineTriangleQuery,
    plane: [f32; 4],
    flags_raw: u16,
    triangle_in_strip: usize,
    vertices: impl FnOnce() -> Result<[[f32; 3]; 3]>,
) -> Result<Option<LineTriangleHit>> {
    if flags_raw & 0x200 != 0 {
        return Ok(None);
    }
    finite(plane)?;
    finite(query.origin)?;
    finite(query.delta)?;
    finite([query.maximum_parameter])?;
    let denominator = plane[2].mul_add(
        query.delta[2],
        plane[1].mul_add(query.delta[1], plane[0] * query.delta[0]),
    );
    finite([denominator])?;
    if denominator.abs() < PARALLEL_THRESHOLD {
        return Ok(None);
    }
    let numerator = -plane[2].mul_add(
        query.origin[2],
        plane[1].mul_add(query.origin[1], plane[0].mul_add(query.origin[0], plane[3])),
    );
    let estimate = 1. / denominator;
    let correction = (-estimate).mul_add(denominator, 1.);
    let reciprocal = estimate.mul_add(correction, estimate);
    let parameter = numerator * reciprocal;
    finite([parameter])?;
    if parameter < 0. || parameter > query.maximum_parameter {
        return Ok(None);
    }
    let flags = u32::from(flags_raw);
    if (query.required_any != 0 && flags & query.required_any == 0)
        || (query.excluded_any != 0 && flags & query.excluded_any != 0)
    {
        return Ok(None);
    }
    // Separate multiply and addition are intentional: the source stores the
    // scaled direction before adding the origin.
    let scaled = query.delta.map(|value| value * parameter);
    finite(scaled)?;
    let point = std::array::from_fn(|axis| query.origin[axis] + scaled[axis]);
    finite(point)?;
    let triangle = vertices()?;
    for vertex in triangle {
        finite(vertex)?;
    }
    let normal = [plane[0], plane[1], plane[2]];
    for (start, end) in [(2, 0), (0, 1), (1, 2)] {
        if !edge_accepts(triangle[start], triangle[end], point, normal)? {
            return Ok(None);
        }
    }
    Ok(Some(LineTriangleHit {
        parameter,
        local_point: [point[0], point[1], point[2], 1.],
        local_plane: if triangle_in_strip & 1 == 0 {
            plane
        } else {
            plane.map(|value| -value)
        },
    }))
}

#[cfg(test)]
#[path = "../tests/collision_line_triangle.rs"]
mod tests;
