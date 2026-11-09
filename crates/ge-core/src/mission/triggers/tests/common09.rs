use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

pub(super) fn common09_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 6,
        class: 0x1e00_0004,
        subtype: 0x1f00_0009,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn common09_retains_all_common_words_and_stops_absent_animator_without_derived_requests() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common(u32::MAX);
    payload.numeric[1] = 0x4200_0007;
    payload.numeric[2] = 0x4108_0042;
    payload.numeric[15] = 0xdead_beef;
    payload.property_mask_raw |= (1 << 1) | (1 << 2) | (1 << 15) | (1 << 26);
    payload.header_flags_raw = 1;
    payload.animator_present = true;
    payload.animator_index_raw = 3;
    let raw = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(common09_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    assert_eq!(object.kind(), TriggerKind::Common09);
    assert!(object.descriptor().is_common09());
    assert_eq!(*object.runtime_numeric(), raw);
    assert_eq!(object.common().numeric, raw);
    assert_eq!(object.runtime_header_flags_raw(), 1);
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(object.generic().is_none() && object.resource08().is_none() && object.ad().is_none());
    assert!(factory.requests.is_empty());
    assert_eq!(registry.scratch(), registry.membership());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn common09_real_owned_reference_keeps_original_ordinal_and_initial_map_is_not_event_relay() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut payload = common(0xdead_0001);
    payload.reference_ordinals[0] = 2;
    payload.reference_ordinals[7] = 1;
    payload.property_mask_raw |= (1 << 16) | (1 << 23);
    let mut factory = FactoryProbe::default();
    registry
        .visit(common09_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let mut disabled = common09_row(1);
    disabled.descriptor.enable_byte_raw = 0;
    registry
        .visit(
            disabled,
            || panic!("disabled row remains lazy"),
            &mut factory,
        )
        .unwrap();
    registry
        .visit(common09_row(2), || Ok(common(0x1234_5678)), &mut factory)
        .unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[0].references()[0],
        Reference::Trigger(registry.objects()[1].id())
    );
    assert_eq!(registry.objects()[0].references()[7], Reference::Null);
    assert_eq!(registry.objects()[1].ordinal(), 2);
    assert_eq!(registry.objects()[0].runtime_numeric()[0], 0xdead_0001);
    assert_eq!(registry.objects()[1].runtime_numeric()[0], 0x1234_5678);
    assert!(registry.is_complete());
    assert!(factory.requests.is_empty());
}
