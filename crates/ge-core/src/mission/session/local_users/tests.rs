use super::*;
use crate::mission::{
    Difficulty,
    session::{FlagsDelta, NewGameRequest, OperationResult, PeerSnapshot, receive_delta},
};

fn admitted() -> Session {
    let mut session = Session::begin(
        NewGameRequest::new(Difficulty::Agent),
        SessionId::new(11).unwrap(),
        UserId::new(42).unwrap(),
    );
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    session.select_pending(session.identity()).unwrap();
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    session
}

#[test]
fn actual_user_constructor_null_is_visited_without_publishing_a_player() {
    let session = admitted();
    let target = PlayerId::new(42).unwrap();
    let before = session.clone();
    let lookup = session.lookup_local_user_for_player(target).unwrap();
    assert_eq!(lookup.visited, 1);
    assert_eq!(lookup.target, target);
    assert_eq!(lookup.matched, None);
    assert_eq!(lookup.association.state(), State::KnownNull);
    assert_eq!(lookup.association.origin(), Origin::LocalUserConstructor);
    assert_eq!(lookup.association.session(), Some(session.identity()));
    assert_eq!(
        lookup.association.user(),
        session.managers().unwrap().user.identity
    );
    assert_eq!(session, before);
}

#[test]
fn readiness_and_created_flags_do_not_determine_the_player_association() {
    let mut session = admitted();
    let managers = session.managers.as_mut().unwrap();
    let association = managers.user.player_association;
    let mut peer = PeerSnapshot {
        identity: managers.user.identity,
        flags_raw: 0,
    };
    receive_delta(
        managers.user.identity,
        Some(&mut peer),
        &mut managers.user,
        FlagsDelta {
            set: 0x100,
            clear: 0,
        },
    );
    managers.user.created_byte_raw = 1;
    assert_eq!(managers.user.player_association, association);
    assert_eq!(
        session
            .lookup_local_user_for_player(PlayerId::new(1).unwrap())
            .unwrap()
            .matched,
        None
    );
    session.managers.as_mut().unwrap().user.player_association = Association::from_supplied(
        session.identity(),
        UserId::new(42).unwrap(),
        State::Unavailable,
    );
    session.managers.as_mut().unwrap().user.created_byte_raw = 0;
    assert!(
        session
            .lookup_local_user_for_player(PlayerId::new(1).unwrap())
            .is_err()
    );
}

#[test]
fn supplied_association_matches_the_exact_player_generation() {
    let mut session = admitted();
    let old = PlayerId::new(7).unwrap();
    let new = PlayerId::new(8).unwrap();
    session.managers.as_mut().unwrap().user.player_association = Association::from_supplied(
        session.identity(),
        UserId::new(42).unwrap(),
        State::Bound(old),
    );
    assert_eq!(
        session.lookup_local_user_for_player(old).unwrap().matched,
        Some(UserId::new(42).unwrap())
    );
    assert_eq!(
        session.lookup_local_user_for_player(new).unwrap().matched,
        None
    );
    // Equal user/player integers are separate namespaces, not an association.
    assert_eq!(
        session
            .lookup_local_user_for_player(PlayerId::new(42).unwrap())
            .unwrap()
            .matched,
        None
    );
}

#[test]
fn unknown_and_foreign_associations_cannot_become_lookup_misses() {
    let target = PlayerId::new(1).unwrap();
    for association in [
        Association::unavailable(UserId::new(42).unwrap()),
        Association::from_supplied(
            SessionId::new(12).unwrap(),
            UserId::new(42).unwrap(),
            State::KnownNull,
        ),
        Association::from_supplied(
            SessionId::new(11).unwrap(),
            UserId::new(43).unwrap(),
            State::KnownNull,
        ),
    ] {
        let mut session = admitted();
        session.managers.as_mut().unwrap().user.player_association = association;
        assert!(session.lookup_local_user_for_player(target).is_err());
        assert_eq!(
            session.managers().unwrap().user.player_association,
            association
        );
    }
}

#[test]
fn lookup_preserves_user_lifetime_across_attempts_and_rejects_teardown() {
    let mut session = admitted();
    let association = session.managers().unwrap().user.player_association;
    for generation in [1, 2] {
        let lookup = session
            .lookup_local_user_for_player(PlayerId::new(generation).unwrap())
            .unwrap();
        assert_eq!(lookup.association, association);
        assert_eq!(lookup.matched, None);
    }
    session.teardown().unwrap();
    assert!(
        session
            .lookup_local_user_for_player(PlayerId::new(3).unwrap())
            .is_err()
    );
}

#[test]
fn actual_collection_backing_is_required_instead_of_descriptor_count() {
    let mut session = admitted();
    session.descriptor.as_mut().unwrap().local_user_count = 99;
    assert_eq!(
        session
            .lookup_local_user_for_player(PlayerId::new(1).unwrap())
            .unwrap()
            .visited,
        1
    );
    session.managers.as_mut().unwrap().local_user_count = 0;
    assert!(
        session
            .lookup_local_user_for_player(PlayerId::new(1).unwrap())
            .is_err()
    );
}
