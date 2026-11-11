use super::*;
use ge_core::mission::player::components::{self, Preparation};

#[derive(Default)]
struct EntryProvider {
    fail: bool,
    calls: Vec<&'static str>,
}
impl components::Provider for EntryProvider {
    fn session_manager_present(&mut self) -> Result<bool> {
        self.calls.push("manager");
        ensure!(!self.fail, "authored missing provider");
        Ok(true)
    }
    fn mark_interface_dirty(&mut self, _: u32) -> Result<()> {
        self.calls.push("dirty");
        anyhow::bail!("constructor NULL must skip notifications")
    }
}

#[test]
fn actual_basic_return_enters_tp_with_same_inventory_and_preserves_deferred_physics() {
    let (mut base, mut prelude, mut reset) = player_state(44);
    let mut world = world();
    let mut registry = Registry::from_constructor();
    let mut init = Owned::new();
    init.initialize(
        &mut base,
        &mut world,
        &mut registry,
        &mut Environment::default(),
    )
    .unwrap();
    let receipt = init.completion().unwrap();
    let item = world.item(receipt.item()).unwrap();
    let inventory_before = prelude.inventory().records().to_vec();
    let selection_before = prelude.inventory().selection();
    let mut preparation = Preparation::new();
    let mut provider = EntryProvider::default();
    preparation
        .prepare(
            components::Bindings {
                base: &base,
                basic: receipt,
                reset: &mut reset,
                prelude: &mut prelude,
                item,
            },
            &mut provider,
        )
        .unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingVisual);
    assert_eq!(provider.calls, ["manager"]);
    assert_eq!(prelude.inventory().records(), inventory_before);
    assert_eq!(prelude.inventory().selection(), selection_before);
    assert_eq!(prelude.inventory().slots().c55, Some(0xff));
    assert_eq!(reset.byte(0xc97), prelude.inventory().slots().c96);
    for at in [0xafc, 0xb00, 0xaf8, 0x70c, 0x710, 0x714, 0x718] {
        assert_eq!(reset.word(at), Some(0));
    }
    assert!((0xfec..0x1150).all(|at| reset.byte(at) == Some(0)));
    assert_eq!(preparation.animator().unwrap().player(), base.player());
    assert_eq!(preparation.animator().unwrap().word(0x114), Some(0));
    assert!(item.components().is_empty()); // allocation has not attached a component
    assert!(init.physics().unwrap().handler_scalar_bits().is_err());
    assert!(
        preparation
            .prepare(
                components::Bindings {
                    base: &base,
                    basic: receipt,
                    reset: &mut reset,
                    prelude: &mut prelude,
                    item
                },
                &mut provider
            )
            .is_err()
    );
    assert_eq!(provider.calls, ["manager"]);
    assert!(
        preparation
            .skin_requested(0x0100_0001, 0x0d00_0001)
            .is_err()
    );
    let character = reset.character().unwrap();
    let mut words = [0; 17];
    words[0] = character.key;
    words[1] = u32::MAX;
    words[6] = 0x0100_0029;
    words[7] = 0x0d00_0051;
    words[10] = u32::MAX;
    let definitions = ge_core::mission::actor_definitions::Registry::build(
        vec![ge_core::mission::actor_definitions::Primary {
            source_index: 9,
            words,
        }],
        ge_core::mission::actor_definitions::Budget {
            rows: 1,
            groups: 0,
            members: 0,
        },
    )
    .unwrap();
    let visual = components::Visual::from_record(character, &definitions.records()[0]).unwrap();
    preparation.bind_visual(visual).unwrap();
    assert!(
        preparation
            .skin_requested(0x0100_0029, 0x0d00_0052)
            .is_err()
    );
    assert_eq!(preparation.stage(), components::Stage::AwaitingResidency);
    preparation
        .skin_requested(visual.file(), visual.skin())
        .unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingTextures);
    assert!(item.components().is_empty());
    let enumeration = |outcome, hashes: &[u32]| components::TextureEnumeration {
        outcome,
        hashes: hashes.to_vec(),
    };
    let enumerated = components::TextureOutcome::Enumerated;
    for (file, skin, textures) in [
        // Another target, repeated hashes and hashes on an absent directory.
        (visual.file(), 0x0d00_0052, enumeration(enumerated, &[1])),
        (
            visual.file(),
            visual.skin(),
            enumeration(enumerated, &[1, 2, 1]),
        ),
        (
            visual.file(),
            visual.skin(),
            enumeration(components::TextureOutcome::TextureDirectoryAbsent, &[1]),
        ),
    ] {
        assert!(
            preparation
                .textures_enumerated(file, skin, textures)
                .is_err()
        );
        assert_eq!(preparation.stage(), components::Stage::AwaitingTextures);
        assert!(preparation.textures().is_none());
    }
    preparation
        .textures_enumerated(
            visual.file(),
            visual.skin(),
            enumeration(enumerated, &[0x8600_0003, 0x0600_0000]),
        )
        .unwrap();
    assert_eq!(
        preparation.stage(),
        components::Stage::AwaitingTextureResidency
    );
    assert!(preparation.finish_texture_requests().is_err());
    let first = preparation.next_texture_request().unwrap().unwrap();
    assert_eq!(
        (first.file, first.target, first.ordinal),
        (visual.file(), 0x8600_0003, 0)
    );
    assert!(
        preparation
            .texture_requested(components::TextureTargetRequest {
                ordinal: 1,
                ..first
            })
            .is_err()
    );
    assert_eq!(preparation.texture_requests(), 0);
    preparation.texture_requested(first).unwrap();
    assert!(preparation.finish_texture_requests().is_err());
    let second = preparation.next_texture_request().unwrap().unwrap();
    assert_eq!(
        (second.file, second.target, second.ordinal),
        (visual.file(), 0x0600_0000, 1)
    );
    assert!(preparation.texture_requested(first).is_err());
    assert_eq!(preparation.texture_requests(), 1);
    preparation.texture_requested(second).unwrap();
    assert!(preparation.next_texture_request().unwrap().is_none());
    preparation.finish_texture_requests().unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingSkeleton);
    assert!(preparation.texture_requested(second).is_err());
    assert_eq!(
        preparation.textures().unwrap().hashes,
        [0x8600_0003, 0x0600_0000]
    );
    // One traversal per TP entry; the attachment boundary is still unbound.
    assert!(
        preparation
            .textures_enumerated(visual.file(), visual.skin(), enumeration(enumerated, &[]))
            .is_err()
    );
    assert!(item.components().is_empty());
    preparation
        .bind_default_skeleton(
            components::SkeletonInput {
                primary: components::ResourceBinding {
                    file: 0x010a_0056,
                    reference: 0x0d00_0001,
                },
                bones: &[ge_core::animation::skeleton::BoneInput {
                    parent: u16::MAX,
                    range_end: 0,
                    auxiliary_raw: [0; 2],
                }],
                auxiliary_words: 4,
                captured_frame_counter: None,
            },
            components::ResourceBinding {
                file: 0x0102_0017,
                reference: 0,
            },
            components::SkeletonBudget::default(),
        )
        .unwrap();
    assert_eq!(
        preparation.stage(),
        components::Stage::AwaitingAnimationRefresh
    );
    let runtime = preparation.animator().unwrap().skeleton().unwrap();
    assert_eq!(runtime.animator(), Some(base.player()));
    assert_eq!(runtime.auxiliary(), &[0; 4]);
    assert!(item.components().is_empty());
}

