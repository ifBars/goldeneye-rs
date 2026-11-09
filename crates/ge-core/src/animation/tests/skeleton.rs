use super::*;

fn input(parent: u16, range_end: u16) -> BoneInput {
    BoneInput {
        parent,
        range_end,
        auxiliary_raw: [0x1234, 0xabcd],
    }
}

fn budget(bones: usize) -> Budget {
    Budget {
        bones,
        parent_visits: bones,
    }
}

#[test]
fn local_commit_preserves_disabled_bones_signed_zero_and_unrebuilt_matrices() {
    let mut skeleton =
        Skeleton::initialize(&[input(ROOT_PARENT, 0), input(0, 1)], budget(2)).unwrap();
    skeleton.set_runtime_flags(RuntimeFlags {
        byte_10_raw: 3,
        byte_0e_raw: 7,
    });
    let before = skeleton.bones().to_vec();
    let locals = [
        LocalPose {
            translation_bits: [(-0.0f32).to_bits(), 2.0f32.to_bits(), 0, 0],
            quaternion_bits: [0, 0, 0, 2.0f32.to_bits()],
        },
        LocalPose {
            translation_bits: [f32::NAN.to_bits(); 4],
            quaternion_bits: [f32::NAN.to_bits(); 4],
        },
    ];
    assert_eq!(
        skeleton
            .commit_local_pose(&locals, Some([1, 0, 0, 0]), 1)
            .unwrap(),
        1
    );
    assert_eq!(
        skeleton.bones()[0].local_translation_bits(),
        locals[0].translation_bits
    );
    assert_eq!(
        skeleton.bones()[0].local_quaternion_bits(),
        locals[0].quaternion_bits
    );
    assert_eq!(skeleton.bones()[0].matrix_bits(), before[0].matrix_bits());
    assert_eq!(skeleton.bones()[1], before[1]);
    assert_eq!(
        skeleton.runtime_flags(),
        RuntimeFlags {
            byte_10_raw: 0,
            byte_0e_raw: 0
        }
    );
}

#[test]
fn commit_preflight_errors_leave_locals_and_dirty_bytes_unchanged() {
    let mut skeleton = Skeleton::initialize(&[input(ROOT_PARENT, 0)], budget(1)).unwrap();
    skeleton.set_runtime_flags(RuntimeFlags {
        byte_10_raw: 9,
        byte_0e_raw: 11,
    });
    let before = skeleton.clone();
    let good = LocalPose {
        translation_bits: [0; 4],
        quaternion_bits: [0; 4],
    };
    assert!(skeleton.commit_local_pose(&[], None, 1).is_err());
    assert!(skeleton.commit_local_pose(&[good], None, 0).is_err());
    let bad = LocalPose {
        translation_bits: [f32::INFINITY.to_bits(); 4],
        ..good
    };
    assert!(skeleton.commit_local_pose(&[bad], None, 1).is_err());
    assert_eq!(skeleton, before);
}

#[test]
fn constructor_keeps_storage_order_for_roots_siblings_and_parent_links() {
    let inputs = [
        input(ROOT_PARENT, 4),
        input(0, 3),
        input(0, 2),
        input(1, 3),
        input(ROOT_PARENT, 5),
        input(4, 5),
    ];
    let skeleton = Skeleton::initialize(&inputs, budget(6)).unwrap();
    assert_eq!(skeleton.roots(), &[0, 4]);
    assert_eq!(skeleton.bones()[0].children(), &[1, 2]);
    assert_eq!(skeleton.bones()[1].children(), &[3]);
    assert_eq!(skeleton.bones()[4].children(), &[5]);
    assert!(skeleton.bones()[2].children().is_empty());
    for (index, bone) in skeleton.bones().iter().enumerate() {
        assert_eq!(usize::from(bone.index()), index);
        assert_eq!(bone.parent_raw(), inputs[index].parent);
    }
    assert_eq!(skeleton.parent_chain(3, 3).unwrap(), [3, 1, 0]);
    assert_eq!(skeleton.parent_chain(5, 2).unwrap(), [5, 4]);
}

#[test]
fn constructor_pose_is_positive_zero_w_one_and_identity_without_sampling() {
    let skeleton = Skeleton::initialize(&[input(ROOT_PARENT, 0), input(0, 1)], budget(2)).unwrap();
    for bone in skeleton.bones() {
        assert_eq!(bone.local_translation_bits(), [0, 0, 0, 0x3f80_0000]);
        assert_eq!(bone.local_quaternion_bits(), [0, 0, 0, 0x3f80_0000]);
        assert_eq!(
            bone.matrix_bits(),
            [
                0x3f80_0000,
                0,
                0,
                0,
                0,
                0x3f80_0000,
                0,
                0,
                0,
                0,
                0x3f80_0000,
                0,
                0,
                0,
                0,
                0x3f80_0000
            ]
        );
        // Authored sampled translation differs; initialization has no sampled
        // pose input and must not substitute skin vectors or a camera offset.
        assert_ne!(
            bone.local_translation_bits(),
            [0x4000_0000, 0, 0, 0x3f80_0000]
        );
    }
}

