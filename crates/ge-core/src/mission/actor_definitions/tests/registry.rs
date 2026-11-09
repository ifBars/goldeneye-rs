use super::*;

fn row(source_index: usize, key: u32, group: u32) -> Primary {
    let mut words = [0; 17];
    words[0] = key;
    words[1] = group;
    Primary {
        source_index,
        words,
    }
}

fn limits() -> Budget {
    Budget {
        rows: 32,
        groups: 16,
        members: 32,
    }
}

fn selected(selection: Selection) -> RecordId {
    match selection {
        Selection::Selected { record, .. } => record,
        other => panic!("expected owned record, got {other:?}"),
    }
}

#[test]
fn unsigned_storage_and_first_arrival_preserve_raw_words_and_diagnostics() {
    let mut first = row(99, 0x80000001, u32::MAX);
    first.words[3] = 0x7fc01234;
    first.words[16] = 0x80000000;
    let registry = Registry::build(
        vec![
            first,
            row(10, u32::MAX, u32::MAX),
            row(30, 7, u32::MAX),
            row(0, 0x80000001, 5),
            row(1, 7, 6),
            row(2, u32::MAX, 7),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(registry.input_rows(), 6);
    assert_eq!(registry.record_count(), 3);
    assert_eq!(
        registry
            .records()
            .iter()
            .map(Record::key)
            .collect::<Vec<_>>(),
        [7, 0x80000001, u32::MAX]
    );
    assert_eq!(
        registry
            .records()
            .iter()
            .map(|r| r.id().get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(registry.records()[1].words(), &first.words);
    assert_eq!(registry.records()[1].source_index(), 99);
    assert_eq!(
        registry
            .duplicates()
            .iter()
            .map(|d| (d.key, d.retained_source_index, d.rejected_source_index))
            .collect::<Vec<_>>(),
        [(0x80000001, 99, 0), (7, 30, 1), (u32::MAX, 10, 2),]
    );
    // Rejected duplicate group words never add membership.
    assert_eq!(registry.group_count(), 0);
    assert_eq!(registry.member_count(), 0);
}

#[test]
fn group_creation_and_members_follow_final_sorted_records_not_arrivals() {
    let registry = Registry::build(
        vec![
            row(0, 0x4d000130, 0xface0002),
            row(1, 0x4d000110, 0xaaaa0009),
            row(2, 0x4d000120, 0x12340002),
            row(3, 0x4d000140, 0x00000009),
            row(4, 0x4d000150, u32::MAX),
            row(5, 0x4d000160, 0xffff0000),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(
        registry.groups().iter().map(Group::key).collect::<Vec<_>>(),
        [9, 2, 0]
    );
    assert_eq!(registry.member_count(), 5);
    let keys = |group: &Group| {
        group
            .members()
            .iter()
            .map(|id| registry.record(*id).unwrap().key())
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&registry.groups()[0]), [0x4d000110, 0x4d000140]);
    assert_eq!(keys(&registry.groups()[1]), [0x4d000120, 0x4d000130]);
    assert!(
        registry
            .groups()
            .iter()
            .all(|g| g.cursor() == 0 && g.count() == g.members().len())
    );
}

#[test]
fn group_cursor_wraps_at_next_selection_and_is_independent_per_group() {
    let mut registry = Registry::build(
        vec![
            row(0, 0x4d000110, 7),
            row(1, 0x4d000120, 7),
            row(2, 0x4d000130, 9),
        ],
        limits(),
    )
    .unwrap();
    let first = selected(registry.selection_mut(0x4d000007).unwrap());
    assert_eq!(registry.record(first).unwrap().key(), 0x4d000110);
    assert_eq!(registry.groups()[0].cursor(), 1);
    let other = selected(registry.selection_mut(0x4d000009).unwrap());
    assert_eq!(registry.record(other).unwrap().key(), 0x4d000130);
    assert_eq!(registry.groups()[1].cursor(), 1);
    let second = registry.selection_mut(0x4d000007).unwrap();
    assert_eq!(
        second,
        Selection::Selected {
            record: registry.records()[1].id(),
            group: Some(7),
            member_index: Some(1)
        }
    );
    assert_eq!(registry.groups()[0].cursor(), 2); // Deliberately not modulo count.
    assert_eq!(selected(registry.selection_mut(0x4d000110).unwrap()), first);
    assert_eq!(registry.groups()[0].cursor(), 2);
    assert_eq!(selected(registry.selection_mut(0x4d000007).unwrap()), first);
    assert_eq!(registry.groups()[0].cursor(), 1);
    assert_eq!(registry.groups()[1].cursor(), 1);
}

#[test]
fn validation_and_group_precedence_never_substitute_an_owned_default() {
    let mut registry = Registry::build(vec![row(0, 0x4d000004, u32::MAX)], limits()).unwrap();
    for reference in [0, 0x4d000000, 0x4c000004, u32::MAX] {
        assert_eq!(
            registry.selection_mut(reference).unwrap(),
            Selection::RequiresDefault {
                reason: DefaultReason::InvalidReference { reference },
            }
        );
    }
    // An exact low-key record exists, but source takes the group branch first.
    assert_eq!(
        registry.selection_mut(0x4d000004).unwrap(),
        Selection::RequiresDefault {
            reason: DefaultReason::MissingGroup { key: 4 },
        }
    );
    assert_eq!(
        registry.selection_mut(0x4d000100).unwrap(),
        Selection::RequiresDefault {
            reason: DefaultReason::MissingRecord {
                reference: 0x4d000100
            },
        }
    );
}

#[test]
fn specials_are_explicit_dependencies_even_when_ordinary_records_exist() {
    let mut registry =
        Registry::build(vec![row(0, 0x4d000102, 2), row(1, 0x4d0001ac, 2)], limits()).unwrap();
    for (reference, kind) in [
        (0x4d000102, SpecialKind::Embedded102),
        (0x4d0001ac, SpecialKind::Static1ac),
    ] {
        assert_eq!(
            registry
                .selection_with_budget(reference, LookupBudget { comparisons: 0 })
                .unwrap(),
            Selection::RequiresSpecial { kind }
        );
    }
    assert_eq!(registry.groups()[0].cursor(), 0);
}

#[test]
fn build_bounds_count_all_arrivals_groups_and_total_members() {
    assert!(
        Registry::build(
            vec![row(0, 1, u32::MAX), row(1, 1, u32::MAX)],
            Budget {
                rows: 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        Registry::build(
            vec![row(0, 1, 7), row(1, 2, 8)],
            Budget {
                groups: 1,
                ..limits()
            }
        )
        .is_err()
    );
    assert!(
        Registry::build(
            vec![row(0, 1, 7), row(1, 2, 8)],
            Budget {
                members: 1,
                ..limits()
            }
        )
        .is_err()
    );
    let empty = Registry::build(
        Vec::new(),
        Budget {
            rows: 0,
            groups: 0,
            members: 0,
        },
    )
    .unwrap();
    assert_eq!(
        (
            empty.input_rows(),
            empty.record_count(),
            empty.group_count(),
            empty.member_count()
        ),
        (0, 0, 0, 0)
    );
    // A duplicate is charged as an input row, but does not consume group/member budgets.
    let registry = Registry::build(
        vec![row(0, 1, u32::MAX), row(1, 1, 2)],
        Budget {
            rows: 2,
            groups: 0,
            members: 0,
        },
    )
    .unwrap();
    assert_eq!(registry.duplicates().len(), 1);
}

#[test]
fn bounded_lookup_errors_are_atomic_and_preserve_lower_midpoint_order() {
    let mut registry =
        Registry::build(vec![row(0, 0x4d000110, 1), row(1, 0x4d000120, 3)], limits()).unwrap();
    assert!(
        registry
            .selection_with_budget(0x4d000003, LookupBudget { comparisons: 1 })
            .is_err()
    );
    assert_eq!(registry.groups()[1].cursor(), 0);
    let result = registry
        .selection_with_budget(0x4d000001, LookupBudget { comparisons: 1 })
        .unwrap();
    assert_eq!(selected(result), registry.records()[0].id());
    assert_eq!(registry.groups()[0].cursor(), 1);
    assert!(
        registry
            .selection_with_budget(0x4d000001, LookupBudget { comparisons: 0 })
            .is_err()
    );
    assert_eq!(registry.groups()[0].cursor(), 1);
    assert!(
        registry
            .selection_with_budget(0x4d000120, LookupBudget { comparisons: 1 })
            .is_err()
    );
    assert_eq!(
        selected(
            registry
                .selection_with_budget(0x4d000110, LookupBudget { comparisons: 1 })
                .unwrap()
        ),
        registry.records()[0].id()
    );
    assert!(matches!(
        registry
            .selection_with_budget(0, LookupBudget { comparisons: 0 })
            .unwrap(),
        Selection::RequiresDefault { .. }
    ));
}

#[test]
fn present_empty_or_corrupt_group_is_an_error_not_a_missing_group() {
    let mut registry = Registry::build(vec![row(0, 0x4d000110, 1)], limits()).unwrap();
    registry.groups[0].members.clear();
    assert!(registry.selection_mut(0x4d000001).is_err());
    assert_eq!(registry.groups()[0].cursor(), 0);
    registry.groups[0].count = 0;
    assert!(registry.selection_mut(0x4d000001).is_err());
    assert_eq!(registry.groups()[0].cursor(), 0);
    // A zero-count *registry* is instead a genuine missing group.
    let mut empty = Registry::build(Vec::new(), limits()).unwrap();
    assert_eq!(
        empty.selection_mut(0x4d000001).unwrap(),
        Selection::RequiresDefault {
            reason: DefaultReason::MissingGroup { key: 1 },
        }
    );
}
