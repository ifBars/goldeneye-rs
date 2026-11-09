use super::*;

#[test]
fn target_is_second_point_and_short_lists_have_no_target() {
    assert_eq!(WaypointRoute::new(&[]).unwrap().target(), None);
    assert_eq!(WaypointRoute::new(&[[9.; 3]]).unwrap().target(), None);
    let points = [[0.; 3], [2., 3., 4.], [8.; 3]];
    assert_eq!(
        WaypointRoute::new(&points).unwrap().target(),
        Some(points[1])
    );
}

#[test]
fn arrival_requires_two_points_and_strict_horizontal_quarter_unit_radius() {
    let points = [[0.; 3], [1., 5., 2.], [3.; 3]];
    assert!(
        !WaypointRoute::new(&points)
            .unwrap()
            .arrived(points[2])
            .unwrap()
    );
    for count in 0..=1 {
        assert!(
            !WaypointRoute::new(&points[..count])
                .unwrap()
                .arrived(points[1])
                .unwrap()
        );
    }
    let route = WaypointRoute::new(&points[..2]).unwrap();
    assert!(route.arrived([1., -2000., 2.]).unwrap());
    assert!(!route.arrived([1.25, 5., 2.]).unwrap());
    assert!(route.arrived([1.249, 2000., 2.]).unwrap());
    assert!(!route.arrived([1., 5., 2.25]).unwrap());
    assert!(!route.arrived([1.2, 5., 2.2]).unwrap());
    let heights = [[0.; 3], [0., f32::MAX, 0.]];
    assert!(
        WaypointRoute::new(&heights)
            .unwrap()
            .arrived([0., -f32::MAX, 0.])
            .unwrap()
    );
}

#[test]
fn route_length_includes_height_duplicates_and_tiny_segments_without_cost_cutoff() {
    assert_eq!(WaypointRoute::new(&[]).unwrap().length().unwrap(), 0.);
    assert_eq!(
        WaypointRoute::new(&[[1.; 3]]).unwrap().length().unwrap(),
        0.
    );
    let points = [[0.; 3], [0., 3., 4.], [0., 3., 4.], [0., 3., 16.]];
    assert_eq!(WaypointRoute::new(&points).unwrap().length().unwrap(), 17.);
    let tiny = [[0.; 3], [0.00001, 0., 0.]];
    assert_eq!(
        WaypointRoute::new(&tiny).unwrap().length().unwrap(),
        0.00001
    );
}

#[test]
fn corridor_is_strict_full_3d_and_squares_signed_radius() {
    let points = [[0.; 3], [2., 0., 0.]];
    let route = WaypointRoute::new(&points).unwrap();
    assert_eq!(
        route.corridor_segment([1., 0.5, 0.], 0.5, None).unwrap(),
        None
    );
    assert_eq!(
        route.corridor_segment([1., 0.49, 0.], 0.5, None).unwrap(),
        Some(0)
    );
    assert_eq!(
        route.corridor_segment([1., 0., 0.49], -0.5, None).unwrap(),
        Some(0)
    );
    assert_eq!(
        route.corridor_segment([1., 0., 0.], 0., None).unwrap(),
        None
    );
    assert_eq!(
        route.corridor_segment([2.1, 0., 0.], 0.5, None).unwrap(),
        Some(0)
    );
    assert_eq!(
        route.corridor_segment([20., 0., 0.], 0.5, None).unwrap(),
        None
    );
}

#[test]
fn first_end_cap_rejects_behind_but_later_segments_and_shared_starts_can_accept() {
    let points = [[0.; 3], [2., 0., 0.], [0.; 3]];
    let route = WaypointRoute::new(&points).unwrap();
    assert_eq!(
        route
            .corridor_segment([-0.1, 0., 0.], 0.5, Some(1))
            .unwrap(),
        None
    );
    assert_eq!(
        route.corridor_segment([-0.1, 0., 0.], 0.5, None).unwrap(),
        Some(1)
    );
    assert_eq!(
        route.corridor_segment([0., 0.1, 0.], 0.5, None).unwrap(),
        Some(0)
    );
    assert_eq!(
        route.corridor_segment([1., 0., 0.], 0.5, Some(0)).unwrap(),
        None
    );
    assert_eq!(
        route
            .corridor_segment([1., 0., 0.], 0.5, Some(usize::MAX))
            .unwrap(),
        Some(0)
    );
}

#[test]
fn degenerate_segments_are_points_without_division_and_order_is_preserved() {
    let points = [[0.; 3], [0.; 3], [2., 0., 0.]];
    let route = WaypointRoute::new(&points).unwrap();
    assert_eq!(
        route.corridor_segment([-0.1, 0., 0.], 0.5, None).unwrap(),
        Some(0)
    );
    assert_eq!(
        route.corridor_segment([1., 0., 0.], 0.5, None).unwrap(),
        Some(1)
    );
    assert_eq!(
        WaypointRoute::new(&[])
            .unwrap()
            .corridor_segment([0.; 3], 1., None)
            .unwrap(),
        None
    );
}

#[test]
fn first_forward_gate_uses_its_own_xz_then_y_accumulation_order() {
    // Authored cancellation: distance projection is -1, while the first-segment
    // paired accumulation rounds (2^25 - 1) before adding -2^25, yielding zero.
    let points = [[0.; 3], [4096., -4096., -1.]];
    let route = WaypointRoute::new(&points).unwrap();
    assert_eq!(
        route
            .corridor_segment([8192., 8192., 1.], 20000., None)
            .unwrap(),
        Some(0)
    );
}

#[test]
fn nonfinite_inputs_and_arithmetic_overflow_are_errors() {
    assert!(WaypointRoute::new(&[[f32::NAN, 0., 0.]]).is_err());
    let points = [[0.; 3], [1.; 3]];
    let route = WaypointRoute::new(&points).unwrap();
    assert!(route.arrived([f32::INFINITY, 0., 0.]).is_err());
    assert!(route.corridor_segment([f32::NAN; 3], 1., None).is_err());
    for radius in [f32::NAN, f32::INFINITY, f32::MAX] {
        assert!(route.corridor_segment([0.; 3], radius, None).is_err());
    }
    let huge = [[-f32::MAX; 3], [f32::MAX; 3]];
    let huge = WaypointRoute::new(&huge).unwrap();
    assert!(huge.length().is_err());
    assert!(huge.arrived([0.; 3]).is_err());
    assert!(huge.corridor_segment([0.; 3], 1., None).is_err());
}
