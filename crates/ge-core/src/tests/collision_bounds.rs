use super::*;
use std::cell::RefCell;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
fn bounds(minimum: [f32; 3], maximum: [f32; 3]) -> Bounds {
    Bounds {
        minimum: [minimum[0], minimum[1], minimum[2], 7.],
        maximum: [maximum[0], maximum[1], maximum[2], 9.],
    }
}
#[test]
fn affine_bounds_keep_negative_and_zero_scale_and_ignore_unused_lanes() {
    let mut matrix = IDENTITY;
    matrix[0] = -2.;
    matrix[5] = 0.;
    matrix[10] = 3.;
    matrix[12..15].copy_from_slice(&[10., 20., 30.]);
    for lane in [3, 7, 11, 15] {
        matrix[lane] = f32::NAN;
    }
    let mut input = bounds([-1., -2., -3.], [4., 5., 6.]);
    input.minimum[3] = f32::NAN;
    input.maximum[3] = f32::INFINITY;
    let actual = input.transformed(&matrix).unwrap();
    assert_eq!(actual.minimum, [2., 20., 21., 0.]);
    assert_eq!(actual.maximum, [12., 20., 48., 0.]);
    assert_eq!(actual.minimum[3].to_bits(), 0);
    assert_eq!(actual.maximum[3].to_bits(), 0);
    matrix[0] = f32::NAN;
    assert!(input.transformed(&matrix).is_err());
}
#[test]
fn endpoint_products_are_unfused_and_translation_accumulates_first() {
    let v = 1. + f32::EPSILON;
    let mut matrix = IDENTITY;
    matrix[0] = v;
    matrix[12] = -(v * v);
    let input = bounds([v, 0., 0.], [v, 0., 0.]);
    assert_eq!(input.transformed(&matrix).unwrap().minimum[0], 0.);
    assert_ne!(v.mul_add(v, matrix[12]), 0.);
    matrix = IDENTITY;
    matrix[4] = 1.;
    matrix[12] = 1e8;
    let input = bounds([-1e8, 1., 0.], [-1e8, 1., 0.]);
    assert_eq!(input.transformed(&matrix).unwrap().minimum[0], 1.);
    assert_eq!((-1e8f32 + 1.) + 1e8, 0.);
}
#[test]
fn union_ties_choose_incoming_minimum_and_existing_maximum_in_all_lanes() {
    let left = Bounds {
        minimum: [0.; 4],
        maximum: [0.; 4],
    };
    let right = Bounds {
        minimum: [-0.; 4],
        maximum: [-0.; 4],
    };
    let merged = left.union(right).unwrap();
    assert!(
        merged
            .minimum
            .into_iter()
            .all(|v| v.to_bits() == (-0f32).to_bits())
    );
    assert!(merged.maximum.into_iter().all(|v| v.to_bits() == 0));
    let mut invalid = right;
    invalid.minimum[3] = f32::NAN;
    assert!(left.union(invalid).is_err());
    let huge = Bounds {
        minimum: [f32::MAX; 4],
        maximum: [f32::MAX; 4],
    };
    let low = Bounds {
        minimum: [-f32::MAX; 4],
        maximum: [-f32::MAX; 4],
    };
    assert!(huge.union(low).is_err());
}
#[test]
fn missing_entity_returns_finite_inverted_sentinel_without_addressing_other_inputs() {
    let result = prepare_instance_bounds(
        &[f32::NAN; 16],
        None,
        |_| panic!("unread"),
        || panic!("unread"),
    )
    .unwrap();
    assert_eq!(result.minimum, [1e6, 1e6, 1e6, 0.]);
    assert_eq!(result.maximum, [-1e6, -1e6, -1e6, -0.]);
    assert_eq!(result.maximum[3].to_bits(), (-0f32).to_bits());
    assert!(!result.contains_with_margin([0.; 3], 0.).unwrap());
}
#[test]
fn component_bounds_preserve_slot_order_and_special_aggregation_errors() {
    let calls = RefCell::new(Vec::new());
    let root = EntityBounds {
        base: bounds([0.; 3], [1.; 3]),
        component_mask: 0b1111_0101,
    };
    let result = prepare_instance_bounds(
        &IDENTITY,
        Some(root),
        |slot| {
            calls.borrow_mut().push(slot);
            Ok(bounds([-(slot as f32); 3], [slot as f32; 3]))
        },
        || {
            calls.borrow_mut().push(6);
            Ok(Some(bounds([-6.; 3], [6.; 3])))
        },
    )
    .unwrap();
    assert_eq!(*calls.borrow(), [0, 2, 4, 6]);
    assert_eq!(result.minimum, [-6., -6., -6., 0.]);
    assert_eq!(result.maximum, [6., 6., 6., 0.]);
    let root = EntityBounds {
        component_mask: 0x40,
        ..root
    };
    assert!(
        prepare_instance_bounds(
            &IDENTITY,
            Some(root),
            |_| panic!("masked"),
            || anyhow::bail!("unavailable shape aggregate")
        )
        .is_err()
    );
    let root = EntityBounds {
        component_mask: 0,
        ..root
    };
    prepare_instance_bounds(
        &IDENTITY,
        Some(root),
        |_| panic!("masked"),
        || panic!("masked special"),
    )
    .unwrap();
    let root = EntityBounds {
        component_mask: 1,
        ..root
    };
    assert!(
        prepare_instance_bounds(
            &IDENTITY,
            Some(root),
            |_| anyhow::bail!("missing component"),
            || panic!("after error")
        )
        .is_err()
    );
}
#[test]
fn margin_containment_is_closed_and_keeps_negative_margin_and_lazy_order() {
    let mut input = bounds([0.; 3], [2.; 3]);
    input.minimum[3] = f32::NAN;
    assert!(input.contains_with_margin([-0.5, 1., 1.], 0.5).unwrap());
    assert!(!input.contains_with_margin([-0.50001, 1., 1.], 0.5).unwrap());
    assert!(input.contains_with_margin([2.5, 1., 1.], 0.5).unwrap());
    assert!(input.contains_with_margin([0.5; 3], -0.5).unwrap());
    assert!(!input.contains_with_margin([0.; 3], -0.5).unwrap());
    input.minimum[1] = f32::NAN;
    assert!(!input.contains_with_margin([-1., f32::NAN, 0.], 0.).unwrap());
    assert!(input.contains_with_margin([1.; 3], 0.).is_err());
    assert!(input.contains_with_margin([1.; 3], f32::NAN).is_err());
}
#[test]
fn used_products_and_sums_report_overflow() {
    let mut matrix = IDENTITY;
    matrix[0] = 2.;
    assert!(
        bounds([f32::MAX; 3], [f32::MAX; 3])
            .transformed(&matrix)
            .is_err()
    );
    matrix = IDENTITY;
    matrix[12] = f32::MAX;
    assert!(
        bounds([f32::MAX; 3], [f32::MAX; 3])
            .transformed(&matrix)
            .is_err()
    );
}
#[test]
fn arbitrary_affine_bounds_agree_with_independent_eight_corner_geometry() {
    let mut word = 941u32;
    let mut next = || {
        word = word.wrapping_mul(1664525).wrapping_add(1013904223);
        ((word >> 24) as i32 - 128) as f32 / 16.
    };
    for _ in 0..1000 {
        let matrix = std::array::from_fn(|_| next());
        let input = Bounds {
            minimum: std::array::from_fn(|_| next()),
            maximum: std::array::from_fn(|_| next()),
        };
        let actual = input.transformed(&matrix).unwrap();
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for corner in 0..8 {
            let point = std::array::from_fn::<_, 3, _>(|i| {
                f64::from(if corner & (1 << i) == 0 {
                    input.minimum[i]
                } else {
                    input.maximum[i]
                })
            });
            for row in 0..3 {
                let value = f64::from(matrix[12 + row])
                    + (0..3)
                        .map(|col| f64::from(matrix[col * 4 + row]) * point[col])
                        .sum::<f64>();
                low[row] = low[row].min(value);
                high[row] = high[row].max(value);
            }
        }
        for row in 0..3 {
            assert!((f64::from(actual.minimum[row]) - low[row]).abs() < 0.0001);
            assert!((f64::from(actual.maximum[row]) - high[row]).abs() < 0.0001);
        }
    }
}
