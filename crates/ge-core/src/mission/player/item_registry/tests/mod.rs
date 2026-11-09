use super::*;
use crate::mission::player::{
    identity::{ItemId, WorldId},
    item::{Item, Registration},
};

fn id(world: u64, ordinal: u64) -> ItemId {
    ItemId::new(WorldId::new(world).unwrap(), ordinal).unwrap()
}
fn budget() -> Budget {
    Budget {
        records: 64,
        native_bytes: 512,
        growth_steps: 4,
    }
}
fn state(registry: &Registry) -> Vec<u32> {
    let mut words = Vec::new();
    registry.visit_state_words(|word| words.push(word));
    words
}
fn item(id: ItemId) -> Item {
    Item::from_constructor(id.world(), id, 10).unwrap()
}

#[test]
fn source_constructor_and_first_append_own_actual_semantic_pair_storage() {
    let mut registry = Registry::from_constructor();
    assert_eq!(
        (
            registry.counter(),
            registry.count(),
            registry.capacity(),
            registry.growth()
        ),
        (0, 0, 0, 16)
    );
    assert_eq!(
        (
            registry.native_storage_bytes(),
            registry.host_capacity_bytes()
        ),
        (0, 0)
    );
    let receipt = registry.register(id(3, 1), budget()).unwrap();
    assert_eq!((receipt.item(), receipt.numeric_id()), (id(3, 1), 1));
    assert_eq!(
        (registry.counter(), registry.count(), registry.capacity()),
        (1, 1, 16)
    );
    assert_eq!(registry.native_storage_bytes(), 128);
    assert!(std::mem::size_of::<Record>() > 8);
    assert!(registry.host_capacity_bytes() >= 16 * std::mem::size_of::<Record>());
    assert!(registry.host_capacity_bytes() > registry.native_storage_bytes());
    assert_eq!(
        (
            registry.records()[0].numeric_id(),
            registry.records()[0].item()
        ),
        (1, id(3, 1))
    );
}

#[test]
fn failed_growth_budgets_retain_consumed_counter_without_publishing_storage_or_receipt() {
    let mut registry = Registry::from_constructor();
    for limits in [
        Budget {
            records: 0,
            ..budget()
        },
        Budget {
            native_bytes: 0,
            ..budget()
        },
        Budget {
            growth_steps: 0,
            ..budget()
        },
    ] {
        let before = registry.counter();
        assert!(registry.register(id(4, 1), limits).is_err());
        assert_eq!(registry.counter(), before + 1);
        assert_eq!(
            (
                registry.count(),
                registry.capacity(),
                registry.host_capacity_bytes()
            ),
            (0, 0, 0)
        );
    }
    let receipt = registry.register(id(4, 1), budget()).unwrap();
    assert_eq!(receipt.numeric_id(), 4);
    assert_eq!(registry.records()[0].numeric_id(), 4);
}

#[test]
fn reuse_and_second_growth_preserve_order_and_prior_storage_on_failed_seventeenth_append() {
    let mut registry = Registry::from_constructor();
    for ordinal in 1..=16 {
        registry.register(id(5, ordinal), budget()).unwrap();
    }
    let old_host_bytes = registry.host_capacity_bytes();
    assert!(
        registry
            .register(
                id(5, 17),
                Budget {
                    records: 16,
                    ..budget()
                }
            )
            .is_err()
    );
    assert_eq!(
        (registry.counter(), registry.count(), registry.capacity()),
        (17, 16, 16)
    );
    assert_eq!(registry.host_capacity_bytes(), old_host_bytes);
    assert_eq!(registry.records()[15].item(), id(5, 16));
    let receipt = registry.register(id(5, 17), budget()).unwrap();
    assert_eq!(receipt.numeric_id(), 18);
    assert_eq!(
        (
            registry.count(),
            registry.capacity(),
            registry.native_storage_bytes()
        ),
        (17, 32, 256)
    );
    assert_eq!(
        registry
            .records()
            .iter()
            .map(Record::numeric_id)
            .collect::<Vec<_>>(),
        (1_u32..=16).chain([18]).collect::<Vec<_>>()
    );
}

