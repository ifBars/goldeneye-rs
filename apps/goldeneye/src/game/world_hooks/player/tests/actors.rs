use crate::game::world_hooks::Shared;
use ge_core::mission::player::item_registry;

#[test]
fn cancelled_factory_references_keep_actual_world_owners_and_monotonic_registry() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    let first = old
        .with_player_actors(|actors| {
            let id = actors.world.construct_item()?;
            actors.world.item_mut(id)?.begin_wrapperless_factory()?;
            let receipt = actors.registry.register(
                id,
                item_registry::Budget {
                    records: 32,
                    native_bytes: 256,
                    growth_steps: 4,
                },
            )?;
            actors
                .world
                .item_mut(id)?
                .finish_wrapperless_factory(&receipt)?;
            assert_eq!(old.player_registration_mask(id)?, 0x2d);
            Ok(id)
        })
        .unwrap();
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    assert!(
        old.with_player_actors::<()>(|_| panic!("stale actor mutation"))
            .is_err()
    );
    current
        .with_player_actors(|actors| {
            assert_eq!(actors.registry.records()[0].item(), first);
            assert_eq!(actors.world.item(first)?.factory().unwrap().numeric_id(), 1);
            let second = actors.world.construct_item()?;
            actors.world.item_mut(second)?.begin_wrapperless_factory()?;
            let receipt = actors.registry.register(
                second,
                item_registry::Budget {
                    records: 32,
                    native_bytes: 256,
                    growth_steps: 4,
                },
            )?;
            actors
                .world
                .item_mut(second)?
                .finish_wrapperless_factory(&receipt)?;
            assert_eq!(first.world(), second.world());
            assert_ne!(first, second);
            assert_eq!(
                (
                    actors.registry.counter(),
                    actors.registry.count(),
                    actors.registry.capacity()
                ),
                (2, 2, 16)
            );
            assert_eq!(actors.world.item_count(), 2);
            assert!(actors.world.actors()?.is_empty());
            Ok(())
        })
        .unwrap();
}
