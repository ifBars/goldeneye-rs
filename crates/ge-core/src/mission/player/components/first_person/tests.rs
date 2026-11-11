use super::*;
use crate::mission::player::{
    components::{Preparation, Registry},
    identity::PlayerId,
};

fn pending() -> FirstPerson {
    let player = PlayerId::new(9).unwrap();
    let identity = Registry::from_constructor(2).allocate(player).unwrap();
    FirstPerson::new(
        identity,
        CharacterBinding {
            registry_resource: 0x0109_0013,
            key: 0x4d00_0123,
            source_index: 4,
        },
        0x010a_0078,
    )
}
fn input() -> SkeletonInput<'static> {
    SkeletonInput {
        primary: ResourceBinding {
            file: 0x010a_0078,
            reference: 0x0d00_016f,
        },
        bones: &[],
        auxiliary_words: 3,
        captured_frame_counter: None,
    }
}

#[test]
fn fp_requires_tp_return_and_actual_visual_lookup_before_binding() {
    let mut fp = pending();
    let identity = fp.animator.identity();
    let mut preparation = Preparation::new();
    assert!(
        preparation
            .begin_first_person(identity, fp.character, fp.skin_file)
            .is_err()
    );
    assert!(
        fp.bind_skeleton(input(), FP_COMPONENT, SkeletonBudget::default())
            .is_err()
    );
    assert!(fp.animator.skeleton().is_none());
    assert!(
        fp.finish_bind_zero_time(
            &mut RefreshContext::default(),
            &mut Arena::from_constructor(1).unwrap()
        )
        .is_err()
    );
    assert_eq!(fp.stage(), FirstPersonStage::AwaitingVisual);
}
#[test]
fn fp_uses_selected_character_file_and_fixed_skin_without_tp_substitution() {
    let mut fp = pending();
    // Authored post-lookup boundary; real app integration exercises the lookup.
    fp.stage = FirstPersonStage::AwaitingSkeleton;
    let mut wrong = input();
    wrong.primary.file = 0x010a_0056;
    assert!(
        fp.bind_skeleton(wrong, FP_COMPONENT, SkeletonBudget::default())
            .is_err()
    );
    assert_eq!(fp.animator.word(0x114), Some(0));
    fp.bind_skeleton(input(), FP_COMPONENT, SkeletonBudget::default())
        .unwrap();
    let runtime = fp.animator.skeleton().unwrap();
    assert_eq!(runtime.primary(), input().primary);
    assert_eq!(runtime.secondary(), Some(input().primary));
    assert_eq!(runtime.auxiliary(), &[0, 0, 0]);
    assert!(runtime.frame_stamp_bits().is_err());
    assert_eq!(runtime.pose_callback(), None);
    let mut context = RefreshContext::default();
    let mut arena = Arena::from_constructor(1).unwrap();
    fp.finish_bind_zero_time(&mut context, &mut arena).unwrap();
    assert_eq!(fp.stage(), FirstPersonStage::AwaitingAttachment);
    assert_eq!(arena.cursor(), 0);
    assert_eq!(arena.live_reservations(), 0);
    assert_eq!(fp.refresh().unwrap().ordinal(), 1);
    assert_eq!(fp.animator.item(), None);
    assert_eq!(fp.animator.byte(0x131), Some(0xff));
    assert!(fp.finish_bind_zero_time(&mut context, &mut arena).is_err());
}
#[test]
fn exhausted_shared_scratch_keeps_fp_skeleton_but_cannot_return_or_replay() {
    let mut fp = pending();
    fp.stage = FirstPersonStage::AwaitingSkeleton;
    fp.bind_skeleton(input(), FP_COMPONENT, SkeletonBudget::default())
        .unwrap();
    let mut context = RefreshContext::default();
    let mut arena = Arena::from_constructor(1).unwrap();
    let mut reservation = arena.reserve(0x2000).unwrap().unwrap();
    assert!(fp.finish_bind_zero_time(&mut context, &mut arena).is_err());
    assert_eq!(fp.stage(), FirstPersonStage::Failed);
    assert!(fp.refresh().is_none());
    assert!(fp.animator.skeleton().is_some());
    assert_eq!(fp.animator.item(), None);
    arena.release(&mut reservation).unwrap();
    assert!(fp.finish_bind_zero_time(&mut context, &mut arena).is_err());
}

#[test]
fn checked_tail_failure_retains_reached_links_without_headers_callbacks_or_return() {
    use crate::mission::player::{
        base_handler,
        components::callbacks::Table,
        identity::{ComponentId, ItemId, WorldId},
        world::{self, World},
    };
    use std::sync::Arc;
    struct NoActivation;
    impl world::Components for NoActivation {
        fn activate(&mut self, _: ItemId, _: ComponentId, _: bool) -> Result<()> {
            anyhow::bail!("registration has no components")
        }
    }
    let player = PlayerId::new(9).unwrap();
    let mut registry = Registry::from_constructor(2);
    let mut tp = Animator::from_constructor(registry.allocate(player).unwrap());
    let mut fp = FirstPerson::new(
        registry.allocate(player).unwrap(),
        pending().character,
        0x010a_0078,
    );
    fp.stage = FirstPersonStage::AwaitingSkeleton;
    fp.bind_skeleton(input(), FP_COMPONENT, SkeletonBudget::default())
        .unwrap();
    fp.finish_bind_zero_time(
        &mut RefreshContext::default(),
        &mut Arena::from_constructor(1).unwrap(),
    )
    .unwrap();
    let mut world = World::from_constructor(
        WorldId::new(2).unwrap(),
        1,
        5,
        world::Budget {
            items: 1,
            memberships: 5,
        },
    )
    .unwrap();
    let item = world.construct_item().unwrap();
    world.item_mut(item).unwrap().bind_handler(Some(player));
    world
        .register_if_absent(item, 0, &mut NoActivation)
        .unwrap();
    let base = base_handler::callback_fixture::owner(player, Some(item));
    tp.prepare_first_item_append(item).unwrap();
    world
        .item_mut(item)
        .unwrap()
        .attach_component(tp.identity().id(), 2)
        .unwrap();
    let table = Arc::new(Table::from_constructor(1).unwrap());
    tp.bind_callback_table(Arc::clone(&table)).unwrap();
    assert!(
        fp.finish_helper(
            &mut tp,
            &base,
            world.item_mut(item).unwrap(),
            Arc::clone(&table),
            1
        )
        .is_err()
    );
    assert_eq!(fp.stage(), FirstPersonStage::Failed);
    assert_eq!(fp.animator.previous_component(), Some(tp.identity()));
    assert_eq!(tp.next_component(), Some(fp.animator.identity()));
    assert_eq!(fp.animator.item(), Some(item));
    assert_eq!(
        world.item(item).unwrap().components(),
        &[tp.identity().id()]
    );
    assert_eq!(fp.animator.byte(0x142), Some(2));
    assert!(fp.animator.callback_table().is_none());
    assert!(
        fp.finish_helper(&mut tp, &base, world.item_mut(item).unwrap(), table, 2)
            .is_err()
    );
}
