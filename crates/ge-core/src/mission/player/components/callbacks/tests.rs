use super::*;
use crate::mission::player::{
    base_handler::{self, Owner},
    components::{Animator, Registry, ResourceBinding, SkeletonBudget, SkeletonInput},
    identity::{ComponentId, ItemId, PlayerId, WorldId},
    world::{self, World},
};
use std::sync::Arc;

struct NoActivation;
impl world::Components for NoActivation {
    fn activate(&mut self, _: ItemId, _: ComponentId, _: bool) -> anyhow::Result<()> {
        anyhow::bail!("fresh registration has no components")
    }
}
fn fixture(bound_receiver: bool) -> (Arc<Table>, Animator, World, Owner) {
    let player = PlayerId::new(7).unwrap();
    let mut world = World::from_constructor(
        WorldId::new(3).unwrap(),
        1,
        5,
        world::Budget {
            items: 2,
            memberships: 5,
        },
    )
    .unwrap();
    let item = world.construct_item().unwrap();
    world.item_mut(item).unwrap().bind_handler(Some(player));
    world
        .register_if_absent(item, 0, &mut NoActivation)
        .unwrap();
    let owner = base_handler::callback_fixture::owner(player, Some(item));
    if bound_receiver {
        world
            .item_mut(item)
            .unwrap()
            .bind_animation_interface(owner.animation_interface().unwrap())
            .unwrap();
    }
    let identity = Registry::from_constructor(1).allocate(player).unwrap();
    let mut animator = Animator::from_constructor(identity);
    animator
        .bind_skeleton(
            SkeletonInput {
                primary: ResourceBinding {
                    file: 0x010a_0056,
                    reference: 0x0d00_0001,
                },
                bones: &[],
                auxiliary_words: 0,
                captured_frame_counter: None,
            },
            ResourceBinding {
                file: 0x0102_0017,
                reference: 0,
            },
            SkeletonBudget::default(),
        )
        .unwrap();
    animator.prepare_first_item_append(item).unwrap();
    let table = Arc::new(Table::from_constructor(1).unwrap());
    animator.bind_callback_table(Arc::clone(&table)).unwrap();
    (table, animator, world, owner)
}
fn event(kind: u8) -> Event {
    Event {
        kind,
        file: u32::MAX,
        parameter: 0x1234,
        control_channel: Some(0xfe),
    }
}

#[test]
fn constructor_has_four_real_slots_and_noops_do_not_masquerade_as_event_handlers() {
    let table = Table::from_constructor(1).unwrap();
    assert_eq!(
        table.handlers(),
        &[
            Handler::Event,
            Handler::Noop,
            Handler::Noop,
            Handler::Acknowledge
        ]
    );
    table.noop(1).unwrap();
    table.noop(2).unwrap();
    assert!(table.noop(0).is_err());
    assert!(table.noop(3).is_err());
    assert!(Table::from_constructor(0).is_err());
}
#[test]
fn event_routes_actual_receiver_and_source_file_without_reading_unreached_shared_byte() {
    let (table, animator, world, owner) = fixture(true);
    for kind in [3, 4] {
        let result = table
            .event(&animator, &world, Some(&owner), event(kind), || {
                panic!("literal interface leaf reads no global")
            })
            .unwrap();
        assert_eq!(result.value, 0);
        let call = result.call.unwrap();
        assert_eq!(call.file, 0x0102_0017);
        assert_eq!(call.parameter, 0x1234);
        assert_eq!(call.control_channel, 0xfe);
        assert_eq!(call.receiver.player(), owner.player());
        assert_eq!(call.kind, kind);
    }
    let mut explicit = event(3);
    explicit.file = 0x0100_0042;
    assert_eq!(
        table
            .event(&animator, &world, Some(&owner), explicit, || panic!())
            .unwrap()
            .call
            .unwrap()
            .file,
        0x0100_0042
    );
    let mut unknown = event(9);
    unknown.control_channel = None;
    assert_eq!(
        table
            .event(&animator, &world, Some(&owner), unknown, || panic!())
            .unwrap()
            .call,
        None
    );
}
#[test]
fn kind_five_sign_extends_actual_byte_and_unknown_control_or_global_is_an_error() {
    let (table, animator, world, owner) = fixture(true);
    for (byte, value) in [(0, 0), (127, 127), (128, -128), (255, -1)] {
        assert_eq!(
            table
                .event(&animator, &world, Some(&owner), event(5), || Ok(byte))
                .unwrap()
                .value,
            value
        );
    }
    assert!(
        table
            .event(&animator, &world, Some(&owner), event(5), || anyhow::bail!(
                "unknown actual shared byte"
            ))
            .is_err()
    );
    let mut missing = event(3);
    missing.control_channel = None;
    assert!(
        table
            .event(&animator, &world, Some(&owner), missing, || panic!())
            .is_err()
    );
}
#[test]
fn native_null_receiver_is_distinct_from_missing_provider_and_wrong_table_owner() {
    let (table, animator, world, owner) = fixture(false);
    assert_eq!(
        table
            .event(&animator, &world, Some(&owner), event(5), || panic!())
            .unwrap()
            .call,
        None
    );
    assert!(
        table
            .event(&animator, &world, None, event(5), || panic!())
            .is_err()
    );
    let wrong = Table::from_constructor(1).unwrap();
    assert!(
        wrong
            .event(&animator, &world, Some(&owner), event(5), || panic!())
            .is_err()
    );
}

#[test]
fn unknown_event_still_requires_fallback_file_before_kind_switch() {
    let (table, animator, world, owner) = fixture(true);
    let mut partial = Animator::from_constructor(animator.identity());
    partial
        .prepare_first_item_append(animator.item().unwrap())
        .unwrap();
    partial.bind_callback_table(Arc::clone(&table)).unwrap();
    assert!(
        table
            .event(&partial, &world, Some(&owner), event(9), || panic!())
            .is_err()
    );
    let mut explicit = event(9);
    explicit.file = 0x0100_0001;
    assert_eq!(
        table
            .event(&partial, &world, Some(&owner), explicit, || panic!())
            .unwrap()
            .value,
        0
    );
}
#[test]
fn acknowledgement_preserves_or_invalidates_token_after_actual_leaf_return() {
    let (table, animator, world, owner) = fixture(true);
    let mut token = 37;
    assert!(
        !table
            .acknowledge(&animator, &world, Some(&owner), &mut token, || Ok(0))
            .unwrap()
    );
    assert_eq!(token, 37);
    assert!(
        table
            .acknowledge(
                &animator,
                &world,
                Some(&owner),
                &mut token,
                || anyhow::bail!("unknown global")
            )
            .is_err()
    );
    assert_eq!(token, 37);
    assert!(
        table
            .acknowledge(&animator, &world, Some(&owner), &mut token, || Ok(128))
            .unwrap()
    );
    assert_eq!(token, u32::MAX);
}