#[test]
fn cleanup_provider_failure_stops_before_late_fields_and_tp_allocation_without_replay() {
    let (mut base, mut prelude, mut reset) = player_state(45);
    let mut world = world();
    let mut registry = Registry::from_constructor();
    let mut init = Owned::new();
    init.initialize(
        &mut base,
        &mut world,
        &mut registry,
        &mut Environment::default(),
    )
    .unwrap();
    let receipt = init.completion().unwrap();
    let item = world.item(receipt.item()).unwrap();
    let mut preparation = Preparation::new();
    let mut provider = EntryProvider {
        fail: true,
        ..EntryProvider::default()
    };
    assert!(
        preparation
            .prepare(
                components::Bindings {
                    base: &base,
                    basic: receipt,
                    reset: &mut reset,
                    prelude: &mut prelude,
                    item
                },
                &mut provider
            )
            .is_err()
    );
    assert_eq!(preparation.stage(), components::Stage::Failed);
    assert!(preparation.animator().is_none());
    assert_eq!(reset.word(0x70c), None);
    assert_eq!(init.physics().unwrap().byte(0x1ff), Some(1));
    assert!(
        preparation
            .prepare(
                components::Bindings {
                    base: &base,
                    basic: receipt,
                    reset: &mut reset,
                    prelude: &mut prelude,
                    item
                },
                &mut provider
            )
            .is_err()
    );
    assert_eq!(provider.calls, ["manager"]);
}
