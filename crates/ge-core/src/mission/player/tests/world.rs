use super::super::identity::{ComponentId, PlayerId};
use super::*;
struct Callbacks {
    ids: Vec<ComponentId>,
    fail: Option<usize>,
}
impl Components for Callbacks {
    fn activate(&mut self, _: ItemId, id: ComponentId, enabled: bool) -> Result<()> {
        assert!(enabled);
        self.ids.push(id);
        ensure!(
            self.fail != Some(self.ids.len()),
            "authored component activation failure"
        );
        Ok(())
    }
}
fn world() -> World {
    World::from_constructor(
        WorldId::new(1).unwrap(),
        1,
        10,
        Budget {
            items: 8,
            memberships: 10,
        },
    )
    .unwrap()
}
fn callbacks() -> Callbacks {
    Callbacks {
        ids: Vec::new(),
        fail: None,
    }
}

#[test]
fn constructor_owns_ten_separate_nodes_without_claiming_a_registered_actor() {
    let mut world = world();
    let id = world.construct_item().unwrap();
    let item = world.item(id).unwrap();
    assert_eq!(item.nodes().len(), 10);
    assert_eq!(item.membership_mask(), 0);
    assert!(item.nodes().iter().all(|n| *n == Node::from_constructor()));
    assert_eq!(item.flags(), 0xb);
    assert_eq!(item.position_bits(), [0; 4]);
    assert_eq!(item.scale_bits(), [0x3f80_0000; 4]);
    assert_eq!(item.zone(), u16::MAX);
    assert!(item.handler().is_none() && item.physics().is_none());
    assert!(world.actors().unwrap().is_empty());
    world.item_mut(id).unwrap().prepare_player_owner();
    assert_eq!(world.item(id).unwrap().flags(), 0x2b);
    let player = PlayerId::new(31).unwrap();
    assert_eq!(
        world.item_mut(id).unwrap().bind_handler(Some(player)),
        Some((player, id))
    );
}

#[test]
fn actor_tail_order_and_membership_head_order_differ() {
    let mut world = world();
    let a = world.construct_item().unwrap();
    let b = world.construct_item().unwrap();
    let mut callback = callbacks();
    world.register_if_absent(a, 0x21, &mut callback).unwrap();
    world.register_if_absent(b, 0x21, &mut callback).unwrap();
    assert_eq!(world.actors().unwrap(), vec![a, b]);
    assert_eq!(world.members(0).unwrap(), vec![b, a]);
    assert_eq!(world.members(5).unwrap(), vec![b, a]);
    assert!(world.members(4).unwrap().is_empty());
    assert_eq!(world.item(a).unwrap().priority(), 0xff);
    assert!(callback.ids.is_empty());
}

#[test]
fn xor_updates_preserve_unaffected_lists_and_existing_payload_after_removal() {
    let mut world = world();
    let a = world.construct_item().unwrap();
    let b = world.construct_item().unwrap();
    let mut callback = callbacks();
    world.register_if_absent(a, 0x21, &mut callback).unwrap();
    world.register_if_absent(b, 0x21, &mut callback).unwrap();
    let before = world.item(b).unwrap().nodes()[5];
    assert_eq!(
        world.update_membership(a, 0x31).unwrap(),
        Update {
            inserted: 1,
            removed: 0
        }
    );
    assert_eq!(world.members(4).unwrap(), vec![a]);
    assert_eq!(world.item(b).unwrap().nodes()[5], before);
    assert_eq!(
        world.update_membership(a, 0x10).unwrap(),
        Update {
            inserted: 0,
            removed: 2
        }
    );
    assert_eq!(world.members(0).unwrap(), vec![b]);
    assert_eq!(world.members(5).unwrap(), vec![b]);
    assert_eq!(world.item(a).unwrap().nodes()[5].payload, Payload::Item(a));
    assert_eq!(world.update_membership(a, 0x10).unwrap(), Update::default());
}

