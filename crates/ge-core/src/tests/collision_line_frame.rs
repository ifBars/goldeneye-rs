use super::*;

fn identity() -> [f32; 16] {
    [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ]
}

fn rotated_placement() -> [f32; 16] {
    // Column-major Z90 rotation, then world translation.
    [
        0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 10., 20., 30., 1.,
    ]
}

#[test]
fn translation_precedes_noncommuting_transpose_rotation_and_retains_output_w() {
    let (origin, delta) =
        working([7., 22., 34., 99.], [1., 2., 3., 0.], &rotated_placement()).unwrap();
    assert_eq!(origin, [2., 3., 4., 131.]);
    assert_eq!(delta, [2., -1., 3., 140.]);
    let world = world_normal([1., 0., 0., -7.], &rotated_placement()).unwrap();
    assert_eq!(world, [0., 1., 0., -7.]);
}

#[test]
fn scale_is_transposed_without_reciprocals_and_zero_matrix_has_no_inverse_error() {
    let matrix = [
        2., 0., 0., 0., 0., 3., 0., 0., 0., 0., 4., 0., 10., 20., 30., 1.,
    ];
    let (origin, delta) = working([11., 22., 33., 1.], [1., 2., 3., 0.], &matrix).unwrap();
    assert_eq!(origin, [2., 6., 12., 141.]);
    assert_eq!(delta, [2., 6., 12., 140.]);
    assert_eq!(
        working([4.; 4], [3.; 4], &[0.; 16]).unwrap(),
        ([0.; 4], [0.; 4])
    );
    assert_eq!(
        world_normal([1., 2., 3., 7.], &[0.; 16]).unwrap(),
        [0., 0., 0., 7.]
    );
}

#[test]
fn full_matrix_rows_and_delta_w_are_used_while_origin_w_is_forced_to_one() {
    let matrix = [
        2., 0., 0., 7., 0., 3., 0., 11., 0., 0., 4., 13., 10., 20., 30., 17.,
    ];
    let expected = ([9., 17., 25., 157.], [16., 28., 38., 174.]);
    assert_eq!(
        working([11., 22., 33., -123.], [1., 2., 3., 2.], &matrix).unwrap(),
        expected
    );
    assert_eq!(
        working([11., 22., 33., 456.], [1., 2., 3., 2.], &matrix).unwrap(),
        expected
    );
}

#[test]
fn current_translation_is_reloaded_each_call_without_reusing_an_earlier_frame() {
    let first = rotated_placement();
    let mut second = first;
    second[12] = 15.;
    let (a, _) = working([7., 22., 34., 1.], [0.; 4], &first).unwrap();
    let (b, _) = working([7., 22., 34., 1.], [0.; 4], &second).unwrap();
    assert_eq!(a[..3], [2., 3., 4.]);
    assert_eq!(b[..3], [2., 8., 4.]);
}

#[test]
fn world_normal_uses_matrix_direction_then_normalizes_and_keeps_plane_offset_bits() {
    let matrix = [
        2., 0., 0., 0., 0., 3., 0., 0., 0., 0., 4., 0., 100., 200., 300., 1.,
    ];
    let normal = world_normal([1., 1., 0., -0.], &matrix).unwrap();
    assert!((normal[0] - 2. / 13f32.sqrt()).abs() < 1e-6);
    assert!((normal[1] - 3. / 13f32.sqrt()).abs() < 1e-6);
    assert_eq!(normal[2], 0.);
    assert_eq!(normal[3].to_bits(), (-0f32).to_bits());
    // The original plane offset is independent of both scaling and translation.
    assert_eq!(world_normal([1., 1., 0., 123.], &matrix).unwrap()[3], 123.);
}

#[test]
fn zero_short_and_threshold_normals_stay_unchanged_but_above_threshold_normalizes() {
    let matrix = identity();
    for plane in [
        [0., 0., 0., 7.],
        [3e-6, 4e-6, 0., 8.],
        [1e-5, 0., 0., 9.],
        [1e-30, 0., 0., 10.],
    ] {
        assert_eq!(world_normal(plane, &matrix).unwrap(), plane);
    }
    let normal = world_normal([2e-5, 0., 0., 11.], &matrix).unwrap();
    assert!((normal[0] - 1.).abs() < 1e-6);
    assert_eq!(normal[1..], [0., 0., 11.]);
}

#[test]
fn world_normal_ignores_homogeneous_output_row_and_original_plane_d_in_xyz() {
    let mut matrix = identity();
    matrix[3] = f32::MAX;
    matrix[7] = f32::MAX;
    matrix[11] = f32::MAX;
    matrix[12] = 100.;
    let normal = world_normal([1., 1., 1., 29.], &matrix).unwrap();
    for value in &normal[..3] {
        assert!((*value - 1. / 3f32.sqrt()).abs() < 1e-6);
    }
    assert_eq!(normal[3], 29.);
}

#[test]
fn nonfinite_inputs_and_overflowing_used_arithmetic_are_errors() {
    let matrix = identity();
    assert!(working([f32::NAN; 4], [0.; 4], &matrix).is_err());
    assert!(working([0.; 4], [f32::INFINITY; 4], &matrix).is_err());
    assert!(world_normal([0., 0., 0., f32::INFINITY], &matrix).is_err());
    let mut matrix = matrix;
    matrix[0] = f32::MAX;
    assert!(working([2., 0., 0., 1.], [0.; 4], &matrix).is_err());
    assert!(world_normal([2., 0., 0., 1.], &matrix).is_err());
    matrix[0] = 1.;
    matrix[12] = -f32::MAX;
    assert!(working([f32::MAX, 0., 0., 1.], [0.; 4], &matrix).is_err());
    assert!(world_normal([f32::MAX, 0., 0., 1.], &identity()).is_err());
}
