use super::super::{NewGameRequest, OperationResult, SessionId, UserId};
use super::*;
use crate::mission::{
    Difficulty,
    loader::work::{Budget, WorkOwner},
};

fn admitted(identity: u64, user: u64) -> Session {
    let id = SessionId::new(identity).unwrap();
    let mut session = Session::begin(
        NewGameRequest::new(Difficulty::Operative),
        id,
        UserId::new(user).unwrap(),
    );
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    assert!(session.select_pending(id).unwrap());
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    session
}
fn attempt(value: u64) -> AttemptId {
    AttemptId::new(value).unwrap()
}

#[test]
fn only_actual_admitted_local_context_binds_one_peer_and_mirror() {
    let mut pending = Session::begin(
        NewGameRequest::new(Difficulty::Operative),
        SessionId::new(1).unwrap(),
        UserId::new(9).unwrap(),
    );
    assert!(pending.bind_loader(attempt(2)).is_err());
    assert!(pending.readiness_load_snapshot().is_none());
    let mut session = admitted(1, 9);
    let binding = session.bind_loader(attempt(2)).unwrap();
    assert_eq!(
        (
            binding.session().get(),
            binding.user().get(),
            binding.attempt().get()
        ),
        (1, 9, 2)
    );
    assert_eq!(binding.level_hash(), 0x0181_016b);
    let snapshot = session.readiness_load_snapshot().unwrap();
    assert_eq!(
        (
            snapshot.owner_flags_raw,
            snapshot.peer_flags_raw,
            snapshot.user_flags_raw,
            snapshot.local_mirror_flags_raw,
            snapshot.created_byte_raw
        ),
        (0, Some(0), 0, 1, 0)
    );
    assert!(session.bind_loader(attempt(3)).is_err());
    assert_eq!(session.readiness_load_snapshot(), Some(snapshot));
}

#[test]
fn whole_phase19_delivers_two_changed_events_and_four_actual_mirror_stores() {
    let mut session = admitted(11, 97);
    let binding = session.bind_loader(attempt(12)).unwrap();
    // Authored phase snapshot exercises this pure bridge; production must own
    // the real preceding operations, catalog and Wait28 promotion to19.
    let mut phase = PhaseState::new(attempt(12), 19).unwrap();
    let receipt = session
        .finish_tutorial_phase19(&binding, &mut phase)
        .unwrap();
    assert_eq!(
        receipt.actions.map(|action| action.action),
        [
            FlagAction::Clear(2),
            FlagAction::Set(0x100),
            FlagAction::Set(8),
            FlagAction::Clear(0x400)
        ]
    );
    assert_eq!(
        receipt.actions.map(|action| action.owner_after),
        [0, 0x100, 0x108, 0x108]
    );
    assert_eq!(
        receipt.actions.map(|action| action.local_after),
        [1, 0x101, 0x109, 0x109]
    );
    assert_eq!(
        (
            receipt.changed_events,
            receipt.peer_updates,
            receipt.user_updates,
            receipt.local_mirror_stores
        ),
        (2, 2, 2, 4)
    );
    let events = receipt
        .actions
        .iter()
        .filter_map(|action| action.event)
        .collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .map(|event| event.event.delta)
            .collect::<Vec<_>>(),
        [
            FlagsDelta {
                set: 0x100,
                clear: 0
            },
            FlagsDelta { set: 8, clear: 0 }
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.event.session == binding.session()
                && event.event.source == binding.user())
    );
    assert_eq!(
        (
            receipt.owner_flags_raw,
            receipt.peer_flags_raw,
            receipt.user_flags_raw,
            receipt.local_mirror_flags_raw
        ),
        (0x108, 0x108, 0x108, 0x109)
    );
    assert_eq!(
        (
            receipt.created_byte_raw,
            receipt.local_observer_callbacks,
            receipt.user_observer_callbacks
        ),
        (0, 0, 0)
    );
    assert!(!receipt.native_timing_verified);
    assert_eq!((phase.current(), phase.pending()), (28, Some(21)));
    let work = WorkOwner::new(binding.attempt(), Budget { tickets: 0 }).unwrap();
    phase
        .poll(binding.attempt(), &work, || {
            Ok(crate::mission::loader::phase::ResourcePoll::Complete)
        })
        .unwrap();
    assert_eq!(phase.current(), 21);
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 0);
}

