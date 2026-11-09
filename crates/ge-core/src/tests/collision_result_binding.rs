use super::*;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
const TRIANGLE: [[f32; 3]; 3] = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
fn contact() -> BoxTriangleContact {
    BoxTriangleContact {
        axis: 2,
        normal: [1., 0., 0.],
        point: [0.25, 0.5, 0.75],
        point_minus_normal: [-0.75, 0.5, 0.75],
        depth: 0.25,
    }
}
fn provenance() -> ContactProvenance {
    ContactProvenance {
        collected_record_key: 123,
        first_corner_key: 456,
        position_indices: [4, 8, 12],
        triangle_flags_raw: 0x1234,
        material_raw: 0xabcd,
        query_shape_index: 258,
        triangle_in_strip: 0,
    }
}
#[test]
fn witnesses_map_in_native_order_and_triangle_normal_is_not_the_sat_normal() {
    let mut frame = IDENTITY;
    frame[0] = 2.;
    frame[5] = 3.;
    frame[10] = 4.;
    frame[12..15].copy_from_slice(&[10., 20., 30.]);
    let actual = bind_type_three(&contact(), TRIANGLE, &frame, provenance()).unwrap();
    assert_eq!(actual.normal, [0., 0., 4., 0.]);
    assert_eq!(actual.witness_a, [8.5, 21.5, 33., 0.]);
    assert_eq!(actual.witness_b, [10.5, 21.5, 33., 0.]);
    assert_eq!(actual.depth, 0.25);
    assert_eq!(actual.position_indices, [4, 8, 12]);
    assert_eq!(
        (actual.collected_record_key, actual.first_corner_key),
        (123, 456)
    );
    assert_eq!(
        (actual.triangle_flags_raw, actual.material_raw),
        (0x1234, 0xabcd)
    );
    assert_eq!((actual.query_shape_index, actual.flags_raw), (2, 0));
}
#[test]
fn odd_strip_provenance_flips_only_normal_and_indices_not_witnesses() {
    let even = bind_type_three(&contact(), TRIANGLE, &IDENTITY, provenance()).unwrap();
    let odd = bind_type_three(
        &contact(),
        TRIANGLE,
        &IDENTITY,
        ContactProvenance {
            triangle_in_strip: 3,
            ..provenance()
        },
    )
    .unwrap();
    assert_eq!(odd.normal, even.normal.map(|v| -v));
    assert_eq!(odd.normal[3].to_bits(), (-0f32).to_bits());
    assert_eq!(odd.position_indices, [4, 12, 8]);
    assert_eq!(odd.witness_a, even.witness_a);
    assert_eq!(odd.witness_b, even.witness_b);
}
#[test]
fn exact_zero_and_underflow_use_x_fallback_but_small_nonzero_normals_normalize() {
    let zero = bind_type_three(&contact(), [[0.; 3]; 3], &IDENTITY, provenance()).unwrap();
    assert_eq!(zero.normal, [1., 0., 0., 0.]);
    let underflow = [[0.; 3], [1e-20, 0., 0.], [0., 1e-20, 0.]];
    assert_eq!(
        bind_type_three(&contact(), underflow, &IDENTITY, provenance())
            .unwrap()
            .normal,
        [1., 0., 0., 0.]
    );
    let small = [[0.; 3], [1e-5, 0., 0.], [0., 1e-5, 0.]];
    let normal = bind_type_three(&contact(), small, &IDENTITY, provenance())
        .unwrap()
        .normal;
    assert!(normal[0] == 0. && normal[1] == 0. && (normal[2] - 1.).abs() < 2e-7);
}
#[test]
fn normal_uses_input_w_zero_points_use_one_then_force_output_w_zero() {
    let mut frame = IDENTITY;
    frame[11] = 2.;
    frame[12] = 10.;
    let actual = bind_type_three(&contact(), TRIANGLE, &frame, provenance()).unwrap();
    assert_eq!(actual.normal, [0., 0., 1., 2.]);
    assert_eq!(actual.witness_b, [10.25, 0.5, 0.75, 0.]);
    assert_eq!(actual.witness_a[3].to_bits(), 0);
    assert_eq!(actual.witness_b[3].to_bits(), 0);
}
#[test]
fn point_transform_rounds_x_before_fused_y_z_and_translation() {
    let mut frame = IDENTITY;
    frame[4] = 1.;
    frame[12] = 1e8;
    let mut value = contact();
    value.point = [-1e8, 1., 0.];
    value.point_minus_normal = value.point;
    assert_eq!(
        bind_type_three(&value, TRIANGLE, &frame, provenance())
            .unwrap()
            .witness_b[0],
        0.
    );
}
#[test]
fn nonfinite_used_values_and_overflow_are_errors_while_sat_normal_is_unread() {
    let mut value = contact();
    value.normal = [f32::NAN; 3];
    assert!(bind_type_three(&value, TRIANGLE, &IDENTITY, provenance()).is_ok());
    value.point[0] = f32::NAN;
    assert!(bind_type_three(&value, TRIANGLE, &IDENTITY, provenance()).is_err());
    let mut frame = IDENTITY;
    frame[0] = f32::MAX;
    let mut value = contact();
    value.point[0] = 2.;
    assert!(bind_type_three(&value, TRIANGLE, &frame, provenance()).is_err());
    assert!(
        bind_type_three(
            &contact(),
            [[f32::MAX; 3], [-f32::MAX; 3], [0.; 3]],
            &IDENTITY,
            provenance()
        )
        .is_err()
    );
}
#[test]
fn capacity_checks_keep_reserved_slot_unsigned_zero_and_post_hit_distinction() {
    assert!(can_begin_mesh(0, 0));
    assert!(!can_append(0, 0));
    assert!(!can_begin_mesh(u32::MAX, 0));
    assert!(!can_begin_mesh(0, 1));
    assert!(can_append(0, 1));
    assert!(can_begin_mesh(0, 2));
    assert!(!can_begin_mesh(1, 2));
    assert!(can_append(1, 2));
    assert!(!can_append(2, 2));
    assert!(!can_append(3, 2));
    assert!(!can_begin_mesh(u32::MAX - 1, u32::MAX));
    assert!(can_append(u32::MAX - 1, u32::MAX));
}
#[test]
fn authored_contacts_agree_with_independent_f64_geometry_and_frame_mapping() {
    let mut state = 972u32;
    let mut next = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((state >> 24) as i32 - 128) as f32 / 16.
    };
    let mut compared = 0;
    for _ in 0..1000 {
        let triangle = std::array::from_fn(|_| std::array::from_fn(|_| next()));
        let a = triangle[0].map(f64::from);
        let b = triangle[1].map(f64::from);
        let c = triangle[2].map(f64::from);
        let e = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let f = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
        let cross = [
            e[1] * f[2] - e[2] * f[1],
            e[2] * f[0] - e[0] * f[2],
            e[0] * f[1] - e[1] * f[0],
        ];
        let length = cross.into_iter().map(|v| v * v).sum::<f64>().sqrt();
        if length < 0.0001 {
            continue;
        }
        let expected = cross.map(|v| v / length);
        let mut frame = IDENTITY;
        for col in 0..3 {
            for row in 0..3 {
                frame[col * 4 + row] = next() / 4.;
            }
        }
        frame[12..15].copy_from_slice(&[next(), next(), next()]);
        let mut value = contact();
        value.point = std::array::from_fn(|_| next());
        value.point_minus_normal = std::array::from_fn(|_| next());
        let actual = bind_type_three(&value, triangle, &frame, provenance()).unwrap();
        for row in 0..3 {
            let normal = (0..3)
                .map(|col| f64::from(frame[col * 4 + row]) * expected[col])
                .sum::<f64>();
            assert!((f64::from(actual.normal[row]) - normal).abs() < 0.00001);
            for (point, bound) in [
                (value.point, actual.witness_b),
                (value.point_minus_normal, actual.witness_a),
            ] {
                let mapped = f64::from(frame[12 + row])
                    + (0..3)
                        .map(|col| f64::from(frame[col * 4 + row]) * f64::from(point[col]))
                        .sum::<f64>();
                assert!((f64::from(bound[row]) - mapped).abs() < 0.00001);
            }
        }
        compared += 1;
    }
    assert!(compared > 990);
}
