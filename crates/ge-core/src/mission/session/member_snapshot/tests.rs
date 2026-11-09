use super::*;
use crate::mission::{
    Difficulty,
    session::{NewGameRequest, OperationResult},
};

fn pending() -> Session {
    Session::begin(
        NewGameRequest::new(Difficulty::Agent),
        SessionId::new(5).unwrap(),
        UserId::new(19).unwrap(),
    )
}
fn created() -> Session {
    let mut session = pending();
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    session
}

#[test]
fn matching_metadata_is_constructed_alongside_actual_user_and_cleared_by_teardown() {
    let mut session = pending();
    let id = session.identity();
    let user = UserId::new(19).unwrap();
    assert!(session.member_snapshot(id, user).is_err());
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Pending).unwrap();
    assert!(session.member_records.is_none());
    session.create_result(OperationResult::Succeeded).unwrap();
    let snapshot = session.member_snapshot(id, user).unwrap();
    assert_eq!(snapshot.session(), id);
    assert_eq!(snapshot.user(), user);
    assert_eq!(snapshot.source_flag_raw(), Some(0));
    assert_eq!(snapshot.origin(), Origin::FreshConstructor);
    assert_eq!(session.managers().unwrap().user.created_byte_raw, 0);
    assert_eq!(session.managers().unwrap().user.flags_raw, 0);
    session.teardown().unwrap();
    assert!(session.member_records.is_none());
    assert!(session.member_snapshot(id, user).is_err());
}

#[test]
fn wrong_identity_or_lifecycle_cannot_read_or_replace_owned_metadata() {
    let mut session = created();
    let id = session.identity();
    let user = UserId::new(19).unwrap();
    for (id, user) in [
        (SessionId::new(6).unwrap(), user),
        (id, UserId::new(20).unwrap()),
    ] {
        let before = session.clone();
        assert!(session.member_snapshot(id, user).is_err());
        assert!(
            session
                .replace_member_snapshot(MemberSnapshot::from_supplied(id, user, Some(0x80)))
                .is_err()
        );
        assert_eq!(session, before);
    }
    session.teardown().unwrap();
    assert!(
        session
            .replace_member_snapshot(MemberSnapshot::from_supplied(id, user, None))
            .is_err()
    );
}

#[test]
fn supplied_unknown_and_high_bit_bytes_are_preserved_without_changing_ready_flags() {
    let mut session = created();
    let id = session.identity();
    let user = UserId::new(19).unwrap();
    let managers = *session.managers().unwrap();
    for raw in [None, Some(0x80), Some(0xff), Some(0)] {
        session
            .replace_member_snapshot(MemberSnapshot::from_supplied(id, user, raw))
            .unwrap();
        let snapshot = session.member_snapshot(id, user).unwrap();
        assert_eq!(snapshot.source_flag_raw(), raw);
        assert_eq!(snapshot.origin(), Origin::SuppliedSnapshot);
        assert_eq!(session.managers(), Some(&managers));
    }
}
