use super::*;

fn initial() -> InitialSelection {
    // An explicit authored snapshot, not a claimed native constructor default.
    InitialSelection {
        current_hash: u32::MAX,
        previous_hash: 0x1122_3344,
        primary_hash: None,
        secondary_hash: None,
    }
}

fn budget() -> Budget {
    Budget {
        primary: 8,
        secondary: 8,
    }
}

fn primary(hash: u32, marker: u32) -> [u32; 45] {
    let mut words = [marker; 45];
    words[0] = hash;
    words
}

fn secondary(hash: u32, marker: u32) -> [u32; 29] {
    let mut words = [marker; 29];
    words[0] = hash;
    words
}

#[test]
fn registration_sorts_unsigned_hashes_and_retains_original_source_indices() {
    let registry = LevelRegistry::new(
        vec![
            primary(u32::MAX, 1),
            primary(0x8000_0000, 2),
            primary(0, 3),
            primary(7, 4),
        ],
        vec![
            secondary(0x8000_0000, 5),
            secondary(0, 6),
            secondary(u32::MAX, 7),
        ],
        budget(),
        initial(),
    )
    .unwrap();
    assert_eq!(
        registry
            .primary()
            .iter()
            .map(LevelRow::hash)
            .collect::<Vec<_>>(),
        [0, 7, 0x8000_0000, u32::MAX]
    );
    assert_eq!(
        registry
            .primary()
            .iter()
            .map(LevelRow::source_index)
            .collect::<Vec<_>>(),
        [2, 3, 1, 0]
    );
    assert_eq!(
        registry
            .secondary()
            .iter()
            .map(TuningRow::hash)
            .collect::<Vec<_>>(),
        [0, 0x8000_0000, u32::MAX]
    );
    assert_eq!(registry.secondary()[1].source_index(), 0);
    assert!(registry.duplicates().is_empty());
}

#[test]
fn duplicate_arrivals_are_reported_without_replacing_the_first_record() {
    let registry = LevelRegistry::new(
        vec![
            primary(10, 1),
            primary(5, 2),
            primary(10, 3),
            primary(10, 4),
        ],
        vec![secondary(5, 5), secondary(1, 6), secondary(5, 7)],
        budget(),
        initial(),
    )
    .unwrap();
    assert_eq!(registry.primary().len(), 2);
    assert_eq!(registry.primary()[1].words_raw(), &primary(10, 1));
    assert_eq!(registry.secondary().len(), 2);
    assert_eq!(registry.secondary()[1].words_raw(), &secondary(5, 5));
    assert_eq!(
        registry.duplicates(),
        &[
            DuplicateRow {
                table: TableKind::Primary,
                hash: 10,
                source_index: 2,
                retained_source_index: 0
            },
            DuplicateRow {
                table: TableKind::Primary,
                hash: 10,
                source_index: 3,
                retained_source_index: 0
            },
            DuplicateRow {
                table: TableKind::Secondary,
                hash: 5,
                source_index: 2,
                retained_source_index: 0
            },
        ]
    );
}

#[test]
fn missing_primary_preserves_tuning_but_a_found_primary_can_clear_it() {
    let mut registry = LevelRegistry::new(
        vec![primary(1, 11), primary(2, 22)],
        vec![secondary(1, 33), secondary(3, 44)],
        budget(),
        initial(),
    )
    .unwrap();
    assert_eq!(
        registry.select(1),
        SelectionOutcome::Selected { has_tuning: true }
    );
    assert_eq!(registry.previous_hash(), 0x1122_3344);
    assert_eq!(registry.selected_secondary().unwrap().hash(), 1);
    // A tuning record for 3 exists, but the missing-primary early return must
    // neither select that record nor clear the prior selected tuning record.
    assert_eq!(registry.select(3), SelectionOutcome::PrimaryMissing);
    assert_eq!(registry.current_hash(), 3);
    assert_eq!(registry.previous_hash(), 1);
    assert!(registry.selected_primary().is_none());
    assert_eq!(registry.selected_secondary().unwrap().hash(), 1);
    assert_eq!(
        registry.select(2),
        SelectionOutcome::Selected { has_tuning: false }
    );
    assert_eq!(registry.previous_hash(), 3);
    assert_eq!(registry.selected_primary().unwrap().hash(), 2);
    assert!(registry.selected_secondary().is_none());
}

