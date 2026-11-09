use super::*;
use crate::mission::player::{
    identity::{ComponentId, ItemId, PlayerId, WorldId},
    item_registry,
    world::{self, World},
};

fn owner() -> Owner {
    // Focused authored scalar snapshot at this subsystem's entry. Full reset
    // and constructor-flag proof behavior have their own pipeline tests.
    let mut owner = Owner::new(PlayerId::new(7).unwrap());
    owner.stage = Stage::ConstructorFlagComplete;
    owner.w(4, 0);
    owner.w(0x760, u32::MAX);
    owner.b(0x4e4, 13);
    owner.b(0x4e5, 17);
    owner.b(0x4e7, 19);
    owner.w(0x4a4, 0x8000_0000);
    owner
}
fn state(owner: &Owner) -> Vec<u32> {
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    words
}
fn fixture(world_id: u64, finish_factory: bool) -> (World, item_registry::Registry, ItemId) {
    let mut world = World::from_constructor(
        WorldId::new(world_id).unwrap(),
        1,
        10,
        world::Budget {
            items: 4,
            memberships: 10,
        },
    )
    .unwrap();
    let item = world.construct_item().unwrap();
    let mut registry = item_registry::Registry::from_constructor();
    if finish_factory {
        world
            .item_mut(item)
            .unwrap()
            .begin_wrapperless_factory()
            .unwrap();
        let receipt = registry
            .register(
                item,
                item_registry::Budget {
                    records: 16,
                    native_bytes: 128,
                    growth_steps: 2,
                },
            )
            .unwrap();
        world
            .item_mut(item)
            .unwrap()
            .finish_wrapperless_factory(&receipt)
            .unwrap();
    }
    world.item_mut(item).unwrap().prepare_player_owner();
    world
        .item_mut(item)
        .unwrap()
        .bind_handler(Some(PlayerId::new(7).unwrap()));
    (world, registry, item)
}
struct NoComponents;
impl world::Components for NoComponents {
    fn activate(&mut self, _: ItemId, _: ComponentId, _: bool) -> Result<()> {
        anyhow::bail!("authored item constructor has no components")
    }
}

#[test]
fn literal_base_callback_and_motion_defaults_follow_real_item_registration() {
    let mut owner = owner();
    let (mut world, _registry, item) = fixture(21, true);
    owner.bind_item(world.item(item).unwrap()).unwrap();
    assert_eq!(owner.item(), Some(item));
    assert_eq!(owner.stage(), Stage::VirtualItemBound);
    assert_eq!(owner.word(4), None);
    assert_eq!(owner.bindings.get(&4), Some(&Binding::LinkedItem));
    world
        .register_if_absent(item, 0x21, &mut NoComponents)
        .unwrap();
    owner
        .initialize_virtual_motion(world.item(item).unwrap())
        .unwrap();
    assert_eq!(owner.stage(), Stage::VirtualMotionComplete);
    assert_eq!(world.actors().unwrap(), [item]);
    assert_eq!(owner.byte(0x4e4), Some(0));
    assert_eq!(owner.word(0x4a4), Some(0));
    assert_eq!(owner.byte(0x4e7), Some(0));
    assert_eq!(owner.word(0x760), Some(0x4300_0000));
    assert_eq!(owner.word(0x678), Some(0x3f80_0000));
    assert_eq!(owner.word(0x680), Some(0));
    assert_eq!(owner.byte(0x5e4), Some(0));
    assert_eq!(owner.word(0x628), Some(0));
    assert_eq!(owner.word(0x664), Some(0));
    assert_eq!(owner.byte(0x4e5), Some(17)); // adjacent byte preserved
    assert_eq!(owner.word(0x668), None); // no whole-range zero fill
    assert_eq!(owner.word(0x224), None);
    assert_eq!(world.item(item).unwrap().physics(), None);
    let after = state(&owner);
    assert!(
        owner
            .initialize_virtual_motion(world.item(item).unwrap())
            .is_err()
    );
    assert!(owner.bind_item(world.item(item).unwrap()).is_err());
    assert_eq!(state(&owner), after);
}

