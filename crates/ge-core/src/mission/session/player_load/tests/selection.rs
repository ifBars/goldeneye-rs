use super::super::*;
use super::fixtures::{ready, registry};
use crate::mission::spawn::{self, SelectionResult};

fn budget() -> spawn::Budget {
    spawn::Budget {
        entries: 4,
        visits: 8,
    }
}
fn owner() -> OperationOwner {
    OperationOwner::fresh(OperationBudget { entries: 0 }).unwrap()
}

#[test]
fn request_uses_live_trigger_identity_and_pose_and_dispatches_once() {
    let (mut session, binding) = ready();
    let registry = registry(&[0, 1], true);
    let PlayerAttemptOutcome::Started(request) = session
        .begin_player_attempt(&binding, &owner(), &registry, budget())
        .unwrap()
    else {
        panic!("request missing")
    };
    assert_eq!(request.trigger(), registry.catalog()[1]);
    assert_eq!(request.ordinal(), 1);
    assert_eq!(request.map().instance.get(), 29);
    assert_eq!(request.map().map_selector, 3);
    assert_eq!(
        request.pose().position_bits,
        [0x3f80_0001, 0x8000_0000, 7, 0]
    );
    assert_eq!(request.pose().rotation_bits, [0, 0xbe80_0000, 0, 0]);
    assert_eq!(request.source_flag_raw(), Some(0));
    assert_eq!(request.attempt(), binding.attempt());
    assert_eq!(request.session(), binding.session());
    assert_eq!(request.user(), binding.user());
    assert_eq!(
        request.selection().result,
        SelectionResult::Requested { catalog_index: 1 }
    );
    session
        .record_player_dependency(&binding, PlayerDependency::ComponentInitialization)
        .unwrap();
    let PlayerAttemptOutcome::InProgress(existing) = session
        .begin_player_attempt(&binding, &owner(), &registry, budget())
        .unwrap()
    else {
        panic!("duplicate dispatch")
    };
    assert_eq!(existing.trigger(), request.trigger());
    assert_eq!(
        existing.dependency(),
        PlayerDependency::ComponentInitialization
    );
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 0);
    assert_eq!(
        session.readiness_load_snapshot().unwrap().created_byte_raw,
        0
    );
    session.teardown().unwrap();
    assert!(session.player_attempt().is_none());
}

#[test]
fn fallback_is_actual_catalog_entry_and_missing_or_unrebuilt_is_explicit() {
    let (mut session, binding) = ready();
    assert!(matches!(
        session
            .begin_player_attempt(&binding, &owner(), &registry(&[1], false), budget())
            .unwrap(),
        PlayerAttemptOutcome::Gated(PlayerGate::CatalogNotReady)
    ));
    assert!(matches!(
        session
            .begin_player_attempt(&binding, &owner(), &registry(&[9], true), budget())
            .unwrap(),
        PlayerAttemptOutcome::Gated(PlayerGate::StartPointMissing)
    ));
    assert!(session.player_attempt().is_none());
    let registry = registry(&[7, 0], true);
    let PlayerAttemptOutcome::Started(request) = session
        .begin_player_attempt(&binding, &owner(), &registry, budget())
        .unwrap()
    else {
        panic!("fallback missing")
    };
    assert_eq!(request.trigger(), registry.catalog()[1]);
    assert_eq!(request.selection().requested_id_before, 1);
    assert_eq!(request.selection().requested_id_after, 0);
    assert!(request.selection().used_fallback());
}

#[test]
fn selector_budget_error_and_unstarted_dependency_do_not_publish_an_attempt() {
    let (mut session, binding) = ready();
    assert!(
        session
            .record_player_dependency(&binding, PlayerDependency::PhysicsInitialization)
            .is_err()
    );
    let registry = registry(&[9, 1], true);
    assert!(
        session
            .begin_player_attempt(
                &binding,
                &owner(),
                &registry,
                spawn::Budget {
                    entries: 4,
                    visits: 1
                }
            )
            .is_err()
    );
    assert!(session.player_attempt().is_none());
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 0);
    assert!(matches!(
        session
            .begin_player_attempt(&binding, &owner(), &registry, budget())
            .unwrap(),
        PlayerAttemptOutcome::Started(_)
    ));
}