#[test]
fn current_max_suppresses_previous_update_without_special_casing_lookup_keys() {
    let mut registry = LevelRegistry::new(
        vec![primary(u32::MAX, 9), primary(0, 8)],
        vec![secondary(u32::MAX, 7)],
        budget(),
        initial(),
    )
    .unwrap();
    assert_eq!(
        registry.select(u32::MAX),
        SelectionOutcome::Selected { has_tuning: true }
    );
    assert_eq!(registry.selected_primary().unwrap().hash(), u32::MAX);
    assert_eq!(registry.previous_hash(), 0x1122_3344);
    assert_eq!(
        registry.select(0),
        SelectionOutcome::Selected { has_tuning: false }
    );
    assert_eq!(registry.previous_hash(), 0x1122_3344);
    assert_eq!(registry.current_hash(), 0);
    assert_eq!(
        registry.select(0),
        SelectionOutcome::Selected { has_tuning: false }
    );
    assert_eq!(registry.previous_hash(), 0);
}

#[test]
fn explicit_initial_snapshot_can_retain_independent_rows_and_rejects_missing_rows() {
    let snapshot = InitialSelection {
        current_hash: 99,
        previous_hash: 88,
        primary_hash: Some(1),
        secondary_hash: Some(2),
    };
    let mut registry = LevelRegistry::new(
        vec![primary(1, 1)],
        vec![secondary(2, 2)],
        budget(),
        snapshot,
    )
    .unwrap();
    assert_eq!(registry.current_hash(), 99);
    assert_eq!(registry.previous_hash(), 88);
    assert_eq!(registry.selected_primary().unwrap().hash(), 1);
    assert_eq!(registry.selected_secondary().unwrap().hash(), 2);
    assert_eq!(registry.select(55), SelectionOutcome::PrimaryMissing);
    assert_eq!(registry.previous_hash(), 99);
    assert_eq!(registry.selected_secondary().unwrap().hash(), 2);
    for snapshot in [
        InitialSelection {
            primary_hash: Some(3),
            ..initial()
        },
        InitialSelection {
            secondary_hash: Some(3),
            ..initial()
        },
    ] {
        assert!(
            LevelRegistry::new(
                vec![primary(1, 1)],
                vec![secondary(2, 2)],
                budget(),
                snapshot
            )
            .is_err()
        );
    }
}

#[test]
fn budgets_count_duplicate_input_and_empty_catalog_has_explicit_selection() {
    assert!(
        LevelRegistry::new(
            vec![primary(1, 1), primary(1, 2)],
            vec![],
            Budget {
                primary: 1,
                secondary: 0
            },
            initial()
        )
        .is_err()
    );
    assert!(
        LevelRegistry::new(
            vec![],
            vec![secondary(1, 1), secondary(1, 2)],
            Budget {
                primary: 0,
                secondary: 1
            },
            initial()
        )
        .is_err()
    );
    let mut registry = LevelRegistry::new(
        vec![],
        vec![],
        Budget {
            primary: 0,
            secondary: 0,
        },
        initial(),
    )
    .unwrap();
    assert_eq!(registry.select(1), SelectionOutcome::PrimaryMissing);
    assert_eq!(registry.current_hash(), 1);
    assert_eq!(registry.previous_hash(), 0x1122_3344);
    assert!(registry.selected_primary().is_none());
    assert!(registry.selected_secondary().is_none());
}

#[test]
fn raw_words_and_distinct_movie_bank_selectors_are_never_float_validated() {
    let mut words = primary(0x0181_ffff, 0x7fc0_1234);
    words[11] = 0;
    words[12] = u32::MAX;
    words[20] = 0x2d50_0123;
    words[21] = u32::MAX;
    words[22] = 0;
    let tuning = secondary(words[0], 0xff80_0000);
    let mut registry = LevelRegistry::new(vec![words], vec![tuning], budget(), initial()).unwrap();
    registry.select(words[0]);
    let selected = registry.selected_primary().unwrap();
    assert_eq!(selected.words_raw(), &words);
    assert_eq!(selected.intro_movie_hash(), 0);
    assert_eq!(selected.outro_movie_hash(), u32::MAX);
    assert_eq!(selected.bank_hashes(), [0x2d50_0123, u32::MAX, 0]);
    assert_eq!(registry.selected_secondary().unwrap().words_raw(), &tuning);
}

#[test]
fn difficulty_accepts_only_the_four_source_values_and_labels() {
    for (value, expected, label) in [
        (0, Difficulty::Operative, "Operative"),
        (1, Difficulty::Agent, "Agent"),
        (2, Difficulty::DoubleOSeven, "007"),
        (3, Difficulty::Classic, "007 Classic"),
    ] {
        let difficulty = Difficulty::try_from(value).unwrap();
        assert_eq!(difficulty, expected);
        assert_eq!(difficulty.value(), value);
        assert_eq!(difficulty.label(), label);
    }
    assert!(Difficulty::try_from(4).is_err());
    assert!(Difficulty::try_from(u8::MAX).is_err());
}
