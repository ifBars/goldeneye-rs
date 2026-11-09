//! Current placement frames for the independent line-query path.
//! Host reciprocal-square-root seeds replace Wii estimates; their numerical
//! parity and original floating-point state remain unverified.
use anyhow::{Result, ensure};

fn finite(values: &[f32]) -> Result<()> {
    ensure!(
        values.iter().all(|value| value.is_finite()),
        "nonfinite line frame input or intermediate"
    );
    Ok(())
}

fn accumulate(coefficients: [f32; 4], vector: [f32; 4]) -> Result<f32> {
    let x = coefficients[0] * vector[0];
    let y = coefficients[1].mul_add(vector[1], x);
    let z = coefficients[2].mul_add(vector[2], y);
    let w = coefficients[3].mul_add(vector[3], z);
    finite(&[x, y, z, w])?;
    Ok(w)
}

/// Subtract CURRENT translation before multiplying by its full transpose.
/// The origin's multiplication input W is forced to one; delta retains all
/// four supplied lanes. Both outputs retain all four lanes without division.
/// This path has no inverse selection, determinant, frame cache or scale repair.
pub fn working(
    origin: [f32; 4],
    delta: [f32; 4],
    matrix: &[f32; 16],
) -> Result<([f32; 4], [f32; 4])> {
    finite(&origin)?;
    finite(&delta)?;
    finite(matrix)?;
    let relative = [
        origin[0] - matrix[12],
        origin[1] - matrix[13],
        origin[2] - matrix[14],
        1.,
    ];
    finite(&relative)?;
    let mut local_origin = [0.; 4];
    let mut local_delta = [0.; 4];
    for row in 0..4 {
        let coefficients = std::array::from_fn(|column| matrix[row * 4 + column]);
        local_origin[row] = accumulate(coefficients, relative)?;
        local_delta[row] = accumulate(coefficients, delta)?;
    }
    Ok((local_origin, local_delta))
}

/// Transform local plane XYZ through CURRENT placement with W=0, then normalize
/// only when the derived length is strictly greater than the source1e-5 limit.
/// Preserve the original plane offset W, independent of the homogeneous row.
/// This uses a single reciprocal-sqrt refinement, length=squared*refined, and a
/// direct1/length division, distinct from box-contact result normalization.
pub fn world_normal(local_plane: [f32; 4], matrix: &[f32; 16]) -> Result<[f32; 4]> {
    finite(&local_plane)?;
    finite(matrix)?;
    let vector = [local_plane[0], local_plane[1], local_plane[2], 0.];
    let mut normal = [0.; 4];
    for row in 0..3 {
        let coefficients = std::array::from_fn(|column| matrix[column * 4 + row]);
        normal[row] = accumulate(coefficients, vector)?;
    }
    let x = normal[0] * normal[0];
    let y = normal[1] * normal[1];
    let xy = x + y;
    let squared = normal[2].mul_add(normal[2], xy);
    finite(&[x, y, xy, squared])?;
    let length = if squared == 0. {
        0.
    } else {
        let seed = 1. / squared.sqrt();
        let seed_squared = seed * seed;
        let half_seed = seed * 0.5;
        let residual = -squared.mul_add(seed_squared, -1.);
        let refined = residual.mul_add(half_seed, seed);
        let length = refined * squared;
        finite(&[seed, seed_squared, half_seed, residual, refined, length])?;
        length
    };
    if length > f32::from_bits(0x3727_c5ac) {
        let reciprocal = 1. / length;
        finite(&[reciprocal])?;
        for value in &mut normal[..3] {
            *value *= reciprocal;
        }
    }
    normal[3] = local_plane[3];
    finite(&normal)?;
    Ok(normal)
}

#[cfg(test)]
#[path = "../tests/collision_line_frame.rs"]
mod tests;
