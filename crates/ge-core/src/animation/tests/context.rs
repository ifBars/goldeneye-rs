use super::*;
use crate::animation::frame::{Rotation, Translation, Work};
fn vector(x: f32, y: f32, z: f32, w: f32) -> [u32; 4] {
    [x, y, z, w].map(f32::to_bits)
}
fn frame() -> DecodedFrame {
    DecodedFrame {
        frame: 0,
        rotations: (0..3)
            .map(|bone| Rotation {
                bone_index: bone,
                quaternion_bits: vector(0.0, 0.0, 0.0, 1.0),
                precision: if bone == 1 {
                    Precision::HostSqrtUnverified
                } else {
                    Precision::ExactBranch
                },
            })
            .collect(),
        translations: vec![
            Translation {
                bone_index: 0,
                translation_bits: vector(2.0, 3.0, 4.0, 1.0),
            },
            Translation {
                bone_index: 2,
                translation_bits: vector(5.0, 6.0, 7.0, 1.0),
            },
        ],
        valid_mask: [7, 0, 0, 0],
        work: Work::default(),
    }
}
fn inputs<'a>(frame: &'a DecodedFrame, vectors: &'a [[u32; 4]]) -> Inputs<'a> {
    let skin = SkinTranslations {
        key: SkinKey {
            file: 0x010a_00f1,
            skin: 0x0d00_00f1,
        },
        vectors,
        auxiliary_count: 0,
    };
    Inputs {
        frame,
        source: skin,
        target: skin,
        translation_mask: [5, 0, 0, 0],
        root_bias_bits: [0; 3],
        format_flags_raw: 0,
        frame_fraction_bits: 0,
        consumer_flags_raw: 0,
    }
}
fn defaults() -> [[u32; 4]; 3] {
    [
        vector(0.0, 0.0, 0.0, 1.0),
        vector(8.0, 9.0, -0.0, 0.5),
        vector(10.0, 11.0, 12.0, 1.0),
    ]
}
#[test]
fn sparse_candidates_use_real_target_defaults_and_preserve_four_lanes_and_precision() {
    let frame = frame();
    let vectors = defaults();
    let candidate = prepare(inputs(&frame, &vectors), Budget { bones: 3 }).unwrap();
    assert_eq!(
        (
            candidate.animation_translations,
            candidate.fallback_translations
        ),
        (2, 1)
    );
    assert_eq!(
        candidate.locals[0].translation_bits,
        frame.translations[0].translation_bits
    );
    assert_eq!(candidate.locals[1].translation_bits, vectors[1]);
    assert_eq!(
        candidate.locals[1].translation_source,
        TranslationSource::TargetSkin
    );
    assert_eq!(
        candidate.locals[2].translation_bits,
        frame.translations[1].translation_bits
    );
    assert_eq!(
        candidate.locals[2].translation_source,
        TranslationSource::Animation
    );
    assert!(candidate.has_unverified_sqrt());
    assert_eq!(
        candidate.locals[1].rotation_precision,
        Precision::HostSqrtUnverified
    );
}
#[test]
fn root_adjustment_is_derived_once_and_leaves_other_translations_and_rotations_unchanged() {
    let frame = frame();
    let vectors = defaults();
    let original = frame.clone();
    let mut input = inputs(&frame, &vectors);
    input.root_bias_bits = [1.0f32.to_bits(), (-2.0f32).to_bits(), 3.0f32.to_bits()];
    let candidate = prepare(input, Budget { bones: 3 }).unwrap();
    assert_eq!(
        candidate.locals[0].translation_bits,
        vector(3.0, 1.0, 7.0, 1.0)
    );
    assert_eq!(prepare(input, Budget { bones: 3 }).unwrap(), candidate);
    assert_eq!(frame, original);
    for bone in 0..3 {
        assert_eq!(
            candidate.locals[bone].quaternion_bits,
            frame.rotations[bone].quaternion_bits
        );
    }
    assert_eq!(
        candidate.locals[2].translation_bits,
        frame.translations[1].translation_bits
    );
}
#[test]
fn same_skin_zero_and_nonzero_root_heights_both_execute_guarded_identity_scale() {
    let frame = frame();
    for y in [0.0f32, -0.0, 2.5] {
        let mut vectors = defaults();
        vectors[0][1] = y.to_bits();
        assert_eq!(
            prepare(inputs(&frame, &vectors), Budget { bones: 3 })
                .unwrap()
                .root_y_scale_bits,
            1.0f32.to_bits()
        );
    }
}
#[test]
fn unsupported_context_branches_and_owner_mismatches_are_explicit() {
    let frame = frame();
    let vectors = defaults();
    let input = inputs(&frame, &vectors);
    for changed in [
        Inputs {
            format_flags_raw: 1,
            ..input
        },
        Inputs {
            consumer_flags_raw: 0x1000,
            ..input
        },
        Inputs {
            frame_fraction_bits: 0.5f32.to_bits(),
            ..input
        },
        Inputs {
            target: SkinTranslations {
                auxiliary_count: 1,
                ..input.target
            },
            ..input
        },
        Inputs {
            target: SkinTranslations {
                key: SkinKey { file: 1, skin: 2 },
                ..input.target
            },
            ..input
        },
    ] {
        assert!(prepare(changed, Budget { bones: 3 }).is_err());
    }
    let mut other = vectors;
    other[1][0] = 1;
    assert!(
        prepare(
            Inputs {
                source: SkinTranslations {
                    vectors: &other,
                    ..input.source
                },
                ..input
            },
            Budget { bones: 3 }
        )
        .is_err()
    );
    assert!(prepare(input, Budget { bones: 2 }).is_err());
}
#[test]
fn incomplete_duplicate_sparse_and_nonfinite_frames_never_produce_candidates() {
    let vectors = defaults();
    let mut bad = frame();
    bad.valid_mask[0] = 3;
    assert!(prepare(inputs(&bad, &vectors), Budget { bones: 3 }).is_err());
    let mut bad = frame();
    bad.rotations[1].bone_index = 0;
    assert!(prepare(inputs(&bad, &vectors), Budget { bones: 3 }).is_err());
    let mut bad = frame();
    bad.translations[1].bone_index = 1;
    assert!(prepare(inputs(&bad, &vectors), Budget { bones: 3 }).is_err());
    let mut bad = frame();
    bad.rotations[0].quaternion_bits[0] = f32::NAN.to_bits();
    assert!(prepare(inputs(&bad, &vectors), Budget { bones: 3 }).is_err());
    let good = frame();
    let mut input = inputs(&good, &vectors);
    input.translation_mask = [4, 0, 0, 0];
    assert!(prepare(input, Budget { bones: 3 }).is_err());
}
