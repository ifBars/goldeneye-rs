use super::*;
use crate::{
    animation::scratch,
    mission::player::{
        components::Registry,
        identity::{ComponentId, WorldId},
        world::{self, World},
    },
};

struct NoActivation;
impl world::Components for NoActivation {
    fn activate(&mut self, _: ItemId, _: ComponentId, _: bool) -> Result<()> {
        anyhow::bail!("fresh registration/attachment must not activate a component")
    }
}
fn fixture(registry: &mut Registry) -> (Preparation, World, ItemId) {
    // Authored boundary inputs feed real item registration, skeleton bind and
    // shared refresh. The app integration test covers the preceding PlayerBasic.
    let player = PlayerId::new(7).unwrap();
    let mut world = World::from_constructor(
        WorldId::new(9).unwrap(),
        1,
        5,
        world::Budget {
            items: 4,
            memberships: 5,
        },
    )
    .unwrap();
    let item = world.construct_item().unwrap();
    world.item_mut(item).unwrap().bind_handler(Some(player));
    world
        .register_if_absent(item, 0, &mut NoActivation)
        .unwrap();
    let mut owner = Preparation::new();
    owner.player = Some(player);
    owner.item = Some(item);
    owner.stage = Stage::AwaitingSkeleton;
    owner.animator = Some(Animator::from_constructor(
        registry.allocate(player).unwrap(),
    ));
    owner
        .bind_default_skeleton(
            SkeletonInput {
                primary: super::super::skeleton::DEFAULT_SKIN,
                bones: &[],
                auxiliary_words: 0,
                captured_frame_counter: None,
            },
            super::super::skeleton::TP_COMPONENT,
            SkeletonBudget::default(),
        )
        .unwrap();
    owner
        .finish_bind_zero_time(
            &mut scratch::Context::default(),
            &mut scratch::Arena::from_constructor(1).unwrap(),
        )
        .unwrap();
    (owner, world, item)
}

#[test]
fn successful_bind_attaches_first_node_once_without_activation_or_visual_postbind() {
    let mut registry = Registry::from_constructor(4);
    let (mut owner, mut world, item) = fixture(&mut registry);
    let identity = owner.animator().unwrap().identity();
    owner.attach_tp(world.item_mut(item).unwrap(), 4).unwrap();
    assert_eq!(owner.stage(), Stage::AwaitingVisualPostbind);
    assert_eq!(owner.attachment().unwrap().component(), identity);
    assert_eq!(owner.attachment().unwrap().item(), item);
    assert_eq!(owner.attachment().unwrap().ordinal(), 0);
    assert_eq!(registry.resolve(identity.id()), Some(identity));
    assert_eq!(world.item(item).unwrap().components(), &[identity.id()]);
    let animator = owner.animator().unwrap();
    assert_eq!(animator.item(), Some(item));
    assert_eq!(animator.word(8), None);
    assert_eq!(animator.word(0), Some(0));
    assert_eq!(animator.word(4), Some(0));
    assert_eq!(animator.byte(0x8e), Some(0));
    assert!(owner.attach_tp(world.item_mut(item).unwrap(), 4).is_err());
    assert_eq!(world.item(item).unwrap().components(), &[identity.id()]);
}

#[test]
fn wrong_item_handler_or_bind_identity_is_rejected_before_backlink_writes() {
    for case in 0..3 {
        let mut registry = Registry::from_constructor(4);
        let (mut owner, mut world, item) = fixture(&mut registry);
        let target = if case == 0 {
            let other = world.construct_item().unwrap();
            world
                .item_mut(other)
                .unwrap()
                .bind_handler(Some(PlayerId::new(7).unwrap()));
            world
                .register_if_absent(other, 0, &mut NoActivation)
                .unwrap();
            other
        } else {
            item
        };
        if case == 1 {
            world
                .item_mut(item)
                .unwrap()
                .bind_handler(Some(PlayerId::new(8).unwrap()));
        }
        if case == 2 {
            let (mut other, _, _) = fixture(&mut registry);
            owner.bind_return = other.bind_return.take();
        }
        assert!(owner.attach_tp(world.item_mut(target).unwrap(), 4).is_err());
        assert_eq!(owner.stage(), Stage::AwaitingAttachment);
        assert_eq!(owner.animator().unwrap().item(), None);
        assert_eq!(owner.animator().unwrap().word(8), Some(0));
        assert!(world.item(target).unwrap().components().is_empty());
    }
}

#[test]
fn checked_append_failure_retains_prior_backlink_but_never_claims_membership_or_replays() {
    let mut registry = Registry::from_constructor(4);
    let (mut owner, mut world, item) = fixture(&mut registry);
    assert!(owner.attach_tp(world.item_mut(item).unwrap(), 0).is_err());
    assert_eq!(owner.stage(), Stage::Failed);
    assert_eq!(owner.animator().unwrap().item(), Some(item));
    assert_eq!(owner.animator().unwrap().word(8), None);
    assert_eq!(owner.animator().unwrap().byte(0x8e), Some(0));
    assert!(owner.attachment().is_none());
    assert!(world.item(item).unwrap().components().is_empty());
    assert!(owner.attach_tp(world.item_mut(item).unwrap(), 4).is_err());
    let mut unreached = Preparation::new();
    assert!(
        unreached
            .attach_tp(world.item_mut(item).unwrap(), 4)
            .is_err()
    );
}
