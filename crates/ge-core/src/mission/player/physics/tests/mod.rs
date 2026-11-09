use super::*;
use std::cell::Cell;
mod handler_scalar;

#[test]
fn constructor_retains_unknowns_and_owns_empty_buffer() {
    let owner = Owner::from_constructor();
    assert_eq!(owner.item(), None);
    assert_eq!(owner.word(0x10), Some(3));
    assert_eq!(owner.word(0x160), Some(0x411c_cccd));
    assert_eq!(owner.word(0x1d0), Some(0x3f19_999a));
    assert_eq!(owner.word(0x190), Some(0x3f19_999a));
    assert_eq!(owner.word(0x19c), Some(0x3f19_999a));
    assert_eq!(owner.word(0x1c8), Some(0));
    assert_eq!(owner.half(0xc), Some(0));
    for offset in [0, 4, 0xe, 0x34, 0x48, 0x1a0, 0x1b0, 0x1f4] {
        assert_eq!(owner.byte(offset), None);
    }
    assert_eq!(owner.buffer().growth(), 16);
    assert_eq!(owner.buffer().count(), 0);
    assert!(!owner.buffer().has_backing());
    assert_eq!(owner.byte(SOURCE_BYTES), None);
    assert_eq!(owner.word(usize::MAX), None);
}

#[test]
fn player_capsule_rounding_and_unwritten_tail_are_preserved() {
    let mut owner = Owner::from_constructor();
    owner.set_height_radius(0x3fd3_3333, 0x3f80_0000).unwrap();
    assert_eq!(owner.word(0x1d0), Some(0x3f53_3333));
    assert_eq!(owner.word(0x1cc), Some(0x3f00_0000));
    assert_eq!(owner.word(0x190), Some(0x3f00_0000));
    assert_eq!(owner.word(0x19c), Some(0));
    assert_eq!(owner.word(0x1a0), Some(0x3f26_6666));
    assert_eq!(owner.word(0x1ac), Some(0x3f00_0000));
    assert_eq!(owner.word(0x1c8), Some(1));
    assert_eq!(owner.word(0x1b0), None);
    // The paired stores end at 1e0, before the separate owned buffer header.
    assert_eq!(owner.word(0x1e4), None);
    assert_eq!(owner.buffer().growth(), 16);
}

#[test]
fn sphere_branch_preserves_unreached_capsule_scratch() {
    let mut owner = Owner::from_constructor();
    owner
        .set_height_radius(1f32.to_bits(), 2f32.to_bits())
        .unwrap();
    assert_eq!(owner.word(0x19c), Some(0.5f32.to_bits()));
    assert_eq!(owner.word(0x1c8), Some(0));
    assert_eq!(owner.word(0x1a4), None);
    owner
        .set_height_radius(3f32.to_bits(), 1f32.to_bits())
        .unwrap();
    let scratch = owner.word(0x1ac);
    owner
        .set_height_radius(1f32.to_bits(), 2f32.to_bits())
        .unwrap();
    assert_eq!(owner.word(0x1ac), scratch);
    // The capsule-only diameter would overflow; this source branch does not
    // compute it and must not reject a finite sphere input for that reason.
    owner
        .set_height_radius((-f32::MAX).to_bits(), f32::MAX.to_bits())
        .unwrap();
    assert_eq!(owner.word(0x1c8), Some(0));
}

