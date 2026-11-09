use super::*;

fn budget() -> Budget {
    Budget {
        records: 1024,
        work: 1_000_000,
    }
}
fn record(key: u16, value: u16) -> Record {
    Record { key, value }
}
fn admission() -> Admission {
    Admission {
        captured: true,
        saved_generation: 9,
        current_generation: 9,
    }
}

#[test]
fn fresh_owner_is_empty_and_missing_lookup_preserves_full_fallback() {
    let owner = Owner::from_constructor(2500);
    assert_eq!(owner.origin(), Origin::FreshConstructor);
    assert_eq!(
        (owner.capacity(), owner.reset_limit(), owner.rows().len()),
        (0, 2500, 0)
    );
    let result = owner
        .lookup(
            0x4200_0012,
            0xfedc_ba98,
            Budget {
                records: 0,
                work: 0,
            },
        )
        .unwrap();
    assert_eq!(
        result,
        Lookup {
            value: 0xfedc_ba98,
            matched: false,
            scanned: 0
        }
    );
}

#[test]
fn snapshot_order_duplicates_zero_keys_and_lookup_validation_are_distinct() {
    let rows = vec![record(7, 0xabcd), record(7, 0x1234), record(0, 0x5678)];
    let owner = Owner::from_snapshot(rows.clone(), 32, 2, budget()).unwrap();
    assert_eq!(owner.origin(), Origin::SuppliedSnapshot);
    assert_eq!(owner.rows(), rows);
    assert_eq!(
        owner.lookup(0xdead_0007, 9, budget()).unwrap(),
        Lookup {
            value: 0xabcd,
            matched: true,
            scanned: 1
        }
    );
    assert_eq!(
        owner.lookup(0x4200_0000, 9, budget()).unwrap(),
        Lookup {
            value: 0x5678,
            matched: true,
            scanned: 3
        }
    );
    assert_eq!(
        owner
            .lookup(
                0,
                u32::MAX,
                Budget {
                    records: 0,
                    work: 0
                }
            )
            .unwrap(),
        Lookup {
            value: u32::MAX,
            matched: false,
            scanned: 0
        }
    );
    assert!(
        owner
            .lookup(
                0x4200_0000,
                9,
                Budget {
                    records: 32,
                    work: 2
                }
            )
            .is_err()
    );
    assert_eq!(owner.rows(), rows);
}

#[test]
fn setter_validates_prefix_and_truncates_values_without_replacing_duplicate_rows() {
    let mut fresh = Owner::from_constructor(2);
    for token in [0, 0x4200_0000, 0x4300_0001, 1] {
        assert_eq!(
            fresh
                .set(
                    token,
                    9,
                    Budget {
                        records: 0,
                        work: 0
                    }
                )
                .unwrap(),
            SetOutcome::Invalid
        );
    }
    assert_eq!(fresh.capacity(), 0);
    assert_eq!(
        fresh.set(0x4200_0007, 0xabcd_1234, budget()).unwrap(),
        SetOutcome::Inserted { index: 0 }
    );
    assert_eq!(fresh.capacity(), 32);
    assert_eq!(fresh.rows(), [record(7, 0x1234)]);
    let mut supplied =
        Owner::from_snapshot(vec![record(7, 1), record(7, 2)], 32, 1, budget()).unwrap();
    assert_eq!(
        supplied.set(0x4200_0007, 0xffff_5678, budget()).unwrap(),
        SetOutcome::Updated { index: 0 }
    );
    assert_eq!(supplied.rows(), [record(7, 0x5678), record(7, 2)]);
}

#[test]
fn capacity_grows_by_source_blocks_and_failed_insert_is_atomic() {
    let mut owner = Owner::from_constructor(1);
    for key in 1..=32 {
        owner.set(0x4200_0000 | key, key, budget()).unwrap();
    }
    assert_eq!(owner.capacity(), 32);
    let before = owner.rows().to_vec();
    assert!(
        owner
            .set(
                0x4200_0021,
                7,
                Budget {
                    records: 32,
                    work: 100
                }
            )
            .is_err()
    );
    assert_eq!(owner.rows(), before);
    assert_eq!(owner.capacity(), 32);
    assert!(
        owner
            .set(
                0x4200_0001,
                99,
                Budget {
                    records: 32,
                    work: 1
                }
            )
            .is_err()
    );
    assert_eq!(owner.rows(), before);
    assert_eq!(
        owner.set(0x4200_0021, 7, budget()).unwrap(),
        SetOutcome::Inserted { index: 32 }
    );
    assert_eq!(owner.capacity(), 64);
}

