//! Column-major placement matrices. Raw Euler angles remain available on placements.
use anyhow::{Result, ensure};
mod trig;

pub const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
pub const PLACEMENT_CONVENTION: &str =
    "T*S*Rz*Rx*Ry; native initial order-six polynomial; original-runtime parity unverified";

/// Initial native placement builder: T * S * Rz * Rx * Ry, column-vector form.
/// Euler order-six polynomial and paired concatenation come from static SJBE52
/// evidence. Original execution/FP state and later live mutation remain unverified.
pub fn placement(position: [f32; 3], rotation: [f32; 3], scale: [f32; 3]) -> Result<[f32; 16]> {
    ensure!(
        position
            .iter()
            .chain(&rotation)
            .chain(&scale)
            .all(|v| v.is_finite()),
        "non-finite placement transform"
    );
    let (sx, cx) = trig::sin_cos(rotation[0])?;
    let (sy, cy) = trig::sin_cos(rotation[1])?;
    let (sz, cz) = trig::sin_cos(rotation[2])?;
    let t0 = sz * sx;
    let t1 = cz * (-sx);
    let rotation = [
        t0.mul_add(-sy, cz * cy),
        t1.mul_add(-sy, sz * cy),
        cx.mul_add(-sy, 0. * cy),
        (0f32 * cx).mul_add(-sy, 0. * cy),
        -sz * cx,
        cz * cx,
        sx,
        0. * sx,
        trig::nmsub(cz, -sy, t0 * cy),
        trig::nmsub(sz, -sy, t1 * cy),
        trig::nmsub(0., -sy, cx * cy),
        trig::nmsub(0., -sy, (0. * cx) * cy),
        0.,
        0.,
        0.,
        1.,
    ];
    let mut scaling = IDENTITY;
    scaling[0] = scale[0];
    scaling[5] = scale[1];
    scaling[10] = scale[2];
    let mut matrix = multiply(scaling, rotation);
    matrix[12..15].copy_from_slice(&position);
    ensure!(
        matrix.iter().all(|v| v.is_finite()),
        "placement matrix overflow"
    );
    Ok(matrix)
}

fn multiply(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    std::array::from_fn(|index| {
        let col = index / 4;
        let row = index % 4;
        let first = a[row] * b[col * 4];
        let second = a[4 + row].mul_add(b[col * 4 + 1], first);
        let third = a[8 + row].mul_add(b[col * 4 + 2], second);
        a[12 + row].mul_add(b[col * 4 + 3], third)
    })
}

pub fn point(matrix: &[f32; 16], point: [f32; 3]) -> Result<[f32; 3]> {
    let transformed = std::array::from_fn(|row| {
        let first = matrix[row] * point[0];
        let second = matrix[4 + row].mul_add(point[1], first);
        let third = matrix[8 + row].mul_add(point[2], second);
        matrix[12 + row].mul_add(1., third)
    });
    ensure!(
        transformed.iter().all(|v| v.is_finite()),
        "transformed position overflow"
    );
    Ok(transformed)
}

#[cfg(test)]
#[path = "tests/transform.rs"]
mod tests;
