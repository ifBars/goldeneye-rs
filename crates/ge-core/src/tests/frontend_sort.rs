use super::*;

fn explicit(values: &[f32]) -> Vec<Entity> {
    values
        .iter()
        .enumerate()
        .map(|(payload, &depth)| Entity {
            payload,
            depth: Depth::Explicit(depth),
        })
        .collect()
}

fn computed(payload: usize, depth: f32) -> Entity {
    Entity {
        payload,
        depth: Depth::Computed {
            center: [0., depth, 0.],
            view_row: [0., 1., 0., 0.],
        },
    }
}

fn payloads(entities: &[Entity]) -> Vec<usize> {
    order_entities(entities, entities.len())
        .unwrap()
        .iter()
        .map(|record| record.payload)
        .collect()
}

#[test]
fn translucent_forward_order_is_ascending_depth_across_signs() {
    assert_eq!(payloads(&explicit(&[-18., -28., -8.])), [1, 0, 2]);
    assert_eq!(payloads(&explicit(&[-1., 2., -2., 1.])), [2, 0, 3, 1]);
    assert_eq!(payloads(&explicit(&[4., -4., 0., -0.])), [1, 3, 2, 0]);
}

#[test]
fn negative_exact_ties_reverse_but_positive_exact_ties_are_stable() {
    assert_eq!(payloads(&explicit(&[-18., -18., -18.])), [2, 1, 0]);
    assert_eq!(payloads(&explicit(&[18., 18., 18.])), [0, 1, 2]);
}

#[test]
fn signed_zero_keys_retain_sign_classes_and_each_classes_tie_direction() {
    let ordered = order_entities(&explicit(&[-0., 0., -0., 0.]), 4).unwrap();
    assert_eq!(
        ordered.iter().map(|r| r.payload).collect::<Vec<_>>(),
        [2, 0, 1, 3]
    );
    assert_eq!(ordered[0].depth.to_bits(), 0x8000_0000);
    assert_eq!(ordered[1].depth.to_bits(), 0x8000_0000);
    assert_eq!(ordered[2].depth.to_bits(), 0);
    assert_eq!(ordered[3].depth.to_bits(), 0);
}

#[test]
fn computed_bias_can_distinguish_nearby_depths_or_round_away() {
    assert_eq!(
        payloads(&[computed(0, -18.), computed(1, -18.), computed(2, -18.)]),
        [0, 1, 2]
    );
    assert_eq!(
        payloads(&[
            computed(0, -100_000.),
            computed(1, -100_000.),
            computed(2, -100_000.)
        ]),
        [2, 1, 0]
    );
    assert_eq!(
        payloads(&[
            computed(0, 100_000.),
            computed(1, 100_000.),
            computed(2, 100_000.)
        ]),
        [0, 1, 2]
    );
}

#[test]
fn running_bias_uses_repeated_f32_additions_not_ordinal_multiplication() {
    let entities: Vec<_> = (0..12).map(|i| computed(i, 0.)).collect();
    let ordered = order_entities(&entities, 12).unwrap();
    assert_eq!(ordered[0].depth.to_bits(), 0);
    assert_eq!(ordered[1].depth.to_bits(), 0x38d1_b717);
    assert_eq!(ordered[11].depth.to_bits(), 0x3a90_2ddf);
    assert_eq!((11.0_f32 * BIAS_STEP).to_bits(), 0x3a90_2de0);
    assert_ne!(
        ordered[11].depth.to_bits(),
        (11.0_f32 * BIAS_STEP).to_bits()
    );
    // Each invocation is a new native queue section; no cross-frame bias leaks.
    assert_eq!(
        order_entities(&entities[..1], 1).unwrap()[0]
            .depth
            .to_bits(),
        0
    );
}

#[test]
fn explicit_keys_and_signed_header_overrides_skip_both_bias_operations() {
    assert_eq!(override_depth(-1).unwrap(), 200_001.);
    assert_eq!(override_depth(1).unwrap(), -200_001.);
    assert_eq!(override_depth(-32_768).unwrap(), 232_768.);
    assert_eq!(override_depth(32_767).unwrap(), -232_767.);
    assert!(override_depth(i32::MIN).unwrap().is_finite());
    assert!(override_depth(i32::MAX).unwrap().is_finite());
    assert!(override_depth(0).is_err());
    let entities = [
        Entity {
            payload: 0,
            depth: Depth::Explicit(override_depth(1).unwrap()),
        },
        computed(1, 0.),
        Entity {
            payload: 2,
            depth: Depth::Explicit(override_depth(-1).unwrap()),
        },
        computed(3, 0.),
    ];
    let ordered = order_entities(&entities, 4).unwrap();
    assert_eq!(
        ordered.iter().map(|r| r.payload).collect::<Vec<_>>(),
        [0, 1, 3, 2]
    );
    assert_eq!(ordered[1].depth.to_bits(), 0);
    assert_eq!(ordered[2].depth.to_bits(), BIAS_STEP.to_bits());
}

#[test]
fn depth_transform_preserves_separate_products_and_left_associated_sums() {
    assert_eq!(center_depth([2., 3., 4.], [5., 6., 7., 8.]).unwrap(), 64.);
    // The first product rounds before addition; fusing it would return one.
    assert_eq!(
        center_depth([4097., 4096., 0.], [4097., -4098., 0., 0.]).unwrap(),
        0.
    );
    assert_eq!(4097.0_f32.mul_add(4097., 4096. * -4098.), 1.);
    // The final translation is added only after the three weighted components.
    assert_eq!(
        center_depth([1., 1., 1.], [16_777_216., 1., -16_777_216., 1.]).unwrap(),
        1.
    );
}

#[test]
fn entity_payload_groups_remain_contiguous_regardless_of_batch_counts() {
    let batches = [vec![10, 11], vec![20], vec![30, 31, 32]];
    let entities = [computed(0, -18.), computed(1, -28.), computed(2, -8.)];
    let ordered = order_entities(&entities, 3).unwrap();
    let drawn: Vec<_> = ordered
        .iter()
        .flat_map(|entity| batches[entity.payload].iter().copied())
        .collect();
    assert_eq!(drawn, [20, 10, 11, 30, 31, 32]);
    assert_eq!(
        ordered.iter().map(|r| r.source_index).collect::<Vec<_>>(),
        [1, 0, 2]
    );
}

#[test]
fn nonfinite_keys_inputs_outputs_and_exhausted_record_budgets_are_errors() {
    assert!(order_entities(&explicit(&[0., 1.]), 1).is_err());
    assert!(order_entities(&[], 0).unwrap().is_empty());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(order_entities(&explicit(&[value]), 1).is_err());
        for axis in 0..3 {
            let mut center = [0.; 3];
            center[axis] = value;
            assert!(center_depth(center, [0.; 4]).is_err());
        }
        for axis in 0..4 {
            let mut row = [0.; 4];
            row[axis] = value;
            assert!(center_depth([0.; 3], row).is_err());
        }
    }
    assert!(center_depth([f32::MAX, 0., 0.], [2., 0., 0., 0.]).is_err());
}
