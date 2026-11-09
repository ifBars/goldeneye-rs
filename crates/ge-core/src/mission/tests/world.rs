use super::*;

fn key(wrapper: u64, directory: u32) -> Key {
    Key {
        wrapper: WrapperId::new(wrapper).unwrap(),
        map_reference: MapReference::from_directory_index(directory).unwrap(),
    }
}

fn instance(value: u64) -> InstanceId {
    InstanceId::new(value).unwrap()
}

fn registry(slots: usize, loaded: usize) -> Registry {
    Registry::fresh(Budget {
        preregistered_slots: slots,
        loaded_instances: loaded,
    })
    .unwrap()
}

#[test]
fn explicit_fresh_constructor_seeds_only_null_slot() {
    let state = registry(3, 2);
    assert_eq!(
        state.slots(),
        &[Slot {
            wrapper: None,
            map_reference_raw: 0,
            index: 0,
            instance: None,
        }]
    );
    assert!(state.loaded().is_empty());
    assert!(WrapperId::new(0).is_err());
    assert!(InstanceId::new(0).is_err());
    assert_eq!(WrapperId::new(91).unwrap().get(), 91);
    assert_eq!(instance(92).get(), 92);
}

#[test]
fn source_slots_and_loaded_ordinals_have_distinct_orders() {
    let mut state = registry(3, 2);
    let first = key(10, 0);
    let second = key(20, 0);
    assert_eq!(state.preregister(first).unwrap().packed_tag(), 0x0100_0000);
    assert_eq!(state.preregister(second).unwrap().packed_tag(), 0x0200_0000);
    // Construction and publication may be observed separately. Neither index
    // may be replaced with the other's ordinal.
    let second_result = state.publish_constructed(second, instance(2)).unwrap();
    let first_result = state.publish_constructed(first, instance(1)).unwrap();
    assert_eq!((second_result.slot, second_result.loaded_ordinal), (2, 0));
    assert_eq!((first_result.slot, first_result.loaded_ordinal), (1, 1));
    assert_eq!(state.loaded()[0].key, second);
    assert_eq!(state.loaded()[1].key, first);
    assert_eq!(state.slots()[1].instance, Some(instance(1)));
    assert_eq!(state.slots()[2].instance, Some(instance(2)));
}

#[test]
fn duplicate_pair_reuses_before_candidate_or_full_budget_checks() {
    let mut state = registry(3, 2);
    let first = key(1, 0);
    let second = key(2, 0);
    state.preregister(first).unwrap();
    state.preregister(second).unwrap();
    state.publish_constructed(first, instance(11)).unwrap();
    state.publish_constructed(second, instance(22)).unwrap();
    let before = state.clone();
    // Candidate 22 already belongs to another pair, but the loaded-key hit
    // returns before inspecting it, just as it precedes native construction.
    let reused = state.publish_constructed(first, instance(22)).unwrap();
    assert_eq!(reused.instance, instance(11));
    assert_eq!(reused.loaded_ordinal, 0);
    assert_eq!(reused.slot, 1);
    assert!(reused.reused);
    assert_eq!(
        state.preregister(first).unwrap().instance,
        Some(instance(11))
    );
    assert_eq!(state, before);
}

#[test]
fn wrapper_identity_and_canonical_directory_reference_both_define_pair() {
    let mut state = registry(4, 3);
    let keys = [key(4, 0), key(5, 0), key(4, 1)];
    for (ordinal, current) in keys.into_iter().enumerate() {
        state.preregister(current).unwrap();
        state
            .publish_constructed(current, instance(u64::try_from(ordinal).unwrap() + 1))
            .unwrap();
    }
    assert_eq!(state.loaded().len(), 3);
    assert_eq!(state.slots()[1].map_reference_raw, 0x8500_0000);
    assert_eq!(state.slots()[3].map_reference_raw, 0x8500_0001);
    // No literal-hash constructor exists here: resolving a literal hash to its
    // directory ordinal is an asset-reader responsibility.
    assert!(MapReference::from_directory_index(0x0100_0000).is_err());
    assert_eq!(
        MapReference::from_directory_index(0x00ff_ffff)
            .unwrap()
            .raw(),
        0x85ff_ffff
    );
}

#[test]
fn failed_alias_bind_never_publishes_or_invents_a_preregistration() {
    let mut state = registry(3, 2);
    let first_body_alias = key(7, 0);
    let requested_later_alias = key(7, 1);
    state.preregister(first_body_alias).unwrap();
    let before = state.clone();
    assert_eq!(
        state.publish_constructed(requested_later_alias, instance(9)),
        Err(PublicationError::MissingPreregistration {
            key: requested_later_alias,
        })
    );
    assert_eq!(state, before);
    assert_eq!(state.slots()[1].instance, None);
    assert!(state.loaded().is_empty());
    // Constructor-side preregistration survives a failed later publication.
    assert_eq!(state.slots()[1].index, 1);
}

#[test]
fn optional_instance_alias_rebinding_is_explicitly_unsupported_and_atomic() {
    let mut state = registry(3, 2);
    let first = key(1, 0);
    let second = key(2, 0);
    state.preregister(first).unwrap();
    state.preregister(second).unwrap();
    state.publish_constructed(first, instance(10)).unwrap();
    let before = state.clone();
    let error = state.publish_constructed(second, instance(10)).unwrap_err();
    assert_eq!(
        error,
        PublicationError::UnsupportedInstanceReuse {
            instance: instance(10)
        }
    );
    assert!(error.to_string().contains("UnsupportedInstanceReuse"));
    assert_eq!(state, before);
}

#[test]
fn host_budgets_keep_failed_mutations_atomic_and_existing_slots_reusable() {
    assert!(
        Registry::fresh(Budget {
            preregistered_slots: 0,
            loaded_instances: 0,
        })
        .is_err()
    );
    assert!(
        Registry::fresh(Budget {
            preregistered_slots: 257,
            loaded_instances: 1,
        })
        .is_err()
    );
    let mut state = registry(2, 0);
    let only = key(1, 0);
    state.preregister(only).unwrap();
    let before = state.clone();
    assert!(state.preregister(key(2, 0)).is_err());
    assert_eq!(state, before);
    assert_eq!(state.preregister(only).unwrap().index, 1);
    assert_eq!(
        state.publish_constructed(only, instance(1)),
        Err(PublicationError::LoadedBudgetExhausted { limit: 0 })
    );
    assert_eq!(state, before);
}

#[test]
fn supported_slot_encoding_preserves_last_representable_index() {
    let mut state = registry(256, 255);
    for ordinal in 1..=255 {
        let current = key(1000 + ordinal, 0);
        let slot = state.preregister(current).unwrap();
        assert_eq!(u64::from(slot.index), ordinal);
    }
    assert_eq!(state.slots()[255].packed_tag(), 0xff00_0000);
    let before = state.clone();
    assert!(state.preregister(key(9999, 0)).is_err());
    assert_eq!(state, before);
}
