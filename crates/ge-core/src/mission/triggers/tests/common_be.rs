use crate::mission::triggers::*;

use super::registry::{FactoryProbe, budget, common, finish, map, row};

fn be_row(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor.native_type_index = 89;
    row.descriptor.class = 0x1e00_0004;
    row.descriptor.subtype = 0x1f00_00be;
    row
}

#[test]
fn common_be_preserves_numeric_tokens_and_uses_status_one_shutdown_without_derived_state() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut payload = common(0x4700_00a5);
    payload.numeric[1] = 0x4200_0001;
    payload.header_flags_raw = 0x201;
    payload.animator_present = true;
    payload.property_mask_raw |= 1 << 26;
    payload.animator_index_raw = 7;
    let original = payload.clone();
    registry
        .visit(be_row(0), || Ok(payload), &mut factory)
        .unwrap();
    assert!(factory.requests.is_empty());
    let object = &registry.objects()[0];
    assert!(object.descriptor().is_common_be());
    assert_eq!(object.kind(), TriggerKind::CommonBe);
    assert_eq!(object.factory_status(), FactoryStatus::Status1);
    assert_eq!(object.runtime_flags_raw(), 0);
    assert_eq!(object.runtime_header_flags_raw(), 0x201);
    assert_eq!(object.runtime_numeric(), &original.numeric);
    assert_eq!(object.common().numeric, original.numeric);
    assert!(
        object.generic().is_none(),
        "CommonBe is not a generic radius/provider consumer"
    );
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    assert!(object.is_registered());
    assert_eq!(registry.scratch(), registry.membership());
    finish(&mut registry);
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(
        registry.catalog().is_empty(),
        "CommonBe objects are not StartPoint wrappers"
    );
}

#[test]
fn generic_links_resolve_original_common_be_ordinal_without_activation_or_parent_flag_two_hundred()
{
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut generic_row = row(0);
    generic_row.descriptor.native_type_index = 0;
    generic_row.descriptor.class = 0x1e00_0001;
    generic_row.descriptor.subtype = 0x1f00_0001;
    let mut generic = common(0);
    generic.reference_ordinals[4] = 2;
    registry
        .visit(generic_row, || Ok(generic), &mut factory)
        .unwrap();
    let mut disabled = be_row(1);
    disabled.descriptor.enable_byte_raw = 0;
    registry
        .visit(disabled, || panic!("disabled payload"), &mut factory)
        .unwrap();
    let mut payload = common(0x4700_00a7);
    payload.header_flags_raw = 0x201;
    payload.reference_ordinals[7] = 0;
    registry
        .visit(be_row(2), || Ok(payload), &mut factory)
        .unwrap();
    finish(&mut registry);
    let target = &registry.objects()[1];
    assert_eq!(
        registry.objects()[0].references()[4],
        Reference::Trigger(target.id())
    );
    assert_eq!(target.ordinal(), 2);
    assert_eq!(target.runtime_flags_raw(), 0);
    assert_eq!(target.parent(), None, "flag200 is not the parenting gate");
    assert_eq!(
        target.references()[7],
        Reference::Trigger(registry.objects()[0].id())
    );
    assert!(registry.is_complete());
}
