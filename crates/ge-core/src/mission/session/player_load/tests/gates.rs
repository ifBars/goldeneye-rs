use super::super::*;
use super::fixtures::{admitted, ready, registry};
use crate::mission::{loader::phase::AttemptId, spawn};

fn budget() -> spawn::Budget {
    spawn::Budget {
        entries: 4,
        visits: 8,
    }
}
fn owner() -> OperationOwner {
    OperationOwner::fresh(OperationBudget { entries: 2 }).unwrap()
}
fn gate(outcome: PlayerAttemptOutcome) -> PlayerGate {
    match outcome {
        PlayerAttemptOutcome::Gated(reason) => reason,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn live_flags_and_external_queue_gate_without_reading_an_unready_catalog() {
    let (mut session, binding) = ready();
    let registry = registry(&[1], false);
    let mut operations = owner();
    session.lifecycle_byte_41 = 0x80;
    session.lifecycle_byte_40 = 1;
    operations
        .enqueue(OperationKind::TYPE_0B, OperationState::Complete)
        .unwrap();
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &operations, &registry, budget())
                .unwrap()
        ),
        PlayerGate::Session41
    );
    session.lifecycle_byte_41 = 0;
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &operations, &registry, budget())
                .unwrap()
        ),
        PlayerGate::Session40
    );
    session.lifecycle_byte_40 = 0;
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &operations, &registry, budget())
                .unwrap()
        ),
        PlayerGate::ExternalOperation0b
    );
    assert!(session.player_attempt().is_none());
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 0);
}

#[test]
fn readiness_owner_and_same_user_flags_are_independent_gates() {
    let (mut session, binding) = ready();
    let registry = registry(&[1], true);
    session.managers.as_mut().unwrap().readiness_flags_raw = 8;
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &owner(), &registry, budget())
                .unwrap()
        ),
        PlayerGate::ManagerNotReady
    );
    session.managers.as_mut().unwrap().readiness_flags_raw = 0x108;
    session.managers.as_mut().unwrap().user.flags_raw = 8;
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &owner(), &registry, budget())
                .unwrap()
        ),
        PlayerGate::UserNotEligible
    );
    session.managers.as_mut().unwrap().user.created_byte_raw = 1;
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &owner(), &registry, budget())
                .unwrap()
        ),
        PlayerGate::UserAlreadyCreated
    );
    assert!(session.player_attempt().is_none());
}

#[test]
fn same_ids_but_foreign_attempt_and_torn_down_session_cannot_dispatch() {
    let original = admitted();
    let mut other = original.clone();
    let binding = other.bind_loader(AttemptId::new(51).unwrap()).unwrap();
    let mut session = original;
    let correct = session.bind_loader(AttemptId::new(52).unwrap()).unwrap();
    let registry = registry(&[1], true);
    assert!(
        session
            .begin_player_attempt(&binding, &owner(), &registry, budget())
            .is_err()
    );
    assert!(session.player_attempt().is_none());
    session.teardown().unwrap();
    assert!(
        session
            .begin_player_attempt(&correct, &owner(), &registry, budget())
            .is_err()
    );
}

#[test]
fn admitted_without_phase19_actual_flag_writes_is_not_ready() {
    let mut session = admitted();
    let binding = session.bind_loader(AttemptId::new(5).unwrap()).unwrap();
    assert_eq!(
        gate(
            session
                .begin_player_attempt(&binding, &owner(), &registry(&[1], true), budget())
                .unwrap()
        ),
        PlayerGate::ManagerNotReady
    );
    assert!(session.player_attempt().is_none());
}
