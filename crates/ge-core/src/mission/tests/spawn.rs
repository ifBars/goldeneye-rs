use super::*;

fn entry(id: u32) -> CatalogEntry {
    CatalogEntry {
        wrapper_flag_raw: 1,
        id,
        trigger_flags_raw: 0,
        position_bits: [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 0],
        rotation_bits: [4.0f32.to_bits(), 5.0f32.to_bits(), 6.0f32.to_bits(), 0],
    }
}

fn budget() -> Budget {
    Budget {
        entries: 16,
        visits: 32,
    }
}

#[test]
fn requested_id_wins_over_earlier_fallback_and_keeps_first_eligible_duplicate() {
    let catalog = [entry(0), entry(9), entry(9)];
    let plan = SourceSpawnPlan::prepare(&catalog, 9, budget()).unwrap();
    assert_eq!(
        plan.selection(),
        Selection {
            result: SelectionResult::Requested { catalog_index: 1 },
            requested_id_before: 9,
            requested_id_after: 9,
            visits: 2,
        }
    );
    assert!(!plan.selection().used_fallback());
    assert_eq!(plan.selection().selected_index(), Some(1));
}

#[test]
fn wrapper_nonzero_is_signed_byte_eligibility_and_only_trigger_bit_zero_disables() {
    let catalog = [
        CatalogEntry {
            wrapper_flag_raw: 0,
            ..entry(7)
        },
        CatalogEntry {
            trigger_flags_raw: 1,
            ..entry(7)
        },
        CatalogEntry {
            wrapper_flag_raw: 0x80,
            trigger_flags_raw: 0xffff_fffe,
            ..entry(7)
        },
        entry(7),
    ];
    let plan = SourceSpawnPlan::prepare(&catalog, 7, budget()).unwrap();
    assert_eq!(plan.selection().selected_index(), Some(2));
    assert_eq!(plan.selection().visits, 3);
    let max_id = SourceSpawnPlan::prepare(&[entry(u32::MAX)], u32::MAX, budget()).unwrap();
    assert_eq!(max_id.selection().selected_index(), Some(0));
    assert_eq!(max_id.selection().requested_id_after, u32::MAX);
}

#[test]
fn fallback_rescans_storage_order_and_changes_requested_id_only_after_a_match() {
    let catalog = [
        CatalogEntry {
            wrapper_flag_raw: 0,
            ..entry(0)
        },
        CatalogEntry {
            trigger_flags_raw: 1,
            ..entry(0)
        },
        entry(0),
        entry(0),
        entry(4),
    ];
    let plan = SourceSpawnPlan::prepare(&catalog, 99, budget()).unwrap();
    assert_eq!(
        plan.selection(),
        Selection {
            result: SelectionResult::Fallback { catalog_index: 2 },
            requested_id_before: 99,
            requested_id_after: 0,
            visits: 8,
        }
    );
    assert!(plan.selection().used_fallback());
    let direct_zero = SourceSpawnPlan::prepare(&catalog, 0, budget()).unwrap();
    assert_eq!(
        direct_zero.selection().result,
        SelectionResult::Requested { catalog_index: 2 }
    );
    assert!(!direct_zero.selection().used_fallback());
}

#[test]
fn missing_selection_keeps_request_and_has_no_fabricated_origin_pose() {
    let plan = SourceSpawnPlan::prepare(&[entry(2)], 3, budget()).unwrap();
    assert_eq!(plan.selection().result, SelectionResult::Missing);
    assert_eq!(plan.selection().requested_id_after, 3);
    assert_eq!(plan.selection().visits, 2);
    assert_eq!(plan.pose(), None);
    let empty = SourceSpawnPlan::prepare(
        &[],
        1,
        Budget {
            entries: 0,
            visits: 0,
        },
    )
    .unwrap();
    assert_eq!(empty.selection().result, SelectionResult::Missing);
    assert_eq!(empty.selection().visits, 0);
    assert_eq!(empty.pose(), None);
}

#[test]
fn pose_producer_copies_raw_lanes_and_overwrites_only_rotation_x_z_with_positive_zero() {
    let source = CatalogEntry {
        position_bits: [0x7fc0_1234, 0x8000_0000, 0x7f80_0000, 0x3f80_0000],
        rotation_bits: [0x8000_0000, 0x7fa0_4321, 0xff80_0000, 0xdead_beef],
        ..entry(1)
    };
    let plan = SourceSpawnPlan::prepare(&[source], 1, budget()).unwrap();
    assert_eq!(
        plan.pose(),
        Some(Pose {
            position_bits: source.position_bits,
            rotation_bits: [0, 0x7fa0_4321, 0, 0xdead_beef],
            rotation_mode: 3,
        })
    );
    assert_eq!(source.rotation_bits[0], 0x8000_0000);
    assert_eq!(source.rotation_bits[2], 0xff80_0000);
}

