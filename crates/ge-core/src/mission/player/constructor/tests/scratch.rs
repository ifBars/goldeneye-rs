use super::*;

fn budget() -> ScratchBudget {
    ScratchBudget {
        native_bytes: 64 * 148,
        growth_steps: 64,
    }
}
fn words(owner: &Scratch) -> Vec<u32> {
    let mut out = Vec::new();
    owner.visit_state_words(|word| out.push(word));
    out
}
fn authored(capacity: u32, growth: u32, count: u32) -> Scratch {
    let mut backing = Vec::new();
    backing.resize_with(capacity as usize * 148, MaybeUninit::uninit);
    Scratch {
        capacity,
        growth,
        count,
        backing: Some(backing),
        realloc_calls: 0,
    }
}

#[test]
fn actual_constructor_and_first_reset_allocate_unknown_bytes_without_records() {
    let mut owner = Scratch::from_constructor();
    assert_eq!(
        (owner.capacity(), owner.growth(), owner.count()),
        (0, 16, 0)
    );
    assert!(!owner.has_backing());
    assert_eq!(owner.host_capacity_bytes(), 0);
    let receipt = owner.prepare_reset(budget()).unwrap();
    assert_eq!(
        receipt,
        ScratchReceipt {
            capacity_before: 0,
            capacity_after: 32,
            count: 0,
            requested_bytes: 4736,
            reallocated: true,
            growth_steps: 2,
        }
    );
    assert_eq!(owner.backing_len(), 4736);
    assert!(owner.host_capacity_bytes() >= owner.backing_len());
    assert_eq!(
        (owner.growth(), owner.count(), owner.realloc_calls()),
        (16, 0, 1)
    );
}

#[test]
fn prepared_owner_reuses_same_backing_and_skips_allocator_and_work_budgets() {
    let mut owner = Scratch::from_constructor();
    owner.prepare_reset(budget()).unwrap();
    let before = words(&owner);
    let pointer = owner.backing.as_ref().unwrap().as_ptr();
    let receipt = owner
        .prepare_reset(ScratchBudget {
            native_bytes: 0,
            growth_steps: 0,
        })
        .unwrap();
    assert!(!receipt.reallocated);
    assert_eq!((receipt.requested_bytes, receipt.growth_steps), (0, 0));
    assert_eq!(words(&owner), before);
    assert_eq!(owner.backing.as_ref().unwrap().as_ptr(), pointer);
}

#[test]
fn deficit_comparison_preserves_count_and_growth_instead_of_forcing_capacity32() {
    let mut owner = authored(20, 4, 7);
    let receipt = owner.prepare_reset(budget()).unwrap();
    assert_eq!(receipt.capacity_after, 24);
    assert_eq!(receipt.requested_bytes, 3552);
    assert_eq!(receipt.growth_steps, 1);
    assert_eq!((owner.count(), owner.growth()), (7, 4));
    // A second invocation still enters source growth because 24 < 32.
    assert_eq!(owner.prepare_reset(budget()).unwrap().capacity_after, 28);
    assert_eq!(owner.realloc_calls(), 2);
}

#[test]
fn authored_initialized_prefix_survives_reallocation_without_becoming_records() {
    let mut owner = authored(20, 4, 0);
    for (index, value) in [17_u8, 29, 43].into_iter().enumerate() {
        owner.backing.as_mut().unwrap()[index].write(value);
    }
    owner.prepare_reset(budget()).unwrap();
    for (index, value) in [17_u8, 29, 43].into_iter().enumerate() {
        // Only the three bytes initialized by this authored fixture are read.
        assert_eq!(
            unsafe { owner.backing.as_ref().unwrap()[index].assume_init() },
            value
        );
    }
    assert_eq!(owner.count(), 0);
}

#[test]
fn byte_and_work_budgets_fail_before_any_header_or_allocation_changes() {
    for limit in [
        ScratchBudget {
            native_bytes: 4735,
            growth_steps: 8,
        },
        ScratchBudget {
            native_bytes: 4736,
            growth_steps: 1,
        },
        ScratchBudget {
            native_bytes: 4736,
            growth_steps: 0,
        },
    ] {
        let mut owner = Scratch::from_constructor();
        let before = words(&owner);
        assert!(owner.prepare_reset(limit).is_err());
        assert_eq!(words(&owner), before);
        assert!(!owner.has_backing());
    }
}

#[test]
fn signed_growth_size_and_counter_overflows_do_not_replace_existing_backing() {
    for (capacity, growth) in [
        (20, i32::MAX as u32),
        (0, i32::MAX as u32),
        (0, 0),
        (0, u32::MAX),
    ] {
        let mut owner = authored(capacity, growth, 0);
        let pointer = owner.backing.as_ref().unwrap().as_ptr();
        let before = words(&owner);
        assert!(
            owner
                .prepare_reset(ScratchBudget {
                    native_bytes: usize::MAX,
                    growth_steps: 8
                })
                .is_err()
        );
        assert_eq!(words(&owner), before);
        assert_eq!(owner.backing.as_ref().unwrap().as_ptr(), pointer);
    }
    let mut owner = Scratch::from_constructor();
    owner.realloc_calls = u64::MAX;
    let before = words(&owner);
    assert!(owner.prepare_reset(budget()).is_err());
    assert_eq!(words(&owner), before);
}

#[test]
fn signed_negative_growth_is_supported_only_when_source_finishes_without_loop() {
    let mut owner = authored(20, (-4_i32) as u32, 3);
    assert_eq!(owner.prepare_reset(budget()).unwrap().capacity_after, 16);
    assert_eq!((owner.count(), owner.growth()), (3, (-4_i32) as u32));
    let mut live = authored(20, (-4_i32) as u32, 19);
    let before = words(&live);
    assert!(live.prepare_reset(budget()).is_err());
    assert_eq!(words(&live), before);
}

#[test]
fn canonical_metadata_never_reads_payload_or_hashes_host_allocation_slack() {
    let mut owner = authored(20, 4, 0);
    let before = words(&owner);
    owner.backing.as_mut().unwrap()[0].write(71);
    owner.backing.as_mut().unwrap().reserve(1000);
    assert_eq!(words(&owner), before);
    assert!(!format!("{owner:?}").contains("MaybeUninit"));
    owner.prepare_reset(budget()).unwrap();
    assert_ne!(words(&owner), before);
}