#[test]
fn missing_factory_wrong_handler_stage_or_existing_backlink_does_not_bind() {
    for case in 0..4 {
        let mut owner = owner();
        let (mut world, _registry, item) = fixture(21, case != 0);
        match case {
            1 => {
                world
                    .item_mut(item)
                    .unwrap()
                    .bind_handler(Some(PlayerId::new(8).unwrap()));
            }
            2 => owner.stage = Stage::RuntimeResetComplete,
            3 => owner.item = Some(item),
            _ => {}
        }
        let before = state(&owner);
        assert!(owner.bind_item(world.item(item).unwrap()).is_err());
        assert_eq!(state(&owner), before);
        assert_eq!(owner.word(4), Some(0));
    }
}

#[test]
fn motion_requires_actual_complete_actor_registration_and_rechecks_handler() {
    let mut owner = owner();
    let (mut world, _registry, item) = fixture(21, true);
    owner.bind_item(world.item(item).unwrap()).unwrap();
    let before = state(&owner);
    assert!(
        owner
            .initialize_virtual_motion(world.item(item).unwrap())
            .is_err()
    );
    assert_eq!(state(&owner), before);
    assert_eq!(owner.word(0x760), Some(u32::MAX));
    world
        .register_if_absent(item, 0x21, &mut NoComponents)
        .unwrap();
    world.item_mut(item).unwrap().bind_handler(None);
    assert!(
        owner
            .initialize_virtual_motion(world.item(item).unwrap())
            .is_err()
    );
    assert_eq!(state(&owner), before);
    world
        .item_mut(item)
        .unwrap()
        .bind_handler(Some(owner.player()));
    owner
        .initialize_virtual_motion(world.item(item).unwrap())
        .unwrap();
}

#[test]
fn same_ordinal_in_a_different_actual_world_is_not_the_bound_item() {
    let mut owner = owner();
    let (world, _registry, item) = fixture(21, true);
    let (mut other, _other_registry, other_item) = fixture(22, true);
    assert_eq!(item.get(), other_item.get());
    assert_ne!(item, other_item);
    owner.bind_item(world.item(item).unwrap()).unwrap();
    other
        .register_if_absent(other_item, 0x21, &mut NoComponents)
        .unwrap();
    let before = state(&owner);
    assert!(
        owner
            .initialize_virtual_motion(other.item(other_item).unwrap())
            .is_err()
    );
    assert_eq!(state(&owner), before);
}

#[test]
fn retained_actor_list_side_effect_is_not_a_completed_registration_receipt() {
    let mut owner = owner();
    let (mut world, _registry, item) = fixture(21, true);
    owner.bind_item(world.item(item).unwrap()).unwrap();
    world
        .item_mut(item)
        .unwrap()
        .attach_component(ComponentId::new(4).unwrap(), 1)
        .unwrap();
    assert!(
        world
            .register_if_absent(item, 0x21, &mut NoComponents)
            .is_err()
    );
    assert_eq!(world.actors().unwrap(), [item]);
    assert_eq!(
        world.item(item).unwrap().registration(),
        Registration::Failed
    );
    let before = state(&owner);
    assert!(
        owner
            .initialize_virtual_motion(world.item(item).unwrap())
            .is_err()
    );
    assert_eq!(state(&owner), before);
    assert_eq!(owner.byte(0x4e4), Some(13));
}

#[test]
fn canonical_version_two_records_typed_world_and_item_without_native_pointer_words() {
    let mut first = owner();
    let mut second = owner();
    let (a, _registry_a, item_a) = fixture(21, true);
    let (b, _registry_b, item_b) = fixture(22, true);
    first.bind_item(a.item(item_a).unwrap()).unwrap();
    second.bind_item(b.item(item_b).unwrap()).unwrap();
    let first_words = state(&first);
    assert_eq!(first_words[0], 2);
    assert_eq!(&first_words[first_words.len() - 5..], &[1, 0, 21, 0, 1]);
    assert_ne!(first_words, state(&second));
    assert_eq!(first.word(4), None);
    assert_eq!(first.word(0x224), None);
}
