//! Independently authored scalar equivalents of the observed paired register
//! order. No normalization, affine shortcut, reassociation or implicit cache.

pub(super) fn identity() -> [u32; 16] {
    let mut result = [0; 16];
    for lane in [0, 5, 10, 15] {
        result[lane] = 1.0f32.to_bits();
    }
    result
}

/// Source 802a5360 register groups, stored as four contiguous columns.
/// Inputs may be nonunit; this operation never normalizes the quaternion.
/// Host f32/FMA order is provisional, even with exact-branch input W.
pub fn quaternion_matrix(quaternion_bits: [u32; 4]) -> [u32; 16] {
    let [x, y, z, w] = quaternion_bits.map(f32::from_bits);
    let one = 1.0f32;
    #[allow(
        clippy::eq_op,
        reason = "Retain the source paired-register zero construction"
    )]
    let zero = one - one;
    let double_xy = [x + x, y + y];
    let double_z = z + z;
    let a = [z * double_z, w * double_z];
    let b = [double_xy[1] * w, double_xy[0] * w];
    let c = [x * double_z, y * double_z];
    let d = [y * double_xy[1], x * double_xy[1]];
    let e = [c[0] - b[0], c[1] - b[1]];
    let f = [d[0] + a[0], d[1] + a[1]];
    let g = [x.mul_add(double_xy[0], a[0]), y.mul_add(double_xy[0], a[1])];
    let h = [c[0] + b[0], c[1] + b[1]];
    let i = [d[0] - a[0], d[1] - a[1]];
    let j = [x.mul_add(double_xy[0], d[0]), y.mul_add(double_xy[0], d[1])];
    [
        f[0].mul_add(-one, one),
        f[1].mul_add(one, zero),
        e[0],
        zero,
        negative_fused_subtract(i[1], -one, zero),
        negative_fused_subtract(g[0], one, one),
        h[1],
        zero,
        h[0],
        e[1],
        j[0].mul_add(-one, one),
        zero.mul_add(one, zero),
        zero,
        zero,
        zero,
        one,
    ]
    .map(f32::to_bits)
}

fn negative_fused_subtract(a: f32, b: f32, c: f32) -> f32 {
    // ps_nmsub: fused product-minus-addend is rounded BEFORE negation.
    // Moving the negation into a multiplicand can change exact-zero signs.
    -a.mul_add(b, -c)
}

/// Full 4x4 source product: X initial multiply, then Y/Z/W fused accumulation.
/// Translation and homogeneous lanes receive the same four-term operation;
/// identities are multiplied rather than copied. No native precision assertion.
pub fn multiply(left_bits: &[u32; 16], right_bits: &[u32; 16]) -> [u32; 16] {
    let left = left_bits.map(f32::from_bits);
    let right = right_bits.map(f32::from_bits);
    let mut result = [0; 16];
    for column in 0..4 {
        for row in 0..4 {
            let initial = left[row] * right[column * 4];
            let y = left[4 + row].mul_add(right[column * 4 + 1], initial);
            let z = left[8 + row].mul_add(right[column * 4 + 2], y);
            let w = left[12 + row].mul_add(right[column * 4 + 3], z);
            result[column * 4 + row] = w.to_bits();
        }
    }
    result
}
