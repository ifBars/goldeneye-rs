use super::*;
use crate::animation::skeleton::{BoneInput, LocalPose, ROOT_PARENT};

fn bits(values: [f32; 4]) -> [u32; 4] {
    values.map(f32::to_bits)
}

fn skeleton(parents: &[u16], locals: &[([f32; 4], [f32; 4])]) -> Skeleton {
    let inputs: Vec<_> = parents
        .iter()
        .enumerate()
        .map(|(index, &parent)| BoneInput {
            parent,
            range_end: index as u16,
            auxiliary_raw: [0; 2],
        })
        .collect();
    let mut skeleton = Skeleton::initialize(
        &inputs,
        crate::animation::skeleton::Budget {
            bones: parents.len(),
            parent_visits: parents.len(),
        },
    )
    .unwrap();
    let locals: Vec<_> = locals
        .iter()
        .map(|&(translation, quaternion)| LocalPose {
            translation_bits: bits(translation),
            quaternion_bits: bits(quaternion),
        })
        .collect();
    skeleton
        .commit_local_pose(&locals, None, parents.len())
        .unwrap();
    skeleton
}

fn evaluate(skeleton: &Skeleton, bone: usize, visits: usize) -> Result<RequestedBone> {
    requested_bone(
        skeleton,
        bone,
        CacheSelection::Disabled,
        &vec![Precision::ExactBranch; skeleton.bones().len()],
        Budget {
            parent_visits: visits,
        },
    )
}

#[test]
fn quaternion_register_order_preserves_nonunit_input() {
    let matrix = quaternion_matrix(bits([2.0, 3.0, 4.0, 5.0]));
    assert_eq!(
        matrix.map(f32::from_bits),
        [
            -49.0, 52.0, -14.0, 0.0, -28.0, -39.0, 44.0, 0.0, 46.0, 4.0, -25.0, 0.0, 0.0, 0.0, 0.0,
            1.0
        ]
    );
    assert_eq!(matrix[0], (-49.0f32).to_bits());
}

#[test]
fn full_product_includes_all_four_lanes_without_affine_normalization() {
    let left = std::array::from_fn(|index| (index as f32 + 1.0).to_bits());
    let right = std::array::from_fn(|index| (index as f32 + 17.0).to_bits());
    assert_eq!(
        multiply(&left, &right).map(f32::from_bits),
        [
            538.0, 612.0, 686.0, 760.0, 650.0, 740.0, 830.0, 920.0, 762.0, 868.0, 974.0, 1080.0,
            874.0, 996.0, 1118.0, 1240.0
        ]
    );
}

#[test]
fn y_accumulation_is_fused_after_the_initial_x_product() {
    let mut left = [0; 16];
    let mut right = [0; 16];
    left[0] = (-1.0f32).to_bits();
    left[4] = 0x3f80_0001; // 1 + 2^-23
    right[0] = 1.0f32.to_bits();
    right[1] = 0x3f7f_fffe; // 1 - 2^-23
    assert_eq!(multiply(&left, &right)[0], 0xa880_0000); // -2^-46
    let unfused = f32::from_bits(left[4]) * f32::from_bits(right[1]) - 1.0;
    assert_eq!(unfused.to_bits(), 0);
}

#[test]
fn requested_and_ancestor_outputs_follow_noncommuting_source_order() {
    let skeleton = skeleton(
        &[ROOT_PARENT, 0, 1],
        &[
            ([3.0, 4.0, 5.0, 7.0], [0.0, 0.0, 1.0, 1.0]),
            ([1.0, 2.0, 3.0, 8.0], [1.0, 0.0, 0.0, 1.0]),
            ([6.0, 7.0, 8.0, 9.0], [0.0, 1.0, 0.0, 1.0]),
        ],
    );
    let result = evaluate(&skeleton, 2, 3).unwrap();
    assert_eq!(result.chain, [2, 1, 0]);
    assert_eq!(result.bone, 2);
    assert_eq!(result.local_transforms, 3);
    assert_eq!(result.matrix_products, 3);
    assert_eq!(
        result.ancestors_bits.map(f32::from_bits),
        [
            -1.0, 2.0, 0.0, 0.0, 2.0, 1.0, 2.0, 0.0, 4.0, 2.0, -1.0, 0.0, -2.0, 4.0, 8.0, 1.0
        ]
    );
    assert_eq!(
        result.full_bits.map(f32::from_bits),
        [
            -7.0, -6.0, 2.0, 0.0, 2.0, 1.0, 2.0, 0.0, -6.0, 2.0, 1.0, 0.0, 38.0, 39.0, 14.0, 1.0
        ]
    );
}

