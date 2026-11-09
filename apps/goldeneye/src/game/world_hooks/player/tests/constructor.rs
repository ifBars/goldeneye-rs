use super::*;
use crate::game::world_hooks::Shared;

#[test]
fn cancelled_constructor_keeps_reached_storage_and_reentry_uses_a_new_identity() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    let first = old
        .begin_player_constructor(PlayerId::new(1).unwrap())
        .unwrap();
    let before = old.player_constructor_snapshot(first).unwrap();
    assert_eq!(
        (before.count, before.reset_calls, before.allocations),
        (1, 1, 3)
    );
    assert_eq!((before.native_bytes, before.entry_noops), (63_680, 98));
    assert!(
        old.begin_player_constructor(PlayerId::new(1).unwrap())
            .is_err()
    );
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    assert!(old.player_constructor_snapshot(first).is_err());
    assert!(old.reset_player_runtime_float(0).is_err());
    assert!(
        current
            .begin_player_constructor(PlayerId::new(1).unwrap())
            .is_err()
    );
    let second = current
        .begin_player_constructor(PlayerId::new(2).unwrap())
        .unwrap();
    let after = current.player_constructor_snapshot(second).unwrap();
    assert_eq!(
        (after.count, after.live_leases, after.reset_calls),
        (2, 2, 2)
    );
    assert_eq!(
        (after.native_bytes, after.host_capacity_bytes),
        (before.native_bytes, before.host_capacity_bytes)
    );
    assert_eq!((after.entry_noops, after.cache_stores), (98, 1));
    assert!(current.player_constructor_snapshot(first).is_err());
    assert_ne!(before.state_sha256, after.state_sha256);
}
