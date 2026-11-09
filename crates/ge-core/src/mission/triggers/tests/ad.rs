use super::common09::common09_row;
use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn ad_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 73,
        class: 0x1e00_0009,
        subtype: 0x1f00_00ad,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn ad_has_no_zero_radius_default_and_uses_signed_tenths_before_squaring() {
    let zero = ad::State::from_serialized(0, 0x4200_0000, 0x4108_0000);
    assert_eq!(zero.radius_bits(), 0);
    assert_eq!(zero.radius_squared_bits(), 0x7f7f_ffff);
    assert_eq!(
        (zero.numeric2_reference(), zero.numeric15_reference()),
        (0, 0)
    );
    let one = ad::State::from_serialized(1, 0x4300_0001, 0x4109_0001);
    assert_eq!(one.radius_bits(), 0x3dcc_cccd);
    assert_eq!(one.radius_squared_bits(), 0x3c23_d70b);
    assert_eq!(
        (one.numeric2_reference(), one.numeric15_reference()),
        (0, 0)
    );
    let negative = ad::State::from_serialized(u32::MAX, 0x4200_0001, 0x4108_0001);
    assert_eq!(negative.radius_bits(), 0xbdcc_cccd);
    assert_eq!(negative.radius_squared_bits(), 0x3c23_d70b);
    assert_eq!(
        (
            negative.numeric2_reference(),
            negative.numeric15_reference()
        ),
        (0x4200_0001, 0x4108_0001)
    );
}

#[test]
fn accepted_ad_tokens_are_retained_without_provider_and_header1_remains_inactive() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common((-20i32) as u32);
    payload.numeric[1] = 0x4200_0041;
    payload.numeric[2] = 0x4200_0007;
    payload.numeric[15] = 0x4108_0009;
    payload.numeric[14] = 0xdead_beef;
    payload.header_flags_raw = 1;
    payload.animator_present = true;
    payload.property_mask_raw |= (1 << 1) | (1 << 2) | (1 << 14) | (1 << 15) | (1 << 26);
    let raw = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(ad_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.ad().unwrap();
    assert_eq!(object.kind(), TriggerKind::Ad);
    assert!(object.descriptor().is_ad());
    assert_eq!(object.common().numeric, raw);
    assert_eq!(state.radius_bits(), (-2.0f32).to_bits());
    assert_eq!(state.radius_squared_bits(), 4.0f32.to_bits());
    assert_eq!(
        [
            object.runtime_numeric()[1],
            object.runtime_numeric()[2],
            object.runtime_numeric()[14],
            object.runtime_numeric()[15]
        ],
        [0x4200_0041, 0x4200_0007, 0xdead_beef, 0x4108_0009]
    );
    assert_eq!(object.runtime_header_flags_raw(), 1);
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn ad_resolves_actual_owned_references_and_retains_script_metadata_without_initial_execution() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut payload = common(0);
    payload.word_74_raw = 0x01b5_0042;
    payload.word_78_raw = 0x0400_0077;
    payload.reference_ordinals[0] = 2;
    payload.reference_ordinals[1] = 1;
    payload.reference_ordinals[6] = 2;
    payload.property_mask_raw |= (1 << 16) | (1 << 17) | (1 << 22) | (1 << 24) | (1 << 25);
    let mut factory = FactoryProbe::default();
    registry
        .visit(ad_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let mut disabled = common09_row(1);
    disabled.descriptor.enable_byte_raw = 0;
    registry
        .visit(disabled, || panic!("disabled payload"), &mut factory)
        .unwrap();
    let mut other = common(0xdead_0001);
    other.reference_ordinals[0] = 0;
    other.property_mask_raw |= 1 << 16;
    registry
        .visit(common09_row(2), || Ok(other), &mut factory)
        .unwrap();
    finish(&mut registry);
    let objects = registry.objects();
    assert_eq!(
        objects[0].references()[0],
        Reference::Trigger(objects[1].id())
    );
    assert_eq!(objects[0].references()[1], Reference::Null);
    assert_eq!(
        objects[0].references()[6],
        Reference::Trigger(objects[1].id())
    );
    assert_eq!(
        objects[1].references()[0],
        Reference::Trigger(objects[0].id())
    );
    assert_eq!(
        (
            objects[0].common().word_74_raw,
            objects[0].common().word_78_raw
        ),
        (0x01b5_0042, 0x0400_0077)
    );
    assert_eq!(objects[1].runtime_numeric()[0], 0xdead_0001);
    assert!(registry.is_complete());
    assert!(factory.requests.is_empty());
}
