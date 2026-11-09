use super::*;

fn close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} differs from {expected} by more than {tolerance}"
    );
}

#[test]
fn source_fov_conversion_preserves_engine_denominator_and_sdk_half_angle() {
    let camera = Camera::new(Aspect::Wide).unwrap();
    assert_eq!(camera.aspect(), Aspect::Wide);
    assert_eq!(camera.fov_degrees().to_bits(), 1.997_261_6_f32.to_bits());
    close(camera.cotangent(), 57.368_523, 0.000_01);
    assert_ne!(camera.fov_degrees(), 2.);
    assert_eq!(camera.near().to_bits(), 0x3ca3_d70a);
    assert_eq!(camera.far(), 13_000.);
}

#[test]
fn root_override_and_camera_eye_put_script_origin_at_depth_twenty_eight() {
    let camera = Camera::new(Aspect::Wide).unwrap();
    let projected = camera.project([0., 0., 0.]).unwrap();
    assert_eq!(projected.position, [320., 240.]);
    assert_eq!(projected.clip_w, 28.);
    let right = camera.project([0.25, 0., 0.]).unwrap();
    let up = camera.project([0., 0., 0.25]).unwrap();
    close(right.position[0], 412.199_4, 0.000_1);
    close(up.position[1], 117.067_45, 0.000_1);
    assert_eq!(right.position[1], 240.);
    assert_eq!(up.position[0], 320.);
}

#[test]
fn standard_aspect_changes_horizontal_projection_without_changing_vertical_or_depth() {
    let wide = Camera::new(Aspect::Wide)
        .unwrap()
        .project([0.25, 0., 0.25])
        .unwrap();
    let standard = Camera::new(Aspect::Standard)
        .unwrap()
        .project([0.25, 0., 0.25])
        .unwrap();
    close(standard.position[0], 442.932_56, 0.000_1);
    close(
        (standard.position[0] - 320.) / (wide.position[0] - 320.),
        4. / 3.,
        0.000_01,
    );
    assert_eq!(standard.position[1], wide.position[1]);
    assert_eq!(standard.clip_w, wide.clip_w);
}

#[test]
fn projection_uses_each_point_depth_instead_of_a_fixed_pixels_per_unit_fit() {
    let camera = Camera::new(Aspect::Wide).unwrap();
    let far = camera.project([0.125, 0., 0.125]).unwrap();
    let near = camera.project([0.125, 14., 0.125]).unwrap();
    assert_eq!(near.clip_w * 2., far.clip_w);
    close(
        near.position[0] - 320.,
        2. * (far.position[0] - 320.),
        0.000_1,
    );
    close(
        240. - near.position[1],
        2. * (240. - far.position[1]),
        0.000_1,
    );
    assert!(camera.project([1., 0., 1.]).unwrap().position[0] > 640.);
}

#[test]
fn depth_row_reads_column_major_y_only_and_applies_root_then_camera_translation() {
    let camera = Camera::new(Aspect::Wide).unwrap();
    let mut model = [f32::NAN; 16];
    model[1] = 2.;
    model[5] = -3.;
    model[9] = 0.5;
    model[13] = 7.;
    assert_eq!(camera.depth_row(model).unwrap(), [2., -3., 0.5, -21.]);
    model[13] = 0.;
    assert_eq!(camera.depth_row(model).unwrap()[3], -28.);
    model[13] = 28.;
    assert_eq!(camera.depth_row(model).unwrap()[3], 0.);
    model[5] = f32::INFINITY;
    assert!(camera.depth_row(model).is_err());
}

#[test]
fn invalid_points_and_unsupported_depth_clipping_fail_without_clamping() {
    let camera = Camera::new(Aspect::Wide).unwrap();
    for point in [
        [f32::NAN, 0., 0.],
        [0., f32::INFINITY, 0.],
        [0., 0., f32::NEG_INFINITY],
        [0., 28., 0.],
        [0., 29., 0.],
        [0., 27.99, 0.],
        [0., -12_973., 0.],
        [f32::MAX, 0., 0.],
    ] {
        assert!(camera.project(point).is_err(), "{point:?}");
    }
    assert_eq!(camera.project([0., -12_972., 0.]).unwrap().clip_w, 13_000.);
    assert!(camera.project([0., 27.98, 0.]).is_ok());
}
