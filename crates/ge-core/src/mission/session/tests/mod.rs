use super::*;

fn session() -> Session {
    Session::begin(
        NewGameRequest::new(Difficulty::Agent),
        SessionId::new(41).unwrap(),
        UserId::new(72).unwrap(),
    )
}

fn created() -> Session {
    let mut session = session();
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    session
}

#[test]
fn source_request_is_tutorial_one_user_start_one_for_each_difficulty() {
    for difficulty in [
        Difficulty::Operative,
        Difficulty::Agent,
        Difficulty::DoubleOSeven,
        Difficulty::Classic,
    ] {
        let request = NewGameRequest::new(difficulty);
        assert_eq!(request.difficulty(), difficulty);
        assert_eq!(request.level_hash(), 0x0181_016b);
        assert_eq!(request.start_point_id(), 1);
        assert_eq!(request.local_user_count(), 1);
    }
    assert!(SessionId::new(0).is_err());
    assert!(UserId::new(0).is_err());
}

#[test]
fn pending_operations_never_publish_and_create_counter_is_not_user_count() {
    let mut session = session();
    session.mode_result(OperationResult::Pending).unwrap();
    assert_eq!(session.mode(), 0);
    assert_eq!(session.source_create_counter(), 8);
    assert!(session.descriptor().is_none());
    session.mode_result(OperationResult::Succeeded).unwrap();
    assert_eq!((session.mode(), session.backend()), (3, 2));
    session.begin_create().unwrap();
    assert_eq!(session.source_create_counter(), 7);
    session.create_result(OperationResult::Pending).unwrap();
    assert_eq!(session.active(), None);
    assert!(session.managers().is_none());
    session.create_result(OperationResult::Succeeded).unwrap();
    assert_eq!(session.descriptor().unwrap().local_user_count, 1);
    assert_eq!(session.descriptor().unwrap().level_hashes, [0x0181_016b; 3]);
    assert_eq!(session.source_create_counter(), 7);
}

#[test]
fn admission_uses_exact_created_identity_and_starts_with_no_world_readiness() {
    let mut session = created();
    let correct = SessionId::new(41).unwrap();
    let wrong = SessionId::new(42).unwrap();
    assert!(session.select_pending(correct).unwrap());
    assert!(!session.select_pending(wrong).unwrap());
    assert_eq!(session.pending(), None);
    let before = session.clone();
    assert!(
        session
            .admission_result(OperationResult::Succeeded)
            .is_err()
    );
    assert_eq!(session, before);
    session.select_pending(correct).unwrap();
    session.admission_result(OperationResult::Pending).unwrap();
    assert_eq!(session.active(), None);
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    assert_eq!(session.active(), Some(correct));
    assert_eq!(session.pending(), None);
    assert_eq!(session.stage(), Stage::Active);
    assert!(!session.multiple_local_users_in_mode_three());
    let managers = session.managers().unwrap();
    assert_eq!(managers.local_user_count, 1);
    assert_eq!(managers.readiness_flags_raw, 0);
    assert_eq!(managers.user.flags_raw, 0);
    assert_eq!(managers.user.created_byte_raw, 0);
}

#[test]
fn each_operation_failure_is_explicit_and_never_activates() {
    let mut mode = session();
    mode.mode_result(OperationResult::Failed).unwrap();
    assert_eq!(mode.stage(), Stage::Failed(Failure::Mode));
    assert!(mode.descriptor().is_none());
    let mut create = session();
    create.mode_result(OperationResult::Succeeded).unwrap();
    create.begin_create().unwrap();
    create.create_result(OperationResult::Failed).unwrap();
    assert_eq!(create.stage(), Stage::Failed(Failure::Create));
    let mut admission = created();
    admission
        .select_pending(SessionId::new(41).unwrap())
        .unwrap();
    admission.admission_result(OperationResult::Failed).unwrap();
    assert_eq!(admission.stage(), Stage::Failed(Failure::Admission));
    assert_eq!(admission.pending(), None);
    for session in [mode, create, admission] {
        assert_eq!(session.active(), None);
    }
}

