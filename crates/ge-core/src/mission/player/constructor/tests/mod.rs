use super::*;
use std::cell::Cell;

fn player(id: u64) -> PlayerId {
    PlayerId::new(id).unwrap()
}
fn budget() -> Budget {
    Budget {
        native_bytes: 63_680,
        leases: 8,
    }
}
fn words(owner: &Shared) -> Vec<u32> {
    let mut out = Vec::new();
    owner.visit_state_words(|word| out.push(word));
    out
}

#[test]
fn boot_and_first_acquire_own_exact_allocations_and_unknown_headers() {
    let mut shared = Shared::from_boot();
    assert_eq!(
        (
            shared.count(),
            shared.cache_enabled(),
            shared.retained_native_bytes()
        ),
        (0, 1, 0)
    );
    let lease = shared.acquire(player(1), budget(), || Ok(())).unwrap();
    assert_eq!(shared.reset_stage(lease).unwrap(), ResetStage::Complete);
    assert_eq!(
        (
            shared.count(),
            shared.live_leases(),
            shared.entry_noops(),
            shared.cache_stores()
        ),
        (1, 1, 98, 1)
    );
    assert_eq!(shared.retained_native_bytes(), 63_680);
    let raw = shared.allocation(Kind::Raw).unwrap();
    assert_eq!(
        (
            raw.payload_offset(),
            raw.payload_bytes(),
            raw.header_word(0)
        ),
        (0, 512, None)
    );
    for (kind, stride, count, payload) in [
        (Kind::Array34, 208, 34, 7072),
        (Kind::Array64, 876, 64, 56064),
    ] {
        let allocation = shared.allocation(kind).unwrap();
        assert_eq!(
            (allocation.header_word(0), allocation.header_word(4)),
            (Some(stride), Some(count))
        );
        assert_eq!(
            (allocation.header_word(8), allocation.header_word(12)),
            (None, None)
        );
        assert_eq!(
            (
                allocation.payload_offset(),
                allocation.payload_bytes(),
                allocation.entry_noops()
            ),
            (16, payload, count)
        );
    }
}

#[test]
fn second_player_reuses_owned_storage_and_calls_its_reset() {
    let mut shared = Shared::from_boot();
    shared.acquire(player(1), budget(), || Ok(())).unwrap();
    let capacity = shared.retained_host_capacity_bytes();
    let calls = Cell::new(0);
    shared
        .acquire(player(2), budget(), || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(capacity, shared.retained_host_capacity_bytes());
    assert_eq!(
        (
            shared.count(),
            shared.entry_noops(),
            shared.cache_stores(),
            shared.reset_calls(),
            calls.get()
        ),
        (2, 98, 1, 2, 1)
    );
}

#[test]
fn allocation_budget_failure_retains_the_reached_first_block() {
    let mut shared = Shared::from_boot();
    let reset = Cell::new(false);
    assert!(
        shared
            .acquire(
                player(1),
                Budget {
                    native_bytes: 512,
                    leases: 1
                },
                || {
                    reset.set(true);
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(shared.allocation_failed());
    assert!(!reset.get());
    assert_eq!(
        (
            shared.count(),
            shared.retained_native_bytes(),
            shared.live_leases()
        ),
        (0, 512, 0)
    );
    assert!(shared.allocation(Kind::Raw).is_some());
    assert!(shared.allocation(Kind::Array34).is_none());
    assert!(shared.acquire(player(2), budget(), || Ok(())).is_err());
    assert_eq!(shared.retained_native_bytes(), 512);
}

#[test]
fn reset_failure_keeps_count_and_a_typed_recoverable_lease() {
    let mut shared = Shared::from_boot();
    assert!(
        shared
            .acquire(player(7), budget(), || anyhow::bail!(
                "reset provider absent"
            ))
            .is_err()
    );
    let lease = shared.lease(player(7)).unwrap();
    assert_eq!(shared.reset_stage(lease).unwrap(), ResetStage::Failed);
    assert_eq!(
        (
            shared.count(),
            shared.retained_native_bytes(),
            shared.reset_calls()
        ),
        (1, 63_680, 1)
    );
    assert!(shared.acquire(player(7), budget(), || Ok(())).is_err());
    shared.release(lease, || Ok(())).unwrap();
    assert_eq!(shared.retained_native_bytes(), 0);
}

#[test]
fn only_last_release_calls_cleanup_and_drops_actual_backings() {
    let mut shared = Shared::from_boot();
    let a = shared.acquire(player(1), budget(), || Ok(())).unwrap();
    let b = shared.acquire(player(2), budget(), || Ok(())).unwrap();
    let calls = Cell::new(0);
    let first = shared
        .release(a, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(!first.last);
    assert_eq!(first.native_bytes_released, 0);
    assert_eq!(calls.get(), 0);
    let final_receipt = shared
        .release(b, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(final_receipt.last);
    assert_eq!(final_receipt.native_bytes_released, 63_680);
    assert!(final_receipt.host_bytes_released >= 63_680);
    assert_eq!(
        (
            shared.count(),
            shared.live_leases(),
            shared.retained_host_capacity_bytes(),
            calls.get()
        ),
        (0, 0, 0, 1)
    );
    assert_eq!(shared.cache_enabled(), 1);
    assert!(shared.release(b, || Ok(())).is_err());
}

#[test]
fn failed_last_cleanup_preserves_decrement_and_owned_storage() {
    let mut shared = Shared::from_boot();
    let lease = shared.acquire(player(1), budget(), || Ok(())).unwrap();
    assert!(
        shared
            .release(lease, || anyhow::bail!("cache callback missing"))
            .is_err()
    );
    assert_eq!(
        (shared.count(), shared.retained_native_bytes()),
        (0, 63_680)
    );
    assert_eq!(
        shared.reset_stage(lease).unwrap(),
        ResetStage::ReleaseFailed
    );
    assert!(shared.release(lease, || Ok(())).is_err());
    assert!(shared.acquire(player(2), budget(), || Ok(())).is_err());
    assert_eq!(shared.cleanup_calls(), 1);
}

#[test]
fn duplicate_stale_and_budget_rejections_do_not_call_providers() {
    let mut shared = Shared::from_boot();
    let a = shared.acquire(player(1), budget(), || Ok(())).unwrap();
    let snapshot = words(&shared);
    assert!(
        shared
            .acquire(player(1), budget(), || panic!("duplicate reset called"))
            .is_err()
    );
    assert!(
        shared
            .acquire(
                player(2),
                Budget {
                    native_bytes: 63_680,
                    leases: 1
                },
                || panic!("budget reset called")
            )
            .is_err()
    );
    assert_eq!(words(&shared), snapshot);
    let foreign = Lease {
        player: player(3),
        sequence: 1,
    };
    assert!(
        shared
            .release(foreign, || panic!("foreign cleanup called"))
            .is_err()
    );
    assert_eq!(shared.reset_stage(a).unwrap(), ResetStage::Complete);
}

#[test]
fn state_visitor_is_deterministic_without_reading_unwritten_payloads() {
    let mut a = Shared::from_boot();
    let mut b = Shared::from_boot();
    let la = a.acquire(player(1), budget(), || Ok(())).unwrap();
    b.acquire(player(1), budget(), || Ok(())).unwrap();
    assert_eq!(words(&a), words(&b));
    a.release(la, || Ok(())).unwrap();
    assert_ne!(words(&a), words(&b));
    let next = a.acquire(player(1), budget(), || Ok(())).unwrap();
    assert_ne!(next, la);
    assert_eq!((a.entry_noops(), a.cache_stores()), (196, 2));
}
