use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn radius(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 29,
        class: 0x1e00_000a,
        subtype: 0x1f00_0074,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn radius74_uses_signed_scale_square_zero_rule_and_top_byte_file_validation() {
    let zero = radius74::State::from_serialized(0, 0, 0);
    assert_eq!(
        (zero.radius_bits(), zero.radius_squared_bits()),
        (0, 0x7f7f_ffff)
    );
    let negative = radius74::State::from_serialized(u32::MAX, 0, u32::MAX);
    assert_eq!(
        (negative.radius_bits(), negative.radius_squared_bits()),
        (0xbdcc_cccd, 0x3c23_d70b)
    );
    assert_eq!(negative.numeric2_raw(), 0x7fff_ffff);
    for token in [0x0101_0007, 0x01ff_0042, 0x0180_1234] {
        assert_eq!(
            radius74::State::from_serialized(1, token, 0x4000_0011).file_reference(),
            token
        );
    }
    for token in [0x0101_0000, 0x0201_0007, 0x0001_0007, u32::MAX] {
        assert_eq!(
            radius74::State::from_serialized(1, token, 0).file_reference(),
            0
        );
    }
    assert_eq!(
        radius74::State::from_serialized(1, 0, 0xc000_0011).numeric2_raw(),
        0x4000_0011
    );
}

#[test]
fn valid_file_metadata_does_not_load_or_activate_and_raw_common_stays_separate() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common((-20i32) as u32);
    payload.numeric[1] = 0x01ff_0042;
    payload.numeric[2] = 0xc000_0055;
    payload.numeric[3] = 0xdead_beef;
    payload.numeric[15] = 0x4108_0001;
    payload.header_flags_raw = 0x200;
    payload.animator_present = true;
    payload.property_mask_raw |= (1 << 1) | (1 << 2) | (1 << 3) | (1 << 15) | (1 << 26);
    let raw = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(radius(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.radius74().unwrap();
    assert_eq!(object.kind(), TriggerKind::Radius74);
    assert_eq!(object.common().numeric, raw);
    let mut expected = raw;
    expected[0] = (-2.0f32).to_bits();
    expected[2] = 0x4000_0055;
    assert_eq!(object.runtime_numeric(), &expected);
    assert_eq!(state.radius_squared_bits(), 4.0f32.to_bits());
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn unreferenced_rows_still_get_owned_slots_membership_and_noop_map_initialization() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    for ordinal in 0..3 {
        registry
            .visit(radius(ordinal), || Ok(common(0)), &mut factory)
            .unwrap();
    }
    assert_eq!(registry.objects().len(), 3);
    assert_eq!(registry.scratch(), registry.membership());
    assert!(
        registry
            .slots(0)
            .unwrap()
            .iter()
            .all(|slot| matches!(slot, Slot::Owned(_)))
    );
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}
