use super::*;

fn blend(scalar: f32) -> BlendInputs {
    BlendInputs {
        scalar_bits: scalar.to_bits(),
        child_flags_raw: 0,
        component_flags_raw: 0,
    }
}

fn frame() -> FrameInputs {
    FrameInputs {
        start: 0,
        end: 61,
        animation_frames: 61,
        normalized_position_bits: 0,
        child_flags_raw: 0x23,
        component_flags_raw: 0,
        animation_word10_raw: 0,
        child_format_raw: None,
        bone_mask: [u32::MAX; 4],
        skin_bone_mask: None,
    }
}

#[test]
fn strict_equality_selects_later_ordered_entry_without_sorting_or_merging() {
    let keys = [0.0f32, 0.5, 0.7, 1.0].map(f32::to_bits);
    for (scalar, expected) in [(0.0, 0), (0.5, 1), (0.7, 2), (1.0, 3), (2.0, 3), (-1.0, 0)] {
        let selected = select(&keys, blend(scalar), BlendConstants::source_initial(), 32).unwrap();
        assert_eq!(selected.source_index, expected);
        assert_eq!(selected.fraction_bits, 0);
        assert_eq!(selected.second_index, None);
    }
    let equal = [0.0f32, 0.5, 0.5, 1.0].map(f32::to_bits);
    assert_eq!(
        select(&equal, blend(0.5), BlendConstants::source_initial(), 32)
            .unwrap()
            .source_index,
        2
    );
    // No global sorting validation: later, unreached keys do not rewrite the
    // first interval or its original storage ordinal.
    let unsorted = [0.0f32, 1.0, -3.0].map(f32::to_bits);
    let selected = select(&unsorted, blend(0.25), BlendConstants::source_initial(), 8).unwrap();
    assert_eq!((selected.source_index, selected.second_index), (0, Some(1)));
}

#[test]
fn snap_and_pair_thresholds_are_distinct_and_flag_overrides_do_not_undo_index_advance() {
    let keys = [0.0f32.to_bits(), 1.0f32.to_bits()];
    let constants = BlendConstants::source_initial();
    let snapped = select(
        &keys,
        blend(f32::from_bits(constants.snap_fraction_bits)),
        constants,
        8,
    )
    .unwrap();
    assert_eq!(
        (
            snapped.source_index,
            snapped.fraction_bits,
            snapped.second_index
        ),
        (1, 0, None)
    );
    let below = select(
        &keys,
        blend(f32::from_bits(constants.snap_fraction_bits - 1)),
        constants,
        8,
    )
    .unwrap();
    assert_eq!((below.source_index, below.second_index), (0, Some(1)));
    let threshold = select(
        &keys,
        blend(f32::from_bits(constants.pair_threshold_bits)),
        constants,
        8,
    )
    .unwrap();
    assert_eq!(threshold.second_index, None);
    assert_eq!(threshold.fraction_bits, constants.pair_threshold_bits);
    assert_eq!(
        select(
            &keys,
            blend(f32::from_bits(constants.pair_threshold_bits + 1)),
            constants,
            8
        )
        .unwrap()
        .second_index,
        Some(1)
    );
    let mut forced = blend(0.5);
    forced.child_flags_raw = 0x80;
    assert_eq!(
        select(&keys, forced, constants, 8).unwrap().fraction_bits,
        0
    );
    forced.scalar_bits = constants.snap_fraction_bits;
    assert_eq!(select(&keys, forced, constants, 8).unwrap().source_index, 1);
    forced = blend(0.5);
    forced.component_flags_raw = 4;
    assert_eq!(
        select(&keys, forced, constants, 8).unwrap().second_index,
        None
    );
    let ignored = select(&[f32::NAN.to_bits()], blend(f32::NAN), constants, 0).unwrap();
    assert_eq!(
        (
            ignored.source_index,
            ignored.key_reads,
            ignored.fraction_bits
        ),
        (0, 0, 0)
    );
    assert!(select(&[], blend(0.0), constants, 8).is_err());
}

#[test]
fn reached_key_budgets_nonfinite_arithmetic_and_minimum_span_have_explicit_behavior() {
    let constants = BlendConstants::source_initial();
    let lazy = [0.0f32.to_bits(), 1.0f32.to_bits(), f32::NAN.to_bits()];
    let selected = select(&lazy, blend(0.0), constants, 5).unwrap();
    assert_eq!(selected.key_reads, 5);
    assert!(select(&lazy, blend(0.0), constants, 4).is_err());
    assert!(select(&lazy, blend(f32::NAN), constants, 32).is_err());
    assert!(select(&[f32::NAN.to_bits(), 0], blend(0.0), constants, 32).is_err());
    let tiny = [0.0f32.to_bits(), f32::from_bits(0x3300_0000).to_bits()];
    let selected = select(&tiny, blend(f32::from_bits(0x3280_0000)), constants, 8).unwrap();
    assert_eq!(
        selected.fraction_bits,
        0.125f32.to_bits(),
        "minimum denominator2^-23, not the smaller key span"
    );
}

#[test]
fn explicit_constructor_position_requests_frame_zero_without_advancing_state() {
    let inputs = frame();
    let request = frame_request(inputs, FrameConstants::source_initial()).unwrap();
    assert_eq!((request.requested_frame0, request.cache_frame0), (0, 0));
    assert_eq!(
        (request.requested_frame1, request.cache_frame1),
        (None, None)
    );
    assert_eq!(request.frame_fraction_bits, 0);
    assert_eq!((request.range_frames, request.range_end), (61, 61));
    assert_eq!(
        inputs,
        frame(),
        "planning leaves constructor inputs untouched"
    );
}

