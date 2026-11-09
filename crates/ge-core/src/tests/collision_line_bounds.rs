use super::*;
fn unit() -> StoredBounds {
    StoredBounds {
        minimum: [0.; 3],
        maximum: [1.; 3],
    }
}
#[test]
fn axis_and_cross_tests_keep_closed_contacts_and_reject_oblique_aabb_false_positives() {
    for (origin, delta, expected) in [
        ([-1., 0.5, 0.5], [2., 0., 0.], true),
        ([-1., 1., 0.5], [1., 0., 0.], true),
        ([0.5; 3], [0.; 3], true),
        ([2., 0.5, 0.5], [0.; 3], false),
        ([-1., 0.9, 0.5], [1.1, 1.1, 0.], false),
    ] {
        for rotate in 0..3 {
            let rotate_vector = |v: [f32; 3]| std::array::from_fn(|i| v[(i + rotate) % 3]);
            assert_eq!(
                segment_overlaps(unit(), rotate_vector(origin), rotate_vector(delta)).unwrap(),
                expected
            );
        }
    }
}
#[test]
fn overlap_is_lazy_and_endpoint_bounds_remain_a_snapshot_across_delta_and_max_updates() {
    let bad = StoredBounds {
        minimum: [2., f32::NAN, f32::NAN],
        maximum: [f32::NAN; 3],
    };
    assert!(!overlap(unit(), bad).unwrap());
    let origin = [1., 2., 3.];
    let mut delta = [-2., 0., 4.];
    let world = endpoint_bounds(origin, delta, 1.).unwrap();
    delta[1] = 10.;
    let local = endpoint_bounds(origin, delta, 0.25).unwrap();
    assert_eq!(world.minimum, [-1., 2., 3.]);
    assert_eq!(world.maximum, [1., 2., 7.]);
    assert_eq!(local.minimum, [0.5, 2., 3.]);
    assert_eq!(local.maximum, [1., 4.5, 4.]);
    assert!(
        !overlap(
            world,
            StoredBounds {
                minimum: [0., 3., 4.],
                maximum: [1., 4., 5.]
            }
        )
        .unwrap()
    );
}
#[test]
fn early_axis_rejection_does_not_evaluate_overflowing_cross_products() {
    assert!(!segment_overlaps(unit(), [1e30; 3], [1e30; 3]).unwrap());
    assert!(segment_overlaps(unit(), [f32::NAN, 0., 0.], [0.; 3]).is_err());
    assert!(endpoint_bounds([f32::MAX; 3], [f32::MAX; 3], 1.).is_err());
}
#[test]
fn packed_face_gate_keeps_its_direction_epsilon_and_lazy_inside_path() {
    assert!(packed_segment_overlaps(unit(), [0.5; 3], [f32::NAN; 3]).unwrap());
    assert!(segment_overlaps(unit(), [-1e-7, 0.5, 0.5], [2e-7, 0., 0.]).unwrap());
    assert!(!packed_segment_overlaps(unit(), [-1e-7, 0.5, 0.5], [2e-7, 0., 0.]).unwrap());
    let epsilon = f32::from_bits(0x3727_c5ac);
    assert!(packed_segment_overlaps(unit(), [-5e-6, 0.5, 0.5], [epsilon, 0., 0.]).unwrap());
    assert!(
        !packed_segment_overlaps(
            unit(),
            [-5e-6, 0.5, 0.5],
            [f32::from_bits(epsilon.to_bits() - 1), 0., 0.]
        )
        .unwrap()
    );
}
#[test]
fn deterministic_integer_segments_agree_with_independent_f64_slab_intervals() {
    let mut state = 42u32;
    let mut integer = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((state >> 24) % 31) as i32 - 15
    };
    for _ in 0..512 {
        let origin = std::array::from_fn(|_| integer() as f32);
        let delta = std::array::from_fn(|_| integer() as f32);
        let minimum = std::array::from_fn(|_| integer() as f32);
        let maximum = std::array::from_fn(|i| minimum[i] + (integer().unsigned_abs() % 7) as f32);
        let bounds = StoredBounds { minimum, maximum };
        let mut lo = 0f64;
        let mut hi = 1f64;
        let mut expected = true;
        for axis in 0..3 {
            if delta[axis] == 0. {
                expected &= origin[axis] >= minimum[axis] && origin[axis] <= maximum[axis];
            } else {
                let a = f64::from(minimum[axis] - origin[axis]) / f64::from(delta[axis]);
                let b = f64::from(maximum[axis] - origin[axis]) / f64::from(delta[axis]);
                lo = lo.max(a.min(b));
                hi = hi.min(a.max(b));
            }
        }
        expected &= lo <= hi;
        assert_eq!(
            segment_overlaps(bounds, origin, delta).unwrap(),
            expected,
            "{origin:?}+t{delta:?} vs{minimum:?}/{maximum:?}"
        );
    }
}
