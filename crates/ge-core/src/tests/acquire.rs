use super::*;

fn plane(y: f32) -> [SurfaceTriangle; 1] {
    [SurfaceTriangle {
        index: 0,
        vertices: [[-2., y, -2.], [2., y, -2.], [0., y, 2.]],
    }]
}
fn instance<'a>(
    triangles: &'a [SurfaceTriangle],
    centers: &'a [[f32; 3]],
    y: f32,
) -> NavigationInstance<'a> {
    NavigationInstance {
        frame: NavigationFrame {
            translation: [0.; 3],
        },
        bounds: [[-2., y, -2.], [2., y, 2.]],
        triangles,
        centers,
    }
}

#[test]
fn acquisition_uses_center_height_instead_of_surface_parameter_and_keeps_first_tie() {
    let (a, b) = (plane(0.), plane(-2.));
    let (ca, cb) = ([[0., 100., 0.]], [[0., 0.1, 0.]]);
    let candidates = [instance(&a, &ca, 0.), instance(&b, &cb, -2.)];
    let result = acquire_navigation(&candidates, [0.; 3], 10.).unwrap();
    assert_eq!((result.bounds_passed, result.surface_candidates), (2, 2));
    assert_eq!(result.selected.unwrap().instance, 1);
    let tied = [instance(&a, &cb, 0.), instance(&b, &cb, -2.)];
    assert_eq!(
        acquire_navigation(&tied, [0.; 3], 10.)
            .unwrap()
            .selected
            .unwrap()
            .instance,
        0
    );
}

#[test]
fn translation_precedes_local_lift_and_scores_centers_in_world_height() {
    let triangles = plane(0.);
    let centers = [[0.; 3]];
    let mut candidate = instance(&triangles, &centers, 0.);
    candidate.frame.translation = [8., 16_777_216., -3.];
    let result = acquire_navigation(&[candidate], [8., 16_777_216., -3.], 3.)
        .unwrap()
        .selected
        .unwrap();
    assert_eq!(result.hit.parameter, 1.);
    assert_eq!(result.center_height_difference, 0.);
}

#[test]
fn aabb_includes_segment_ends_but_surface_window_excludes_its_maximum() {
    let triangles = plane(0.);
    let centers = [[0.; 3]];
    let candidates = [instance(&triangles, &centers, 0.)];
    let result = acquire_navigation(&candidates, [0., 2., 0.], 3.).unwrap();
    assert_eq!(result.bounds_passed, 1);
    assert!(result.selected.is_none());
    assert!(
        acquire_navigation(&candidates, [0., -1., 0.], 3.)
            .unwrap()
            .selected
            .is_some()
    );
    assert_eq!(
        acquire_navigation(&candidates, [3., 0., 0.], 3.)
            .unwrap()
            .bounds_passed,
        0
    );
    assert_eq!(
        acquire_navigation(&candidates, [0., 0., 0.], 1e-6)
            .unwrap()
            .bounds_passed,
        0
    );
    assert!(
        acquire_navigation(&[], [0.; 3], 3.)
            .unwrap()
            .selected
            .is_none()
    );
}

#[test]
fn stored_bounds_are_honored_and_invalid_tables_numbers_or_overflow_are_errors() {
    let triangles = plane(0.);
    let centers = [[0.; 3]];
    let mut candidate = instance(&triangles, &centers, 0.);
    candidate.bounds = [[-2., 10., -2.], [2., 11., 2.]];
    assert_eq!(
        acquire_navigation(&[candidate], [0.; 3], 3.)
            .unwrap()
            .bounds_passed,
        0
    );
    let mut bad = instance(&triangles, &centers, 0.);
    bad.bounds[1][0] = -3.;
    assert!(acquire_navigation(&[bad], [0.; 3], 3.).is_err());
    let mut bad = instance(&triangles, &centers, 0.);
    bad.centers = &[];
    assert!(acquire_navigation(&[bad], [0.; 3], 3.).is_err());
    let mut bad = instance(&triangles, &centers, 0.);
    bad.frame.translation[0] = -f32::MAX;
    assert!(acquire_navigation(&[bad], [f32::MAX, 0., 0.], 3.).is_err());
    for distance in [0., -1., f32::NAN, f32::INFINITY] {
        assert!(acquire_navigation(&[], [0.; 3], distance).is_err());
    }
    assert!(acquire_navigation(&[], [f32::NAN; 3], 3.).is_err());
}