#[test]
fn budget_exhaustion_is_an_error_without_mutating_input_or_caching_missing() {
    let catalog = [entry(2), entry(0)];
    let original = catalog;
    assert!(
        SourceSpawnPlan::prepare(
            &catalog,
            9,
            Budget {
                entries: 1,
                visits: 32
            }
        )
        .is_err()
    );
    assert!(
        SourceSpawnPlan::prepare(
            &catalog,
            9,
            Budget {
                entries: 2,
                visits: 3
            }
        )
        .is_err()
    );
    let matched = SourceSpawnPlan::prepare(
        &catalog,
        9,
        Budget {
            entries: 2,
            visits: 4,
        },
    )
    .unwrap();
    assert_eq!(matched.selection().selected_index(), Some(1));
    assert!(matched.selection().used_fallback());
    assert_eq!(catalog, original);
    // Even requested ID zero follows the source's second scan on a first miss.
    assert!(
        SourceSpawnPlan::prepare(
            &[entry(2)],
            0,
            Budget {
                entries: 1,
                visits: 1
            }
        )
        .is_err()
    );
}

fn inputs() -> CreationInputs {
    // Fully authored producer snapshot. No inferred live-ready defaults exist.
    CreationInputs {
        session_byte_41_raw: 0,
        session_byte_40_raw: 0,
        external_state_0b_active: false,
        selected_level_hash: 0x0181_0001,
        manager_10_present: true,
        manager_14_present: true,
        manager_14_ready_100: true,
        user_snapshot: Some(UserSnapshot {
            byte_4a_raw: 0,
            flags_28_raw: 0x100,
        }),
        session_mode: 0,
        local_user_count: 1,
    }
}

#[test]
fn explicit_creation_dependencies_gate_without_addressing_catalog_or_claiming_creation() {
    for (snapshot, reason) in [
        (
            CreationInputs {
                session_byte_41_raw: 0x80,
                ..inputs()
            },
            GateReason::Session41,
        ),
        (
            CreationInputs {
                session_byte_40_raw: 1,
                ..inputs()
            },
            GateReason::Session40,
        ),
        (
            CreationInputs {
                external_state_0b_active: true,
                ..inputs()
            },
            GateReason::ExternalState0b,
        ),
        (
            CreationInputs {
                selected_level_hash: 0x0108_0004,
                ..inputs()
            },
            GateReason::FrontendLevel,
        ),
        (
            CreationInputs {
                manager_10_present: false,
                ..inputs()
            },
            GateReason::Manager10Absent,
        ),
        (
            CreationInputs {
                manager_14_present: false,
                ..inputs()
            },
            GateReason::Manager14Absent,
        ),
        (
            CreationInputs {
                manager_14_ready_100: false,
                ..inputs()
            },
            GateReason::Manager14NotReady,
        ),
        (
            CreationInputs {
                user_snapshot: None,
                ..inputs()
            },
            GateReason::UserSnapshotAbsent,
        ),
        (
            CreationInputs {
                user_snapshot: Some(UserSnapshot {
                    byte_4a_raw: 0xff,
                    flags_28_raw: 0x100,
                }),
                ..inputs()
            },
            GateReason::UserAlreadyCreated,
        ),
        (
            CreationInputs {
                user_snapshot: Some(UserSnapshot {
                    byte_4a_raw: 0,
                    flags_28_raw: 0x200,
                }),
                ..inputs()
            },
            GateReason::UserNotEligible,
        ),
    ] {
        assert_eq!(
            evaluate(
                snapshot,
                &[entry(1)],
                1,
                Budget {
                    entries: 0,
                    visits: 0
                }
            )
            .unwrap(),
            Evaluation::Gated(reason)
        );
    }
    let mut snapshot = inputs();
    snapshot.session_byte_41_raw = 1;
    snapshot.session_byte_40_raw = 1;
    snapshot.external_state_0b_active = true;
    assert_eq!(
        evaluate(snapshot, &[], 1, budget()).unwrap(),
        Evaluation::Gated(GateReason::Session41)
    );
    snapshot.session_byte_41_raw = 0;
    assert_eq!(
        evaluate(snapshot, &[], 1, budget()).unwrap(),
        Evaluation::Gated(GateReason::Session40)
    );
}

#[test]
fn unsupported_caller_branches_stay_separate_from_missing_and_prepared_selection() {
    for mode in [1, 2] {
        assert_eq!(
            evaluate(
                CreationInputs {
                    session_mode: mode,
                    ..inputs()
                },
                &[entry(1)],
                1,
                Budget {
                    entries: 0,
                    visits: 0
                }
            )
            .unwrap(),
            Evaluation::Unsupported(UnsupportedPath::SessionMode { mode })
        );
    }
    for count in [0, 2, u32::MAX] {
        assert_eq!(
            evaluate(
                CreationInputs {
                    local_user_count: count,
                    ..inputs()
                },
                &[entry(1)],
                1,
                Budget {
                    entries: 0,
                    visits: 0
                }
            )
            .unwrap(),
            Evaluation::Unsupported(UnsupportedPath::LocalUserCount { count })
        );
    }
    let Evaluation::Planned(plan) = evaluate(inputs(), &[entry(1)], 1, budget()).unwrap() else {
        panic!("authored supported snapshot did not plan");
    };
    assert_eq!(plan.selection().selected_index(), Some(0));
    let Evaluation::Planned(missing) = evaluate(inputs(), &[], 1, budget()).unwrap() else {
        panic!("authored snapshot did not reach selection");
    };
    assert_eq!(missing.selection().result, SelectionResult::Missing);
    assert_eq!(missing.pose(), None);
}
