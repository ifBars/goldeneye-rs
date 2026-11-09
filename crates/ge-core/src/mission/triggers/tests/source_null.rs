use super::registry::{FactoryProbe, budget, common, finish, map, row};
use crate::mission::triggers::*;

fn source_null(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 62,
        class: 0x1e00_0004,
        subtype: 0x1f00_00a1,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn exact_source_null_bypasses_common_and_object_budget_without_inventing_status_or_kind() {
    let mut limits = budget();
    limits.objects = 0;
    limits.fixups = 0;
    limits.catalog_entries = 0;
    let mut registry = Registry::new(limits);
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    assert!(source_null(0).descriptor.is_source_null());
    assert_eq!(source_null(0).descriptor.kind(), None);
    assert_eq!(
        registry
            .visit(
                source_null(0),
                || panic!("sourceNULL has no common payload"),
                &mut factory
            )
            .unwrap(),
        Slot::SourceNull
    );
    assert_eq!(registry.slots(0).unwrap()[0].retained_trigger(), None);
    assert!(
        registry.objects().is_empty()
            && registry.scratch().is_empty()
            && registry.membership().is_empty()
    );
    assert!(factory.requests.is_empty());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn disabled_and_subtype_filter_admission_precede_source_null_factory_result() {
    let mut limits = budget();
    limits.objects = 0;
    let mut registry = Registry::new(limits);
    registry.begin_map(map(1), 2, Some(&[])).unwrap();
    let mut factory = FactoryProbe::default();
    let mut disabled = source_null(0);
    disabled.descriptor.enable_byte_raw = 0;
    assert_eq!(
        registry
            .visit(disabled, || panic!("disabled payload"), &mut factory)
            .unwrap(),
        Slot::Disabled
    );
    assert_eq!(
        registry
            .visit(source_null(1), || panic!("filtered payload"), &mut factory)
            .unwrap(),
        Slot::Filtered
    );
    finish(&mut registry);
    assert!(registry.is_complete());
}

#[test]
fn source_null_target_is_real_null_in_reference_parent_and_three_entry_catalog_passes() {
    let mut limits = budget();
    limits.objects = 3;
    let mut registry = Registry::new(limits);
    registry.begin_map(map(1), 4, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut first = common(1);
    first.header_flags_raw = 0x100;
    first.reference_ordinals[0] = 1;
    first.reference_ordinals[7] = 1;
    first.property_mask_raw |= (1 << 16) | (1 << 23);
    registry.visit(row(0), || Ok(first), &mut factory).unwrap();
    registry
        .visit(source_null(1), || panic!("no common forNULL"), &mut factory)
        .unwrap();
    registry
        .visit(row(2), || Ok(common(2)), &mut factory)
        .unwrap();
    registry
        .visit(row(3), || Ok(common(3)), &mut factory)
        .unwrap();
    finish(&mut registry);
    assert_eq!(registry.objects()[0].references()[0], Reference::Null);
    assert_eq!(registry.objects()[0].references()[7], Reference::Null);
    assert_eq!(registry.objects()[0].parent(), None);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert_eq!(registry.catalog().len(), 3);
    assert_eq!(registry.membership().len(), 3);
    assert_eq!(registry.objects()[1].ordinal(), 2);
}

#[test]
fn neighboring_unsupported_factory_does_not_become_a_successful_source_null() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut unsupported = source_null(0);
    unsupported.descriptor.subtype = 0x1f00_00a2;
    assert!(!unsupported.descriptor.is_source_null());
    assert!(matches!(
        registry
            .visit(unsupported, || panic!("unsupported payload"), &mut factory)
            .unwrap(),
        Slot::Unsupported(Unsupported::Factory { .. })
    ));
    finish(&mut registry);
    assert!(!registry.is_complete());
    assert!(registry.rebuild_catalog().is_err());
}
