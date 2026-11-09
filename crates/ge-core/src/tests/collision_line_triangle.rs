use super::*;

const TRIANGLE: [[f32; 3]; 3] = [[0., 0., 2.], [2., 0., 2.], [0., 2., 2.]];
fn query() -> LineTriangleQuery {
    LineTriangleQuery {
        origin: [0.5, 0.5, 3.],
        delta: [0., 0., -2.],
        maximum_parameter: 1.,
        required_any: 0,
        excluded_any: 0,
    }
}
fn hit(q: &LineTriangleQuery, vertices: [[f32; 3]; 3]) -> Option<LineTriangleHit> {
    let plane = prepare_line_plane(vertices, 0).unwrap();
    intersect_line_triangle(q, plane, 0, 0, || Ok(vertices)).unwrap()
}

#[test]
fn plane_and_hit_preserve_offset_and_fractional_parameter() {
    assert_eq!(prepare_line_plane(TRIANGLE, 0).unwrap(), [0., 0., 1., -2.]);
    let result = hit(&query(), TRIANGLE).unwrap();
    assert_eq!(result.parameter, 0.5);
    assert_eq!(result.local_point, [0.5, 0.5, 2., 1.]);
    assert_eq!(result.local_plane, [0., 0., 1., -2.]);
}

#[test]
fn both_faces_and_both_windings_are_queryable() {
    let mut reverse_ray = query();
    reverse_ray.origin[2] = 1.;
    reverse_ray.delta[2] = 2.;
    for q in [query(), reverse_ray] {
        for vertices in [TRIANGLE, [TRIANGLE[0], TRIANGLE[2], TRIANGLE[1]]] {
            assert_eq!(hit(&q, vertices).unwrap().parameter, 0.5);
        }
    }
}

#[test]
fn closed_interval_accepts_start_end_and_equal_previous_parameter() {
    for (height, maximum, expected) in [(2., 1., 0.), (4., 1., 1.), (3., 0.5, 0.5)] {
        let mut q = query();
        q.origin[2] = height;
        q.maximum_parameter = maximum;
        assert_eq!(hit(&q, TRIANGLE).unwrap().parameter, expected);
    }
    let mut q = query();
    q.maximum_parameter = 0.49;
    assert!(hit(&q, TRIANGLE).is_none());
    q.maximum_parameter = -1.;
    assert!(hit(&q, TRIANGLE).is_none());
    q.origin[2] = 1.;
    q.maximum_parameter = 1.;
    assert!(hit(&q, TRIANGLE).is_none());
}

#[test]
fn triangle_vertices_edges_and_exterior_have_independent_geometric_answers() {
    for (xy, expected) in [
        ([0., 0.], true),
        ([2., 0.], true),
        ([0., 2.], true),
        ([1., 1.], true),
        ([1., 0.], true),
        ([-0.01, 0.5], false),
        ([1.01, 1.], false),
        ([0.5, -0.01], false),
    ] {
        let mut q = query();
        q.origin[..2].copy_from_slice(&xy);
        assert_eq!(hit(&q, TRIANGLE).is_some(), expected);
    }
}

#[test]
fn flags_and_plane_gates_leave_query_positions_unread() {
    let plane = prepare_line_plane(TRIANGLE, 0).unwrap();
    let q = query();
    for (flags, q) in [
        (0x200, q),
        (
            0,
            LineTriangleQuery {
                required_any: 8,
                ..q
            },
        ),
        (
            8,
            LineTriangleQuery {
                excluded_any: 8,
                ..q
            },
        ),
        (
            0,
            LineTriangleQuery {
                maximum_parameter: 0.1,
                ..q
            },
        ),
        (
            0,
            LineTriangleQuery {
                delta: [1., 0., 0.],
                ..q
            },
        ),
        (
            0,
            LineTriangleQuery {
                required_any: 0x10000,
                ..q
            },
        ),
    ] {
        assert!(
            intersect_line_triangle(&q, plane, flags, 0, || panic!("unused positions"))
                .unwrap()
                .is_none()
        );
    }
    assert!(
        intersect_line_triangle(
            &LineTriangleQuery {
                required_any: 12,
                excluded_any: 2,
                ..q
            },
            plane,
            8,
            0,
            || Ok(TRIANGLE)
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn exact_parallel_threshold_is_included_and_smaller_direction_is_rejected() {
    let mut q = query();
    q.origin[2] = 2.;
    q.delta[2] = 1e-10;
    assert!(hit(&q, TRIANGLE).is_some());
    q.delta[2] = 0.5e-10;
    assert!(hit(&q, TRIANGLE).is_none());
}

#[test]
fn odd_step_negates_all_plane_coefficients_after_acceptance() {
    let plane = prepare_line_plane(TRIANGLE, 1).unwrap();
    let even = intersect_line_triangle(&query(), plane, 0, 0, || Ok(TRIANGLE))
        .unwrap()
        .unwrap();
    let odd = intersect_line_triangle(&query(), plane, 0, 1, || Ok(TRIANGLE))
        .unwrap()
        .unwrap();
    assert_eq!(odd.parameter, even.parameter);
    assert_eq!(odd.local_point, even.local_point);
    assert_eq!(
        odd.local_plane.map(f32::to_bits),
        even.local_plane.map(|v| (-v).to_bits())
    );
    assert_eq!(odd.local_plane[3], 2.);
}

#[test]
fn resident_plane_has_threshold_fallback_and_respects_rotating_strip_slots() {
    assert_eq!(
        prepare_line_plane([[7., 8., 9.]; 3], 0).unwrap(),
        [1., 0., 0., 0.]
    );
    let tiny = [[0., 0., 2.], [0.001, 0., 2.], [0., 0.001, 2.]];
    assert_eq!(prepare_line_plane(tiny, 0).unwrap(), [1., 0., 0., 0.]);
    for ordinal in 0..9 {
        assert_eq!(
            prepare_line_plane(TRIANGLE, ordinal).unwrap(),
            [0., 0., 1., -2.]
        );
    }
}

#[test]
fn authored_grid_agrees_with_independent_f64_triangle_region() {
    for ix in -2..=10 {
        for iy in -2..=10 {
            let x = f64::from(ix) / 4.;
            let y = f64::from(iy) / 4.;
            let expected = x >= 0. && y >= 0. && x + y <= 2.;
            let mut q = query();
            q.origin = [x as f32, y as f32, 3.];
            assert_eq!(hit(&q, TRIANGLE).is_some(), expected, "{x}, {y}");
        }
    }
}

#[test]
fn malformed_consumed_values_error_and_disabled_values_remain_unused() {
    assert!(prepare_line_plane([[f32::NAN; 3]; 3], 0).is_err());
    assert!(prepare_line_plane([[f32::MAX; 3], [-f32::MAX; 3], [0.; 3]], 0).is_err());
    assert!(intersect_line_triangle(&query(), [f32::NAN; 4], 0, 0, || Ok(TRIANGLE)).is_err());
    assert!(
        intersect_line_triangle(&query(), [f32::NAN; 4], 0x200, 0, || panic!("disabled"))
            .unwrap()
            .is_none()
    );
    assert!(
        intersect_line_triangle(&query(), [0., 0., 1., -2.], 0, 0, || Ok([[f32::NAN; 3]; 3]))
            .is_err()
    );
}
