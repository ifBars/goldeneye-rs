use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn a3_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 63,
        class: 0x1e00_0004,
        subtype: 0x1f00_00a3,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn signed_square_then_percent_preserves_source_rounding_order() {
    assert_eq!(
        a3::State::from_serialized(0).radius_squared_bits(),
        0x7f7f_ffff
    );
    for raw in [1, u32::MAX] {
        assert_eq!(
            a3::State::from_serialized(raw).radius_squared_bits(),
            0x3c23_d70a
        );
    }
    // Scaling before squaring gives 0x3c23d70b for1: it is a different rule.
    assert_ne!(
        a3::State::from_serialized(1).radius_squared_bits(),
        generic::State::from_serialized(1, 0).radius_squared_bits()
    );
    assert_eq!(
        a3::State::from_serialized((-10i32) as u32).radius_squared_bits(),
        1.0f32.to_bits()
    );
    for raw in [i32::MAX as u32, i32::MIN as u32] {
        // Both signed magnitudes round to2^31 before the separate products.
        assert_eq!(
            a3::State::from_serialized(raw).radius_squared_bits(),
            0x5b23_d70a
        );
    }
}

#[test]
fn a3_keeps_numeric_words_raw_and_header_bit0_does_not_activate_it() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common((-17i32) as u32);
    payload.numeric[1] = 0xdead_beef;
    payload.numeric[2] = 0xabcd_1234;
    payload.numeric[15] = u32::MAX;
    payload.property_mask_raw |= (1 << 1) | (1 << 2) | (1 << 15);
    payload.header_flags_raw = 1;
    payload.animator_present = true;
    payload.property_mask_raw |= 1 << 26;
    let raw = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(a3_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let trigger = &registry.objects()[0];
    assert_eq!(trigger.kind(), TriggerKind::A3);
    assert_eq!(trigger.common().numeric, raw);
    assert_eq!(*trigger.runtime_numeric(), raw);
    assert_eq!(trigger.runtime_header_flags_raw(), 1);
    assert_eq!(trigger.runtime_flags_raw(), 0);
    assert_eq!(trigger.factory_status(), FactoryStatus::Status1);
    assert_eq!(trigger.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(trigger.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    assert!(trigger.a3().is_some());
    assert!(trigger.generic().is_none() && trigger.radius06().is_none());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn a3_common_references_and_parent_link_use_original_slots_before_noop_map_init() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut child = common(0);
    child.header_flags_raw = 0x100;
    child.reference_ordinals[0] = 2;
    child.reference_ordinals[7] = 2;
    child.property_mask_raw |= (1 << 16) | (1 << 23);
    let mut factory = FactoryProbe::default();
    registry
        .visit(a3_row(0), || Ok(child), &mut factory)
        .unwrap();
    let mut disabled = a3_row(1);
    disabled.descriptor.enable_byte_raw = 0;
    registry
        .visit(
            disabled,
            || panic!("disabled payload is lazy"),
            &mut factory,
        )
        .unwrap();
    registry
        .visit(a3_row(2), || Ok(common(0)), &mut factory)
        .unwrap();
    finish(&mut registry);
    let child = &registry.objects()[0];
    let parent = &registry.objects()[1];
    assert_eq!(parent.ordinal(), 2);
    assert_eq!(child.references()[0], Reference::Trigger(parent.id()));
    assert_eq!(child.parent(), Some(parent.id()));
    assert_eq!(parent.children(), [child.id()]);
    assert_eq!(registry.scratch(), registry.membership());
    assert!(registry.is_complete());
}
