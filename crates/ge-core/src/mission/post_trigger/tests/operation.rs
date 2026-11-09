use super::{cursor, grouped_registry, level, lookup_budget, registry, row};
use crate::mission::{
    actor_definitions::{DefaultReason, LookupBudget, SpecialKind},
    post_trigger::{Operation, PreparationError, Request, SkipReason, TailReset},
};

fn complete(operation: &mut Operation) -> TailReset {
    operation.record_returned(0).unwrap();
    operation.record_returned(1).unwrap();
    operation.finish().unwrap()
}

#[test]
fn shared_cursor_after_ten_calls_is_consumed_once_and_retry_reads_do_not_reselect() {
    let mut definitions = grouped_registry();
    for _ in 0..10 {
        definitions.selection_mut(0x4d00_0008).unwrap();
    }
    assert_eq!(cursor(&definitions), 2);
    let selected_level = level(0x4d00_0008);
    let mut operation = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        lookup_budget(),
    )
    .unwrap();
    let selected = operation.selected_definition().unwrap();
    assert_eq!(selected.record.get(), 3);
    assert_eq!(selected.key, 0x4d00_0703);
    assert_eq!(selected.source_index, 82);
    assert_eq!(selected.group, Some(8));
    assert_eq!(selected.member_index, Some(2));
    assert_eq!(operation.selection_reference(), Some(0x4d00_0008));
    assert!(operation.selection_performed());
    assert_eq!(cursor(&definitions), 3);
    // A host failure before a source return leaves the retained plan pending.
    let pending = *operation.next_request().unwrap();
    for _ in 0..3 {
        assert_eq!(operation.next_request(), Some(&pending));
        assert_eq!(operation.requests().len(), 2);
        assert!(!operation.is_finished());
        assert_eq!(operation.returned_count(), 0);
    }
    assert_eq!(cursor(&definitions), 3);
    complete(&mut operation);
    assert_eq!(cursor(&definitions), 3);

    // A distinct invocation really selects again. End-of-group cursor remains
    // count, then wraps only at the beginning of the following invocation.
    let mut second = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        lookup_budget(),
    )
    .unwrap();
    assert_eq!(second.selected_definition().unwrap().record.get(), 4);
    assert_eq!(cursor(&definitions), 4);
    complete(&mut second);
    let third = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        lookup_budget(),
    )
    .unwrap();
    assert_eq!(third.selected_definition().unwrap().record.get(), 1);
    assert_eq!(cursor(&definitions), 1);
}

#[test]
fn predicate_and_absent_level_skip_lookup_but_both_reach_the_exact_tail_reset() {
    let mut definitions = grouped_registry();
    definitions.selection_mut(0x4d00_0008).unwrap();
    let invalid_level = level(0);
    for (predicate, selected, reason) in [
        (true, Some(&invalid_level), SkipReason::Predicate),
        (true, None, SkipReason::Predicate),
        (false, None, SkipReason::NoSelectedLevel),
    ] {
        let mut operation = Operation::prepare(
            predicate,
            selected,
            &mut definitions,
            LookupBudget { comparisons: 0 },
        )
        .unwrap();
        assert_eq!(operation.skip_reason(), Some(reason));
        assert!(!operation.selection_performed());
        assert!(operation.selected_definition().is_none());
        assert!(operation.selection_reference().is_none());
        assert!(operation.requests().is_empty());
        assert_eq!(operation.tail_reset(), None);
        assert_eq!(
            operation.finish().unwrap(),
            TailReset {
                words_00_0c: [0; 4],
                word_338: u32::MAX,
                scalar_340_bits: 0,
                byte_344: 0,
            }
        );
        assert!(operation.is_finished());
        assert_eq!(cursor(&definitions), 1);
    }
}

#[test]
fn two_identical_requests_stay_ordered_and_a_null_return_does_not_skip_the_second() {
    let mut definitions = registry(vec![row(
        0x4d00_0300,
        u32::MAX,
        12,
        0x0100_0000,
        0x0600_0789,
        0x0600_0789,
    )]);
    let selected_level = level(0x4d00_0300);
    let mut operation = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        lookup_budget(),
    )
    .unwrap();
    assert_eq!(
        operation.requests(),
        [
            Request {
                ordinal: 0,
                file: 0x0100_0000,
                target: 0x0600_0789
            },
            Request {
                ordinal: 1,
                file: 0x0100_0000,
                target: 0x0600_0789
            },
        ]
    );
    assert!(operation.finish().is_err());
    assert!(operation.record_returned(1).is_err());
    assert_eq!(operation.returned_count(), 0);
    // The source helper may return NULL; its value is deliberately absent from
    // this API because the caller ignores it and proceeds to invocation two.
    operation.record_returned(0).unwrap();
    assert_eq!(operation.next_request().unwrap().ordinal, 1);
    assert_eq!(operation.returned_count(), 1);
    assert!(operation.tail_reset().is_none());
    assert!(operation.finish().is_err());
    operation.record_returned(1).unwrap();
    assert!(operation.next_request().is_none());
    assert!(operation.finish().is_ok());
}

