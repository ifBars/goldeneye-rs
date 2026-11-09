use super::*;
use crate::mission::player::identity::{ItemId, WorldId};

fn identity(world: u64, ordinal: u64) -> ItemId {
    ItemId::new(WorldId::new(world).unwrap(), ordinal).unwrap()
}
fn state(owner: &Owner) -> Vec<u32> {
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    words
}

#[test]
fn deferred_result_invalidates_all_four_constructor_bytes_and_freezes_time_local_operands() {
    let mut owner = Owner::from_constructor();
    assert_eq!(owner.handler_scalar_bits().unwrap(), 1.0_f32.to_bits());
    owner.bind_item_identity(identity(7, 1)).unwrap();
    owner
        .set_height_radius(2.0_f32.to_bits(), 1.0_f32.to_bits())
        .unwrap();
    owner.apply_handler_kind(None).unwrap();
    let captured = owner.deferred_handler_scalar().unwrap();
    assert_eq!(captured.item(), Some(1));
    assert_eq!(captured.item_identity(), Some(identity(7, 1)));
    assert_eq!(captured.half_height_bits(), 1.0_f32.to_bits());
    assert_eq!(captured.setter_sequence(), 1);
    assert!((0x168..0x16c).all(|offset| owner.byte(offset).is_none()));
    assert_eq!(owner.half(0x168), None);
    assert_eq!(owner.word(0x168), None);
    owner
        .set_height_radius(8.0_f32.to_bits(), 1.0_f32.to_bits())
        .unwrap();
    owner.bind_item_identity(identity(8, 1)).unwrap();
    assert_eq!(owner.deferred_handler_scalar(), Some(captured));
    assert_eq!(owner.word(0x1d0), Some(4.0_f32.to_bits()));
    assert_eq!(owner.item_identity(), Some(identity(8, 1)));
}

#[test]
fn pending_getter_has_a_specific_dependency_and_never_mutates_captured_or_other_state() {
    let mut owner = Owner::from_constructor();
    owner.bind_item(23).unwrap();
    owner.apply_handler_kind(None).unwrap();
    let before = state(&owner);
    for _ in 0..2 {
        assert!(
            owner
                .handler_scalar_bits()
                .unwrap_err()
                .is::<MissingHandlerKind>()
        );
        assert_eq!(owner.word(0x168), None);
        assert_eq!(state(&owner), before);
    }
    assert_eq!(before[0], 2);
}

#[test]
fn activation_failure_retains_pending_expression_and_real_prefix_without_running_suffix() {
    let mut owner = Owner::from_constructor();
    owner.bind_item_identity(identity(9, 4)).unwrap();
    let calls = Cell::new(0);
    assert!(
        owner
            .apply_player_initialization(None, || {
                calls.set(calls.get() + 1);
                anyhow::bail!("authored membership failure")
            })
            .is_err()
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(owner.word(0x160), Some(30.0_f32.to_bits()));
    assert_eq!(owner.word(0x1c8), Some(1));
    assert_eq!(owner.word(0x168), None);
    assert!(owner.scalar_pending());
    assert_eq!(
        owner.deferred_handler_scalar().unwrap().item_identity(),
        Some(identity(9, 4))
    );
    assert_eq!(owner.byte(0x1ff), Some(0));
    assert_eq!(owner.word(0x180), Some(0));
    assert_eq!(owner.half(8), Some(0));
}

#[test]
fn later_setter_is_a_distinct_operation_not_retroactive_resolution_of_a_capture() {
    let mut owner = Owner::from_constructor();
    owner
        .set_height_radius(2.0_f32.to_bits(), 1.0_f32.to_bits())
        .unwrap();
    owner.apply_handler_kind(None).unwrap();
    let first = owner.deferred_handler_scalar().unwrap();
    owner
        .set_height_radius(4.0_f32.to_bits(), 1.0_f32.to_bits())
        .unwrap();
    owner.apply_handler_kind(None).unwrap();
    let second = owner.deferred_handler_scalar().unwrap();
    assert_eq!((first.setter_sequence(), second.setter_sequence()), (1, 2));
    assert_eq!(
        (first.half_height_bits(), second.half_height_bits()),
        (1.0_f32.to_bits(), 2.0_f32.to_bits())
    );
    owner.apply_handler_kind(Some(0x0100_0021)).unwrap();
    assert_eq!(owner.handler_scalar_bits().unwrap(), 8000.0_f32.to_bits());
    assert!(!owner.scalar_pending());
    assert_eq!(first.setter_sequence(), 1);
    assert_eq!(first.half_height_bits(), 1.0_f32.to_bits());
}

#[test]
fn only_known_top_byte_one_requires_height_and_deferred_capture_does_not_infer_a_branch() {
    let mut owner = Owner::from_constructor();
    for byte in &mut owner.known[0x1d0..0x1d4] {
        *byte = None;
    }
    for (kind, value) in [
        (0x4003_ffff, 1000.0_f32),
        (0x4002_0000, 500.0_f32),
        (0xffff_ffff, 0.0_f32),
    ] {
        owner.apply_handler_kind(Some(kind)).unwrap();
        assert_eq!(owner.handler_scalar_bits().unwrap(), value.to_bits());
    }
    assert!(owner.apply_handler_kind(Some(0x0100_0000)).is_err());
    owner.store_word(0x1d0, 0x7fc0_0123);
    owner.apply_handler_kind(None).unwrap();
    assert_eq!(
        owner.deferred_handler_scalar().unwrap().half_height_bits(),
        0x7fc0_0123
    );
    assert!(
        owner
            .handler_scalar_bits()
            .unwrap_err()
            .is::<MissingHandlerKind>()
    );
    owner.apply_handler_kind(Some(0x4004_0000)).unwrap();
    assert_eq!(owner.handler_scalar_bits().unwrap(), 1000.0_f32.to_bits());
}

#[test]
fn legacy_rebind_clears_world_identity_without_changing_an_existing_capture() {
    let mut owner = Owner::from_constructor();
    owner.bind_item_identity(identity(10, 2)).unwrap();
    owner.apply_handler_kind(None).unwrap();
    let captured = owner.deferred_handler_scalar().unwrap();
    owner.bind_item(2).unwrap();
    assert_eq!(owner.item_identity(), None);
    assert_eq!(owner.deferred_handler_scalar(), Some(captured));
    owner.apply_handler_kind(None).unwrap();
    assert_eq!(
        owner.deferred_handler_scalar().unwrap().item_identity(),
        None
    );
    assert_eq!(owner.deferred_handler_scalar().unwrap().item(), Some(2));
}

#[test]
fn finite_checked_branch_and_operation_identity_overflow_preserve_prior_reached_state() {
    let mut owner = Owner::from_constructor();
    owner.store_word(0x1d0, f32::INFINITY.to_bits());
    owner.apply_handler_kind(None).unwrap();
    let before = state(&owner);
    assert!(owner.apply_handler_kind(Some(0x0100_0000)).is_err());
    assert_eq!(state(&owner), before);
    owner.handler_scalar_sequence = u64::MAX;
    let before = state(&owner);
    assert!(owner.apply_handler_kind(None).is_err());
    assert_eq!(state(&owner), before);
}
