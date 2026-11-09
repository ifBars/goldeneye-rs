use super::*;

#[test]
fn player_reset_setter_mutates_shared_state_without_counting_a_trigger_lookup() {
    let shared = Shared::from_constructor();
    let mut attempt = Attempt::new(shared.clone());
    attempt.set(0x4200_050b, 0, || Ok(())).unwrap();
    let evidence = attempt.evidence().unwrap();
    assert_eq!(
        (evidence.records, evidence.capacity, evidence.nonzero_values),
        (1, 32, 0)
    );
    assert_eq!(
        (evidence.lookups, evidence.hits, evidence.misses),
        (0, 0, 0)
    );
    let mut next = Attempt::new(shared);
    assert_eq!(next.lookup(0x4200_050b, 99).unwrap(), 0);
}

#[test]
fn stale_player_setter_is_rejected_after_lock_without_publishing_a_record() {
    let mut attempt = Attempt::new(Shared::from_constructor());
    let mut checks = 0;
    assert!(
        attempt
            .set(0x4200_050b, 0, || {
                checks += 1;
                ensure!(
                    checks < 2,
                    "authored cancellation at locked mutation boundary"
                );
                Ok(())
            })
            .is_err()
    );
    assert_eq!(checks, 2);
    assert_eq!(attempt.evidence().unwrap().records, 0);
}

#[test]
fn attempts_share_actual_mutable_owner_and_cancellation_does_not_reset_it() {
    let mut controller =
        crate::game::Controller::new(std::path::PathBuf::from("authored-unused.rvz"));
    let shared = controller.resource_states.clone();
    let mut first = Attempt::new(shared.clone());
    assert_eq!(first.lookup(0x4200_0042, 0x1234_5678).unwrap(), 0x1234_5678);
    shared
        .0
        .lock()
        .unwrap()
        .set(0x4200_0042, 0x1234_00ab, BUDGET)
        .unwrap();
    assert_eq!(first.lookup(0x4200_0042, 7).unwrap(), 0xab);
    let before = first.evidence().unwrap();
    assert_eq!((before.lookups, before.hits, before.misses), (2, 1, 1));
    controller.snapshot = Some(crate::game::Snapshot::admitting(
        ge_core::mission::session::NewGameRequest::new(ge_core::mission::Difficulty::Classic),
    ));
    assert!(controller.cancel());
    drop(first);
    assert!(Arc::ptr_eq(&shared.0, &controller.resource_states.0));
    let mut second = Attempt::new(controller.resource_states.clone());
    assert_eq!(second.lookup(0x4200_0042, 9).unwrap(), 0xab);
    let after = second.evidence().unwrap();
    assert_eq!((after.lookups, after.hits, after.misses), (1, 1, 0));
    assert_eq!(
        (after.records, after.capacity, after.nonzero_values),
        (1, 32, 1)
    );
    assert_eq!(before.state_sha256, after.state_sha256);
    assert!(!after.original_history_verified && !after.snapshot_bound);
}

#[test]
fn closed_evidence_fingerprints_private_values_without_serializing_rows() {
    let shared = Shared::from_constructor();
    let attempt = Attempt::new(shared.clone());
    let before = attempt.evidence().unwrap();
    shared
        .0
        .lock()
        .unwrap()
        .set(0x4200_0009, 3, BUDGET)
        .unwrap();
    let first = attempt.evidence().unwrap();
    shared
        .0
        .lock()
        .unwrap()
        .set(0x4200_0009, 4, BUDGET)
        .unwrap();
    let second = attempt.evidence().unwrap();
    assert_ne!(before.state_sha256, first.state_sha256);
    assert_ne!(first.state_sha256, second.state_sha256);
    assert_eq!(first.records, second.records);
    assert_eq!(first.capacity, second.capacity);
    let value = serde_json::to_value(second).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 14);
    for field in ["rows", "values", "keys", "pointer", "records_raw"] {
        assert!(value.get(field).is_none());
    }
}

#[test]
fn new_game_handoff_clears_only_value_prefix_on_retained_owner() {
    use ge_core::mission::resource_state::Record;
    let shared = Shared(Arc::new(Mutex::new(
        Owner::from_snapshot(
            vec![
                Record { key: 9, value: 5 },
                Record { key: 7, value: 6 },
                Record { key: 3, value: 7 },
            ],
            32,
            2,
            BUDGET,
        )
        .unwrap(),
    )));
    let before = Attempt::new(shared.clone()).evidence().unwrap();
    let worker_owner = shared.clone();
    std::thread::spawn(move || worker_owner.new_game_handoff())
        .join()
        .unwrap()
        .unwrap();
    let mut attempt = Attempt::new(shared.clone());
    assert_eq!(attempt.lookup(0x4200_0009, 99).unwrap(), 0);
    assert_eq!(attempt.lookup(0x4200_0007, 99).unwrap(), 0);
    assert_eq!(attempt.lookup(0x4200_0003, 99).unwrap(), 7);
    let after = attempt.evidence().unwrap();
    assert_eq!(
        (after.records, after.capacity, after.nonzero_values),
        (3, 32, 1)
    );
    assert_eq!((after.lookups, after.hits, after.misses), (3, 3, 0));
    assert_eq!(after.origin, before.origin);
    assert_ne!(after.state_sha256, before.state_sha256);
    // Later mutations survive attempt destruction and clear at the next
    // handoff, rather than on cancel/drop or before each lookup.
    shared
        .0
        .lock()
        .unwrap()
        .set(0x4200_0009, 11, BUDGET)
        .unwrap();
    drop(attempt);
    let mut next = Attempt::new(shared.clone());
    assert_eq!(next.lookup(0x4200_0009, 99).unwrap(), 11);
    shared.new_game_handoff().unwrap();
    assert_eq!(next.lookup(0x4200_0009, 99).unwrap(), 0);
    assert_eq!(next.lookup(0x4200_0003, 99).unwrap(), 7);
}