#[test]
fn frame_position_uses_double_half_remainder_and_strict_snap_bounds() {
    let constants = FrameConstants::source_initial();
    let mut inputs = frame();
    inputs.end = 1;
    inputs.animation_frames = 2;
    for (position, expected) in [
        (0.009f32, 0.0f32),
        (0.01, 0.01),
        (0.49, 0.49),
        (0.491, 0.5),
        (0.5, 0.5),
        (0.501, 0.5),
    ] {
        inputs.normalized_position_bits = position.to_bits();
        let request = frame_request(inputs, constants).unwrap();
        assert_eq!(
            request.scaled_position_bits,
            expected.to_bits(),
            "source half-grid snap for {position}"
        );
    }
}

#[test]
fn signed_spans_and_endpoint_looping_keep_source_requests_separate_from_cache_bounds() {
    let constants = FrameConstants::source_initial();
    let mut inputs = frame();
    inputs.normalized_position_bits = 1.0f32.to_bits();
    let looping = frame_request(inputs, constants).unwrap();
    assert_eq!(
        (looping.requested_frame0, looping.requested_frame1),
        (60, Some(0))
    );
    assert_eq!(looping.frame_fraction_bits, 1.0f32.to_bits());
    inputs.child_flags_raw &= !1;
    let holding = frame_request(inputs, constants).unwrap();
    assert_eq!(
        holding.requested_frame1,
        Some(60),
        "native still requests duplicate endpoint cache"
    );
    inputs.component_flags_raw = 2;
    assert_eq!(
        frame_request(inputs, constants).unwrap().requested_frame1,
        None
    );
    inputs = frame();
    inputs.start = 5;
    inputs.end = 5;
    inputs.animation_frames = 10;
    inputs.normalized_position_bits = 1.0f32.to_bits();
    let fallback = frame_request(inputs, constants).unwrap();
    assert_eq!((fallback.range_frames, fallback.range_end), (10, 15));
    assert_eq!((fallback.requested_frame0, fallback.cache_frame0), (14, 9));
    assert_eq!(
        (fallback.requested_frame1, fallback.cache_frame1),
        (Some(5), Some(5))
    );
    inputs = frame();
    inputs.start = -5;
    inputs.end = -1;
    let negative = frame_request(inputs, constants).unwrap();
    assert_eq!((negative.requested_frame0, negative.cache_frame0), (-5, 0));
    inputs.start = 5;
    inputs.end = 2;
    let reversed = frame_request(inputs, constants).unwrap();
    assert_eq!(reversed.range_frames, -3);
    assert_eq!((reversed.requested_frame0, reversed.cache_frame0), (1, 1));
}

#[test]
fn masks_formats_and_optional_skin_context_require_only_the_reached_source_inputs() {
    let mut inputs = frame();
    inputs.animation_word10_raw = 0x9234_5678;
    let request = frame_request(inputs, FrameConstants::source_initial()).unwrap();
    assert_eq!(
        request.format_flags_raw, 0x1200,
        "native high halfword is masked"
    );
    inputs.child_flags_raw |= 0x100;
    assert!(frame_request(inputs, FrameConstants::source_initial()).is_err());
    inputs.child_format_raw = Some(0xe501);
    inputs.component_flags_raw = 0x80;
    assert!(frame_request(inputs, FrameConstants::source_initial()).is_err());
    inputs.bone_mask = [0xff00, 2, 3, 4];
    inputs.skin_bone_mask = Some([0xf0f0, u32::MAX, 1, 0]);
    let request = frame_request(inputs, FrameConstants::source_initial()).unwrap();
    assert_eq!(request.format_flags_raw, 0x6500);
    assert_eq!(request.bone_mask, [0xf000, 2, 1, 0]);
    assert_eq!(request.cache_mask, [0xf001, 2, 1, 0]);
    let secondary = SkinPair {
        owner: 9,
        reference: 0,
    };
    assert_eq!(
        skin_request(0, u32::MAX, Some(secondary)).unwrap(),
        SkinRequest::BoundSecondary(secondary)
    );
    assert_eq!(
        skin_request(3, 0, None).unwrap(),
        SkinRequest::DescriptorReference(SkinPair {
            owner: 3,
            reference: 0
        })
    );
    assert!(skin_request(3, u32::MAX, None).is_err());
    assert!(skin_request(0, 0, None).is_err());
}

#[test]
fn unsupported_frame_inputs_are_errors_and_source_clamp_preserves_known_zero_behavior() {
    let constants = FrameConstants::source_initial();
    let mut inputs = frame();
    inputs.animation_frames = 0;
    assert!(frame_request(inputs, constants).is_err());
    for bits in [
        f32::NAN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
    ] {
        inputs = frame();
        inputs.normalized_position_bits = bits;
        assert!(frame_request(inputs, constants).is_err());
    }
    inputs = frame();
    inputs.normalized_position_bits = (-0.0f32).to_bits();
    assert_eq!(
        frame_request(inputs, constants)
            .unwrap()
            .scaled_position_bits,
        0,
        "source <=lower clamp chooses the supplied positive-zero bound"
    );
    inputs.normalized_position_bits = (-1.0f32).to_bits();
    assert_eq!(frame_request(inputs, constants).unwrap().cache_frame0, 0);
}
