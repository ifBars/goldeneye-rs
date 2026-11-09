//! Independently expressed source802d3b80/80028a90/802d12c0 operations.
//! Finite/range checks are supported-host bounds, not native validation. The
//! audited quaternion helper is reused; the assets placement order is different.
use super::{Geometry, GeometryInput};
use crate::animation::transform::quaternion_matrix;
use anyhow::{Result, ensure};

fn finite(words: &[u32]) -> bool {
    words.iter().all(|word| f32::from_bits(*word).is_finite())
}

pub(super) fn materialize(input: GeometryInput) -> Result<Geometry> {
    ensure!(
        input.type_raw == 0,
        "unsupported subtype71 source shape type"
    );
    ensure!(
        finite(&input.center_bits)
            && finite(&input.orientation_bits)
            && finite(&input.dimensions_bits),
        "nonfinite source shape geometry outside host support"
    );
    let center_bits = [
        (f32::from_bits(input.center_bits[0]) - 0.0).to_bits(),
        (f32::from_bits(input.center_bits[1]) - 0.0).to_bits(),
        (f32::from_bits(input.center_bits[2]) - 0.0).to_bits(),
        (1.0f32 - 0.0).to_bits(),
    ];
    let matrix = quaternion_matrix(input.orientation_bits);
    // Keep all zero products and fourth FMA; copying columns changes signs.
    let axis_a_bits = product(&matrix, [1.0f32.to_bits(), 0, 0, 0]);
    let axis_b_bits = product(&matrix, [0, 1.0f32.to_bits(), 0, 0]);
    ensure!(
        finite(&axis_a_bits) && finite(&axis_b_bits),
        "shape orientation arithmetic outside host support"
    );
    Ok(Geometry::Type2 {
        center_bits,
        axis_a_bits,
        axis_b_bits,
        dimensions_bits: input.dimensions_bits,
    })
}

fn product(matrix: &[u32; 16], vector: [u32; 4]) -> [u32; 4] {
    let matrix = matrix.map(f32::from_bits);
    let vector = vector.map(f32::from_bits);
    std::array::from_fn(|row| {
        let initial = matrix[row] * vector[0];
        let y = matrix[4 + row].mul_add(vector[1], initial);
        let z = matrix[8 + row].mul_add(vector[2], y);
        matrix[12 + row].mul_add(vector[3], z).to_bits()
    })
}

pub(super) fn transform(geometry: Geometry, placement: &[u32; 16]) -> Result<Geometry> {
    let Geometry::Type2 {
        mut center_bits,
        axis_a_bits,
        axis_b_bits,
        dimensions_bits,
    } = geometry
    else {
        anyhow::bail!("subtype71 transform requires materialized internal type2");
    };
    center_bits[3] = 1.0f32.to_bits();
    let mut center_bits = product(placement, center_bits);
    center_bits[3] = 0;
    let axis_a_bits = product(placement, axis_a_bits);
    let axis_b_bits = product(placement, axis_b_bits);
    ensure!(
        finite(&center_bits) && finite(&axis_a_bits) && finite(&axis_b_bits),
        "shape transform arithmetic outside host support"
    );
    Ok(Geometry::Type2 {
        center_bits,
        axis_a_bits,
        axis_b_bits,
        dimensions_bits,
    })
}

/// Serialized A/B/C inputs, source Rx(A)*Ry(B)*Rz(C), no scale concatenate.
pub(super) fn placement(position_bits: [u32; 3], rotation_bits: [u32; 3]) -> Result<[u32; 16]> {
    ensure!(
        finite(&position_bits) && finite(&rotation_bits),
        "nonfinite subtype71 placement outside host support"
    );
    let position = position_bits.map(f32::from_bits);
    let rotation = rotation_bits.map(f32::from_bits);
    let (sa, ca) = trig(rotation[0])?;
    let (sb, cb) = trig(rotation[1])?;
    let (sc, cc) = trig(rotation[2])?;
    let u = -sa;
    let v = -sb;
    #[allow(
        clippy::eq_op,
        reason = "Retain source paired positive-zero construction"
    )]
    let zero = 1.0f32 - 1.0f32;
    let p = [1.0 * cb, u * v];
    let q = [ca * v, zero * v];
    let r = [zero, ca];
    let s = [sa, zero];
    let result = [
        p[0].mul_add(cc, r[0] * sc),
        p[1].mul_add(cc, r[1] * sc),
        q[0].mul_add(cc, s[0] * sc),
        q[1].mul_add(cc, s[1] * sc),
        nmsub(p[0], sc, r[0] * cc),
        nmsub(p[1], sc, r[1] * cc),
        nmsub(q[0], sc, s[0] * cc),
        nmsub(q[1], sc, s[1] * cc),
        1.0 * sb,
        u * cb,
        ca * cb,
        zero * cb,
        position[0],
        position[1],
        position[2],
        1.0,
    ]
    .map(f32::to_bits);
    ensure!(
        finite(&result),
        "subtype71 placement arithmetic outside host support"
    );
    Ok(result)
}

fn nmsub(a: f32, b: f32, c: f32) -> f32 {
    -a.mul_add(b, -c)
}

fn polynomial(t: f32) -> f32 {
    let square = t * t;
    let p = square.mul_add(f32::from_bits(0x362e_9c5b), f32::from_bits(0xb94f_b222));
    let p = square.mul_add(p, f32::from_bits(0x3c08_873e));
    let p = square.mul_add(p, f32::from_bits(0xbe2a_aaa4));
    t.mul_add(square * p, t)
}

fn trig(angle: f32) -> Result<(f32, f32)> {
    ensure!(angle.is_finite(), "nonfinite subtype71 Euler input");
    let absolute = angle.abs();
    let scaled = absolute * f32::from_bits(0x3ea2_f983);
    let quadrant = ((f64::from(scaled) + 4503599627370496.0) - 4503599627370496.0) as f32;
    ensure!(
        scaled.is_finite() && quadrant.is_finite() && f64::from(quadrant) <= f64::from(i32::MAX),
        "subtype71 Euler reduction outside signed integer support"
    );
    let odd = (quadrant as i32) & 1 != 0;
    let pi = f32::from_bits(0x4049_0fdb);
    let residual = nmsub(quadrant, pi, absolute);
    let positive = residual >= 0.0;
    let mut sine_arg = if odd { -residual } else { residual };
    if angle < 0.0 {
        sine_arg = -sine_arg;
    }
    let offset = if positive { 0.5 } else { -0.5 };
    let cosine_residual = nmsub(quadrant + offset, pi, absolute);
    let cosine_sign = if odd { positive } else { !positive };
    let cosine_arg = if cosine_sign {
        cosine_residual
    } else {
        -cosine_residual
    };
    let result = (polynomial(sine_arg), polynomial(cosine_arg));
    ensure!(
        result.0.is_finite() && result.1.is_finite(),
        "subtype71 Euler polynomial overflow"
    );
    Ok(result)
}

#[cfg(test)]
#[path = "../tests/shape71_math.rs"]
mod tests;
