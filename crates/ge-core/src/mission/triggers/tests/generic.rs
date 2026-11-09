use crate::mission::{spawn, triggers::*};

use super::registry::{FactoryProbe, budget, common, finish, map, row};

fn generic_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor.native_type_index = 0;
    row.descriptor.class = 0x1e00_0001;
    row.descriptor.subtype = 0x1f00_0001;
    row
}

#[test]
fn zero_radius_uses_maximum_finite_square_and_rejected_resource_has_known_zero_mode() {
    let state = generic::State::from_serialized(0, 0x4200_0000);
    assert_eq!(state.radius_bits(), 0);
    assert_eq!(state.radius_squared_bits(), 0x7f7f_ffff);
    assert_eq!(state.resource_reference(), 0);
    assert_eq!(state.mode_bytes(), Some([0, 0]));
    assert!(!state.requires_resource_provider());
}

#[test]
fn signed_integer_conversion_precedes_single_precision_scale_and_square() {
    // Authored integer cases with independently calculated f32 bit expectations.
    for (integer, radius, square) in [
        (-10_i32, 0xbf80_0000, 0x3f80_0000),
        (15, 0x3fc0_0000, 0x4010_0000),
        (16_777_217, 0x49cc_cccd, 0x5423_d70b),
        (i32::MIN, 0xcd4c_cccd, 0x5b23_d70b),
        (i32::MAX, 0x4d4c_cccd, 0x5b23_d70b),
    ] {
        let state = generic::State::from_serialized(integer as u32, 0);
        assert_eq!(state.radius_bits(), radius, "{integer}");
        assert_eq!(state.radius_squared_bits(), square, "{integer}");
    }
}

#[test]
fn resource_requires_exact_prefix_and_nonzero_local_id_and_never_invents_provider_mode() {
    for resource in [0, 0x4200_0000, 0x4300_0001, 0x4201_0001] {
        let state = generic::State::from_serialized(1, resource);
        assert_eq!(state.resource_reference(), 0);
        assert_eq!(state.mode_bytes(), Some([0, 0]));
    }
    for resource in [0x4200_0001, 0x4200_ffff] {
        let state = generic::State::from_serialized(1, resource);
        assert_eq!(state.resource_reference(), resource);
        assert_eq!(state.mode_bytes(), None);
        assert!(state.requires_resource_provider());
    }
}

#[test]
fn status_one_shuts_down_absent_animator_while_start_point_status_two_constructs_it() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut generic = common((-10_i32) as u32);
    generic.numeric[1] = 0x4200_0000;
    generic.numeric[15] = 0x1234_5678;
    generic.property_mask_raw |= 1 << 26;
    generic.animator_present = true;
    generic.animator_index_raw = 7;
    let original = generic.clone();
    registry
        .visit(generic_row(0), || Ok(generic), &mut factory)
        .unwrap();
    let mut start = common(9);
    start.property_mask_raw |= 1 << 26;
    start.animator_present = true;
    start.animator_index_raw = 0;
    registry.visit(row(1), || Ok(start), &mut factory).unwrap();
    assert_eq!(factory.requests.len(), 1);
    assert_eq!(factory.requests[0].trigger, TriggerId::new(2).unwrap());
    let object = &registry.objects()[0];
    assert_eq!(object.kind(), TriggerKind::Generic);
    assert_eq!(object.factory_status(), FactoryStatus::Status1);
    assert_eq!(object.factory_status().raw(), 1);
    assert!(object.is_registered());
    assert_eq!(object.runtime_flags_raw() & 1, 0);
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert_eq!(object.common().numeric, original.numeric);
    assert_eq!(object.common().header_flags_raw, original.header_flags_raw);
    assert_eq!(object.runtime_numeric()[0], 0xbf80_0000);
    assert_eq!(object.runtime_numeric()[1], 0);
    assert_eq!(object.runtime_numeric()[15], original.numeric[15]);
    assert_eq!(object.generic().unwrap().radius_squared_bits(), 0x3f80_0000);
    let start = &registry.objects()[1];
    assert_eq!(start.factory_status(), FactoryStatus::Status2);
    assert_eq!(start.runtime_flags_raw() & 1, 1);
    assert!(matches!(start.animator(), AnimatorState::Initialized(_)));
    assert_eq!(registry.scratch(), registry.membership());
    finish(&mut registry);
    registry.rebuild_catalog().unwrap();
    assert_eq!(registry.catalog(), &[TriggerId::new(2).unwrap()]);
}

