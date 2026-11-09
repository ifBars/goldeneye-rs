use super::*;

#[derive(Clone)]
struct Segment {
    delta: u16,
    mask: u16,
    coefficients: Vec<f32>,
}
fn constant(value: f32) -> Vec<Segment> {
    vec![Segment {
        delta: 3,
        mask: 1,
        coefficients: vec![value],
    }]
}
fn payload(streams: Vec<Vec<Segment>>) -> Vec<u8> {
    let mut metadata = vec![0; 2];
    let mut floats = Vec::new();
    for segments in streams {
        let count = segments.iter().map(|s| s.coefficients.len()).sum::<usize>();
        metadata.extend_from_slice(&(count as u16).to_be_bytes());
        metadata.extend_from_slice(&((2 + segments.len()) as u16).to_be_bytes());
        for segment in segments {
            metadata.extend_from_slice(&((segment.delta << 6) | segment.mask).to_be_bytes());
            for coefficient in segment.coefficients {
                floats.extend_from_slice(&coefficient.to_bits().to_be_bytes());
            }
        }
    }
    while !metadata.len().is_multiple_of(4) {
        metadata.push(0);
    }
    let offset = metadata.len() / 4;
    metadata[..2].copy_from_slice(&(offset as u16).to_be_bytes());
    metadata.extend(floats);
    metadata
}
fn layout() -> Layout {
    Layout {
        frame_count: 4,
        rotation_count: 1,
        translation_mask: [0; 4],
        auxiliary_count: 0,
    }
}
fn budget() -> Budget {
    Budget {
        scalar_records: 100,
        keys: 100,
        coefficients: 100,
    }
}
fn rotation_streams(x: Vec<Segment>) -> Vec<Vec<Segment>> {
    vec![x, constant(0.0), constant(0.0)]
}
fn sample(bytes: &[u8], frame: u32) -> DecodedFrame {
    decode(bytes, layout(), frame, [1, 0, 0, 0], budget()).unwrap()
}

#[test]
fn sparse_polynomial_coefficients_follow_degree_order_and_divide_frame_by_n() {
    // x = .125 + .5*t^2; frame2/N4 gives .25. N-1 would give a different result.
    let bytes = payload(rotation_streams(vec![Segment {
        delta: 3,
        mask: 0b000101,
        coefficients: vec![0.125, 0.5],
    }]));
    let frame = sample(&bytes, 2);
    assert_eq!(frame.rotations[0].quaternion_bits[0], 0.25f32.to_bits());
    assert_eq!(frame.valid_mask, [1, 0, 0, 0]);
    assert_eq!(
        frame.work,
        Work {
            scalar_records: 3,
            keys: 3,
            coefficients: 4
        }
    );
    assert!(frame.has_unverified_sqrt());
}

#[test]
fn exact_endpoint_uses_first_segment_and_later_segment_skips_prior_coefficients() {
    let bytes = payload(rotation_streams(vec![
        Segment {
            delta: 1,
            mask: 3,
            coefficients: vec![0.125, 0.0],
        },
        Segment {
            delta: 2,
            mask: 1,
            coefficients: vec![0.75],
        },
    ]));
    assert_eq!(
        sample(&bytes, 1).rotations[0].quaternion_bits[0],
        0.125f32.to_bits()
    );
    assert_eq!(
        sample(&bytes, 2).rotations[0].quaternion_bits[0],
        0.75f32.to_bits()
    );
}

#[test]
fn sparse_translation_order_and_unrequested_stream_skips_keep_real_channel_cursors() {
    let bytes = payload(vec![
        constant(f32::NAN),
        constant(0.0),
        constant(0.0),
        constant(2.0),
        constant(3.0),
        constant(4.0),
        constant(0.0),
        constant(0.0),
        constant(0.0),
    ]);
    let frame = decode(
        &bytes,
        Layout {
            rotation_count: 2,
            translation_mask: [2, 0, 0, 0],
            ..layout()
        },
        0,
        [2, 0, 0, 0],
        budget(),
    )
    .unwrap();
    assert_eq!(frame.rotations.len(), 1);
    assert_eq!(frame.rotations[0].bone_index, 1);
    assert_eq!(
        frame.translations,
        vec![Translation {
            bone_index: 1,
            translation_bits: [
                2.0f32.to_bits(),
                3.0f32.to_bits(),
                4.0f32.to_bits(),
                1.0f32.to_bits()
            ]
        }]
    );
    assert_eq!(frame.valid_mask, [2, 0, 0, 0]);
    assert_eq!(frame.work.scalar_records, 9);
    assert_eq!(frame.work.keys, 6);
    assert_eq!(frame.work.coefficients, 6);
}

#[test]
fn epsilon_reconstruction_preserves_sign_of_zero_without_native_estimate() {
    let (negative, precision) = quaternion([0.0, f32::from_bits(0x3f7f_ffff), 4.0]);
    assert_eq!(negative, [0, 0x3f7f_ffff, 0, (-0.0f32).to_bits()]);
    assert_eq!(precision, Precision::ExactBranch);
    let (positive, precision) = quaternion([0.0, f32::from_bits(0x3f7f_ffff), 0.0]);
    assert_eq!(positive[3], 0);
    assert_eq!(precision, Precision::ExactBranch);
    let bytes = payload(vec![
        constant(0.0),
        constant(f32::from_bits(0x3f7f_ffff)),
        constant(4.0),
    ]);
    assert_eq!(sample(&bytes, 0).rotations[0].quaternion_bits, negative);
}

