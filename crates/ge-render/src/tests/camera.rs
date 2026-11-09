use super::*;

#[test]
fn target_projects_to_center_and_depth_is_zero_to_one() {
    let c = Camera {
        eye: [0., 0., 5.],
        target: [0., 0., 0.],
        vertical_fov_degrees: 60.,
        near: 1.,
        far: 10.,
    };
    let m = c.matrix(1.).unwrap();
    let project = |p: [f32; 3]| {
        let v = [p[0], p[1], p[2], 1.];
        let q: [f32; 4] =
            std::array::from_fn(|row| (0..4).map(|col| m[col * 4 + row] * v[col]).sum());
        [q[0] / q[3], q[1] / q[3], q[2] / q[3]]
    };
    assert_eq!(&project([0., 0., 0.])[..2], &[0., 0.]);
    assert!(project([0., 0., 4.])[2].abs() < 1e-5);
    assert!((project([0., 0., -5.])[2] - 1.).abs() < 1e-5);
    assert!(project([1., 0., 0.])[0] > 0.);
}

#[test]
fn degenerate_cameras_and_invalid_bounds_are_errors() {
    assert!(Camera::fit([[0.; 3], [0.; 3]]).is_err());
    assert!(Camera::fit([[1.; 3], [0.; 3]]).is_err());
    let mut camera = Camera::fit([[-1.; 3], [1.; 3]]).unwrap();
    camera.eye = camera.target;
    assert!(camera.matrix(1.).is_err());
    camera.eye = [0., 1., 0.];
    camera.target = [0., 0., 0.];
    assert!(camera.matrix(1.).is_err());
}

#[test]
fn inspection_orbit_pan_zoom_preserve_their_geometric_invariants() {
    let c = Camera {
        eye: [0., 0., 5.],
        target: [0.; 3],
        vertical_fov_degrees: 60.,
        near: 0.1,
        far: 100.,
    };
    let mut orbit = OrbitCamera::new(c).unwrap();
    orbit.orbit([std::f32::consts::FRAC_PI_2, 0.]).unwrap();
    let rotated = orbit.camera();
    assert!((rotated.eye[0] - 5.).abs() < 1e-5 && rotated.eye[2].abs() < 1e-5);
    assert_eq!(rotated.target, c.target);
    orbit.pan([120., 60.], 600.).unwrap();
    let panned = orbit.camera();
    for axis in 0..3 {
        assert!(
            (panned.eye[axis] - panned.target[axis] - (rotated.eye[axis] - rotated.target[axis]))
                .abs()
                < 1e-5
        );
    }
    assert_ne!(panned.target, rotated.target);
    orbit.zoom(100.).unwrap();
    assert!((length(sub(orbit.camera().eye, orbit.camera().target)) - 0.2).abs() < 1e-5);
    orbit.zoom(-100.).unwrap();
    assert!((length(sub(orbit.camera().eye, orbit.camera().target)) - 90.).abs() < 1e-4);
    orbit.orbit([0., 100.]).unwrap();
    assert!(orbit.camera().matrix(1.).is_ok());
    orbit.reset();
    assert_eq!(orbit.camera().eye, c.eye);
    assert_eq!(orbit.camera().target, c.target);
    let mut narrow = OrbitCamera::new(Camera {
        near: 1.,
        far: 1.01,
        ..c
    })
    .unwrap();
    for lines in [-100., 100.] {
        narrow.zoom(lines).unwrap();
        let distance = length(sub(narrow.camera().eye, narrow.camera().target));
        assert!(distance > 1. && distance < 1.01);
    }
}

#[test]
fn invalid_inspection_input_does_not_poison_the_camera() {
    let c = Camera::fit([[-1.; 3], [1.; 3]]).unwrap();
    let mut orbit = OrbitCamera::new(c).unwrap();
    assert!(orbit.orbit([f32::NAN, 0.]).is_err());
    assert!(orbit.pan([1., 1.], 0.).is_err());
    assert!(orbit.pan([f32::MAX, f32::MAX], f32::MIN_POSITIVE).is_err());
    assert!(orbit.zoom(f32::INFINITY).is_err());
    assert_eq!(orbit.camera().eye, c.eye);
    assert_eq!(orbit.camera().target, c.target);
}
