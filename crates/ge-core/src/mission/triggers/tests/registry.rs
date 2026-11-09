use std::cell::Cell;

use crate::mission::{spawn, triggers::*, world::InstanceId};

pub(super) fn budget() -> Budget {
    Budget {
        maps: 3,
        rows: 12,
        objects: 12,
        filter_words: 8,
        fixups: 96,
        catalog_entries: 12,
    }
}

pub(super) fn map(index: u64) -> MapContext {
    MapContext {
        instance: InstanceId::new(index).unwrap(),
        map_selector: index as i8,
    }
}

pub(super) fn row(ordinal: usize) -> RowInput {
    RowInput {
        ordinal,
        zone: 0x1234,
        descriptor: Descriptor {
            native_type_index: 28,
            class: 0x1e00_0003,
            subtype: 0x1f00_0081,
            enable_byte_raw: 1,
        },
    }
}

pub(super) fn common(id: u32) -> CommonInput {
    let mut numeric = [0; 16];
    numeric[0] = id;
    CommonInput {
        header_flags_raw: 0,
        property_mask_raw: 1,
        position_bits: [0x8000_0000, 0x7fc0_0001, 3, 0xffff_ffff],
        rotation_bits: [4, 5, 6, 0x8000_0000],
        numeric,
        reference_ordinals: [u32::MAX; 8],
        word_74_raw: 0,
        word_78_raw: 0,
        animator_present: false,
        animator_index_raw: 0,
        shape_index_raw: None,
    }
}

#[derive(Default)]
pub(super) struct FactoryProbe {
    pub(super) requests: Vec<AnimatorRequest>,
    pub(super) fail: bool,
}
impl Factory for FactoryProbe {
    fn initialize_animator(&mut self, request: AnimatorRequest) -> anyhow::Result<AnimatorId> {
        self.requests.push(request);
        anyhow::ensure!(!self.fail, "authored missing animator dependency");
        AnimatorId::new(request.trigger.get())
    }
}

pub(super) fn finish(registry: &mut Registry) {
    registry.fix_references().unwrap();
    registry.link_parents().unwrap();
    registry.initialize_map().unwrap();
}

#[test]
fn disabled_filter_and_unsupported_keep_original_slots_without_payload_reads() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, Some(&[])).unwrap();
    let read = Cell::new(0);
    let mut factory = FactoryProbe::default();
    let mut disabled = row(0);
    disabled.descriptor.enable_byte_raw = 0;
    assert_eq!(
        registry
            .visit(
                disabled,
                || {
                    read.set(1);
                    Ok(common(1))
                },
                &mut factory
            )
            .unwrap(),
        Slot::Disabled
    );
    assert_eq!(
        registry
            .visit(
                row(1),
                || {
                    read.set(1);
                    Ok(common(1))
                },
                &mut factory
            )
            .unwrap(),
        Slot::Filtered
    );
    registry
        .mark_unsupported(
            2,
            Unsupported::Unclassified {
                native_type_index: 99,
            },
        )
        .unwrap();
    assert_eq!(read.get(), 0);
    assert_eq!(
        registry.slots(0).unwrap(),
        &[
            Slot::Disabled,
            Slot::Filtered,
            Slot::Unsupported(Unsupported::Unclassified {
                native_type_index: 99
            })
        ]
    );
    finish(&mut registry);
    assert!(!registry.is_complete());
    assert!(registry.rebuild_catalog().is_err());
    registry.begin_map(map(2), 1, None).unwrap();
    let mut unsupported = row(0);
    unsupported.descriptor.native_type_index = 99;
    unsupported.descriptor.class = 0x1e00_0004;
    unsupported.descriptor.subtype = 0x1f00_ffff;
    assert!(matches!(
        registry
            .visit(
                unsupported,
                || panic!("unsupported class payload"),
                &mut factory
            )
            .unwrap(),
        Slot::Unsupported(Unsupported::Factory { .. })
    ));
    finish(&mut registry);
    assert!(!registry.is_complete());
}

#[test]
fn original_ordinal_fixup_and_parent_links_preserve_membership_order_across_maps() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut child = common(7);
    child.header_flags_raw = 0x100;
    child.reference_ordinals[7] = 2;
    registry.visit(row(0), || Ok(child), &mut factory).unwrap();
    let mut disabled = row(1);
    disabled.descriptor.enable_byte_raw = 0;
    registry
        .visit(disabled, || panic!("disabled payload"), &mut factory)
        .unwrap();
    registry
        .visit(row(2), || Ok(common(8)), &mut factory)
        .unwrap();
    finish(&mut registry);
    let child = TriggerId::new(1).unwrap();
    let parent = TriggerId::new(2).unwrap();
    assert_eq!(
        registry.trigger(child).unwrap().references()[7],
        Reference::Trigger(parent)
    );
    assert_eq!(registry.trigger(child).unwrap().parent(), Some(parent));
    assert_eq!(registry.trigger(parent).unwrap().children(), &[child]);
    registry.begin_map(map(2), 1, None).unwrap();
    let mut second = common(9);
    second.reference_ordinals[0] = 0;
    registry.visit(row(0), || Ok(second), &mut factory).unwrap();
    finish(&mut registry);
    let second = TriggerId::new(3).unwrap();
    assert_eq!(
        registry.trigger(second).unwrap().references()[0],
        Reference::Trigger(second),
        "ordinal zero is local to the second map"
    );
    assert_eq!(registry.scratch(), &[child, parent, second]);
    assert_eq!(registry.membership(), registry.scratch());
    assert_eq!(
        registry.trigger(child).unwrap().common().position_bits,
        [0x8000_0000, 0x7fc0_0001, 3, 0]
    );
    assert_eq!(
        registry.trigger(child).unwrap().original_rotation_bits(),
        [4, 5, 6, 0]
    );
}