#[test]
fn invalid_parent_bounds_and_cycles_fail_without_fabricated_roots() {
    let out_of_range = Skeleton::initialize(&[input(1, 0)], budget(1)).unwrap_err();
    assert!(out_of_range.to_string().contains("out of range"));
    for inputs in [vec![input(0, 0)], vec![input(1, 0), input(0, 1)]] {
        let error = Skeleton::initialize(&inputs, budget(inputs.len())).unwrap_err();
        assert!(error.to_string().contains("cycle"));
    }
}

#[test]
fn acyclic_forward_parent_is_explicitly_unsupported_instead_of_repaired() {
    let error = Skeleton::initialize(&[input(1, 0), input(ROOT_PARENT, 1)], budget(2)).unwrap_err();
    assert!(error.to_string().contains("unsupported forward parent"));
}

#[test]
fn initialization_and_chain_limits_are_distinct_and_empty_storage_is_valid() {
    let inputs = [input(ROOT_PARENT, 2), input(0, 2), input(1, 2)];
    assert!(Skeleton::initialize(&inputs, budget(2)).is_err());
    let error = Skeleton::initialize(
        &inputs,
        Budget {
            bones: 3,
            parent_visits: 2,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("parent-validation budget"));
    let skeleton = Skeleton::initialize(&inputs, budget(3)).unwrap();
    let before = skeleton.clone();
    assert!(skeleton.parent_chain(2, 2).is_err());
    assert!(skeleton.parent_chain(0, 0).is_err());
    assert!(skeleton.parent_chain(3, 9).is_err());
    assert_eq!(skeleton.parent_chain(2, 3).unwrap(), [2, 1, 0]);
    assert_eq!(skeleton, before);
    let empty = Skeleton::initialize(&[], budget(0)).unwrap();
    assert!(empty.bones().is_empty() && empty.roots().is_empty());
    assert!(empty.parent_chain(0, 1).is_err());
}

#[test]
fn source_equal_range_end_marks_self_at_word_boundaries() {
    let inputs: Vec<_> = (0..128).map(|index| input(ROOT_PARENT, index)).collect();
    let skeleton = Skeleton::initialize(&inputs, budget(128)).unwrap();
    assert_eq!(skeleton.bones()[0].range_bits(), [1, 0, 0, 0]);
    assert_eq!(skeleton.bones()[31].range_bits(), [0x8000_0000, 0, 0, 0]);
    assert_eq!(skeleton.bones()[32].range_bits(), [0, 1, 0, 0]);
    assert_eq!(skeleton.bones()[127].range_bits(), [0, 0, 0, 0x8000_0000]);
    let oversized = vec![input(ROOT_PARENT, 0); 129];
    let error = Skeleton::initialize(&oversized, budget(129)).unwrap_err();
    assert!(error.to_string().contains("host bound"));
}

#[test]
fn unequal_range_end_uses_exclusive_prefix_xor_including_reversed_endpoints() {
    let mut inputs: Vec<_> = (0..128).map(|index| input(ROOT_PARENT, index)).collect();
    inputs[31].range_end = 65;
    inputs[65].range_end = 31;
    inputs[96].range_end = u16::MAX;
    let skeleton = Skeleton::initialize(&inputs, budget(128)).unwrap();
    assert_eq!(
        skeleton.bones()[31].range_bits(),
        [0x8000_0000, u32::MAX, 1, 0]
    );
    assert_eq!(
        skeleton.bones()[65].range_bits(),
        skeleton.bones()[31].range_bits()
    );
    assert_eq!(skeleton.bones()[96].range_bits(), [0, 0, 0, u32::MAX]);
    // Source masks can mention absent bones: do not reinterpret range_end as
    // a validated parent index, infer descendants, or silently clamp to count.
    let one = Skeleton::initialize(&[input(ROOT_PARENT, u16::MAX)], budget(1)).unwrap();
    assert_eq!(one.bones()[0].range_bits(), [u32::MAX; 4]);
}

#[test]
fn raw_range_and_auxiliary_words_survive_without_pose_or_parent_semantics() {
    let skeleton = Skeleton::initialize(&[input(ROOT_PARENT, 0xfffe)], budget(1)).unwrap();
    let bone = &skeleton.bones()[0];
    assert_eq!(bone.range_end(), 0xfffe);
    assert_eq!(bone.auxiliary_raw(), [0x1234, 0xabcd]);
    assert_eq!(bone.parent_raw(), ROOT_PARENT);
    assert_eq!(bone.local_translation_bits(), [0, 0, 0, 0x3f80_0000]);
}