#[test]
fn removal_of_head_middle_and_tail_repairs_actual_neighbor_links() {
    let mut world = world();
    let mut ids = Vec::new();
    let mut callback = callbacks();
    for _ in 0..3 {
        let id = world.construct_item().unwrap();
        world.register_if_absent(id, 1, &mut callback).unwrap();
        ids.push(id);
    }
    world.update_membership(ids[1], 0).unwrap();
    assert_eq!(world.members(0).unwrap(), vec![ids[2], ids[0]]);
    world.update_membership(ids[2], 0).unwrap();
    assert_eq!(world.members(0).unwrap(), vec![ids[0]]);
    world.update_membership(ids[0], 0).unwrap();
    assert!(world.members(0).unwrap().is_empty());
    assert_eq!(world.membership_header(0).unwrap(), Header::default());
    assert_eq!(world.actors().unwrap(), ids);
}

#[test]
fn callback_failure_retains_tail_registration_without_fake_membership_or_retry() {
    let mut world = world();
    let id = world.construct_item().unwrap();
    let first = ComponentId::new(9).unwrap();
    let second = ComponentId::new(2).unwrap();
    world
        .item_mut(id)
        .unwrap()
        .attach_component(first, 8)
        .unwrap();
    world
        .item_mut(id)
        .unwrap()
        .attach_component(second, 8)
        .unwrap();
    let mut callback = Callbacks {
        ids: Vec::new(),
        fail: Some(2),
    };
    assert!(world.register_if_absent(id, 0x21, &mut callback).is_err());
    assert_eq!(callback.ids, vec![first, second]);
    assert_eq!(world.actors().unwrap(), vec![id]);
    assert_eq!(world.item(id).unwrap().registration(), Registration::Failed);
    assert_eq!(world.item(id).unwrap().priority(), 0);
    assert_eq!(world.item(id).unwrap().membership_mask(), 0);
    assert!(world.register_if_absent(id, 0x21, &mut callback).is_err());
    assert_eq!(callback.ids.len(), 2);
}

#[test]
fn repeated_actor_registration_does_not_reactivate_components_or_replace_mask() {
    let mut world = world();
    let id = world.construct_item().unwrap();
    let mut callback = callbacks();
    world
        .item_mut(id)
        .unwrap()
        .attach_component(ComponentId::new(3).unwrap(), 8)
        .unwrap();
    assert!(world.register_if_absent(id, 0x21, &mut callback).unwrap());
    assert!(!world.register_if_absent(id, 0x31, &mut callback).unwrap());
    assert_eq!(callback.ids.len(), 1);
    assert_eq!(world.item(id).unwrap().membership_mask(), 0x21);
}

#[test]
fn foreign_world_handles_and_unsupported_bits_do_not_mutate_lists() {
    let mut world = world();
    let id = world.construct_item().unwrap();
    let mut callback = callbacks();
    let mut other = World::from_constructor(
        WorldId::new(2).unwrap(),
        1,
        10,
        Budget {
            items: 8,
            memberships: 10,
        },
    )
    .unwrap();
    let foreign = other.construct_item().unwrap();
    assert!(world.register_if_absent(foreign, 1, &mut callback).is_err());
    assert!(
        world
            .register_if_absent(id, 1 << 10, &mut callback)
            .is_err()
    );
    assert!(world.actors().unwrap().is_empty());
    assert_eq!(world.item(id).unwrap().membership_mask(), 0);
    assert!(
        World::from_constructor(
            WorldId::new(1).unwrap(),
            1,
            4,
            Budget {
                items: 8,
                memberships: 10
            }
        )
        .is_err()
    );
}

#[test]
fn item_budget_exhaustion_keeps_existing_owner_and_next_identity_intact() {
    let mut world = World::from_constructor(
        WorldId::new(1).unwrap(),
        1,
        10,
        Budget {
            items: 1,
            memberships: 10,
        },
    )
    .unwrap();
    let first = world.construct_item().unwrap();
    assert!(world.construct_item().is_err());
    assert_eq!(world.item_count(), 1);
    assert_eq!(world.item(first).unwrap().id(), first);
}
