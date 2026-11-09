use crate::mission::triggers::*;

use super::registry::{FactoryProbe, budget, common, finish, map, row};

fn kind_row(ordinal: usize, native: u32, class: u32, subtype: u32) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: native,
        class,
        subtype,
        enable_byte_raw: 1,
    };
    row
}
fn active(ordinal: usize) -> RowInput {
    kind_row(ordinal, 47, 0x1e00_0004, 0x1f00_0091)
}
fn range(ordinal: usize) -> RowInput {
    kind_row(ordinal, 8, 0x1e00_0005, 0x1f00_000c)
}
fn radius(ordinal: usize) -> RowInput {
    kind_row(ordinal, 3, 0x1e00_0004, 0x1f00_0006)
}

#[test]
fn active91_preserves_factory_status_and_promotes_only_the_effective_callback_status() {
    assert_eq!(active91::initialize_runtime_flags(0, 0x40), 0x41);
    assert_eq!(active91::initialize_runtime_flags(1, 0x40), 0x40);
    assert_eq!(
        active91::initialize_runtime_flags(1, 0x41),
        0x41,
        "disabled header leaves existing runtime bits unchanged"
    );
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    registry
        .visit(active(0), || Ok(common(0)), &mut factory)
        .unwrap();
    let mut disabled = common(0);
    disabled.header_flags_raw = 1;
    registry
        .visit(active(1), || Ok(disabled), &mut factory)
        .unwrap();
    let on = &registry.objects()[0];
    let off = &registry.objects()[1];
    assert_eq!(on.kind(), TriggerKind::Active91);
    assert_eq!(on.factory_status(), FactoryStatus::Status1);
    assert_eq!(on.registration_status(), Some(FactoryStatus::Status2));
    assert_eq!(on.runtime_flags_raw(), 1);
    assert_eq!(on.animator(), AnimatorState::NotRequired);
    assert_eq!(off.factory_status(), FactoryStatus::Status1);
    assert_eq!(off.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(off.animator(), AnimatorState::StoppedAbsent);
    assert!(on.is_registered() && off.is_registered());
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn promoted_non_startpoint_animator_request_keeps_its_kind_and_can_fail_without_fake_sp_init() {
    struct OnlyStartPoint {
        kinds: Vec<TriggerKind>,
    }
    impl Factory for OnlyStartPoint {
        fn initialize_animator(&mut self, request: AnimatorRequest) -> anyhow::Result<AnimatorId> {
            self.kinds.push(request.kind);
            anyhow::ensure!(
                request.kind == TriggerKind::StartPoint,
                "unsupported non-StartPoint callback"
            );
            AnimatorId::new(request.trigger.get())
        }
    }
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = OnlyStartPoint { kinds: Vec::new() };
    let mut payload = common(0);
    payload.animator_present = true;
    payload.animator_index_raw = 0;
    payload.property_mask_raw |= 1 << 26;
    let mut off = payload.clone();
    off.header_flags_raw = 1;
    registry
        .visit(active(0), || Ok(payload), &mut factory)
        .unwrap();
    registry.visit(active(1), || Ok(off), &mut factory).unwrap();
    assert_eq!(factory.kinds, [TriggerKind::Active91]);
    assert_eq!(registry.objects()[0].animator(), AnimatorState::Failed);
    assert_eq!(
        registry.objects()[0].registration_status(),
        Some(FactoryStatus::Status2)
    );
    assert_eq!(
        registry.objects()[1].animator(),
        AnimatorState::StoppedAbsent
    );
    assert_eq!(registry.membership().len(), 2);
    finish(&mut registry);
    assert!(!registry.is_complete());
}

#[test]
fn range0c_has_distinct_zero_defaults_signed_ranges_and_mutable_token_fields() {
    let defaults = range0c::State::from_serialized(0, 0, 0x0900_0000, 0x4200_0000);
    assert_eq!(defaults.radius_bits(), 0x4348_0000);
    assert_eq!(defaults.radius_squared_bits(), 0x471c_4000);
    assert_eq!(defaults.secondary_radius_bits(), 0x3f80_0000);
    assert_eq!(defaults.secondary_radius_squared_bits(), 0x3f80_0000);
    assert_eq!(defaults.mode_low_byte(), Some(0));
    assert_eq!(range0c::initialize_runtime_flags(0x2041), 0x41);
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common((-30_i32) as u32);
    payload.numeric[2] = 15;
    payload.numeric[4] = 0x0900_0037;
    payload.numeric[13] = 0x4200_0000;
    payload.animator_present = true;
    payload.animator_index_raw = 0;
    payload.property_mask_raw |= 1 << 26;
    let original = payload.numeric;
    registry
        .visit(range(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.range0c().unwrap();
    assert_eq!(state.radius_bits(), 0xc040_0000);
    assert_eq!(state.radius_squared_bits(), 0x4110_0000);
    assert_eq!(state.secondary_radius_bits(), 0x3fc0_0000);
    assert_eq!(state.secondary_radius_squared_bits(), 0x4010_0000);
    assert_eq!(object.common().numeric, original);
    assert_eq!(object.runtime_numeric()[0], state.radius_bits());
    assert_eq!(object.runtime_numeric()[2], state.secondary_radius_bits());
    assert_eq!(object.runtime_numeric()[4], 0x0900_0037);
    assert_eq!(object.runtime_numeric()[13], 0);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn radius06_uses_zero_squared_maximum_and_preserves_nonradius_numeric_words() {
    let zero = radius06::State::from_serialized(0, 0x4200_0000);
    assert_eq!(zero.radius_bits(), 0);
    assert_eq!(zero.radius_squared_bits(), 0x7f7f_ffff);
    assert_eq!(zero.provider_activation(), Some(false));
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common((-10_i32) as u32);
    payload.numeric[4] = 0x4200_0000;
    payload.numeric[1] = 0xdead_beef;
    payload.animator_present = true;
    payload.animator_index_raw = 0;
    payload.property_mask_raw |= 1 << 26;
    let original = payload.numeric;
    registry
        .visit(radius(0), || Ok(payload), &mut factory)
        .unwrap();
    let object = &registry.objects()[0];
    assert_eq!(object.radius06().unwrap().radius_bits(), 0xbf80_0000);
    assert_eq!(
        object.radius06().unwrap().radius_squared_bits(),
        0x3f80_0000
    );
    assert_eq!(object.common().numeric, original);
    assert_eq!(object.runtime_numeric()[0], 0xbf80_0000);
    assert_eq!(object.runtime_numeric()[4], 0);
    assert_eq!(object.runtime_numeric()[1], original[1]);
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn new_provider_dependencies_keep_staged_state_and_block_original_reference_slots() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut a = common(0);
    a.numeric[13] = 0x4200_0007;
    let mut b = common(0);
    b.numeric[4] = 0x4200_0009;
    assert!(matches!(
        registry.visit(range(0), || Ok(a), &mut factory).unwrap(),
        Slot::Unsupported(Unsupported::Range0cProvider { .. })
    ));
    assert!(matches!(
        registry.visit(radius(1), || Ok(b), &mut factory).unwrap(),
        Slot::Unsupported(Unsupported::Radius06Provider { .. })
    ));
    assert_eq!(
        registry.objects()[0].range0c().unwrap().mode_low_byte(),
        None
    );
    assert_eq!(
        registry.objects()[1]
            .radius06()
            .unwrap()
            .provider_activation(),
        None
    );
    assert!(
        registry
            .objects()
            .iter()
            .all(|object| !object.is_registered() && object.registration_status().is_none())
    );
    assert!(registry.membership().is_empty());
    assert_eq!(registry.scratch().len(), 2);
    let mut start = common(1);
    start.reference_ordinals[0] = 0;
    start.reference_ordinals[1] = 1;
    registry.visit(row(2), || Ok(start), &mut factory).unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[2].references()[0],
        Reference::Blocked { ordinal: 0 }
    );
    assert_eq!(
        registry.objects()[2].references()[1],
        Reference::Blocked { ordinal: 1 }
    );
    assert!(registry.rebuild_catalog().is_err());
}

#[test]
fn raw_shape_selectors_are_explicit_dependencies_before_object_or_scratch_creation() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut mismatch = common(0);
    mismatch.shape_index_raw = Some(7);
    assert!(
        registry
            .visit(radius(0), || Ok(mismatch), &mut factory)
            .is_err()
    );
    assert_eq!(registry.slots(0).unwrap(), &[Slot::Unvisited]);
    let mut payload = common(0);
    payload.property_mask_raw |= 1 << 27;
    payload.shape_index_raw = Some(u32::MAX);
    assert_eq!(
        registry
            .visit(radius(0), || Ok(payload), &mut factory)
            .unwrap(),
        Slot::Unsupported(Unsupported::Shape { index: u32::MAX })
    );
    assert!(registry.objects().is_empty() && registry.scratch().is_empty());
    finish(&mut registry);
    assert!(!registry.is_complete());
}

#[test]
fn range_token_classes_and_provider_rules_do_not_conflate_animation_and_state_references() {
    for token in [0, 0x0900_0000, 0x0901_0001, 0x4200_0001] {
        assert_eq!(
            range0c::State::from_serialized(1, 1, token, 0).animation_reference(),
            0
        );
    }
    let animation = range0c::State::from_serialized(1, 1, 0x0900_0001, 0x4200_0000);
    assert_eq!(animation.animation_reference(), 0x0900_0001);
    assert!(!animation.requires_resource_provider());
    let provider = range0c::State::from_serialized(1, 1, 0, 0x4200_0001);
    assert_eq!(provider.requested_resource(), Some(0x4200_0001));
    assert_eq!(provider.mode_low_byte(), None);
    let radius = radius06::State::from_serialized(1, 0x0900_0001);
    assert_eq!(radius.resource_reference(), 0);
    assert_eq!(radius.provider_activation(), Some(false));
}
