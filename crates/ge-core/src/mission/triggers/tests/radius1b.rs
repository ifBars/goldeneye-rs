use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn radius(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 12,
        class: 0x1e00_0004,
        subtype: 0x1f00_001b,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn radius1b_preserves_signed_scale_product_and_its_distinct_zero_rule() {
    assert_eq!(
        radius1b::State::from_serialized(0).radius_squared_bits(),
        0x7f7f_ffff
    );
    assert_eq!(
        radius03::State::from_serialized(0, 0).radius_squared_bits(),
        0
    );
    for raw in [1, u32::MAX] {
        assert_eq!(
            radius1b::State::from_serialized(raw).radius_squared_bits(),
            0x3c23_d70b
        );
    }
    assert_eq!(
        radius1b::State::from_serialized((-20i32) as u32).radius_squared_bits(),
        4.0f32.to_bits()
    );
    assert_ne!(
        radius1b::State::from_serialized(1).radius_squared_bits(),
        a3::State::from_serialized(1).radius_squared_bits()
    );
}

#[test]
fn radius1b_writes_only_runtime14_and_radius_while_retaining_raw_words_and_status1() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common(20);
    payload.numeric[1] = 0xdead_beef;
    payload.numeric[3] = 0x4200_0007;
    payload.numeric[14] = 0x3f80_0000;
    payload.header_flags_raw = 1;
    payload.animator_present = true;
    payload.property_mask_raw |= (1 << 1) | (1 << 3) | (1 << 14) | (1 << 26);
    let raw = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(radius(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    assert_eq!(object.kind(), TriggerKind::Radius1b);
    assert_eq!(object.common().numeric, raw);
    let mut expected = raw;
    expected[14] = 0;
    assert_eq!(object.runtime_numeric(), &expected);
    assert_eq!(
        object.radius1b().unwrap().radius_squared_bits(),
        4.0f32.to_bits()
    );
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn radius1b_is_a_real_owned_reference_target_before_noop_map_callback() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut source = common(0);
    source.reference_ordinals[0] = 1;
    source.property_mask_raw |= 1 << 16;
    registry
        .visit(radius(0), || Ok(source), &mut factory)
        .unwrap();
    registry
        .visit(radius(1), || Ok(common(0)), &mut factory)
        .unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[0].references()[0],
        Reference::Trigger(registry.objects()[1].id())
    );
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}
