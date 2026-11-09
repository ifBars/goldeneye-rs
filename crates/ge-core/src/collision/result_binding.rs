//! Pure binding of a type-three primitive contact into a collected mesh result.
//! Wii estimate seeds remain provisional; no loaded-world traversal is implied.
use super::{BoxTriangleContact, math};
use anyhow::{Result, ensure};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct ContactProvenance {
    pub collected_record_key: u32,
    pub first_corner_key: u32,
    pub position_indices: [u16; 3],
    pub triangle_flags_raw: u16,
    pub material_raw: u16,
    pub query_shape_index: u32,
    pub triangle_in_strip: u32,
}

#[derive(Debug, Serialize)]
pub struct BoundContact {
    /// Normal of the query-frame triangle, normalized before the frame multiply.
    /// This is distinct from the primitive's selected separating-axis normal.
    pub normal: [f32; 4],
    pub witness_a: [f32; 4],
    pub witness_b: [f32; 4],
    /// Remains in the primitive query's units, without an inferred scale factor.
    pub depth: f32,
    pub position_indices: [u32; 3],
    pub collected_record_key: u32,
    pub first_corner_key: u32,
    pub triangle_flags_raw: u16,
    pub material_raw: u16,
    pub query_shape_index: u8,
    pub flags_raw: u8,
}

fn nmsub(a: f32, b: f32, c: f32) -> f32 {
    -a.mul_add(b, -c)
}
fn finite(values: &[f32]) -> Result<()> {
    ensure!(
        values.iter().all(|v| v.is_finite()),
        "nonfinite contact binding value/intermediate"
    );
    Ok(())
}
fn triangle_normal(triangle: [[f32; 3]; 3]) -> Result<[f32; 4]> {
    for point in triangle {
        finite(&point)?;
    }
    let edges = [1, 2].map(|corner| math::sub(triangle[corner], triangle[0]));
    for edge in edges {
        finite(&edge)?;
    }
    let normal = math::cross(edges[0], edges[1]);
    finite(&normal)?;
    let squared = math::dot(normal, normal);
    finite(&[squared])?;
    let length = if squared == 0. {
        0.
    } else {
        // A host reciprocal-sqrt seed replaces frsqrte. Retain its observed
        // rounded square/half and one fused correction before multiplying by s.
        let seed = 1. / squared.sqrt();
        let square = seed * seed;
        let half = seed * 0.5;
        let residual = nmsub(squared, square, 1.);
        let refined = residual.mul_add(half, seed);
        let length = refined * squared;
        finite(&[seed, square, half, residual, refined, length])?;
        length
    };
    if length == 0. {
        return Ok([1., 0., 0., 0.]);
    }
    // A host reciprocal seed replaces ps_res. This helper refines it twice,
    // unlike collision-frame inversion's separate single refinement.
    let seed = 1. / length;
    let residual = nmsub(length, seed, 1.);
    let first = seed.mul_add(residual, seed);
    let residual = nmsub(length, first, 1.);
    let reciprocal = first.mul_add(residual, first);
    finite(&[seed, first, residual, reciprocal])?;
    let result = [
        normal[0] * reciprocal,
        normal[1] * reciprocal,
        normal[2] * reciprocal,
        0. * reciprocal,
    ];
    finite(&result)?;
    Ok(result)
}
fn transform(matrix: &[f32; 16], value: [f32; 4]) -> Result<[f32; 4]> {
    finite(matrix)?;
    finite(&value)?;
    let result = std::array::from_fn(|row| {
        let first = matrix[row] * value[0];
        let second = matrix[4 + row].mul_add(value[1], first);
        let third = matrix[8 + row].mul_add(value[2], second);
        matrix[12 + row].mul_add(value[3], third)
    });
    finite(&result)?;
    Ok(result)
}
fn world_point(matrix: &[f32; 16], point: [f32; 3]) -> Result<[f32; 4]> {
    let mut result = transform(matrix, [point[0], point[1], point[2], 1.])?;
    result[3] = 0.;
    Ok(result)
}

/// Bind accepted primitive output. The triangle is in the same query frame as
/// the primitive. The contact's stored SAT normal is not read by this operation.
/// Exact zero (including squared-length underflow) uses triangle-normal +X.
/// Used nonfinite data/overflow are host errors; native FP exceptions are open.
pub fn bind_type_three(
    contact: &BoxTriangleContact,
    triangle: [[f32; 3]; 3],
    world_from_query: &[f32; 16],
    provenance: ContactProvenance,
) -> Result<BoundContact> {
    finite(&[contact.depth])?;
    let mut normal = transform(world_from_query, triangle_normal(triangle)?)?;
    let witness_a = world_point(world_from_query, contact.point_minus_normal)?;
    let witness_b = world_point(world_from_query, contact.point)?;
    let mut position_indices = provenance.position_indices.map(u32::from);
    if provenance.triangle_in_strip & 1 != 0 {
        normal = normal.map(|value| -value);
        position_indices.swap(1, 2);
    }
    Ok(BoundContact {
        normal,
        witness_a,
        witness_b,
        depth: contact.depth,
        position_indices,
        collected_record_key: provenance.collected_record_key,
        first_corner_key: provenance.first_corner_key,
        triangle_flags_raw: provenance.triangle_flags_raw,
        material_raw: provenance.material_raw,
        query_shape_index: provenance.query_shape_index as u8,
        flags_raw: 0,
    })
}

/// The native mesh gate runs after position/frame/coarse-pruning preparation.
/// It reserves one slot by unsigned wrapping subtraction, including capacity zero.
pub fn can_begin_mesh(count: u32, capacity: u32) -> bool {
    count < capacity.wrapping_sub(1)
}
/// This separate check occurs only AFTER a primitive reports a contact. A host
/// adapter can use temporary primitive outputs to avoid speculative array writes.
pub fn can_append(count: u32, capacity: u32) -> bool {
    count < capacity
}

#[cfg(test)]
#[path = "../tests/collision_result_binding.rs"]
mod tests;