#[test]
fn animator_zero_is_required_and_failure_retains_staged_owned_membership() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut factory = FactoryProbe {
        fail: true,
        ..Default::default()
    };
    let mut payload = common(1);
    payload.animator_present = true;
    payload.property_mask_raw |= 1 << 26;
    payload.animator_index_raw = 0;
    let slot = registry
        .visit(row(0), || Ok(payload), &mut factory)
        .unwrap();
    let Slot::Owned(id) = slot else {
        panic!("owned status2 object")
    };
    assert_eq!(factory.requests.len(), 1);
    assert_eq!(factory.requests[0].index, 0);
    assert_eq!(
        registry.trigger(id).unwrap().animator(),
        AnimatorState::Failed
    );
    assert_eq!(registry.trigger(id).unwrap().runtime_flags_raw(), 1);
    assert_eq!(registry.scratch(), &[id]);
    assert_eq!(registry.membership(), &[id]);
    let mut negative = common(2);
    negative.animator_present = true;
    negative.property_mask_raw |= 1 << 26;
    negative.animator_index_raw = -1;
    registry
        .visit(row(1), || Ok(negative), &mut factory)
        .unwrap();
    assert_eq!(factory.requests.len(), 1);
    finish(&mut registry);
    assert!(!registry.is_complete());
    assert!(registry.rebuild_catalog().is_err());
}

#[test]
fn filtered_reference_is_null_but_unsupported_target_blocks_catalog() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, Some(&[0x1f00_0081])).unwrap();
    let mut payload = common(1);
    payload.reference_ordinals[0] = 1;
    payload.reference_ordinals[1] = 2;
    let mut factory = FactoryProbe::default();
    registry
        .visit(row(0), || Ok(payload), &mut factory)
        .unwrap();
    let mut filtered = row(1);
    filtered.descriptor.subtype = 0x1f00_0099;
    registry
        .visit(filtered, || panic!("filtered payload"), &mut factory)
        .unwrap();
    registry
        .mark_unsupported(
            2,
            Unsupported::Unclassified {
                native_type_index: 99,
            },
        )
        .unwrap();
    finish(&mut registry);
    let refs = registry.objects()[0].references();
    assert_eq!(refs[0], Reference::Null);
    assert_eq!(refs[1], Reference::Blocked { ordinal: 2 });
    assert!(registry.rebuild_catalog().is_err());
}

#[test]
fn live_catalog_keeps_disabled_entries_and_uses_current_source_selection_rules() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 3, None).unwrap();
    let mut factory = FactoryProbe::default();
    let mut disabled = common(3);
    disabled.header_flags_raw = 1;
    registry
        .visit(row(0), || Ok(disabled), &mut factory)
        .unwrap();
    registry
        .visit(row(1), || Ok(common(0)), &mut factory)
        .unwrap();
    let mut high_byte = row(2);
    high_byte.descriptor.enable_byte_raw = 0x80;
    registry
        .visit(high_byte, || Ok(common(3)), &mut factory)
        .unwrap();
    assert!(registry.rebuild_catalog().is_err());
    finish(&mut registry);
    registry.rebuild_catalog().unwrap();
    registry.rebuild_catalog().unwrap();
    assert_eq!(registry.catalog().len(), 3);
    let limit = spawn::Budget {
        entries: 3,
        visits: 6,
    };
    assert_eq!(
        registry.select_start_point(3, limit).unwrap().trigger,
        Some(TriggerId::new(3).unwrap())
    );
    let fallback = registry.select_start_point(99, limit).unwrap();
    assert_eq!(fallback.trigger, Some(TriggerId::new(2).unwrap()));
    assert!(fallback.selection.used_fallback());
    assert_eq!(fallback.pose.unwrap().rotation_bits, [0, 5, 0, 0]);
}

#[test]
fn sequence_reference_and_budget_errors_do_not_fabricate_pass_completion() {
    let mut registry = Registry::new(Budget {
        fixups: 7,
        ..budget()
    });
    assert!(
        registry
            .begin_map(
                MapContext {
                    map_selector: 0,
                    ..map(1)
                },
                1,
                None
            )
            .is_err()
    );
    registry.begin_map(map(1), 1, None).unwrap();
    let mut factory = FactoryProbe::default();
    assert!(
        registry
            .visit(row(1), || Ok(common(1)), &mut factory)
            .is_err()
    );
    assert_eq!(registry.slots(0).unwrap(), &[Slot::Unvisited]);
    registry
        .visit(row(0), || Ok(common(1)), &mut factory)
        .unwrap();
    assert!(registry.fix_references().is_err());
    assert_eq!(registry.phase(0).unwrap(), MapPhase::Visiting);
    assert_eq!(
        registry.objects()[0].references(),
        &[Reference::Unresolved; 8]
    );
    assert!(registry.initialize_map().is_err());
    let mut bad = Registry::new(budget());
    bad.begin_map(map(1), 1, None).unwrap();
    let mut payload = common(1);
    payload.reference_ordinals[0] = 1;
    bad.visit(row(0), || Ok(payload), &mut factory).unwrap();
    assert!(bad.fix_references().is_err());
    assert_eq!(bad.objects()[0].references(), &[Reference::Unresolved; 8]);
    let mut shape = Registry::new(budget());
    shape.begin_map(map(1), 1, None).unwrap();
    let mut payload = common(1);
    payload.property_mask_raw |= 1 << 27;
    payload.shape_index_raw = Some(7);
    assert_eq!(
        shape.visit(row(0), || Ok(payload), &mut factory).unwrap(),
        Slot::Unsupported(Unsupported::Shape { index: 7 })
    );
    assert!(shape.objects().is_empty());
    finish(&mut shape);
    assert!(!shape.is_complete());
}
