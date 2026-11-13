use super::*;
use crate::animation::skeleton::{self, BoneInput, Skeleton};
use crate::mission::player::constructor::pose;

fn fixture() -> (
    Pending,
    base_handler::Owner,
    PendingReset,
    Preparation,
    Item,
) {
    let (mut pose, mut base, mut reset, mut preparation, mut item, mut source) =
        pose::tests::fixture();
    pose.advance(
        &mut base,
        &mut reset,
        &mut preparation,
        &mut item,
        &mut source,
    )
    .unwrap();
    let mut component = component_tail::Pending::begin(&pose).unwrap();
    component
        .advance(&base, &reset, &mut item, &mut source)
        .unwrap();
    (
        Pending::begin(&component).unwrap(),
        base,
        reset,
        preparation,
        item,
    )
}

fn existing_cache(preparation: &Preparation, item: &Item, player: PlayerId) -> Owner {
    let skeleton = Skeleton::initialize(
        &[
            BoneInput {
                parent: u16::MAX,
                range_end: 0,
                auxiliary_raw: [0; 2],
            },
            BoneInput {
                parent: 0,
                range_end: 0,
                auxiliary_raw: [0; 2],
            },
        ],
        skeleton::Budget {
            bones: 2,
            parent_visits: 2,
        },
    )
    .unwrap();
    let mut caches = Owner::from_boot();
    let input = |file| {
        Some(Input {
            receiver: preparation
                .item_animator_by_file(item, player, file)
                .unwrap()
                .unwrap(),
            skeleton: Some(&skeleton),
        })
    };
    caches
        .initialize_if_unselected(
            input(0x0102_0016),
            input(0x0102_0017),
            transform_cache::Budget::default(),
        )
        .unwrap();
    caches
}

#[test]
fn actual_missing_skeleton_retains_initializer_stores_before_cache_dependency() {
    let (mut pending, mut base, reset, preparation, mut item) = fixture();
    let mut caches = Owner::from_boot();
    let scale = item.scale_bits();
    let flags_c = item.flags();
    assert!(
        pending
            .advance(
                Bindings {
                    base: &mut base,
                    reset: &reset,
                    preparation: &preparation,
                    item: &mut item
                },
                &mut caches,
                &Shared::from_boot(),
                transform_cache::Budget::default()
            )
            .is_err()
    );
    assert_eq!(pending.stage(), Stage::Caches);
    assert_eq!(pending.flags(), Some((0, 0xc)));
    assert_eq!(item.transform_flags_90(), 0xc);
    assert_eq!(item.rotation_mode_93(), Some(3));
    assert!(pending.dirty_cleared() && pending.scalars_written());
    for (at, value) in [
        (0x758, 0),
        (0x75c, 0),
        (0x760, 0x4300_0000),
        (0x768, f32::MAX.to_bits()),
        (0x6b8, 0),
        (0x6dc, 0),
    ] {
        assert_eq!(base.word(at), Some(value));
    }
    assert_eq!(item.scale_bits(), scale);
    assert_eq!(item.flags(), flags_c);
    let trace = pending.cache_trace().unwrap();
    assert!(!trace.returned && !trace.failed);
    assert!(trace.builds[0].as_ref().unwrap().waiting_skeleton);
    assert_eq!(caches.native_bytes(), 0);
    assert_eq!(pending.cache_enabled(), None);
}

#[test]
fn retained_selected_cache_skips_unavailable_skeleton_and_waits_for_real_preload() {
    let (mut pending, mut base, reset, preparation, mut item) = fixture();
    let mut caches = existing_cache(&preparation, &item, pending.player());
    let before = caches.snapshot();
    let shared = Shared::from_boot();
    pending
        .advance(
            Bindings {
                base: &mut base,
                reset: &reset,
                preparation: &preparation,
                item: &mut item,
            },
            &mut caches,
            &shared,
            transform_cache::Budget::default(),
        )
        .unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingPreload);
    assert_eq!(pending.dependency(), "player_animation_preload");
    assert!(pending.cache_trace().unwrap().reused_selected);
    assert_eq!(
        pending.cache_snapshot().unwrap().matrix_backings,
        before.matrix_backings
    );
    assert_eq!(pending.cache_enabled(), Some(1));
    assert_eq!(shared.cache_enabled(), 1);
    assert!(!pending.invalidated());
    let invocations = caches.invocation_count();
    pending
        .advance(
            Bindings {
                base: &mut base,
                reset: &reset,
                preparation: &preparation,
                item: &mut item,
            },
            &mut caches,
            &shared,
            transform_cache::Budget::default(),
        )
        .unwrap();
    assert_eq!(caches.invocation_count(), invocations);
}

#[test]
fn zero_preload_flag_reaches_actual_validity_clear_before_later_initializer_tail() {
    let (mut pending, mut base, reset, preparation, mut item) = fixture();
    let mut caches = existing_cache(&preparation, &item, pending.player());
    let mut shared = Shared::from_boot();
    shared.cache_enabled = 0;
    pending
        .advance(
            Bindings {
                base: &mut base,
                reset: &reset,
                preparation: &preparation,
                item: &mut item,
            },
            &mut caches,
            &shared,
            transform_cache::Budget::default(),
        )
        .unwrap();
    assert_eq!(pending.stage(), Stage::AwaitingTail);
    assert!(pending.invalidated());
    assert_eq!(
        caches
            .header(transform_cache::Slot::FirstPerson)
            .invalidation_reads(),
        (1, 1)
    );
    assert_eq!(shared.cache_enabled(), 0);
}

#[test]
fn genuinely_missing_receiver_skips_cache_build_and_flag_read() {
    let (mut pending, mut base, reset, preparation, mut item) = fixture();
    let mut caches = existing_cache(&preparation, &item, pending.player());
    let before = caches.snapshot();
    let fp = preparation
        .first_person()
        .unwrap()
        .animator()
        .identity()
        .id();
    item.components.retain(|id| *id != fp);
    pending
        .advance(
            Bindings {
                base: &mut base,
                reset: &reset,
                preparation: &preparation,
                item: &mut item,
            },
            &mut caches,
            &Shared::from_boot(),
            transform_cache::Budget::default(),
        )
        .unwrap();
    assert_eq!(pending.searches(), 2);
    assert_eq!(pending.receivers()[0], None);
    assert!(pending.receivers()[1].is_some());
    assert_eq!(pending.cache_trace(), None);
    assert_eq!(pending.cache_enabled(), None);
    assert_eq!(caches.snapshot(), before);
    assert_eq!(
        caches
            .header(transform_cache::Slot::FirstPerson)
            .invalidation_reads(),
        (0, 0)
    );
    assert_eq!(pending.stage(), Stage::AwaitingTail);
    assert!(!pending.invalidated());
}