#[test]
fn source_reinvocation_keeps_flags_and_emits_no_duplicate_changed_event() {
    let mut session = admitted(2, 20);
    let binding = session.bind_loader(attempt(3)).unwrap();
    let mut first = PhaseState::new(binding.attempt(), 19).unwrap();
    session
        .finish_tutorial_phase19(&binding, &mut first)
        .unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut first)
            .is_err(),
        "already queued21"
    );
    let mut invoked_again = PhaseState::new(binding.attempt(), 19).unwrap();
    let repeated = session
        .finish_tutorial_phase19(&binding, &mut invoked_again)
        .unwrap();
    assert_eq!(repeated.changed_events, 0);
    assert_eq!(repeated.local_mirror_stores, 4);
    assert_eq!(
        (
            repeated.owner_flags_raw,
            repeated.user_flags_raw,
            repeated.local_mirror_flags_raw
        ),
        (0x108, 0x108, 0x109)
    );
    let snapshot = session.readiness_load_snapshot().unwrap();
    assert_eq!(
        (
            snapshot.invocations,
            snapshot.changed_events,
            snapshot.local_mirror_stores
        ),
        (2, 2, 8)
    );
    assert_eq!(snapshot.created_byte_raw, 0);
}

#[test]
fn foreign_session_wrong_attempt_pending_wait_and_cancel_do_not_mutate_flags() {
    let mut session = admitted(4, 40);
    let binding = session.bind_loader(attempt(5)).unwrap();
    let initial = session.readiness_load_snapshot();
    let mut foreign = admitted(6, 40);
    foreign.bind_loader(attempt(5)).unwrap();
    let mut phase = PhaseState::new(binding.attempt(), 19).unwrap();
    assert!(
        foreign
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    let mut wrong = PhaseState::new(attempt(6), 19).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut wrong)
            .is_err()
    );
    let mut pending = PhaseState::new(binding.attempt(), 18).unwrap();
    pending.queue(binding.attempt(), 19).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut pending)
            .is_err()
    );
    phase.cancel(binding.attempt()).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    assert_eq!(session.readiness_load_snapshot(), initial);
}

#[test]
fn missing_peer_preserves_reached_source_writes_and_latches_incompletion() {
    let mut session = admitted(7, 70);
    let binding = session.bind_loader(attempt(8)).unwrap();
    session.readiness_load.as_mut().unwrap().peer = None;
    let mut phase = PhaseState::new(binding.attempt(), 19).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    let snapshot = session.readiness_load_snapshot().unwrap();
    assert_eq!(
        (
            snapshot.owner_flags_raw,
            snapshot.user_flags_raw,
            snapshot.local_mirror_flags_raw
        ),
        (0x108, 0, 0x109)
    );
    assert_eq!(snapshot.peer_flags_raw, None);
    assert!(snapshot.delivery_failed);
    assert_eq!(
        (
            snapshot.invocations,
            snapshot.changed_events,
            snapshot.local_mirror_stores
        ),
        (1, 2, 4)
    );
    assert_eq!((phase.current(), phase.pending()), (19, None));
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    assert_eq!(session.readiness_load_snapshot(), Some(snapshot));
}

#[test]
fn live_user_membership_and_mode_remain_required_after_binding() {
    let mut session = admitted(9, 90);
    let binding = session.bind_loader(attempt(10)).unwrap();
    let mut phase = PhaseState::new(binding.attempt(), 19).unwrap();
    session.managers.as_mut().unwrap().user.identity = UserId::new(91).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    assert_eq!(session.managers().unwrap().readiness_flags_raw, 0);
    session.managers.as_mut().unwrap().user.identity = binding.user();
    session.mode = 2;
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    assert_eq!(session.managers().unwrap().readiness_flags_raw, 0);
}

#[test]
fn creation_byte_is_never_written_by_readiness_and_teardown_releases_binding() {
    let mut session = admitted(10, 100);
    let binding = session.bind_loader(attempt(11)).unwrap();
    session.managers.as_mut().unwrap().user.created_byte_raw = 7;
    let mut phase = PhaseState::new(binding.attempt(), 19).unwrap();
    assert_eq!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .unwrap()
            .created_byte_raw,
        7
    );
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 7);
    session.teardown().unwrap();
    assert!(session.readiness_load_snapshot().is_none());
    let mut later = PhaseState::new(binding.attempt(), 19).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut later)
            .is_err()
    );
}

#[test]
fn bounded_counter_failure_happens_before_any_source_flag_write() {
    let mut session = admitted(12, 120);
    let binding = session.bind_loader(attempt(13)).unwrap();
    session.readiness_load.as_mut().unwrap().invocations = usize::MAX;
    let initial = session.readiness_load_snapshot();
    let mut phase = PhaseState::new(binding.attempt(), 19).unwrap();
    assert!(
        session
            .finish_tutorial_phase19(&binding, &mut phase)
            .is_err()
    );
    assert_eq!(session.readiness_load_snapshot(), initial);
    assert_eq!((phase.current(), phase.pending()), (19, None));
}
