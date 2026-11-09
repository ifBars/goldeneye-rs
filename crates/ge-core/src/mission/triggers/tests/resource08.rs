use crate::mission::{spawn, triggers::*};

use super::registry::{FactoryProbe, budget, common, finish, map, row};

fn resource_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor.native_type_index = 4;
    row.descriptor.class = 0x1e00_0004;
    row.descriptor.subtype = 0x1f00_0008;
    row
}

#[test]
fn initial_byte_state_requires_exact_resource_class_and_nonzero_local_id() {
    for token in [0, 0x4200_0000, 0x4201_0001, 0x4300_0001, u32::MAX] {
        let state = resource08::State::from_serialized(token);
        assert_eq!(state.mode_low_byte(), Some(0));
        assert_eq!(state.mode_high_byte(), 0);
        assert_eq!(state.base_radius_squared_bits(), 0);
        assert_eq!(state.requested_resource(), None);
        assert!(!state.requires_resource_provider());
    }
    for token in [0x4200_0001, 0x4200_ffff] {
        let state = resource08::State::from_serialized(token);
        assert_eq!(state.requested_resource(), Some(token));
        assert_eq!(state.mode_low_byte(), None);
        assert_eq!(state.mode_high_byte(), 0);
        assert!(state.requires_resource_provider());
    }
}

#[test]
fn registered_status_one_keeps_raw_numeric_words_and_skips_present_animator() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common(0x8000_0000);
    payload.numeric[1] = 0x4200_0000;
    payload.numeric[15] = 0x8765_4321;
    payload.animator_present = true;
    payload.animator_index_raw = 0;
    payload.property_mask_raw |= 1 << 26;
    payload.header_flags_raw = 1;
    let original = payload.clone();
    registry
        .visit(resource_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    assert_eq!(object.kind(), TriggerKind::Resource08);
    assert!(object.descriptor().is_resource08());
    assert_eq!(object.factory_status(), FactoryStatus::Status1);
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.runtime_header_flags_raw(), 1);
    assert!(object.is_registered());
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    assert_eq!(object.common().numeric, original.numeric);
    assert_eq!(object.runtime_numeric(), &original.numeric);
    assert!(object.generic().is_none());
    assert_eq!(object.resource08().unwrap().mode_low_byte(), Some(0));
    assert_eq!(registry.scratch(), registry.membership());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn provider_unavailable_preserves_staged_common_without_membership_or_fake_byte() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common(0x1234_5678);
    payload.numeric[1] = 0x4200_007b;
    let original = payload.numeric;
    let slot = registry
        .visit(resource_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let staged = TriggerId::new(1).unwrap();
    assert_eq!(
        slot,
        Slot::Unsupported(Unsupported::Resource08Provider {
            trigger: staged,
            resource: 0x4200_007b
        })
    );
    assert_eq!(registry.scratch(), &[staged]);
    assert!(registry.membership().is_empty());
    let object = registry.trigger(staged).unwrap();
    assert!(!object.is_registered());
    assert_eq!(object.runtime_numeric(), &original);
    assert_eq!(object.resource08().unwrap().mode_low_byte(), None);
    assert_eq!(
        object.resource08().unwrap().requested_resource(),
        Some(0x4200_007b)
    );
    assert_eq!(
        object.animator(),
        AnimatorState::NotRequired,
        "status1 shutdown has not been reached"
    );
    assert!(factory.requests.is_empty());
    let mut start = common(1);
    start.reference_ordinals[0] = 0;
    registry.visit(row(1), || Ok(start), &mut factory).unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[1].references()[0],
        Reference::Blocked { ordinal: 0 }
    );
    assert!(!registry.is_complete());
    assert!(registry.rebuild_catalog().is_err());
}

#[test]
fn inactive_resource_objects_resolve_original_slots_without_entering_spawn_catalog() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut start = common(1);
    start.reference_ordinals[0] = 2;
    registry.visit(row(0), || Ok(start), &mut factory).unwrap();
    let mut filtered = resource_row(1);
    filtered.descriptor.enable_byte_raw = 0;
    registry
        .visit(filtered, || panic!("disabled common payload"), &mut factory)
        .unwrap();
    let mut target = common(0x4200_0042);
    target.numeric[1] = 0x4200_0000;
    target.reference_ordinals[0] = 0;
    registry
        .visit(resource_row(2), || Ok(target), &mut factory)
        .unwrap();
    finish(&mut registry);
    let source = &registry.objects()[0];
    let target = &registry.objects()[1];
    assert_eq!(target.ordinal(), 2);
    assert_eq!(target.runtime_flags_raw(), 0);
    assert_eq!(source.references()[0], Reference::Trigger(target.id()));
    assert_eq!(target.references()[0], Reference::Trigger(source.id()));
    let source_id = source.id();
    registry.rebuild_catalog().unwrap();
    assert_eq!(registry.catalog(), &[source_id]);
    let result = registry
        .select_start_point(
            1,
            spawn::Budget {
                entries: 1,
                visits: 1,
            },
        )
        .unwrap();
    assert_eq!(result.trigger, Some(source_id));
}

#[test]
fn admission_filtering_precedes_resource_state_payload() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, Some(&[])).unwrap();
    let mut factory = FactoryProbe::default();
    assert_eq!(
        registry
            .visit(
                resource_row(0),
                || panic!("filtered resource payload"),
                &mut factory
            )
            .unwrap(),
        Slot::Filtered
    );
    assert!(registry.scratch().is_empty());
    assert!(factory.requests.is_empty());
}