#[test]
fn reset_clears_only_bounded_value_prefix_and_keeps_keys_count_capacity() {
    let mut owner = Owner::from_snapshot(
        vec![record(3, 9), record(2, 8), record(1, 7)],
        32,
        2,
        budget(),
    )
    .unwrap();
    assert!(
        owner
            .reset_values(Budget {
                records: 0,
                work: 1
            })
            .is_err()
    );
    assert_eq!(owner.rows(), [record(3, 9), record(2, 8), record(1, 7)]);
    assert_eq!(owner.reset_values(budget()).unwrap(), 2);
    assert_eq!(owner.rows(), [record(3, 0), record(2, 0), record(1, 7)]);
    assert_eq!((owner.capacity(), owner.reset_limit()), (32, 2));
}

#[test]
fn capture_preserves_nonzero_storage_order_and_stops_at_exact_source_cap() {
    let owner = Owner::from_snapshot(
        vec![record(8, 0), record(0, 7), record(8, 6), record(8, 5)],
        32,
        2,
        budget(),
    )
    .unwrap();
    let snapshot = owner.capture_nonzero(budget()).unwrap();
    assert_eq!(snapshot.rows(), [record(0, 7), record(8, 6), record(8, 5)]);
    assert!(!snapshot.source_cap_reached());
    assert!(
        owner
            .capture_nonzero(Budget {
                records: 2,
                work: 4
            })
            .is_err()
    );
    let rows = (0..501).map(|key| record(key, 1)).collect();
    let large = Owner::from_snapshot(rows, 512, 2500, budget()).unwrap();
    let capped = large
        .capture_nonzero(Budget {
            records: 500,
            work: 500,
        })
        .unwrap();
    assert_eq!(capped.rows().len(), 500);
    assert_eq!(capped.rows()[499], record(499, 1));
    assert!(capped.source_cap_reached());
    assert!(
        large
            .capture_nonzero(Budget {
                records: 500,
                work: 499
            })
            .is_err()
    );
    assert_eq!(large.rows().len(), 501);
}

#[test]
fn rejected_restore_is_lazy_and_does_not_reset_even_for_malformed_payload() {
    let mut owner = Owner::from_snapshot(vec![record(7, 9)], 32, 1, budget()).unwrap();
    let malformed = vec![record(1, 2); 501];
    for gate in [
        Admission {
            captured: false,
            ..admission()
        },
        Admission {
            current_generation: 10,
            ..admission()
        },
    ] {
        assert_eq!(
            owner
                .restore_admitted(
                    gate,
                    &malformed,
                    Budget {
                        records: 0,
                        work: 0
                    }
                )
                .unwrap(),
            RestoreOutcome::Rejected
        );
    }
    assert_eq!(owner.rows(), [record(7, 9)]);
    assert!(
        owner
            .restore_admitted(admission(), &malformed, budget())
            .is_err()
    );
    assert_eq!(owner.rows(), [record(7, 9)]);
}

#[test]
fn restore_resets_prefix_then_applies_saved_order_with_zero_key_skip_and_duplicate_last_write() {
    let mut owner = Owner::from_snapshot(
        vec![record(7, 1), record(7, 2), record(9, 3), record(0, 4)],
        32,
        2,
        budget(),
    )
    .unwrap();
    let saved = [
        record(9, 8),
        record(7, 5),
        record(7, 6),
        record(0, 99),
        record(10, 7),
    ];
    assert_eq!(
        owner
            .restore_admitted(admission(), &saved, budget())
            .unwrap(),
        RestoreOutcome::Restored { rows_applied: 4 }
    );
    assert_eq!(
        owner.rows(),
        [
            record(7, 6),
            record(7, 0),
            record(9, 8),
            record(0, 4),
            record(10, 7)
        ]
    );
    assert_eq!(
        (owner.capacity(), owner.reset_limit(), owner.origin()),
        (32, 2, Origin::SuppliedSnapshot)
    );
}

#[test]
fn restore_work_and_capacity_failures_leave_all_original_values_and_capacity_intact() {
    let mut owner =
        Owner::from_snapshot(vec![record(1, 8), record(2, 9)], 32, 2, budget()).unwrap();
    let before = owner.rows().to_vec();
    assert!(
        owner
            .restore_admitted(
                admission(),
                &[record(2, 3)],
                Budget {
                    records: 32,
                    work: 4
                }
            )
            .is_err()
    );
    assert_eq!(owner.rows(), before);
    let rows = (1..=32).map(|key| record(key, 9)).collect();
    let mut full = Owner::from_snapshot(rows, 32, 32, budget()).unwrap();
    let before = full.rows().to_vec();
    assert!(
        full.restore_admitted(
            admission(),
            &[record(33, 1)],
            Budget {
                records: 32,
                work: 1000
            }
        )
        .is_err()
    );
    assert_eq!(full.rows(), before);
    assert_eq!(full.capacity(), 32);
    assert!(Owner::from_snapshot(vec![record(1, 2)], 0, 0, budget()).is_err());
    assert!(
        Owner::from_snapshot(
            Vec::new(),
            32,
            0,
            Budget {
                records: 31,
                work: 0
            }
        )
        .is_err()
    );
}