#[test]
fn nonpositive_radicand_is_preserved_instead_of_clamped_or_identity() {
    let (bits, precision) = quaternion([2.0, 0.0, 0.0]);
    assert_eq!(bits, [2.0f32.to_bits(), 0, 0, (-3.0f32).to_bits()]);
    assert_eq!(precision, Precision::ExactBranch);
    let (bits, _) = quaternion([1.0, 0.0, 4.0]);
    assert_eq!(bits[3], 0); // Nonpositive branch stores radicand before sign handling.
}

#[test]
fn positive_sqrt_path_is_explicitly_unverified_and_never_fake_identity() {
    let bytes = payload(rotation_streams(constant(0.5)));
    let frame = sample(&bytes, 0);
    let rotation = frame.rotations[0];
    assert_eq!(rotation.precision, Precision::HostSqrtUnverified);
    assert_eq!(rotation.quaternion_bits[0], 0.5f32.to_bits());
    assert_eq!(rotation.quaternion_bits[3], 0.75f32.sqrt().to_bits());
    assert!(frame.has_unverified_sqrt());
}

#[test]
fn repeated_zero_delta_keys_are_valid_bounded_traversal_and_empty_masks_read_no_coefficients() {
    let bytes = payload(rotation_streams(vec![
        Segment {
            delta: 0,
            mask: 1,
            coefficients: vec![0.125],
        },
        Segment {
            delta: 0,
            mask: 0,
            coefficients: vec![],
        },
        Segment {
            delta: 3,
            mask: 1,
            coefficients: vec![0.5],
        },
    ]));
    assert_eq!(
        sample(&bytes, 0).rotations[0].quaternion_bits[0],
        0.125f32.to_bits()
    );
    assert_eq!(
        sample(&bytes, 2).rotations[0].quaternion_bits[0],
        0.5f32.to_bits()
    );
    let empty = decode(
        &bytes,
        layout(),
        0,
        [0; 4],
        Budget {
            keys: 0,
            coefficients: 0,
            ..budget()
        },
    )
    .unwrap();
    assert!(empty.rotations.is_empty() && empty.translations.is_empty());
    assert_eq!(empty.valid_mask, [0; 4]);
    assert_eq!(empty.work.keys, 0);
}

#[test]
fn malformed_metadata_coefficients_and_truncated_reached_payloads_are_errors() {
    let original = payload(rotation_streams(constant(0.5)));
    for (at, value) in [(0, 0xffffu16), (2, 0xffff), (4, 1), (6, 0x003f)] {
        let mut bad = original.clone();
        bad[at..at + 2].copy_from_slice(&value.to_be_bytes());
        assert!(decode(&bad, layout(), 0, [1, 0, 0, 0], budget()).is_err());
    }
    for length in [0, 1, 4, original.len() - 1] {
        assert!(decode(&original[..length], layout(), 0, [1, 0, 0, 0], budget()).is_err());
    }
    let missing_segment = payload(rotation_streams(vec![Segment {
        delta: 0,
        mask: 1,
        coefficients: vec![0.5],
    }]));
    assert!(decode(&missing_segment, layout(), 2, [1, 0, 0, 0], budget()).is_err());
}

#[test]
fn explicit_work_budgets_fail_instead_of_reporting_a_missing_frame() {
    let bytes = payload(rotation_streams(constant(0.5)));
    for budget in [
        Budget {
            scalar_records: 2,
            ..budget()
        },
        Budget {
            keys: 0,
            ..budget()
        },
        Budget {
            coefficients: 0,
            ..budget()
        },
    ] {
        assert!(
            decode(&bytes, layout(), 0, [1, 0, 0, 0], budget)
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
    }
}

#[test]
fn unsupported_layouts_frame_indices_and_out_of_range_translation_masks_are_explicit() {
    let bytes = payload(rotation_streams(constant(0.5)));
    for layout in [
        Layout {
            frame_count: 0,
            ..layout()
        },
        Layout {
            frame_count: 129,
            ..layout()
        },
        Layout {
            rotation_count: 0,
            ..layout()
        },
        Layout {
            rotation_count: 129,
            ..layout()
        },
        Layout {
            auxiliary_count: 1,
            ..layout()
        },
        Layout {
            translation_mask: [2, 0, 0, 0],
            ..layout()
        },
    ] {
        assert!(decode(&bytes, layout, 0, [1, 0, 0, 0], budget()).is_err());
    }
    assert!(decode(&bytes, layout(), 4, [1, 0, 0, 0], budget()).is_err());
    let frame = decode(&bytes, layout(), 0, [1, u32::MAX, 0, 0], budget()).unwrap();
    assert_eq!(frame.valid_mask, [1, 0, 0, 0]);
}
