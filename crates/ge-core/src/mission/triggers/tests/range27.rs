use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn range(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 17,
        class: 0x1e00_0005,
        subtype: 0x1f00_0027,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn post_common_absent_numeric_zeroes_replace_any_pre_common_defaults() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    // The reader's actual common snapshot has overwritten every absent word.
    // There is no pre-common fallback input in this constructor interface.
    let mut payload = common(0);
    payload.property_mask_raw = 0;
    let mut factory = FactoryProbe::default();
    registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.range27().unwrap();
    assert_eq!(object.common().numeric, [0; 16]);
    assert_eq!(state.radius_bits(), 200.0f32.to_bits());
    assert_eq!(state.radius_squared_bits(), 40000.0f32.to_bits());
    assert_eq!(state.secondary_radius_bits(), 1.0f32.to_bits());
    assert_eq!(state.secondary_radius_squared_bits(), 1.0f32.to_bits());
    assert_eq!(state.mode_low_byte(), Some(0));
    assert_eq!(
        state.range0c_state().radius_bits(),
        object.runtime_numeric()[0]
    );
    assert!(object.range0c().is_none());
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn range27_preserves_subtype_flags_and_signed_scalars_with_checked_tokens() {
    assert_eq!(range27::initialize_runtime_flags(0x2048), 0x2048);
    assert_eq!(range0c::initialize_runtime_flags(0x2048), 0x48);
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut payload = common((-20i32) as u32);
    payload.numeric[2] = 30;
    payload.numeric[4] = 0x0900_0003;
    payload.numeric[13] = 0x4200_0000;
    payload.numeric[5] = 0xfedc_ba98;
    payload.property_mask_raw |= (1 << 2) | (1 << 4) | (1 << 5) | (1 << 13);
    let raw = payload.numeric;
    let mut factory = FactoryProbe::default();
    registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.range27().unwrap();
    assert_eq!(object.common().numeric, raw);
    assert_eq!(state.radius_bits(), (-2.0f32).to_bits());
    assert_eq!(state.radius_squared_bits(), 4.0f32.to_bits());
    assert_eq!(state.secondary_radius_bits(), 3.0f32.to_bits());
    assert_eq!(state.secondary_radius_squared_bits(), 9.0f32.to_bits());
    assert_eq!(state.animation_reference(), 0x0900_0003);
    assert_eq!(state.resource_reference(), 0);
    assert_eq!(object.runtime_numeric()[5], raw[5]);
    assert_eq!(object.factory_status(), FactoryStatus::Status1);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
}

#[test]
fn accepted_provider_failure_retains_scratch_and_blocks_reference_without_membership() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut payload = common(10);
    payload.numeric[13] = 0x4200_0007;
    payload.property_mask_raw |= 1 << 13;
    let mut factory = FactoryProbe::default();
    let slot = registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    assert!(matches!(
        slot,
        Slot::Unsupported(Unsupported::Range27Provider {
            resource: 0x4200_0007,
            ..
        })
    ));
    let object = &registry.objects()[0];
    assert!(!object.is_registered());
    assert_eq!(object.registration_status(), None);
    assert_eq!(object.range27().unwrap().mode_low_byte(), None);
    assert_eq!(object.runtime_numeric()[13], 0x4200_0007);
    assert_eq!(registry.scratch().len(), 1);
    assert!(registry.membership().is_empty());
    let mut other = common(7);
    other.reference_ordinals[0] = 0;
    other.property_mask_raw |= 1 << 16;
    registry.visit(row(1), || Ok(other), &mut factory).unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[1].references()[0],
        Reference::Blocked { ordinal: 0 }
    );
    assert!(!registry.is_complete());
}

#[test]
fn status1_stops_absent_animator_and_keeps_unbound_metadata_and_ordered_references() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut payload = common(10);
    payload.numeric[1] = 0x0182_0042;
    payload.numeric[3] = 0x0200_0039;
    payload.word_74_raw = 0x1234_5678;
    payload.word_78_raw = 0x7654_3210;
    payload.animator_present = true;
    payload.animator_index_raw = 0;
    payload.property_mask_raw |= 1 << 26;
    payload.property_mask_raw |= (1 << 1) | (1 << 3) | (1 << 24) | (1 << 25);
    payload.reference_ordinals[..4].copy_from_slice(&[2, 1, 2, 1]);
    payload.property_mask_raw |= 0xf << 16;
    let mut factory = FactoryProbe::default();
    registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    registry
        .visit(range(1), || Ok(common(0)), &mut factory)
        .unwrap();
    registry
        .visit(range(2), || Ok(common(0)), &mut factory)
        .unwrap();
    finish(&mut registry);
    let objects = registry.objects();
    assert_eq!(objects[0].animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    assert_eq!(
        [
            objects[0].runtime_numeric()[1],
            objects[0].runtime_numeric()[3]
        ],
        [0x0182_0042, 0x0200_0039]
    );
    assert_eq!(
        (
            objects[0].common().word_74_raw,
            objects[0].common().word_78_raw
        ),
        (0x1234_5678, 0x7654_3210)
    );
    assert_eq!(
        &objects[0].references()[..4],
        &[
            Reference::Trigger(objects[2].id()),
            Reference::Trigger(objects[1].id()),
            Reference::Trigger(objects[2].id()),
            Reference::Trigger(objects[1].id())
        ]
    );
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}
