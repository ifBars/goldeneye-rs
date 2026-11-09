use super::super::*;
use super::fixtures::{admitted, ready, registry};
use crate::mission::{
    session::{
        SessionId, UserId,
        member_snapshot::{MemberSnapshot, Origin},
    },
    spawn,
};

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
fn constructor_origin_flag_is_captured_once_and_later_metadata_changes_do_not_rewrite_request() {
    let (mut session, binding) = ready();
    let registry = registry(&[1], true);
    assert_eq!(
        session
            .member_snapshot(binding.session(), binding.user())
            .unwrap()
            .origin(),
        Origin::FreshConstructor
    );
    let PlayerAttemptOutcome::Started(request) = session
        .begin_player_attempt(&binding, &owner(), &registry, budget())
        .unwrap()
    else {
        panic!("fresh matching request missing")
    };
    assert_eq!(request.source_flag_raw(), Some(0));
    session
        .replace_member_snapshot(MemberSnapshot::from_supplied(
            binding.session(),
            binding.user(),
            Some(0x80),
        ))
        .unwrap();
    let PlayerAttemptOutcome::InProgress(attempt) = session
        .begin_player_attempt(&binding, &owner(), &registry, budget())
        .unwrap()
    else {
        panic!("captured request redispatched")
    };
    assert_eq!(attempt.source_flag_raw(), Some(0));
    assert_eq!(request.source_flag_raw(), Some(0));
    assert_eq!(
        session
            .member_snapshot(binding.session(), binding.user())
            .unwrap()
            .source_flag_raw(),
        Some(0x80)
    );
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 0);
}

#[test]
fn supplied_unknown_and_high_bit_values_are_captured_without_fake_defaults_or_early_gates() {
    for raw in [None, Some(0x80), Some(0xff), Some(3)] {
        let (mut session, binding) = ready();
        session
            .replace_member_snapshot(MemberSnapshot::from_supplied(
                binding.session(),
                binding.user(),
                raw,
            ))
            .unwrap();
        let PlayerAttemptOutcome::Started(request) = session
            .begin_player_attempt(&binding, &owner(), &registry(&[1], true), budget())
            .unwrap()
        else {
            panic!("metadata must not skip earlier constructor/reset prefixes")
        };
        assert_eq!(request.source_flag_raw(), raw);
        assert_eq!(session.player_attempt().unwrap().source_flag_raw(), raw);
        session
            .replace_member_snapshot(MemberSnapshot::from_supplied(
                binding.session(),
                binding.user(),
                Some(17),
            ))
            .unwrap();
        assert_eq!(request.source_flag_raw(), raw);
    }
}

#[test]
fn foreign_or_missing_metadata_is_an_error_and_never_publishes_a_player_attempt() {
    let (mut session, binding) = ready();
    for (id, user) in [
        (SessionId::new(38).unwrap(), binding.user()),
        (binding.session(), UserId::new(95).unwrap()),
    ] {
        session.member_snapshot = Some(MemberSnapshot::from_supplied(id, user, Some(0)));
        assert!(
            session
                .begin_player_attempt(&binding, &owner(), &registry(&[1], true), budget())
                .is_err()
        );
        assert!(session.player_attempt().is_none());
    }
    session.member_snapshot = None;
    assert!(
        session
            .begin_player_attempt(&binding, &owner(), &registry(&[1], true), budget())
            .is_err()
    );
    assert!(session.player_attempt().is_none());
}

#[test]
fn metadata_flag_does_not_supply_readiness_or_override_descriptor_branch() {
    let mut session = admitted();
    let binding = session
        .bind_loader(crate::mission::loader::phase::AttemptId::new(12).unwrap())
        .unwrap();
    session
        .replace_member_snapshot(MemberSnapshot::from_supplied(
            binding.session(),
            binding.user(),
            Some(0xff),
        ))
        .unwrap();
    assert!(matches!(
        session
            .begin_player_attempt(&binding, &owner(), &registry(&[1], true), budget())
            .unwrap(),
        PlayerAttemptOutcome::Gated(PlayerGate::ManagerNotReady)
    ));
    assert_eq!(session.managers().unwrap().readiness_flags_raw, 0);
    assert_eq!(session.managers().unwrap().user.flags_raw, 0);
    let (mut session, binding) = ready();
    session.descriptor.as_mut().unwrap().word_30_raw = 1;
    assert!(
        session
            .begin_player_attempt(&binding, &owner(), &registry(&[1], true), budget())
            .is_err()
    );
    assert!(session.player_attempt().is_none());
}
