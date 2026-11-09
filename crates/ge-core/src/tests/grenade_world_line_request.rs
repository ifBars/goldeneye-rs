use super::*;

#[test]
fn constructor_bounds_retain_unlifted_endpoint_and_source_request_settings() {
    let request = GrenadeWorldLineRequest::from_target([2., 3., 4.], [8., 7., -2.], 0).unwrap();
    assert_eq!(request.origin, [2., 3., 4.]);
    assert_eq!(request.constructor_delta, [6., 4., -6.]);
    assert_eq!(request.delta, [6., 5., -6.]);
    assert_eq!(request.initial_world_bounds.minimum, [2., 3., -2.]);
    assert_eq!(request.initial_world_bounds.maximum, [8., 7., 4.]);
    let lifted_bounds = line_bounds::endpoint_bounds(request.origin, request.delta, 1.).unwrap();
    assert_eq!(lifted_bounds.maximum[1], 8.);
    assert_ne!(request.initial_world_bounds.maximum, lifted_bounds.maximum);
    assert_eq!(request.maximum_parameter, 1.);
    assert_eq!(
        (
            request.filter_slot,
            request.filter_flags,
            request.required_any,
            request.excluded_any,
            request.stop_first_raw
        ),
        (1, 0, 0, 0, 0)
    );
}

#[test]
fn every_nonzero_lift_byte_selects_point_one_without_altering_constructor_delta() {
    let one = GrenadeWorldLineRequest::from_target([0.; 3], [2., -1., 3.], 1).unwrap();
    let raw = GrenadeWorldLineRequest::from_target([0.; 3], [2., -1., 3.], 255).unwrap();
    assert_eq!(raw.constructor_delta, [2., -1., 3.]);
    assert_eq!(raw.delta, one.delta);
    assert_eq!(raw.delta[1], -0.9);
    assert_eq!(raw.initial_world_bounds.minimum, [0., -1., 0.]);
    assert_eq!(raw.initial_world_bounds.maximum, [2., 0., 3.]);
}

#[test]
fn zero_constructor_segment_stays_zero_in_initial_bounds_before_lift() {
    let request = GrenadeWorldLineRequest::from_target([3., 4., 5.], [3., 4., 5.], 0).unwrap();
    assert_eq!(request.constructor_delta, [0.; 3]);
    assert_eq!(request.delta, [0., 1., 0.]);
    assert_eq!(request.initial_world_bounds.minimum, request.origin);
    assert_eq!(request.initial_world_bounds.maximum, request.origin);
}

#[test]
fn invalid_target_or_constructor_arithmetic_is_an_error() {
    assert!(GrenadeWorldLineRequest::from_target([f32::NAN, 0., 0.], [0.; 3], 1).is_err());
    assert!(GrenadeWorldLineRequest::from_target([0.; 3], [0., f32::INFINITY, 0.], 1).is_err());
    assert!(
        GrenadeWorldLineRequest::from_target([-f32::MAX, 0., 0.], [f32::MAX, 0., 0.], 1).is_err()
    );
}
