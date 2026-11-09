//! Scalar form of the observed paired polynomial used by Euler order six.
use anyhow::{Result, ensure};

/// Negated fused multiply-subtract, including the final zero-sign negation.
pub(super) fn nmsub(a: f32, b: f32, c: f32) -> f32 {
    -a.mul_add(b, -c)
}
fn polynomial(t: f32) -> f32 {
    let square = t * t;
    let p = square.mul_add(f32::from_bits(0x362e9c5b), f32::from_bits(0xb94fb222));
    let p = square.mul_add(p, f32::from_bits(0x3c08873e));
    let p = square.mul_add(p, f32::from_bits(0xbe2aaaa4));
    t.mul_add(square * p, t)
}
pub(super) fn sin_cos(angle: f32) -> Result<(f32, f32)> {
    ensure!(angle.is_finite(), "nonfinite native Euler angle");
    let absolute = angle.abs();
    let scaled = absolute * f32::from_bits(0x3ea2f983);
    // Native fadd/fsub use double precision around 2^52 before the paired
    // result returns to single precision. The integer parity uses fctiwz.
    let quadrant = ((f64::from(scaled) + 4503599627370496.) - 4503599627370496.) as f32;
    ensure!(
        quadrant.is_finite() && f64::from(quadrant) <= f64::from(i32::MAX),
        "native Euler range reduction exceeds signed integer support"
    );
    let odd = (quadrant as i32) & 1 != 0;
    let pi = f32::from_bits(0x40490fdb);
    let residual = nmsub(quadrant, pi, absolute);
    let positive = residual >= 0.;
    let mut sine_arg = if odd { -residual } else { residual };
    if angle < 0. {
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
    let value = (polynomial(sine_arg), polynomial(cosine_arg));
    ensure!(
        value.0.is_finite() && value.1.is_finite(),
        "native Euler polynomial overflow"
    );
    Ok(value)
}

#[cfg(test)]
#[path = "../tests/native_trig.rs"]
mod tests;
