use super::*;

fn owner() -> (Shared, Lease) {
    let mut shared = Shared::from_boot();
    let lease = shared
        .acquire(
            PlayerId::new(17).unwrap(),
            Budget {
                native_bytes: 63_680,
                leases: 4,
            },
            || Ok(()),
        )
        .unwrap();
    (shared, lease)
}

fn words(owner: &Shared) -> Vec<u32> {
    let mut out = Vec::new();
    owner.visit_state_words(|value| out.push(value));
    out
}

#[test]
fn prefix_initializes_only_the_32_flags_and_34_big_endian_keys() {
    let (mut shared, lease) = owner();
    for index in 0..32 {
        assert!(shared.preload_raw_flag(lease, index).is_err());
        shared.clear_preload_raw_flag(lease, index).unwrap();
        assert_eq!(shared.preload_raw_flag(lease, index).unwrap(), 0);
        assert!(shared.preload_raw_key(lease, index).is_err());
    }
    for index in 0..34 {
        assert!(shared.preload_entry_key(lease, index).is_err());
        shared.clear_preload_entry_key(lease, index).unwrap();
        assert_eq!(shared.preload_entry_key(lease, index).unwrap(), 0);
    }
    let raw = shared.allocation(Kind::Raw).unwrap();
    assert_eq!(raw.known_payload_bytes(), 32);
    for at in 0..512 {
        assert_eq!(raw.payload_byte(at).unwrap(), (at % 16 == 4).then_some(0));
    }
    let entries = shared.allocation(Kind::Array34).unwrap();
    assert_eq!(entries.known_payload_bytes(), 34 * 4);
    for at in 0..7072 {
        assert_eq!(
            entries.payload_byte(at).unwrap(),
            (at % 208 < 4).then_some(0)
        );
    }
    assert_eq!(
        (entries.header_word(0), entries.header_word(4)),
        (Some(208), Some(34))
    );
    assert_eq!(
        (entries.header_word(8), entries.header_word(12)),
        (None, None)
    );
    assert_eq!(
        shared
            .allocation(Kind::Array64)
            .unwrap()
            .known_payload_bytes(),
        0
    );
    assert_eq!(shared.cache_enabled(), 1);
}

#[test]
fn actual_cache_call_writes_preserve_unknown_padding_and_mapping_order() {
    let (mut shared, lease) = owner();
    shared.clear_preload_raw_flag(lease, 31).unwrap();
    shared.initialize_preload_raw_indices(lease, 31).unwrap();
    shared.clear_preload_entry_key(lease, 33).unwrap();
    // Native source writes the occupied flag before its weapon word.
    shared.store_preload_raw_occupied(lease, 31).unwrap();
    assert_eq!(shared.preload_raw_flag(lease, 31).unwrap(), 1);
    assert!(shared.preload_raw_key(lease, 31).is_err());
    shared
        .store_preload_raw_weapon(lease, 31, 0x0200_0101)
        .unwrap();
    for (alternate, variant, entry) in [
        (false, 0, 33),
        (true, 0, 7),
        (false, 1, 8),
        (true, 1, 9),
        (false, 2, 10),
        (true, 2, 11),
    ] {
        shared
            .store_preload_raw_mapping(lease, 31, alternate, variant, entry)
            .unwrap();
    }
    let raw = shared.allocation(Kind::Raw).unwrap();
    for (field, expected) in [
        (5, 33),
        (6, 8),
        (7, 10),
        (8, 255),
        (9, 7),
        (10, 9),
        (11, 11),
        (12, 255),
    ] {
        assert_eq!(raw.payload_byte(31 * 16 + field).unwrap(), Some(expected));
    }
    for field in [13, 14, 15] {
        assert_eq!(raw.payload_byte(31 * 16 + field).unwrap(), None);
    }
    assert_eq!(shared.preload_raw_key(lease, 31).unwrap(), 0x0200_0101);
    assert_eq!(shared.preload_entry_key(lease, 33).unwrap(), 0);
    shared
        .store_preload_entry_key(lease, 33, 0x0900_0765)
        .unwrap();
    assert_eq!(shared.preload_entry_key(lease, 33).unwrap(), 0x0900_0765);
    assert_eq!(
        shared
            .allocation(Kind::Array34)
            .unwrap()
            .payload_byte(33 * 208 + 3)
            .unwrap(),
        Some(0x65)
    );
    assert_eq!(shared.cache_enabled(), 1);
}

