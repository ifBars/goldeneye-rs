use super::*;

fn triangle(index: usize, y: f32) -> SurfaceTriangle {
    SurfaceTriangle {
        index,
        vertices: [[0., y, 0.], [2., y, 0.], [0., y, 2.]],
    }
}
fn ray() -> SurfaceRay {
    SurfaceRay {
        origin: [0.5, 3., 0.5],
        direction: [0., -1., 0.],
        max_parameter: 10.,
    }
}

#[test]
fn storage_order_wins_over_nearest_hit_and_groups_preserve_original_indices() {
    let triangles = [triangle(0, 0.), triangle(1, 2.)];
    let hit = first_surface_hit(ray(), triangles).unwrap().unwrap();
    assert_eq!(hit.triangle, 0);
    assert!((hit.parameter - 3.).abs() < 1e-4);
    for (actual, expected) in hit.position.into_iter().zip([0.5, 0., 0.5]) {
        assert!((actual - expected).abs() < 1e-4);
    }
    let groups = [0..1, 1..2];
    let selected = group_range(2, &groups, 1).unwrap();
    assert_eq!(selected, 1..2);
    let hit = first_surface_hit(ray(), triangles[selected].iter().copied())
        .unwrap()
        .unwrap();
    assert_eq!(hit.triangle, 1);
    assert!((hit.parameter - 1.).abs() < 1e-4);
    for invalid in [-1, i32::MIN, 2, i32::MAX] {
        assert_eq!(group_range(2, &groups, invalid).unwrap(), 0..2);
    }
    assert!(group_range(2, std::slice::from_ref(&(0..3)), 0).is_err());
}

#[test]
fn strict_parameter_limits_do_not_normalize_nonunit_directions() {
    let t = [triangle(0, 0.)];
    let mut query = SurfaceRay {
        origin: [0.5, 2., 0.5],
        direction: [0., -2., 0.],
        max_parameter: 1.1,
    };
    assert_eq!(first_surface_hit(query, t).unwrap().unwrap().parameter, 1.);
    for maximum in [-1., 0., 1.] {
        query.max_parameter = maximum;
        assert!(first_surface_hit(query, t).unwrap().is_none());
    }
    query.origin[1] = 0.;
    query.max_parameter = 10.;
    assert!(first_surface_hit(query, t).unwrap().is_none());
    query.origin[1] = -1.;
    assert!(first_surface_hit(query, t).unwrap().is_none());
}

#[test]
fn inclusive_edges_two_sided_hits_and_outside_points_are_distinct() {
    for mut t in [triangle(0, 0.)] {
        for winding in 0..2 {
            if winding == 1 {
                t.vertices.swap(1, 2);
            }
            for point in [[0., 3., 0.], [1., 3., 1.], [0.5, 3., 0.5]] {
                assert!(
                    first_surface_hit(
                        SurfaceRay {
                            origin: point,
                            ..ray()
                        },
                        [t]
                    )
                    .unwrap()
                    .is_some()
                );
            }
            assert!(
                first_surface_hit(
                    SurfaceRay {
                        origin: [2., 3., 2.],
                        ..ray()
                    },
                    [t]
                )
                .unwrap()
                .is_none()
            );
            assert!(
                first_surface_hit(
                    SurfaceRay {
                        origin: [0.5, -1., 0.5],
                        direction: [0., 1., 0.],
                        ..ray()
                    },
                    [t]
                )
                .unwrap()
                .is_some()
            );
        }
    }
}

#[test]
fn parallel_threshold_and_degenerate_fallback_are_explicit() {
    let t = triangle(0, 0.);
    let mut query = SurfaceRay {
        origin: [0.5, 1e-7, 0.5],
        direction: [0., -1e-8, 0.],
        max_parameter: 1e6,
    };
    assert!(first_surface_hit(query, [t]).unwrap().is_some());
    // Stay away from the precision-sensitive cutoff until Wii estimates are matched.
    query.direction[1] = -1e-12;
    assert!(first_surface_hit(query, [t]).unwrap().is_none());
    query.direction = [0.; 3];
    assert!(first_surface_hit(query, [t]).unwrap().is_none());
    let degenerate = SurfaceTriangle {
        index: 7,
        vertices: [[4., 5., 6.]; 3],
    };
    let hit = first_surface_hit(
        SurfaceRay {
            origin: [1., 7., 9.],
            direction: [-1., 0., 0.],
            max_parameter: 2.,
        },
        [degenerate],
    )
    .unwrap()
    .unwrap();
    assert_eq!(hit.triangle, 7);
    assert_eq!(hit.normal, [1., 0., 0.]);
    assert_eq!(hit.position, [0., 7., 9.]);
}