#[test]
fn handler_selection_uses_topbyte_before_high_half_and_defers_unknown_input() {
    let mut owner = Owner::from_constructor();
    owner.apply_handler_kind(None).unwrap();
    assert!(owner.scalar_pending());
    assert_eq!(owner.word(0x168), None);
    assert!(
        owner
            .handler_scalar_bits()
            .unwrap_err()
            .is::<MissingHandlerKind>()
    );
    owner
        .set_height_radius(2f32.to_bits(), 1f32.to_bits())
        .unwrap();
    owner.apply_handler_kind(Some(0x01ff_ffff)).unwrap();
    assert_eq!(owner.word(0x168), Some(4000f32.to_bits()));
    assert_eq!(owner.handler_scalar_bits().unwrap(), 4000f32.to_bits());
    assert!(!owner.scalar_pending());
    for kind in [0x4003_0001, 0x4004_ffff] {
        owner.apply_handler_kind(Some(kind)).unwrap();
        assert_eq!(owner.word(0x168), Some(1000f32.to_bits()));
        assert_eq!(owner.handler_scalar_bits().unwrap(), 1000f32.to_bits());
    }
    for kind in [0x4000_0000, 0x4001_0001, 0x4002_ffff] {
        owner.apply_handler_kind(Some(kind)).unwrap();
        assert_eq!(owner.word(0x168), Some(500f32.to_bits()));
        assert_eq!(owner.handler_scalar_bits().unwrap(), 500f32.to_bits());
    }
    owner.apply_handler_kind(Some(0x4200_0000)).unwrap();
    assert_eq!(owner.word(0x168), Some(0));
}

#[test]
fn deferred_handler_continues_independent_activation_and_suffix() {
    let mut owner = Owner::from_constructor();
    owner.bind_item(7).unwrap();
    let calls = Cell::new(0);
    owner
        .apply_player_initialization(None, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(owner.word(0x160), Some(30f32.to_bits()));
    assert_eq!(owner.word(0x1c8), Some(1));
    assert_eq!(owner.word(0x168), None);
    assert!(owner.scalar_pending());
    assert_eq!(owner.byte(0x1ff), Some(1));
    assert_eq!(owner.word(0x180), Some(2f32.to_bits()));
    assert_eq!(owner.half(8), Some(0x4000));
    assert_eq!(calls.get(), 1);
}

#[test]
fn callback_failure_retains_selection_and_skips_suffix() {
    let mut owner = Owner::from_constructor();
    owner.bind_item(8).unwrap();
    owner
        .apply_player_initialization(Some(0x4003_1234), || {
            anyhow::bail!("membership unavailable")
        })
        .unwrap_err();
    assert_eq!(owner.word(0x168), Some(1000f32.to_bits()));
    assert_eq!(owner.byte(0x1ff), Some(0));
    assert_eq!(owner.word(0x180), Some(0));
    let calls = Cell::new(0);
    owner
        .apply_player_initialization(Some(0x4003_1234), || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(owner.item(), Some(8));
    assert_eq!(owner.half(8), Some(0x4000));
    assert_eq!(owner.byte(0x1ff), Some(1));
    assert_eq!(owner.word(0x180), Some(2f32.to_bits()));
}

#[test]
fn invalid_inputs_are_explicit_and_do_not_erase_state() {
    let mut owner = Owner::from_constructor();
    assert!(owner.bind_item(0).is_err());
    assert!(
        owner
            .apply_player_initialization(Some(0), || Ok(()))
            .is_err()
    );
    let mut before = Vec::new();
    owner.visit_state_words(|v| before.push(v));
    assert!(owner.set_height_radius(f32::NAN.to_bits(), 0).is_err());
    let mut after = Vec::new();
    owner.visit_state_words(|v| after.push(v));
    assert_eq!(before, after);
}

#[test]
fn fingerprint_visits_item_and_actual_owned_buffer_without_raw_pointers() {
    let mut owner = Owner::from_constructor();
    let mut initial = Vec::new();
    owner.visit_state_words(|v| initial.push(v));
    owner.bind_item(0x1_0000_0002).unwrap();
    *owner.buffer_mut() = Buffer::from_owned(2, 16, 1, Some(vec![0xab, 0xcd])).unwrap();
    let mut bound = Vec::new();
    owner.visit_state_words(|v| bound.push(v));
    assert_ne!(initial, bound);
    assert_eq!(&bound[bound.len() - 2..], &[0xab, 0xcd]);
    assert_eq!(owner.word(4), None);
    assert_eq!(owner.word(0x1f0), None);
}
