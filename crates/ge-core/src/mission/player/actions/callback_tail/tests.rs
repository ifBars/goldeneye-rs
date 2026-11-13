use super::*;
use crate::mission::player::identity::{ItemId, WorldId};

fn fixture(
    e59: Option<u8>,
    state: Option<u32>,
    flags: Option<u32>,
) -> (base_handler::Owner, PendingReset, Item) {
    let player = PlayerId::new(7).unwrap();
    let item_id = ItemId::new(WorldId::new(9).unwrap(), 3).unwrap();
    let mut item = Item::from_constructor(item_id.world(), item_id, 5).unwrap();
    item.bind_handler(Some(player));
    let base = base_handler::Owner::callback_tail_fixture(player, item_id, state, flags);
    let reset = PendingReset::callback_tail_fixture(player, e59);
    (base, reset, item)
}

fn physics_words(owner: &physics::Owner) -> Vec<u32> {
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    words
}

#[test]
fn signed_branch_tail_returns_with_no_physics_and_keeps_raw_e59() {
    for e59 in [0, 1, 0x80, 0xff] {
        let (mut base, mut reset, item) = fixture(Some(e59), Some(0), Some(0x1234_5678));
        let mut tail = CallbackTail::new(base.player());
        tail.advance(&mut base, &mut reset, &item, None).unwrap();
        let state = if e59 == 0 { 2 } else { 3 };
        assert!(tail.callback_returned() && tail.logging_noop_returned());
        assert_eq!(tail.e59_raw(), Some(e59));
        assert_eq!(tail.initial_state_704(), Some(0));
        assert_eq!(tail.state_704(), Some(state));
        assert_eq!(reset.word(0xe60), Some(0));
        assert_eq!(base.word(0x704), Some(state));
        assert_eq!(base.word(0x708), Some(1));
        assert_eq!(base.word(0x220), Some(0x1234_5679));
        assert_eq!(tail.flags_220_before(), Some(0x1234_5678));
        assert_eq!(tail.flags_220_after(), Some(0x1234_5679));
        assert_eq!(tail.physics_outcome(), Some(PhysicsOutcome::Absent));
        let mut before = Vec::new();
        tail.visit_state_words(|word| before.push(word));
        assert!(tail.advance(&mut base, &mut reset, &item, None).is_err());
        let mut after = Vec::new();
        tail.visit_state_words(|word| after.push(word));
        assert_eq!(before, after);
    }
}

#[test]
fn unknown_branch_fails_before_timer_store_and_player_state() {
    let (mut base, mut reset, item) = fixture(None, Some(0), Some(0x17));
    let mut tail = CallbackTail::new(base.player());
    assert!(tail.advance(&mut base, &mut reset, &item, None).is_err());
    assert_eq!(tail.stage(), Stage::DerivedTimer);
    assert!(tail.logging_noop_returned());
    assert_eq!(reset.word(0xe60), Some(0x7f7f_ffff));
    assert_eq!(base.word(0x704), Some(0));
    assert_eq!(base.word(0x708), Some(0x1234_5678));
    assert_eq!(tail.e59_raw(), None);
}

#[test]
fn diagnostic_old_state_is_optional_and_does_not_block_the_source_overwrite() {
    let (mut base, mut reset, item) = fixture(Some(0), None, Some(0x17));
    let mut tail = CallbackTail::new(base.player());
    tail.advance(&mut base, &mut reset, &item, None).unwrap();
    assert!(tail.callback_returned());
    assert_eq!(reset.word(0xe60), Some(0));
    assert_eq!(base.word(0x704), Some(2));
    assert_eq!(tail.initial_state_704(), None);
    assert_eq!(tail.state_704(), Some(2));
    assert_eq!(tail.physics_outcome(), Some(PhysicsOutcome::Absent));
}

#[test]
fn unavailable_nonnull_physics_keeps_prefix_then_resumes_the_same_owned_object() {
    let (mut base, mut reset, mut item) = fixture(Some(0), Some(0), Some(0x17));
    let identity = PhysicsId::new(base.player().get()).unwrap();
    item.bind_physics(Some(identity));
    let mut tail = CallbackTail::new(base.player());
    assert!(tail.advance(&mut base, &mut reset, &item, None).is_err());
    assert_eq!(tail.stage(), Stage::Physics);
    assert_eq!(reset.word(0xe60), Some(0));
    assert_eq!(base.word(0x704), Some(2));
    assert_eq!(base.word(0x708), Some(0x1234_5678));
    assert_eq!(tail.physics_outcome(), None);
    let mut owner = physics::Owner::from_constructor();
    owner.bind_item_identity(item.id()).unwrap();
    tail.advance(&mut base, &mut reset, &item, Some(&mut owner))
        .unwrap();
    assert_eq!(
        tail.physics_outcome(),
        Some(PhysicsOutcome::Cleared {
            identity,
            flags_before: 0,
            flags_after: 0,
        })
    );
    assert_eq!(owner.half(0xa), Some(0));
    assert_eq!(owner.word(0x15c), Some(0));
    assert!(tail.callback_returned());
}

#[test]
fn physics_backlink_failure_does_not_mutate_the_supplied_other_owner() {
    let (mut base, mut reset, mut item) = fixture(Some(0), Some(0), Some(0x17));
    item.bind_physics(Some(PhysicsId::new(base.player().get()).unwrap()));
    let other = ItemId::new(item.world(), 99).unwrap();
    let mut owner = physics::Owner::from_constructor();
    owner.bind_item_identity(other).unwrap();
    let before = physics_words(&owner);
    let mut tail = CallbackTail::new(base.player());
    assert!(
        tail.advance(&mut base, &mut reset, &item, Some(&mut owner))
            .is_err()
    );
    assert_eq!(tail.stage(), Stage::Physics);
    assert_eq!(physics_words(&owner), before);
    assert_eq!(base.word(0x704), Some(2));
    assert_eq!(base.word(0x708), Some(0x1234_5678));
}

#[test]
fn flags_are_required_before_final708_store_after_physics_return() {
    let (mut base, mut reset, item) = fixture(Some(0), Some(0), None);
    let mut tail = CallbackTail::new(base.player());
    assert!(tail.advance(&mut base, &mut reset, &item, None).is_err());
    assert_eq!(tail.stage(), Stage::FinalFlags);
    assert_eq!(tail.physics_outcome(), Some(PhysicsOutcome::Absent));
    assert_eq!(base.word(0x704), Some(2));
    assert_eq!(base.word(0x708), Some(0x1234_5678));
    assert_eq!(base.word(0x220), None);
    assert_eq!(tail.word_708(), None);
    assert!(!tail.callback_returned());
}

#[test]
fn unrelated_player_or_item_is_rejected_before_even_the_logging_noop() {
    let (mut base, mut reset, mut item) = fixture(Some(0), Some(0), Some(0x17));
    item.bind_handler(Some(PlayerId::new(99).unwrap()));
    let mut tail = CallbackTail::new(base.player());
    assert!(tail.advance(&mut base, &mut reset, &item, None).is_err());
    assert_eq!(tail.stage(), Stage::Logging);
    assert!(!tail.logging_noop_returned());
    assert_eq!(reset.word(0xe60), Some(0x7f7f_ffff));
    assert_eq!(base.word(0x704), Some(0));
}
