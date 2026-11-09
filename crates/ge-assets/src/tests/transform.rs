use super::*;
fn close(a: [f32; 3], b: [f32; 3]) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} != {b:?}");
    }
}

#[test]
fn translation_scale_and_zxy_rotation_are_not_interchangeable() {
    close(
        point(
            &placement([10., 20., 30.], [0.; 3], [2., 3., 4.]).unwrap(),
            [1., 2., 3.],
        )
        .unwrap(),
        [12., 26., 42.],
    );
    let quarter = std::f32::consts::FRAC_PI_2;
    close(
        point(
            &placement([0.; 3], [quarter, quarter, 0.], [1.; 3]).unwrap(),
            [1., 0., 0.],
        )
        .unwrap(),
        [0., 1., 0.],
    );
    close(
        point(
            &placement([0.; 3], [quarter, 0., quarter], [1.; 3]).unwrap(),
            [0., 1., 0.],
        )
        .unwrap(),
        [0., 0., 1.],
    );
    close(
        point(
            &placement([0.; 3], [0.; 3], [-1., 1., 1.]).unwrap(),
            [2., 3., 4.],
        )
        .unwrap(),
        [-2., 3., 4.],
    );
    assert_eq!(placement([0.; 3], [0.; 3], [1.; 3]).unwrap(), IDENTITY);
}

#[test]
fn invalid_and_overflowed_transforms_fail_explicitly() {
    assert!(placement([f32::NAN, 0., 0.], [0.; 3], [1.; 3]).is_err());
    let matrix = placement([0.; 3], [0.; 3], [f32::MAX; 3]).unwrap();
    assert!(point(&matrix, [2.; 3]).is_err());
    assert!(placement([0.; 3], [f32::MAX, 0., 0.], [1.; 3]).is_err());
}

#[test]
fn native_initial_scale_applies_to_rotated_coordinates_before_translation() {
    let matrix = placement(
        [10., 20., 30.],
        [0., 0., std::f32::consts::FRAC_PI_2],
        [2., 3., 4.],
    )
    .unwrap();
    assert_eq!(point(&matrix, [1., 0., 0.]).unwrap(), [10., 23., 30.]);
    assert_eq!(point(&matrix, [0., 1., 0.]).unwrap(), [8., 20., 30.]);
    // Scaling the input before rotation would instead produce Y=22 or X=7.
}

#[test]
fn point_transform_preserves_native_fused_column_accumulation() {
    let a = 1. + f32::EPSILON;
    let mut matrix = IDENTITY;
    matrix[0] = -(1. + 2. * f32::EPSILON);
    matrix[4] = a;
    let transformed = point(&matrix, [1., a, 0.]).unwrap();
    assert_eq!(transformed[0], f32::EPSILON * f32::EPSILON);
    assert_ne!(transformed[0], matrix[0] + matrix[4] * a);
}

#[test]
fn native_homogeneous_zero_products_keep_their_sign_through_concatenation() {
    let matrix = placement([0.; 3], [2., -2., 0.], [1.; 3]).unwrap();
    // The zero lane combines negative cosine products and remains -0 in this
    // branch. Filling every homogeneous zero with a positive literal loses it.
    assert_eq!(matrix[3].to_bits(), 0x80000000);
    assert_eq!(matrix[15], 1.);
}

#[test]
fn native_placement_matches_independent_f64_axis_operations_over_authored_samples() {
    let mut word = 73u32;
    for _ in 0..600 {
        let mut values = [0f32; 12];
        for value in &mut values {
            word = word.wrapping_mul(1664525).wrapping_add(1013904223);
            *value = ((word >> 24) as i32 - 128) as f32 / 32.;
        }
        let position: [f32; 3] = values[0..3].try_into().unwrap();
        let rotation: [f32; 3] = values[3..6].try_into().unwrap();
        let scale: [f32; 3] = values[6..9].try_into().unwrap();
        let p: [f32; 3] = values[9..12].try_into().unwrap();
        let [x, y, z] = p.map(f64::from);
        let [rx, ry, rz] = rotation.map(f64::from);
        let after_y = [ry.cos() * x + ry.sin() * z, y, -ry.sin() * x + ry.cos() * z];
        let after_x = [
            after_y[0],
            rx.cos() * after_y[1] - rx.sin() * after_y[2],
            rx.sin() * after_y[1] + rx.cos() * after_y[2],
        ];
        let after_z = [
            rz.cos() * after_x[0] - rz.sin() * after_x[1],
            rz.sin() * after_x[0] + rz.cos() * after_x[1],
            after_x[2],
        ];
        let actual = point(&placement(position, rotation, scale).unwrap(), p).unwrap();
        for axis in 0..3 {
            let expected = f64::from(position[axis]) + f64::from(scale[axis]) * after_z[axis];
            assert!(
                (f64::from(actual[axis]) - expected).abs() < 0.00003,
                "{actual:?} vs {expected}, angles {rotation:?}"
            );
        }
    }
}