#[test]
fn requested_ancestor_association_differs_from_cached_parent_first_refresh() {
    let identity_q = [0.0, 0.0, 0.0, 1.0];
    let skeleton = skeleton(
        &[ROOT_PARENT, 0, 1, 2],
        &[
            ([1.0e20, 0.0, 0.0, 1.0], identity_q),
            ([-1.0e20, 0.0, 0.0, 1.0], identity_q),
            ([3.0, 0.0, 0.0, 1.0], identity_q),
            ([4.0, 0.0, 0.0, 1.0], identity_q),
        ],
    );
    let result = evaluate(&skeleton, 3, 4).unwrap();
    assert_eq!(result.ancestors_bits[12], 0);
    assert_eq!(result.full_bits[12], 4.0f32.to_bits());
    let mut cached = matrix::identity();
    for bone in skeleton.bones() {
        let mut local = quaternion_matrix(bone.local_quaternion_bits());
        local[12..16].copy_from_slice(&bone.local_translation_bits());
        cached = multiply(&cached, &local);
    }
    assert_eq!(cached[12], 7.0f32.to_bits());
    assert_ne!(result.full_bits, cached);
}

#[test]
fn root_request_forces_translation_w_and_keeps_the_final_identity_product() {
    let skeleton = skeleton(
        &[ROOT_PARENT],
        &[([-0.0, 2.0, 3.0, 19.0], [0.0, 0.0, 0.0, 1.0])],
    );
    let before = skeleton.clone();
    let result = evaluate(&skeleton, 0, 1).unwrap();
    assert_eq!(result.chain, [0]);
    assert_eq!(result.ancestors_bits, matrix::identity());
    assert_eq!(result.full_bits[12], 0); // Identity product is not a memcpy of -0.
    assert_eq!(result.full_bits[13], 2.0f32.to_bits());
    assert_eq!(result.full_bits[15], 1.0f32.to_bits());
    assert_eq!(result.matrix_products, 1);
    assert_eq!(skeleton, before); // No cached matrices/dirty flags are refreshed.
}

#[test]
fn negative_fused_subtract_negates_after_rounding_zero() {
    // g.x = x*2x + z*2z = 1, so column1.y is -(1*1 - 1).
    let matrix = quaternion_matrix(bits([0.5, 0.0, 0.5, 0.0]));
    assert_eq!(matrix[5], (-0.0f32).to_bits());
    let algebraically_rewritten = (-1.0f32).mul_add(1.0, 1.0);
    assert_eq!(algebraically_rewritten.to_bits(), 0);
}