#[test]
fn source_offsets_use_the_chosen_record_without_a_second_group_selection() {
    let mut definitions = registry(vec![row(
        0x4d00_0350,
        8,
        37,
        0x0109_0000,
        0x0d00_0111,
        0x0600_0222,
    )]);
    let selected_level = level(0x4d00_0008);
    let operation = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        lookup_budget(),
    )
    .unwrap();
    assert_eq!(
        operation.requests(),
        [
            Request {
                ordinal: 0,
                file: 0x0109_0000,
                target: 0x0d00_0111
            },
            Request {
                ordinal: 1,
                file: 0x0109_0000,
                target: 0x0600_0222
            },
        ]
    );
    assert_eq!(operation.selected_definition().unwrap().source_index, 37);
    assert_eq!(cursor(&definitions), 1);
}

#[test]
fn unbound_default_and_special_records_are_errors_not_null_definition_skips() {
    let mut definitions = grouped_registry();
    for (reference, expected) in [
        (
            0,
            PreparationError::RequiresDefault {
                reason: DefaultReason::InvalidReference { reference: 0 },
            },
        ),
        (
            0x4d00_0009,
            PreparationError::RequiresDefault {
                reason: DefaultReason::MissingGroup { key: 9 },
            },
        ),
        (
            0x4d00_0900,
            PreparationError::RequiresDefault {
                reason: DefaultReason::MissingRecord {
                    reference: 0x4d00_0900,
                },
            },
        ),
        (
            0x4d00_0102,
            PreparationError::RequiresSpecial {
                kind: SpecialKind::Embedded102,
            },
        ),
        (
            0x4d00_01ac,
            PreparationError::RequiresSpecial {
                kind: SpecialKind::Static1ac,
            },
        ),
    ] {
        let selected_level = level(reference);
        let error = Operation::prepare(
            false,
            Some(&selected_level),
            &mut definitions,
            lookup_budget(),
        )
        .unwrap_err();
        assert_eq!(error.downcast_ref::<PreparationError>(), Some(&expected));
        assert_eq!(cursor(&definitions), 0);
    }
}

#[test]
fn lookup_budget_failure_preserves_the_already_retained_group_cursor() {
    let mut definitions = grouped_registry();
    definitions.selection_mut(0x4d00_0008).unwrap();
    let selected_level = level(0x4d00_0008);
    let error = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        LookupBudget { comparisons: 0 },
    )
    .unwrap_err();
    assert!(error.to_string().contains("budget"));
    assert_eq!(cursor(&definitions), 1);
}

#[test]
fn file_gate_is_only_top_byte_and_never_drops_raw_ordered_requests_from_the_plan() {
    for (file, accepted) in [
        (0x0100_0000, true),
        (0x01ff_0000, true),
        (0x0100_0001, true),
        (0x0000_0001, false),
        (0x8100_0001, false),
        (0xffff_ffff, false),
    ] {
        let mut definitions = registry(vec![row(0x4d00_0300, u32::MAX, 0, file, 1, 2)]);
        let selected_level = level(0x4d00_0300);
        let mut operation = Operation::prepare(
            false,
            Some(&selected_level),
            &mut definitions,
            lookup_budget(),
        )
        .unwrap();
        assert_eq!(operation.requests().len(), 2);
        assert!(
            operation
                .requests()
                .iter()
                .all(|request| request.file == file && request.is_file_request() == accepted)
        );
        complete(&mut operation);
    }
}

#[test]
fn invalid_return_sequences_cannot_repeat_completed_calls_or_reapply_tail() {
    let mut definitions = grouped_registry();
    let selected_level = level(0x4d00_0008);
    let mut operation = Operation::prepare(
        false,
        Some(&selected_level),
        &mut definitions,
        lookup_budget(),
    )
    .unwrap();
    operation.record_returned(0).unwrap();
    assert!(operation.record_returned(0).is_err());
    assert_eq!(operation.returned_count(), 1);
    operation.record_returned(1).unwrap();
    assert!(operation.record_returned(2).is_err());
    assert_eq!(operation.returned_count(), 2);
    let reset = operation.finish().unwrap();
    assert_eq!(operation.tail_reset(), Some(reset));
    assert!(operation.finish().is_err());
    assert!(operation.record_returned(1).is_err());
    assert_eq!(operation.tail_reset(), Some(reset));
    assert_eq!(cursor(&definitions), 1);
}