#[test]
fn invalid_numbers_and_arithmetic_overflow_fail_instead_of_producing_a_fake_hit() {
    assert!(
        first_surface_hit(
            SurfaceRay {
                max_parameter: f32::NAN,
                ..ray()
            },
            [triangle(0, 0.)]
        )
        .is_err()
    );
    let invalid = SurfaceTriangle {
        index: 0,
        vertices: [[f32::INFINITY; 3]; 3],
    };
    assert!(first_surface_hit(ray(), [invalid]).is_err());
    let huge = SurfaceTriangle {
        index: 0,
        vertices: [[0., 0., 0.], [f32::MAX, 0., 0.], [0., f32::MAX, 0.]],
    };
    assert!(first_surface_hit(ray(), [huge]).is_err());
    assert!(first_surface_hit(ray(), []).unwrap().is_none());
}

#[test]
fn nearest_selection_reduces_parameter_and_keeps_storage_order_for_ties() {
    let triangles = [triangle(10, 0.), triangle(20, 2.), triangle(30, 2.)];
    assert_eq!(
        first_surface_hit(ray(), triangles)
            .unwrap()
            .unwrap()
            .triangle,
        10
    );
    let hit = nearest_surface_hit(ray(), triangles).unwrap().unwrap();
    assert_eq!(hit.triangle, 20);
    assert!((hit.parameter - 1.).abs() < 1e-4);
    // A later intersection outside the face must not tighten the nearest bound.
    let outside = SurfaceTriangle {
        index: 40,
        vertices: [[10., 2.5, 10.], [12., 2.5, 10.], [10., 2.5, 12.]],
    };
    assert_eq!(
        nearest_surface_hit(ray(), [outside, triangles[1]])
            .unwrap()
            .unwrap(),
        hit
    );
    let mut nonunit = ray();
    nonunit.direction[1] = -2.;
    nonunit.max_parameter = 0.6;
    assert_eq!(
        nearest_surface_hit(nonunit, triangles)
            .unwrap()
            .unwrap()
            .parameter,
        0.5
    );
    nonunit.max_parameter = 0.5;
    assert!(nearest_surface_hit(nonunit, triangles).unwrap().is_none());
}

#[test]
fn nearest_origin_is_inclusive_but_far_endpoint_and_negative_parameters_are_not() {
    let mut query = ray();
    query.origin[1] = 0.;
    query.max_parameter = 1.;
    let triangles = [triangle(8, -0.5), triangle(9, 0.), triangle(10, 0.)];
    let hit = nearest_surface_hit(query, triangles).unwrap().unwrap();
    assert_eq!(hit.triangle, 9);
    assert_eq!(hit.parameter, 0.);
    assert_eq!(
        first_surface_hit(query, triangles)
            .unwrap()
            .unwrap()
            .triangle,
        8
    );
    query.origin[1] = -1.;
    assert!(nearest_surface_hit(query, triangles).unwrap().is_none());
    query.max_parameter = 0.;
    assert!(nearest_surface_hit(query, triangles).unwrap().is_none());
}

#[test]
fn selected_triangle_segments_include_both_endpoints_and_use_full_displacement() {
    let t = triangle(17, 0.);
    for origin in [[0.5, 0., 0.5], [0.5, 2., 0.5]] {
        let hit = triangle_segment_hit(origin, [0., -2., 0.], t)
            .unwrap()
            .unwrap();
        assert_eq!(hit.triangle, 17);
        assert_eq!(hit.parameter, origin[1] / 2.);
    }
    let hit = triangle_segment_hit([0.5, -2., 0.5], [0., 4., 0.], t)
        .unwrap()
        .unwrap();
    assert_eq!(hit.parameter, 0.5);
    for (origin, displacement) in [
        ([0.5, 3., 0.5], [0., -2., 0.]),  // plane past segment end
        ([0.5, -1., 0.5], [0., -2., 0.]), // plane behind segment start
        ([2., 1., 2.], [0., -2., 0.]),    // plane hit outside triangle
        ([0.5, 0., 0.5], [1., 0., 0.]),   // coplanar, parallel guard rejects
        ([0.5, 1., 0.5], [0.; 3]),        // zero displacement
    ] {
        assert!(
            triangle_segment_hit(origin, displacement, t)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn new_query_variants_keep_finite_input_guards_and_empty_results() {
    let mut query = ray();
    query.direction[0] = f32::NAN;
    assert!(nearest_surface_hit(query, [triangle(0, 0.)]).is_err());
    assert!(triangle_segment_hit([f32::INFINITY; 3], [0., -1., 0.], triangle(0, 0.)).is_err());
    assert!(nearest_surface_hit(ray(), []).unwrap().is_none());
    let mut reversed = triangle(1, 0.);
    reversed.vertices.swap(1, 2);
    for t in [triangle(0, 0.), reversed] {
        assert!(
            triangle_segment_hit([1., 1., 1.], [0., -2., 0.], t)
                .unwrap()
                .is_some()
        );
        assert!(nearest_surface_hit(ray(), [t]).unwrap().is_some());
    }
}
