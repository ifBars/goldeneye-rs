use super::*;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
fn scaled() -> [f32; 16] {
    let mut m = IDENTITY;
    m[0] = 2.;
    m[5] = 4.;
    m[10] = 8.;
    m[12..15].copy_from_slice(&[10., 20., 30.]);
    m
}
#[test]
fn inverse_mode_is_only_collected_flag_two_and_rigid_does_not_normalize_scale() {
    for flags in [0, 1, 4, 8, 0xfffd] {
        let frame = CollisionFrame::from_record(&scaled(), flags).unwrap();
        assert_eq!(frame.method, InverseMethod::Rigid);
        assert_eq!(frame.general_determinant, None);
        assert_eq!(frame.local_from_world[0], 2.);
        assert_eq!(frame.local_from_world[12], -20.);
        assert_eq!(
            frame
                .query_origin([12., 24., 38.], [10., 20., 30.])
                .unwrap(),
            [4., 16., 64., 0.]
        );
    }
    for flags in [2, 3, 6, 10, 0xffff] {
        let frame = CollisionFrame::from_record(&scaled(), flags).unwrap();
        assert_eq!(frame.method, InverseMethod::General);
        assert_eq!(frame.general_determinant, Some(64.));
        assert_eq!(frame.local_from_world[0], 0.5);
        assert_eq!(frame.local_from_world[12], -5.);
        assert_eq!(
            frame
                .query_origin([12., 24., 38.], [10., 20., 30.])
                .unwrap(),
            [1., 1., 1., 0.]
        );
    }
}
#[test]
fn singular_general_inverse_keeps_zero_reciprocal_without_an_epsilon_or_identity_fallback() {
    let mut matrix = IDENTITY;
    matrix[0] = 0.;
    matrix[12] = 20.;
    let frame = CollisionFrame::from_record(&matrix, 2).unwrap();
    assert_eq!(frame.general_determinant, Some(0.));
    assert!(frame.local_from_world.into_iter().all(|v| v == 0.));
    assert_eq!(
        frame.query_origin([1., 2., 3.], [20., 0., 0.]).unwrap(),
        [0.; 4]
    );
    let mut tiny = IDENTITY;
    tiny[0] = 1e-8;
    let frame = CollisionFrame::from_record(&tiny, 2).unwrap();
    assert_ne!(frame.general_determinant, Some(0.));
    assert!((frame.local_from_world[0] - 1e8).abs() < 16.);
    assert!((frame.query_origin([1e-8, 0., 0.], [0.; 3]).unwrap()[0] - 1.).abs() < 1e-6);
}
#[test]
fn rigid_ignores_unused_homogeneous_inputs_while_used_errors_are_explicit() {
    let mut matrix = IDENTITY;
    for i in [3, 7, 11, 15] {
        matrix[i] = f32::NAN;
    }
    let rigid = CollisionFrame::from_record(&matrix, 0).unwrap();
    assert_eq!(rigid.local_from_world[15], 1.);
    assert!(CollisionFrame::from_record(&matrix, 2).is_err());
    matrix[0] = f32::NAN;
    assert!(CollisionFrame::from_record(&matrix, 0).is_err());
    let mut huge = IDENTITY;
    huge[0] = f32::MAX;
    huge[12] = 2.;
    assert!(CollisionFrame::from_record(&huge, 0).is_err());
    let frame = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    assert!(
        frame
            .query_origin([f32::MAX, 0., 0.], [-f32::MAX, 0., 0.])
            .is_err()
    );
    assert!(frame.query_origin([f32::NAN, 0., 0.], [0.; 3]).is_err());
}
#[test]
fn query_origin_subtracts_current_translation_before_inverse_multiplication_with_zero_w() {
    let mut matrix = IDENTITY;
    matrix[0] = 3.;
    matrix[12] = 1e8;
    let frame = CollisionFrame::from_record(&matrix, 2).unwrap();
    let origin = [1e8 + 8., 0., 0.];
    let result = frame.query_origin(origin, [1e8, 0., 0.]).unwrap();
    assert!((result[0] - 8. / 3.).abs() < 1e-6);
    let inverse_point = frame.local_from_world[0].mul_add(origin[0], frame.local_from_world[12]);
    assert_ne!(result[0], inverse_point);
    // Even a reused inverse receives the current record's translation separately.
    assert!((frame.query_origin([12., 0., 0.], [2., 0., 0.]).unwrap()[0] - 10. / 3.).abs() < 1e-6);
}
#[test]
fn projective_query_retains_fourth_lane_without_dividing() {
    let mut matrix = IDENTITY;
    matrix[3] = 0.5;
    let frame = CollisionFrame::from_record(&matrix, 2).unwrap();
    assert_eq!(
        frame.query_origin([2., 3., 4.], [0.; 3]).unwrap(),
        [2., 3., 4., -1.]
    );
}
#[test]
fn cache_reuses_only_consecutive_owner_slot_pairs_and_preserves_the_initial_sentinel() {
    let initial = FrameKey {
        owner_key: 0,
        slot: -2,
    };
    let a = FrameKey {
        owner_key: 10,
        slot: 2,
    };
    let b = FrameKey {
        owner_key: 11,
        slot: 2,
    };
    let mut unknown = FrameCache::new(None);
    assert!(
        unknown
            .resolve(initial, || panic!(
                "matching sentinel must not load a record"
            ))
            .is_err()
    );
    let known = CollisionFrame::from_record(&IDENTITY, 0).unwrap();
    let mut cache = FrameCache::new(Some(known));
    assert_eq!(
        cache.resolve(initial, || panic!("unread")).unwrap().method,
        InverseMethod::Rigid
    );
    let mut calls = 0;
    assert_eq!(
        cache
            .resolve(a, || {
                calls += 1;
                Ok((scaled(), 2))
            })
            .unwrap()
            .method,
        InverseMethod::General
    );
    assert_eq!(
        cache
            .resolve(a, || panic!("matrix and flags are both unread on reuse"))
            .unwrap()
            .method,
        InverseMethod::General
    );
    cache
        .resolve(b, || {
            calls += 1;
            Ok((IDENTITY, 0))
        })
        .unwrap();
    cache
        .resolve(a, || {
            calls += 1;
            Ok((scaled(), 0))
        })
        .unwrap();
    assert_eq!(calls, 3);
    let other_slot = FrameKey {
        owner_key: 10,
        slot: -1,
    };
    cache
        .resolve(other_slot, || {
            calls += 1;
            Ok((IDENTITY, 0))
        })
        .unwrap();
    assert_eq!(calls, 4);
}
#[test]
fn host_preparation_errors_do_not_poison_a_previous_valid_cache() {
    let a = FrameKey {
        owner_key: 1,
        slot: 0,
    };
    let b = FrameKey {
        owner_key: 2,
        slot: 0,
    };
    let mut cache = FrameCache::new(None);
    cache.resolve(a, || Ok((IDENTITY, 0))).unwrap();
    assert!(
        cache
            .resolve(b, || anyhow::bail!("missing matrix"))
            .is_err()
    );
    assert!(cache.resolve(b, || Ok(([f32::NAN; 16], 2))).is_err());
    cache
        .resolve(a, || panic!("previous valid frame remains"))
        .unwrap();
    assert_eq!(
        cache.resolve(b, || Ok((scaled(), 2))).unwrap().method,
        InverseMethod::General
    );
}
fn oracle(matrix: [f32; 16]) -> Option<[f64; 16]> {
    let mut augmented = [[0.; 8]; 4];
    for row in 0..4 {
        for col in 0..4 {
            augmented[row][col] = f64::from(matrix[col * 4 + row]);
        }
        augmented[row][row + 4] = 1.;
    }
    for col in 0..4 {
        let pivot = (col..4)
            .max_by(|&a, &b| augmented[a][col].abs().total_cmp(&augmented[b][col].abs()))
            .unwrap();
        if augmented[pivot][col].abs() < 1e-12 {
            return None;
        }
        augmented.swap(col, pivot);
        let divisor = augmented[col][col];
        for value in &mut augmented[col] {
            *value /= divisor;
        }
        let reference = augmented[col];
        for (row, values) in augmented.iter_mut().enumerate() {
            if row != col {
                let factor = values[col];
                for (value, pivot) in values.iter_mut().zip(reference) {
                    *value -= factor * pivot;
                }
            }
        }
    }
    Some(std::array::from_fn(|i| augmented[i % 4][i / 4 + 4]))
}
#[test]
fn full_projective_general_inverse_agrees_with_independent_f64_elimination() {
    let mut word = 791u32;
    let mut compared = 0;
    for _ in 0..1000 {
        let matrix = std::array::from_fn(|_| {
            word = word.wrapping_mul(1664525).wrapping_add(1013904223);
            ((word >> 24) as i32 - 128) as f32 / 32.
        });
        let Some(expected) = oracle(matrix) else {
            continue;
        };
        let norm = expected.into_iter().map(f64::abs).fold(0f64, f64::max);
        if norm > 1000. {
            continue;
        }
        let frame = CollisionFrame::from_record(&matrix, 2).unwrap();
        assert_ne!(frame.general_determinant, Some(0.));
        for (actual, expected) in frame.local_from_world.into_iter().zip(expected) {
            assert!(
                (f64::from(actual) - expected).abs() <= 0.0001 * norm.max(1.),
                "inverse {actual} vs {expected}; {matrix:?}"
            );
        }
        for row in 0..4 {
            for col in 0..4 {
                let value = (0..4)
                    .map(|k| {
                        f64::from(matrix[k * 4 + row])
                            * f64::from(frame.local_from_world[col * 4 + k])
                    })
                    .sum::<f64>();
                let target = if row == col { 1. } else { 0. };
                assert!((value - target).abs() <= 0.0001 * norm.max(1.));
            }
        }
        compared += 1;
    }
    assert!(compared > 950);
}
