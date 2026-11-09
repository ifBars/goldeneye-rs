use super::*;
use crate::mission::{
    Difficulty,
    session::{NewGameRequest, OperationResult},
};

fn session_id() -> SessionId {
    SessionId::new(31).unwrap()
}
fn user(value: u64) -> UserId {
    UserId::new(value).unwrap()
}
fn budget() -> Budget {
    Budget {
        records: 4,
        visits: 4,
    }
}
fn created() -> Session {
    let mut session = Session::begin(
        NewGameRequest::new(Difficulty::Agent),
        session_id(),
        user(7),
    );
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    session
}
fn active() -> Session {
    let mut session = created();
    session.select_pending(session_id()).unwrap();
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    session
}

#[test]
fn actual_constructor_registers_kind_payload_and_backlink_before_admission() {
    let mut session = created();
    assert!(session.active_member_record_count().is_err());
    let records = session.member_records().unwrap();
    assert_eq!(records.records().len(), 1);
    let row = &records.records()[0];
    assert_eq!(
        (row.id().session(), row.id().get(), row.kind_raw()),
        (session_id(), 1, 2)
    );
    assert_eq!(row.member().record_id(), Some(row.id()));
    assert_eq!(row.member().interface_kind_raw(), Some(2));
    assert_eq!(
        session.member_snapshot(session_id(), user(7)).unwrap(),
        row.member()
    );
    session.select_pending(session_id()).unwrap();
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    assert_eq!(session.active_member_record_count().unwrap(), 1);
    session.teardown().unwrap();
    assert!(session.member_records().is_none());
    assert!(session.active_member_record_count().is_err());
}

#[test]
fn count_traverses_actual_kind_two_records_instead_of_descriptor_or_metadata_fields() {
    let mut session = active();
    session.descriptor.as_mut().unwrap().local_user_count = 99;
    session.managers.as_mut().unwrap().local_user_count = 88;
    let records = session.member_records.as_mut().unwrap();
    records
        .register_member(
            MemberSnapshot::from_constructor(session_id(), user(8)),
            budget(),
        )
        .unwrap();
    assert_eq!(session.active_member_record_count().unwrap(), 2);
    // Authored source record state change: only outer+44 is the predicate.
    let records = session.member_records.as_mut().unwrap();
    records.records[1].kind_raw = 3;
    assert_eq!(session.active_member_record_count().unwrap(), 1);
}

#[test]
fn narrow_unknown_flag_replacement_keeps_class_identity_and_record_backlink() {
    let mut session = active();
    let before = *session.member_snapshot(session_id(), user(7)).unwrap();
    for raw in [None, Some(0xff), Some(0)] {
        session
            .replace_member_snapshot(MemberSnapshot::from_supplied(session_id(), user(7), raw))
            .unwrap();
        let member = session.member_snapshot(session_id(), user(7)).unwrap();
        assert_eq!(member.record_id(), before.record_id());
        assert_eq!(member.interface_kind_raw(), Some(2));
        assert_eq!(member.source_flag_raw(), raw);
        assert_eq!(session.active_member_record_count().unwrap(), 1);
    }
}

#[test]
fn registration_rejects_foreign_unknown_duplicate_or_over_budget_without_publication() {
    let mut records = Owner::from_constructor(session_id());
    for member in [
        MemberSnapshot::from_constructor(SessionId::new(32).unwrap(), user(7)),
        MemberSnapshot::from_supplied(session_id(), user(7), Some(0)),
    ] {
        let before = records.clone();
        assert!(records.register_member(member, budget()).is_err());
        assert_eq!(records, before);
    }
    records
        .register_member(
            MemberSnapshot::from_constructor(session_id(), user(7)),
            budget(),
        )
        .unwrap();
    let before = records.clone();
    assert!(
        records
            .register_member(
                MemberSnapshot::from_constructor(session_id(), user(7)),
                budget()
            )
            .is_err()
    );
    assert!(
        records
            .register_member(
                MemberSnapshot::from_constructor(session_id(), user(8)),
                Budget {
                    records: 1,
                    visits: 1
                }
            )
            .is_err()
    );
    assert_eq!(records, before);
}

#[test]
fn missing_owner_wrong_active_identity_or_broken_backlink_never_returns_a_cached_count() {
    for case in 0..4 {
        let mut session = active();
        match case {
            0 => session.member_records = None,
            1 => session.active = Some(SessionId::new(32).unwrap()),
            2 => {
                session.member_records.as_mut().unwrap().records[0]
                    .id
                    .session = SessionId::new(32).unwrap()
            }
            3 => session.descriptor = None,
            _ => unreachable!(),
        }
        assert!(session.active_member_record_count().is_err());
    }
}