#[test]
fn reached_missing_array_preserves_raw_prefix_and_other_allocation_owners() {
    let (mut shared, lease) = owner();
    let retained = shared.allocations[Kind::Array34.index()].take();
    for index in 0..32 {
        shared.clear_preload_raw_flag(lease, index).unwrap();
    }
    let before = words(&shared);
    assert!(shared.clear_preload_entry_key(lease, 0).is_err());
    assert_eq!(words(&shared), before);
    assert_eq!(
        shared.allocation(Kind::Raw).unwrap().known_payload_bytes(),
        32
    );
    assert_eq!(
        shared
            .allocation(Kind::Array64)
            .unwrap()
            .known_payload_bytes(),
        0
    );
    shared.allocations[Kind::Array34.index()] = retained;
    assert_eq!(shared.preload_raw_flag(lease, 31).unwrap(), 0);
    assert_eq!(shared.cache_enabled(), 1);
}

#[test]
fn bounds_and_mapping_validation_fail_without_payload_mutation() {
    let (mut shared, lease) = owner();
    let before = words(&shared);
    assert!(shared.clear_preload_raw_flag(lease, 32).is_err());
    assert!(shared.initialize_preload_raw_indices(lease, 32).is_err());
    assert!(shared.store_preload_raw_occupied(lease, 32).is_err());
    assert!(shared.store_preload_raw_weapon(lease, 32, 1).is_err());
    assert!(shared.preload_raw_flag(lease, 32).is_err());
    assert!(shared.preload_raw_key(lease, 32).is_err());
    assert!(shared.clear_preload_entry_key(lease, 34).is_err());
    assert!(shared.preload_entry_key(lease, 34).is_err());
    assert!(
        shared
            .store_preload_raw_mapping(lease, 0, false, 3, 0)
            .is_err()
    );
    assert!(
        shared
            .store_preload_raw_mapping(lease, 0, true, 0, 34)
            .is_err()
    );
    assert!(
        shared
            .store_preload_raw_mapping(lease, 32, false, 0, 0)
            .is_err()
    );
    assert_eq!(words(&shared), before);
}

#[test]
fn lease_checks_reject_foreign_unfinished_and_released_resets() {
    let (mut shared, lease) = owner();
    let (foreign, foreign_lease) = {
        let mut other = Shared::from_boot();
        let lease = other
            .acquire(
                PlayerId::new(19).unwrap(),
                Budget {
                    native_bytes: 63_680,
                    leases: 4,
                },
                || Ok(()),
            )
            .unwrap();
        (other, lease)
    };
    let before = words(&shared);
    assert!(shared.clear_preload_raw_flag(foreign_lease, 0).is_err());
    assert!(foreign.preload_raw_flag(lease, 0).is_err());
    assert_eq!(words(&shared), before);
    let unfinished = shared
        .begin(
            PlayerId::new(20).unwrap(),
            Budget {
                native_bytes: 63_680,
                leases: 4,
            },
        )
        .unwrap();
    assert!(shared.clear_preload_raw_flag(unfinished, 0).is_err());
    shared.begin_reset(unfinished).unwrap();
    assert!(shared.preload_entry_key(unfinished, 0).is_err());
    shared.clear_preload_raw_flag(lease, 0).unwrap();
    shared.release(lease, || Ok(())).unwrap();
    let after = words(&shared);
    assert!(shared.store_preload_raw_occupied(lease, 0).is_err());
    assert_eq!(words(&shared), after);
}