#[test]
fn invalid_sequence_is_atomic_and_teardown_requires_fresh_ownership() {
    let mut session = session();
    let before = session.clone();
    assert!(session.begin_create().is_err());
    assert!(session.create_result(OperationResult::Succeeded).is_err());
    assert!(session.select_pending(SessionId::new(41).unwrap()).is_err());
    assert_eq!(session, before);
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    let before = session.clone();
    assert!(session.begin_create().is_err());
    assert_eq!(session, before);
    session.teardown().unwrap();
    assert_eq!(session.lifecycle_bytes(), [0, 1]);
    assert!(session.managers().is_none());
    assert!(session.mode_result(OperationResult::Succeeded).is_err());
    assert!(session.teardown().is_err());
}

#[test]
fn received_delta_requires_known_source_and_matching_admitted_user() {
    let known = UserId::new(9).unwrap();
    let other = UserId::new(10).unwrap();
    let mut peer = PeerSnapshot {
        identity: known,
        flags_raw: 2,
    };
    let mut user = UserSnapshot {
        identity: known,
        flags_raw: 0,
        created_byte_raw: 0,
        player_association: local_users::Association::unavailable(known),
    };
    let ready = FlagsDelta {
        set: 0x100,
        clear: 2,
    };
    assert!(!receive_delta(known, None, &mut user, ready));
    assert!(!receive_delta(other, Some(&mut peer), &mut user, ready));
    assert_eq!(peer.flags_raw, 2);
    assert_eq!(user.flags_raw, 0);
    assert!(receive_delta(known, Some(&mut peer), &mut user, ready));
    assert_eq!(peer.flags_raw, 0x100);
    assert_eq!(user.flags_raw, 0x100);
    user.identity = other;
    assert!(!receive_delta(
        known,
        Some(&mut peer),
        &mut user,
        FlagsDelta { set: 4, clear: 0 }
    ));
    assert_eq!(peer.flags_raw, 0x104);
    assert_eq!(user.flags_raw, 0x100);
    assert_eq!(user.created_byte_raw, 0);
}

#[test]
fn active_teardown_drops_owned_selection_and_next_session_is_fresh() {
    let mut old = created();
    old.select_pending(SessionId::new(41).unwrap()).unwrap();
    old.admission_result(OperationResult::Succeeded).unwrap();
    let active = old.clone();
    assert!(old.admission_result(OperationResult::Succeeded).is_err());
    assert_eq!(old, active);
    old.teardown().unwrap();
    assert_eq!(old.active(), None);
    assert_eq!(old.pending(), None);
    assert!(old.descriptor().is_none());
    assert!(old.managers().is_none());
    let next = Session::begin(
        NewGameRequest::new(Difficulty::Classic),
        SessionId::new(43).unwrap(),
        UserId::new(74).unwrap(),
    );
    assert_ne!(next.identity(), old.identity());
    assert_eq!(next.stage(), Stage::ModePending);
    assert_eq!(next.source_create_counter(), 8);
    assert_eq!(next.lifecycle_bytes(), [0, 0]);
    assert_eq!(next.request().difficulty(), Difficulty::Classic);
}

#[test]
fn clear_wins_over_set_and_readiness_does_not_create_a_player() {
    let delta = FlagsDelta {
        set: 0x105,
        clear: 0x101,
    };
    assert_eq!(delta.apply(2), 6);
    let mut session = created();
    session.select_pending(SessionId::new(41).unwrap()).unwrap();
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    let managers = session.managers().unwrap();
    let mut user = managers.user;
    let mut peer = PeerSnapshot {
        identity: user.identity,
        flags_raw: 0,
    };
    let source = user.identity;
    receive_delta(
        source,
        Some(&mut peer),
        &mut user,
        FlagsDelta {
            set: 0x100,
            clear: 0,
        },
    );
    assert_eq!(user.flags_raw, 0x100);
    assert_eq!(user.created_byte_raw, 0);
    assert_eq!(
        session.managers().unwrap().user.flags_raw,
        0,
        "separate received-input helper cannot mutate session without live completion proof"
    );
}