#[test]
fn source_appends_do_not_deduplicate_items_or_conflate_world_local_ordinals() {
    let mut registry = Registry::from_constructor();
    for item in [id(6, 1), id(7, 1), id(6, 1)] {
        registry.register(item, budget()).unwrap();
    }
    assert_eq!(registry.count(), 3);
    assert_eq!(
        registry
            .records()
            .iter()
            .map(Record::item)
            .collect::<Vec<_>>(),
        [id(6, 1), id(7, 1), id(6, 1)]
    );
    assert_eq!(
        registry
            .records()
            .iter()
            .map(Record::numeric_id)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn unsupported_wrapping_and_nonprogressing_growth_are_explicit_without_record_publication() {
    let mut registry = Registry::from_constructor();
    registry.growth = 0;
    assert!(registry.register(id(8, 1), budget()).is_err());
    assert_eq!((registry.counter(), registry.count()), (1, 0));
    registry.growth = u32::MAX;
    assert!(registry.register(id(8, 1), budget()).is_err());
    assert_eq!((registry.counter(), registry.count()), (2, 0));
    registry.growth = 0x4000_0000;
    assert!(
        registry
            .register(
                id(8, 1),
                Budget {
                    records: usize::MAX,
                    native_bytes: usize::MAX,
                    growth_steps: 1
                }
            )
            .is_err()
    );
    assert_eq!(
        (registry.counter(), registry.count(), registry.capacity()),
        (3, 0, 0)
    );
    registry.counter = u32::MAX;
    let before = state(&registry);
    assert!(registry.register(id(8, 1), budget()).is_err());
    assert_eq!(state(&registry), before);
}

#[test]
fn failed_registry_append_retains_real_derived_prefix_without_completing_item_factory() {
    let mut item = item(id(9, 1));
    let mut registry = Registry::from_constructor();
    assert!(!item.factory_prefix_initialized());
    assert!(item.factory().is_none());
    assert_eq!(item.word_180(), None);
    item.begin_wrapperless_factory().unwrap();
    assert!(
        registry
            .register(
                item.id(),
                Budget {
                    native_bytes: 0,
                    ..budget()
                }
            )
            .is_err()
    );
    assert!(item.factory_prefix_initialized());
    assert!(item.factory().is_none());
    assert_eq!(item.word_180(), None);
    assert_eq!(item.registration(), Registration::Constructed);
    assert_eq!(registry.counter(), 1);
    assert_eq!(registry.count(), 0);
    let receipt = registry.register(item.id(), budget()).unwrap();
    item.finish_wrapperless_factory(&receipt).unwrap();
    assert_eq!(item.factory().unwrap().numeric_id(), 2);
}

#[test]
fn factory_receipt_checks_item_world_prefix_and_replay_without_actor_membership() {
    let item_id = id(10, 1);
    let mut item = item(item_id);
    let mut registry = Registry::from_constructor();
    let receipt = registry.register(item_id, budget()).unwrap();
    assert!(item.finish_wrapperless_factory(&receipt).is_err());
    assert!(item.factory().is_none());
    item.begin_wrapperless_factory().unwrap();
    let wrong_world = registry.register(id(11, 1), budget()).unwrap();
    assert!(item.finish_wrapperless_factory(&wrong_world).is_err());
    let wrong_item = registry.register(id(10, 2), budget()).unwrap();
    assert!(item.finish_wrapperless_factory(&wrong_item).is_err());
    assert!(item.factory().is_none());
    item.set_pose([17; 4], [19; 4]);
    item.finish_wrapperless_factory(&receipt).unwrap();
    let factory = item.factory().unwrap();
    assert_eq!(factory.numeric_id(), 1);
    assert!(!factory.wrapper_bound());
    assert_eq!(
        (
            factory.key(),
            factory.resource_cache_index(),
            factory.cache_index()
        ),
        (u32::MAX, u32::MAX, u32::MAX)
    );
    assert!(!factory.attachment_bound());
    assert_eq!(factory.byte_8e(), 0);
    assert_eq!(item.word_180(), Some(0));
    assert_eq!(item.position_bits(), [0; 4]);
    assert_eq!(item.rotation_bits(), [19; 4]);
    assert_eq!(item.priority(), 0);
    assert_eq!(item.registration(), Registration::Constructed);
    assert_eq!(item.membership_mask(), 0);
    assert_eq!(item.actor_node().previous, None);
    assert_eq!(item.actor_node().next, None);
    assert!(item.begin_wrapperless_factory().is_err());
    assert!(item.finish_wrapperless_factory(&receipt).is_err());
}

#[test]
fn canonical_state_distinguishes_unknown_prefix_success_and_retained_failed_counter() {
    let id = id(12, 1);
    let mut item = item(id);
    let mut registry = Registry::from_constructor();
    let mut common = Vec::new();
    item.visit_state_words(|word| common.push(word));
    item.begin_wrapperless_factory().unwrap();
    let mut prefix = Vec::new();
    item.visit_state_words(|word| prefix.push(word));
    assert_ne!(common, prefix);
    let before = state(&registry);
    assert!(
        registry
            .register(
                id,
                Budget {
                    growth_steps: 0,
                    ..budget()
                }
            )
            .is_err()
    );
    assert_ne!(before, state(&registry));
    let receipt = registry.register(id, budget()).unwrap();
    item.finish_wrapperless_factory(&receipt).unwrap();
    let mut complete = Vec::new();
    item.visit_state_words(|word| complete.push(word));
    assert_ne!(prefix, complete);
}