#[test]
fn subsequent_player_reuses_actual_initialized_payload_until_final_release() {
    let (mut shared, lease) = owner();
    shared.clear_preload_raw_flag(lease, 0).unwrap();
    shared
        .store_preload_entry_key(lease, 0, 0x0900_0765)
        .unwrap();
    let capacity = shared.retained_host_capacity_bytes();
    let next = shared
        .acquire(
            PlayerId::new(21).unwrap(),
            Budget {
                native_bytes: 63_680,
                leases: 4,
            },
            || Ok(()),
        )
        .unwrap();
    assert_eq!(shared.retained_host_capacity_bytes(), capacity);
    assert_eq!(shared.preload_entry_key(next, 0).unwrap(), 0x0900_0765);
    shared.release(lease, || Ok(())).unwrap();
    assert_eq!(shared.preload_raw_flag(next, 0).unwrap(), 0);
    shared.release(next, || Ok(())).unwrap();
    let replacement = shared
        .acquire(
            PlayerId::new(22).unwrap(),
            Budget {
                native_bytes: 63_680,
                leases: 4,
            },
            || Ok(()),
        )
        .unwrap();
    assert!(shared.preload_raw_flag(replacement, 0).is_err());
    assert!(shared.preload_entry_key(replacement, 0).is_err());
}

#[test]
fn scans_skip_unknown_empty_keys_and_stop_at_first_source_choice() {
    let (mut shared, lease) = owner();
    assert!(shared.find_preload_raw_entry(lease, 7).is_err());
    shared.clear_preload_raw_flag(lease, 0).unwrap();
    assert_eq!(shared.find_preload_raw_entry(lease, 7).unwrap(), Some(0));
    assert!(shared.preload_raw_key(lease, 0).is_err());
    shared.store_preload_raw_occupied(lease, 0).unwrap();
    assert!(shared.find_preload_raw_entry(lease, 7).is_err());
    shared.store_preload_raw_weapon(lease, 0, 7).unwrap();
    assert_eq!(shared.find_preload_raw_entry(lease, 7).unwrap(), Some(0));
    assert!(shared.find_preload_raw_entry(lease, 8).is_err());
    shared.clear_preload_raw_flag(lease, 1).unwrap();
    shared.store_preload_raw_occupied(lease, 2).unwrap();
    shared.store_preload_raw_weapon(lease, 2, 8).unwrap();
    assert_eq!(shared.find_preload_raw_entry(lease, 8).unwrap(), Some(1));

    assert!(shared.find_preload_entry(lease, 9).is_err());
    shared.store_preload_entry_key(lease, 0, 9).unwrap();
    assert_eq!(shared.find_preload_entry(lease, 9).unwrap(), Some(0));
    assert!(shared.find_preload_entry(lease, 10).is_err());
    shared.clear_preload_entry_key(lease, 1).unwrap();
    shared.store_preload_entry_key(lease, 2, 10).unwrap();
    assert_eq!(shared.find_preload_entry(lease, 10).unwrap(), Some(1));
}

#[test]
fn complete_known_scans_report_exhaustion_without_mutation() {
    let (mut shared, lease) = owner();
    for index in 0..32 {
        shared.store_preload_raw_occupied(lease, index).unwrap();
        shared
            .store_preload_raw_weapon(lease, index, u32::from(index) + 1)
            .unwrap();
    }
    for index in 0..34 {
        shared
            .store_preload_entry_key(lease, index, u32::from(index) + 1)
            .unwrap();
    }
    let before = words(&shared);
    assert_eq!(shared.find_preload_raw_entry(lease, 99).unwrap(), None);
    assert_eq!(shared.find_preload_entry(lease, 99).unwrap(), None);
    assert_eq!(shared.find_preload_raw_entry(lease, 32).unwrap(), Some(31));
    assert_eq!(shared.find_preload_entry(lease, 34).unwrap(), Some(33));
    assert_eq!(words(&shared), before);
}