#[test]
fn inactive_generic_targets_resolve_and_only_flag_one_hundred_creates_parent_links() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut parent = common(0);
    parent.header_flags_raw = 0x201;
    parent.reference_ordinals[7] = 2;
    registry
        .visit(generic_row(0), || Ok(parent), &mut factory)
        .unwrap();
    let mut child = common(0);
    child.header_flags_raw = 0x100;
    child.reference_ordinals[7] = 0;
    registry
        .visit(generic_row(1), || Ok(child), &mut factory)
        .unwrap();
    let mut start = common(1);
    start.reference_ordinals[0] = 0;
    registry.visit(row(2), || Ok(start), &mut factory).unwrap();
    finish(&mut registry);
    let parent = &registry.objects()[0];
    let child = &registry.objects()[1];
    assert_eq!(parent.runtime_header_flags_raw(), 0x201);
    assert_eq!(parent.parent(), None, "flag200 must not become flag100");
    assert_eq!(child.parent(), Some(parent.id()));
    assert_eq!(parent.children(), &[child.id()]);
    assert_eq!(
        registry.objects()[2].references()[0],
        Reference::Trigger(parent.id())
    );
    assert_eq!(
        parent.runtime_flags_raw(),
        0,
        "inactive does not remove membership"
    );
    registry.rebuild_catalog().unwrap();
    let selected = registry
        .select_start_point(
            1,
            spawn::Budget {
                entries: 1,
                visits: 2,
            },
        )
        .unwrap();
    assert_eq!(selected.trigger, Some(TriggerId::new(3).unwrap()));
}

#[test]
fn unavailable_resource_provider_retains_common_scratch_but_not_final_membership_or_resolved_target()
 {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common(20);
    payload.header_flags_raw = 0x101;
    payload.numeric[1] = 0x4200_0055;
    let slot = registry
        .visit(generic_row(0), || Ok(payload), &mut factory)
        .unwrap();
    let id = TriggerId::new(1).unwrap();
    assert_eq!(
        slot,
        Slot::Unsupported(Unsupported::GenericResource {
            trigger: id,
            resource: 0x4200_0055
        })
    );
    assert_eq!(registry.scratch(), &[id]);
    assert!(registry.membership().is_empty());
    let object = registry.trigger(id).unwrap();
    assert!(!object.is_registered());
    assert_eq!(object.runtime_header_flags_raw(), 0x101);
    assert_eq!(object.generic().unwrap().mode_bytes(), None);
    assert_eq!(object.runtime_numeric()[1], 0x4200_0055);
    assert!(factory.requests.is_empty());
    let mut start = common(1);
    start.reference_ordinals[0] = 0;
    registry.visit(row(1), || Ok(start), &mut factory).unwrap();
    finish(&mut registry);
    assert_eq!(
        registry.objects()[1].references()[0],
        Reference::Blocked { ordinal: 0 }
    );
    assert_eq!(
        registry.objects()[0].references(),
        &[Reference::Unresolved; 8]
    );
    assert!(!registry.is_complete());
    assert!(registry.rebuild_catalog().is_err());
}

#[test]
fn disabled_or_filtered_generic_never_reads_resource_payload_or_creates_scratch() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, Some(&[])).unwrap();
    let mut factory = FactoryProbe::default();
    let mut disabled = generic_row(0);
    disabled.descriptor.enable_byte_raw = 0;
    assert_eq!(
        registry
            .visit(disabled, || panic!("disabled common payload"), &mut factory)
            .unwrap(),
        Slot::Disabled
    );
    assert_eq!(
        registry
            .visit(
                generic_row(1),
                || panic!("filtered common payload"),
                &mut factory
            )
            .unwrap(),
        Slot::Filtered
    );
    assert!(registry.scratch().is_empty());
    assert!(factory.requests.is_empty());
}