#[test]
fn missing_bone_budget_and_enabled_cache_never_return_identity_success() {
    let skeleton = skeleton(
        &[ROOT_PARENT, 0],
        &[
            ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
            ([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
        ],
    );
    assert!(
        evaluate(&skeleton, 2, 3)
            .unwrap_err()
            .to_string()
            .contains("out of range")
    );
    assert!(
        evaluate(&skeleton, 1, 1)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert!(evaluate(&skeleton, 0, 0).is_err());
    assert!(
        requested_bone(
            &skeleton,
            1,
            CacheSelection::Enabled,
            &[Precision::ExactBranch; 2],
            Budget { parent_visits: 2 }
        )
        .unwrap_err()
        .to_string()
        .contains("cache")
    );
    // The only public skeleton constructor rejects cycles/range failures;
    // transform deliberately consumes that actual validated owner.
    assert!(
        Skeleton::initialize(
            &[BoneInput {
                parent: 0,
                range_end: 0,
                auxiliary_raw: [0; 2]
            }],
            crate::animation::skeleton::Budget {
                bones: 1,
                parent_visits: 1
            }
        )
        .is_err()
    );
}

#[test]
fn precision_counts_only_visited_inputs_and_never_promotes_matrix_arithmetic() {
    let skeleton = skeleton(
        &[ROOT_PARENT, 0, 0],
        &[
            ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
            ([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
            ([2.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
        ],
    );
    let result = requested_bone(
        &skeleton,
        1,
        CacheSelection::Disabled,
        &[
            Precision::HostSqrtUnverified,
            Precision::ExactBranch,
            Precision::HostSqrtUnverified,
        ],
        Budget { parent_visits: 2 },
    )
    .unwrap();
    assert_eq!(result.precision.unverified_input_rotations, 1);
    assert!(result.precision.host_matrix_arithmetic_unverified);
    assert!(
        evaluate(&skeleton, 1, 2)
            .unwrap()
            .precision
            .host_matrix_arithmetic_unverified
    );
    assert!(
        requested_bone(
            &skeleton,
            1,
            CacheSelection::Disabled,
            &[Precision::ExactBranch],
            Budget { parent_visits: 2 },
        )
        .is_err()
    );
}

#[test]
fn actor_cache_first_camera_query_builds_only_the_reached_chain_then_hits_parent() {
    let skeleton = skeleton(
        &[ROOT_PARENT, 0, 1, 0],
        &[
            ([1.0, 0.0, 0.0, 7.0], [0.0, 0.0, 0.0, 1.0]),
            ([2.0, 0.0, 0.0, 8.0], [0.0, 0.0, 0.0, 1.0]),
            ([3.0, 0.0, 0.0, 9.0], [0.0, 0.0, 0.0, 1.0]),
            ([99.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
        ],
    );
    let mut cache = ActorBoneCache::fresh(4, 4).unwrap();
    assert!(
        cache
            .slots()
            .iter()
            .all(|slot| !slot.valid() && slot.value().is_none())
    );
    let result = cache
        .camera_bone(
            &skeleton,
            2,
            &[
                Precision::HostSqrtUnverified,
                Precision::HostSqrtUnverified,
                Precision::ExactBranch,
                Precision::HostSqrtUnverified,
            ],
            Budget { parent_visits: 3 },
        )
        .unwrap();
    assert_eq!(result.chain, [2, 1, 0]);
    assert_eq!(result.full_bits[12], 6.0f32.to_bits());
    assert_eq!(result.ancestors_bits[12], 3.0f32.to_bits());
    assert_eq!(result.full_bits[15], 1.0f32.to_bits());
    assert_eq!(result.local_transforms, 3);
    assert_eq!(result.matrix_products, 2);
    assert_eq!(result.precision.unverified_input_rotations, 2);
    assert!(result.precision.host_matrix_arithmetic_unverified);
    assert_eq!(
        cache.stats(),
        CacheStats {
            hits: 1,
            misses: 3,
            matrix_products: 2
        }
    );
    assert_eq!(cache.valid_count(), 3);
    assert!(!cache.slots()[3].valid());
    assert!(cache.slots()[3].matrix_bits().is_none());
}

#[test]
fn actor_cache_root_stores_local_directly_without_final_identity_product() {
    let skeleton = skeleton(
        &[ROOT_PARENT],
        &[([-0.0, 2.0, 3.0, 99.0], [0.0, 0.0, 0.0, 1.0])],
    );
    let mut cache = ActorBoneCache::fresh(1, 1).unwrap();
    let result = cache
        .camera_bone(
            &skeleton,
            0,
            &[Precision::ExactBranch],
            Budget { parent_visits: 1 },
        )
        .unwrap();
    assert_eq!(result.full_bits[12], (-0.0f32).to_bits());
    assert_eq!(result.full_bits[15], 1.0f32.to_bits());
    assert_eq!(result.ancestors_bits, matrix::identity());
    assert_eq!(result.local_transforms, 1);
    assert_eq!(result.matrix_products, 0);
    assert_eq!(
        cache.stats(),
        CacheStats {
            hits: 0,
            misses: 1,
            matrix_products: 0
        }
    );
    assert_eq!(evaluate(&skeleton, 0, 1).unwrap().full_bits[12], 0);
}

#[test]
fn actor_cache_hit_ignores_changed_locals_until_explicit_validity_only_invalidation() {
    let mut skeleton = skeleton(
        &[ROOT_PARENT, 0],
        &[
            ([2.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
            ([3.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
        ],
    );
    let mut cache = ActorBoneCache::fresh(2, 2).unwrap();
    let first = cache
        .request(
            &skeleton,
            1,
            &[Precision::HostSqrtUnverified; 2],
            Budget { parent_visits: 2 },
        )
        .unwrap();
    assert_eq!(first.matrix_bits[12], 5.0f32.to_bits());
    skeleton
        .commit_local_pose(
            &[
                LocalPose {
                    translation_bits: bits([10.0, 0.0, 0.0, 1.0]),
                    quaternion_bits: bits([0.0, 0.0, 0.0, 1.0]),
                },
                LocalPose {
                    translation_bits: bits([20.0, 0.0, 0.0, 1.0]),
                    quaternion_bits: bits([0.0, 0.0, 0.0, 1.0]),
                },
            ],
            None,
            2,
        )
        .unwrap();
    // A hit consumes neither parent-walk budget nor a precision/locals input.
    let hit = cache
        .request(&skeleton, 1, &[], Budget { parent_visits: 0 })
        .unwrap();
    assert_eq!(hit, first);
    assert_eq!(hit.precision.unverified_input_rotations, 2);
    let old_slots = cache.slots().to_vec();
    cache.invalidate_all();
    assert_eq!(cache.valid_count(), 0);
    for (old, invalid) in old_slots.iter().zip(cache.slots()) {
        assert_eq!(old.matrix_bits(), invalid.matrix_bits());
        assert_eq!(old.value(), invalid.value());
        assert!(!invalid.valid());
    }
    let rebuilt = cache
        .request(
            &skeleton,
            1,
            &[Precision::ExactBranch; 2],
            Budget { parent_visits: 2 },
        )
        .unwrap();
    assert_eq!(rebuilt.matrix_bits[12], 30.0f32.to_bits());
    assert_eq!(rebuilt.precision.unverified_input_rotations, 0);
    assert!(rebuilt.precision.host_matrix_arithmetic_unverified);
    assert_eq!(
        cache.stats(),
        CacheStats {
            hits: 1,
            misses: 4,
            matrix_products: 2
        }
    );
}

#[test]
fn actor_cache_uses_parent_first_association_instead_of_uncached_ancestor_association() {
    let q = [0.0, 0.0, 0.0, 1.0];
    let skeleton = skeleton(
        &[ROOT_PARENT, 0, 1, 2],
        &[
            ([1.0e20, 0.0, 0.0, 1.0], q),
            ([-1.0e20, 0.0, 0.0, 1.0], q),
            ([3.0, 0.0, 0.0, 1.0], q),
            ([4.0, 0.0, 0.0, 1.0], q),
        ],
    );
    let mut cache = ActorBoneCache::fresh(4, 4).unwrap();
    let result = cache
        .camera_bone(
            &skeleton,
            3,
            &[Precision::ExactBranch; 4],
            Budget { parent_visits: 4 },
        )
        .unwrap();
    assert_eq!(result.ancestors_bits[12], 3.0f32.to_bits());
    assert_eq!(result.full_bits[12], 7.0f32.to_bits());
    assert_eq!(
        evaluate(&skeleton, 3, 4).unwrap().full_bits[12],
        4.0f32.to_bits()
    );
    assert_eq!(
        cache.stats(),
        CacheStats {
            hits: 1,
            misses: 4,
            matrix_products: 3
        }
    );
}

#[test]
fn actor_cache_rejects_budgets_indices_and_incomplete_precision_atomically() {
    let skeleton = skeleton(
        &[ROOT_PARENT, 0],
        &[
            ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
            ([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]),
        ],
    );
    assert!(ActorBoneCache::fresh(2, 1).is_err());
    assert!(ActorBoneCache::fresh(129, 129).is_err());
    assert!(ActorBoneCache::fresh(0, 0).is_err());
    let mut cache = ActorBoneCache::fresh(2, 2).unwrap();
    let before = cache.clone();
    assert!(
        cache
            .request(
                &skeleton,
                1,
                &[Precision::ExactBranch; 2],
                Budget { parent_visits: 1 }
            )
            .is_err()
    );
    assert_eq!(cache, before);
    assert!(
        cache
            .request(&skeleton, 1, &[], Budget { parent_visits: 2 })
            .is_err()
    );
    assert_eq!(cache, before);
    assert!(
        cache
            .request(
                &skeleton,
                2,
                &[Precision::ExactBranch; 2],
                Budget { parent_visits: 2 }
            )
            .is_err()
    );
    assert_eq!(cache, before);
    assert!(
        ActorBoneCache::fresh(1, 1)
            .unwrap()
            .request(
                &skeleton,
                0,
                &[Precision::ExactBranch; 2],
                Budget { parent_visits: 2 },
            )
            .is_err()
    );
}
