use super::*;
use ge_core::mission::player::components::{self, Preparation};
#[path = "component_preparation/phase_zero.rs"]
mod phase_zero;

struct EntryProvider {
    fail: bool,
    calls: Vec<&'static str>,
    identities: components::Registry,
}
impl Default for EntryProvider {
    fn default() -> Self {
        Self {
            fail: false,
            calls: Vec::new(),
            identities: components::Registry::from_constructor(8),
        }
    }
}
impl components::Provider for EntryProvider {
    fn allocate_animator(
        &mut self,
        player: ge_core::mission::player::identity::PlayerId,
    ) -> Result<components::Identity> {
        self.calls.push("allocate");
        self.identities.allocate(player)
    }
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
    initial_selector_chain(true, false, false);
}
#[test]
fn unavailable_fp_mode_tracks_tp_and_retains_the_actual_primary_head_before_selection() {
    initial_selector_chain(false, false, false);
}
#[test]
fn command_provider_failure_keeps_actual_weight_write_and_unacknowledged_selector() {
    initial_selector_chain(true, true, false);
}
struct ModeSource {
    calls: Vec<&'static str>,
    fail_count: bool,
    fail_body: bool,
}
impl ge_core::animation::mode::runtime::Provider for ModeSource {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>> {
        self.calls.push("resolve");
        match reference {
            0x0900_071e => Ok(Some(1)),
            0 => Ok(Some(0)),
            _ => anyhow::bail!("fallback must not precede pending command effects"),
        }
    }
    fn mode(&mut self, index: usize) -> Result<ge_core::animation::mode::Mode> {
        self.calls.push("mode");
        ensure!(index == 0, "target descriptor must stay unread");
        Ok(ge_core::animation::mode::Mode {
            hash: 0x0900_00bb,
            transition_count: 1,
        })
    }
    fn transition_target(&mut self, index: usize, row: usize) -> Result<u32> {
        self.calls.push("target");
        ensure!((index, row) == (0, 0), "unreached row");
        Ok(1)
    }
    fn command_count(&mut self, index: usize, row: usize) -> Result<usize> {
        self.calls.push("count");
        ensure!((index, row) == (0, 0), "unmatched command list");
        Ok(1)
    }
}
impl ge_core::animation::layers::command::Source for ModeSource {
    fn set_reference(&mut self) -> Result<u32> {
        Ok(0x0a00_00ab)
    }
    fn resolve_set(&mut self, _: u32) -> Result<Option<usize>> {
        Ok(Some(0))
    }
    fn options(&mut self) -> Result<i16> {
        Ok(0)
    }
    fn divisor_override(&mut self) -> Result<u32> {
        anyhow::bail!("disabled divisor override must stay unread")
    }
    fn flags_override(&mut self) -> Result<u16> {
        Ok(0)
    }
    fn begin_set(&mut self, _: usize) -> Result<usize> {
        Ok(1)
    }
    fn group_slot(&mut self) -> Result<u16> {
        Ok(1)
    }
    fn group_weight(&mut self) -> Result<u32> {
        Ok(0x8000_0000)
    }
    fn child_count(&mut self) -> Result<i16> {
        ensure!(!self.fail_count, "authored child count failure");
        Ok(1)
    }
    fn child_divisor(&mut self, _: usize) -> Result<u16> {
        Ok(3)
    }
    fn child_flags(&mut self, _: usize) -> Result<u16> {
        Ok(0x20)
    }
    fn child_reference(&mut self, _: usize) -> Result<u32> {
        Ok(0x0400_00ab)
    }
    fn next_group(&mut self, _: u16) -> Result<()> {
        anyhow::bail!("factory dependency must precede the next group")
    }
}
#[test]
fn script_body_failure_retains_header_cache_and_actual_animator_resource_update() {
    initial_selector_chain(true, false, true);
}
impl ge_core::animation::script_child::Provider for ModeSource {
    fn file(&self) -> u32 {
        0x0102_0017
    }
    fn generation(&mut self) -> Result<u32> {
        self.calls.push("generation");
        Ok(0)
    }
    fn resolve_script(
        &mut self,
        _: u32,
    ) -> Result<Option<ge_core::animation::script_child::Binding>> {
        self.calls.push("script");
        Ok(Some(ge_core::animation::script_child::Binding {
            directory_index: 0,
            hash: 0x0400_00ab,
        }))
    }
    fn bind_body(&mut self, _: ge_core::animation::script_child::Binding) -> Result<bool> {
        self.calls.push("body");
        ensure!(!self.fail_body, "authored body failure");
        Ok(true)
    }
}
fn initial_selector_chain(fp_available: bool, fail_count: bool, fail_body: bool) {
    initial_selector_chain_with_phase(fp_available, fail_count, fail_body, false);
}
#[test]
fn phase_zero_failure_latches_child_command_and_selector_without_reallocating_controls() {
    initial_selector_chain_with_phase(true, false, false, true);
}
fn initial_selector_chain_with_phase(
    fp_available: bool,
    fail_count: bool,
    fail_body: bool,
    fail_phase: bool,
) {
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
    assert_eq!(provider.calls, ["manager", "allocate"]);
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
    assert_eq!(provider.calls, ["manager", "allocate"]);
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
    words[8..13].fill(u32::MAX);
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
    assert_eq!(
        runtime.animator(),
        Some(preparation.animator().unwrap().identity())
    );
    assert_eq!(runtime.auxiliary(), &[0; 4]);
    assert!(item.components().is_empty());
    let mut arena = ge_core::animation::scratch::Arena::from_constructor(1).unwrap();
    let mut context = ge_core::animation::scratch::Context::default();
    preparation
        .finish_bind_zero_time(&mut context, &mut arena)
        .unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingAttachment);
    assert_eq!(preparation.bind_return().unwrap().player(), base.player());
    assert_eq!(arena.cursor(), 0);
    assert!(item.components().is_empty());
    preparation
        .attach_tp(world.item_mut(receipt.item()).unwrap(), 8)
        .unwrap();
    assert_eq!(
        preparation.stage(),
        components::Stage::AwaitingVisualPostbind
    );
    let attachment = preparation.attachment().unwrap();
    assert_eq!(attachment.item(), receipt.item());
    assert_eq!(
        provider.identities.resolve(attachment.component().id()),
        Some(attachment.component())
    );
    assert_eq!(
        world.item(receipt.item()).unwrap().components(),
        &[attachment.component().id()]
    );
    assert_eq!(preparation.animator().unwrap().item(), Some(receipt.item()));
    assert_eq!(preparation.animator().unwrap().word(8), None);
    assert_eq!(preparation.animator().unwrap().byte(0x8e), Some(0));
    preparation
        .begin_visual_postbind(&mut base, world.item(receipt.item()).unwrap())
        .unwrap();
    preparation
        .visual_skin_bound(components::ResourceBinding {
            file: visual.file(),
            reference: visual.callback_skin(),
        })
        .unwrap();
    let repeated = preparation.textures().unwrap().clone();
    assert!(
        preparation
            .finish_visual_postbind(
                &mut base,
                world.item_mut(receipt.item()).unwrap(),
                components::ResourceBinding {
                    file: visual.file(),
                    reference: visual.skin()
                },
                &repeated,
                1
            )
            .is_err()
    );
    assert_eq!(
        preparation.stage(),
        components::Stage::AwaitingPostbindResidency
    );
    assert_eq!(base.word(0x464), Some(0));
    preparation
        .finish_visual_postbind(
            &mut base,
            world.item_mut(receipt.item()).unwrap(),
            components::ResourceBinding {
                file: visual.file(),
                reference: visual.skin(),
            },
            &repeated,
            2,
        )
        .unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingCallbacks);
    assert_eq!(base.word(0x464), None);
    assert_eq!(
        base.visual_skeleton(),
        Some(preparation.animator().unwrap().identity())
    );
    assert_eq!(base.visual_overrides().unwrap().generation, 1);
    assert_eq!(
        world
            .item(receipt.item())
            .unwrap()
            .visual_overrides()
            .unwrap(),
        base.player()
    );
    assert_eq!(world.item(receipt.item()).unwrap().flags(), 0xab);
    assert_eq!(
        preparation
            .animator()
            .unwrap()
            .skeleton()
            .unwrap()
            .secondary(),
        Some(components::ResourceBinding {
            file: visual.file(),
            reference: visual.skin()
        })
    );
    for start in [0x138, 0x158, 0x178, 0x198] {
        assert_eq!(preparation.animator().unwrap().byte(start + 8), Some(0));
        assert_eq!(preparation.animator().unwrap().byte(start + 9), Some(1));
        assert_eq!(preparation.animator().unwrap().byte(start + 10), Some(3));
    }
    assert_eq!(preparation.animator().unwrap().word(0x118), Some(0));
    assert_eq!(preparation.animator().unwrap().byte(0x8c), Some(0x11));
    let table = std::sync::Arc::new(components::callbacks::Table::from_constructor(1).unwrap());
    preparation
        .finish_third_person(
            &base,
            world.item_mut(receipt.item()).unwrap(),
            std::sync::Arc::clone(&table),
        )
        .unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingFirstPerson);
    let returned = preparation.third_person_return().unwrap();
    assert_eq!(
        returned.component(),
        preparation.animator().unwrap().identity()
    );
    assert_eq!(
        world.item(receipt.item()).unwrap().animation_interface(),
        Some(base.animation_interface().unwrap())
    );
    assert!(std::sync::Arc::ptr_eq(
        preparation.animator().unwrap().callback_table().unwrap(),
        &table
    ));
    assert_eq!(preparation.animator().unwrap().word(0x118), None);
    assert_eq!(
        preparation
            .animator()
            .unwrap()
            .skeleton()
            .unwrap()
            .pose_callback(),
        Some(components::PoseCallback::PlayerNewPostprocess)
    );
    assert!(
        preparation
            .finish_third_person(
                &base,
                world.item_mut(receipt.item()).unwrap(),
                std::sync::Arc::clone(&table)
            )
            .is_err()
    );
    let tp = preparation.animator().unwrap().identity();
    let fp_identity = provider.identities.allocate(base.player()).unwrap();
    assert!(
        preparation
            .begin_first_person(tp, character, 0x010a_0078)
            .is_err()
    );
    preparation
        .begin_first_person(fp_identity, character, 0x010a_0078)
        .unwrap();
    assert!(
        preparation
            .configure_first_person_masks(world.item(receipt.item()).unwrap())
            .is_err()
    );
    preparation
        .first_person_mut()
        .unwrap()
        .visual_lookup(visual)
        .unwrap();
    preparation
        .first_person_mut()
        .unwrap()
        .bind_skeleton(
            components::SkeletonInput {
                primary: components::ResourceBinding {
                    file: 0x010a_0078,
                    reference: 0x0d00_016f,
                },
                bones: &[],
                auxiliary_words: 3,
                captured_frame_counter: None,
            },
            components::ResourceBinding {
                file: 0x0102_0016,
                reference: 0,
            },
            components::SkeletonBudget::default(),
        )
        .unwrap();
    preparation
        .first_person_mut()
        .unwrap()
        .finish_bind_zero_time(&mut context, &mut arena)
        .unwrap();
    assert_eq!(
        preparation
            .first_person()
            .unwrap()
            .refresh()
            .unwrap()
            .ordinal(),
        2
    );
    assert_eq!(arena.cursor(), 0);
    assert_eq!(arena.live_reservations(), 0);
    preparation
        .finish_first_person(
            &base,
            world.item_mut(receipt.item()).unwrap(),
            std::sync::Arc::clone(&table),
            8,
        )
        .unwrap();
    assert_eq!(
        preparation.stage(),
        components::Stage::AwaitingFirstPersonMasks
    );
    let fp = preparation.first_person().unwrap().animator();
    assert_eq!(fp.byte(0x131), Some(0xff));
    assert_eq!(fp.byte(0x136), Some(0xff));
    assert_eq!(fp.previous_component(), Some(tp));
    assert_eq!(fp.next_component(), None);
    assert_eq!(fp.word(0), None);
    assert_eq!(fp.word(4), Some(0));
    assert_eq!(fp.item(), Some(receipt.item()));
    assert!(std::sync::Arc::ptr_eq(fp.callback_table().unwrap(), &table));
    assert_eq!(
        fp.skeleton().unwrap().pose_callback(),
        Some(components::PoseCallback::PlayerNewPostprocess)
    );
    assert_eq!(
        preparation.animator().unwrap().next_component(),
        Some(fp_identity)
    );
    assert_eq!(preparation.animator().unwrap().word(4), None);
    assert_eq!(
        world.item(receipt.item()).unwrap().components(),
        &[tp.id(), fp_identity.id()]
    );
    preparation
        .configure_first_person_masks(world.item(receipt.item()).unwrap())
        .unwrap();
    assert_eq!(preparation.stage(), components::Stage::AwaitingInitialModes);
    let fp = preparation.first_person().unwrap().animator();
    assert_eq!(fp.byte(0x131), Some(0xfb));
    assert_eq!(fp.byte(0x136), Some(0xfb));
    assert_eq!(base.visual_skeleton(), Some(tp));
    assert_eq!(base.visual_overrides().unwrap().generation, 1);
    assert!(
        preparation
            .finish_first_person(&base, world.item_mut(receipt.item()).unwrap(), table, 8)
            .is_err()
    );
    assert!(
        preparation
            .configure_first_person_masks(world.item(receipt.item()).unwrap())
            .is_err()
    );
    let mut action = ge_core::mission::player::actions::Pending::begin(&base, &mut reset).unwrap();
    assert_eq!(reset.byte(0xc7c), Some(0xff));
    assert_eq!(action.generic(), 0x0900_071e);
    action
        .bind_components(&preparation, world.item(receipt.item()).unwrap())
        .unwrap();
    action.bind_inventory(&base, prelude.inventory()).unwrap();
    let slot = prelude.inventory().slots().c96.unwrap() as usize;
    assert_eq!(
        action.weapon(),
        Some(prelude.inventory().records()[slot].key)
    );
    assert_eq!(action.variant(), Some(0));
    for expected in [fp_identity, tp, tp] {
        let request = action.next_remap().unwrap().unwrap();
        assert_eq!(request.component, expected);
        action.mapped(request, request.generic, false).unwrap();
    }
    assert_eq!(
        action.stage(),
        ge_core::mission::player::actions::Stage::AwaitingThirdPersonDispatch
    );
    assert_eq!(base.word(0x704), Some(0));
    for at in [0x70c, 0x710, 0x714, 0x718] {
        assert_eq!(reset.word(at), Some(0));
    }
    let mut fallback = ge_core::animation::dispatch::Fallback::default();
    assert!(fallback.value().is_err());
    action
        .capture_third_person(&mut preparation, &mut context, &mut arena, &mut fallback)
        .unwrap();
    assert_eq!(
        action.stage(),
        ge_core::mission::player::actions::Stage::AwaitingThirdPersonSelector
    );
    assert_eq!(fallback.value().unwrap(), 0x0900_06c0);
    assert_eq!(fallback.writes(), 2);
    let tp = preparation.animator().unwrap();
    let layers = tp.layers().unwrap();
    assert_eq!(layers.active_count(), 1);
    assert_eq!(tp.byte(0x130), Some(1));
    assert_eq!(tp.byte(0xc), Some(0));
    assert_eq!(tp.byte(0xd), Some(3));
    assert_eq!(tp.controls().len(), 4);
    assert_eq!(tp.capture_refresh().unwrap().ordinal(), 3);
    assert_eq!(arena.cursor(), 0);
    assert_eq!(arena.live_reservations(), 0);
    for (index, at) in [0x138, 0x158, 0x178, 0x198].into_iter().enumerate() {
        assert_eq!(tp.word(at), None);
        assert_eq!(tp.word(at + 4), None);
        let id = layers.slots()[index].nodes_head()[0];
        let control = tp.controls().get(id).unwrap();
        assert_eq!(control.component().get(), tp.identity().id().get());
        assert_eq!(control.locals().unwrap().len(), 1);
        assert_eq!(control.auxiliary().unwrap(), &[0; 4]);
        assert_eq!(control.transition_bits(), 1f32.to_bits());
    }
    assert!(tp.skeleton().unwrap().frame_stamp_bits().is_err());
    assert_eq!(
        preparation.first_person().unwrap().animator().word(0x138),
        Some(0)
    );
    assert_eq!(
        preparation
            .first_person()
            .unwrap()
            .animator()
            .controls()
            .len(),
        0
    );
    assert!(
        action
            .capture_third_person(&mut preparation, &mut context, &mut arena, &mut fallback)
            .is_err()
    );
    assert_eq!(fallback.writes(), 2);
    let prior_scalars = (base.word(0x1c0), base.word(0x6e4));
    let primary = preparation.animator().unwrap().layers().unwrap().slots()[0].nodes_head()[0];
    action
        .begin_third_person_selector(&mut base, &reset, &preparation, fp_available)
        .unwrap();
    assert_eq!(action.tracks_player_modes(), Some(!fp_available));
    assert_eq!(
        action.primary_head(),
        if fp_available {
            None
        } else {
            Some((Some(primary), 0))
        }
    );
    assert_eq!(
        (base.word(0x1c0), base.word(0x6e4)),
        if fp_available {
            prior_scalars
        } else {
            (Some(0), Some(0))
        }
    );
    let mut source = ModeSource {
        calls: Vec::new(),
        fail_count,
        fail_body,
    };
    let command = action
        .advance_third_person_selector(&mut source)
        .unwrap()
        .unwrap();
    assert_eq!(
        (command.mode, command.transition, command.ordinal),
        (0, 0, 0)
    );
    assert_eq!(
        source.calls,
        ["resolve", "resolve", "mode", "target", "count"]
    );
    assert_eq!(
        action.stage(),
        ge_core::mission::player::actions::Stage::AwaitingThirdPersonCommand
    );
    let cursor = action.selector().unwrap();
    assert_eq!(cursor.inputs().requested, action.resolved()[1].output);
    assert_eq!(cursor.inputs().previous, 0);
    assert_eq!(cursor.inputs().fallback, fallback.value().unwrap());
    assert_eq!(cursor.pending(), Some(command));
    assert_eq!(cursor.completed_commands(), 0);
    assert_eq!(cursor.return_word(), None);
    assert_eq!(preparation.animator().unwrap().controls().len(), 4);
    assert_eq!(
        preparation
            .first_person()
            .unwrap()
            .animator()
            .controls()
            .len(),
        0
    );
    for at in [0x70c, 0x710, 0x714, 0x718] {
        assert_eq!(reset.word(at), Some(0));
    }
    assert_eq!(base.word(0x704), Some(0));
    assert!(
        action
            .begin_third_person_selector(&mut base, &reset, &preparation, fp_available)
            .is_err()
    );
    let result = action.prepare_third_person_command(
        &mut preparation,
        0x0c00_0002,
        &mut source,
        ge_core::animation::layers::SlotOverride {
            enabled_raw: 0,
            slot_raw: 0,
        },
    );
    assert_eq!(result.is_err(), fail_count);
    let tp = preparation.animator().unwrap();
    assert_eq!(tp.word(0x164), Some(0));
    assert_eq!(tp.layers().unwrap().slots()[1].weight_bits(), 0);
    assert_eq!(tp.layers().unwrap().failed(), fail_count);
    assert_eq!(tp.controls().len(), 4);
    assert_eq!(tp.layers().unwrap().active_count(), 1);
    assert_eq!(action.command().unwrap().weight_writes(), 1);
    assert_eq!(
        action.command().unwrap().stage(),
        if fail_count {
            ge_core::animation::layers::command::Stage::Failed
        } else {
            ge_core::animation::layers::command::Stage::ChildFactoryPending
        }
    );
    assert_eq!(action.selector().unwrap().return_word(), None);
    assert_eq!(action.selector().unwrap().completed_commands(), 0);
    for at in [0x70c, 0x710, 0x714, 0x718] {
        assert_eq!(reset.word(at), Some(0));
    }
    assert!(
        action
            .prepare_third_person_command(
                &mut preparation,
                0x0c00_0002,
                &mut source,
                ge_core::animation::layers::SlotOverride {
                    enabled_raw: 0,
                    slot_raw: 0
                }
            )
            .is_err()
    );
    if !fail_count {
        let result = action.initialize_third_person_child_prefix(&mut preparation, &mut source);
        assert_eq!(result.is_err(), fail_body);
        let animator = preparation.animator().unwrap();
        let id = action.command().unwrap().factory_node().unwrap();
        let header = animator.script_children().get(id).unwrap();
        assert_eq!(animator.script_children().len(), 1);
        assert_eq!(animator.controls().len(), 4);
        assert_eq!(
            animator.node_ids().kind(id),
            Some(ge_core::animation::nodes::Kind::Script)
        );
        assert_eq!(animator.layers().unwrap().created_nodes().len(), 5);
        assert!(
            animator
                .layers()
                .unwrap()
                .slots()
                .iter()
                .all(|slot| !slot.nodes_head().contains(&id))
        );
        assert_eq!(animator.layers().unwrap().active_count(), 1);
        assert_eq!(animator.word(0x20), Some(0x0400_00ab));
        assert_eq!(animator.word(0x24), Some(u32::MAX));
        assert_eq!(animator.component().unwrap().reference, 0x0400_00ab);
        assert_eq!(header.word(0x14), Some(0));
        assert!(header.cached().is_some());
        assert_eq!(header.word(0), None);
        assert_eq!(header.word(4), None);
        assert_eq!(
            header.stage(),
            if fail_body {
                ge_core::animation::script_child::Stage::Failed
            } else {
                ge_core::animation::script_child::Stage::AwaitingInitializer
            }
        );
        assert_eq!(action.selector().unwrap().return_word(), None);
        assert_eq!(action.selector().unwrap().completed_commands(), 0);
        let calls = source.calls.clone();
        assert!(
            action
                .initialize_third_person_child_prefix(&mut preparation, &mut source)
                .is_err()
        );
        assert_eq!(source.calls, calls);
        if !fail_body {
            let result = action.initialize_third_person_child_phase_zero(
                &mut preparation,
                &mut source,
                if fail_phase {
                    f32::NAN.to_bits()
                } else {
                    1f32.to_bits()
                },
                100,
            );
            assert_eq!(result.is_err(), fail_phase);
            let animator = preparation.animator().unwrap();
            assert_eq!(animator.script_children().len(), 1);
            assert_eq!(animator.controls().len(), 4);
            assert_eq!(animator.layers().unwrap().active_count(), 1);
            assert_eq!(animator.layers().unwrap().failed(), fail_phase);
            assert_eq!(
                animator.script_children().get(id).unwrap().stage(),
                if fail_phase {
                    ge_core::animation::script_child::Stage::Failed
                } else {
                    ge_core::animation::script_child::Stage::ReturnedWithoutPlay
                }
            );
            assert_eq!(action.selector().unwrap().completed_commands(), 0);
            assert_eq!(action.selector().unwrap().return_word(), None);
            let calls = source.calls.clone();
            assert!(
                action
                    .initialize_third_person_child_phase_zero(
                        &mut preparation,
                        &mut source,
                        1f32.to_bits(),
                        100
                    )
                    .is_err()
            );
            assert_eq!(source.calls, calls);
        }
        for at in [0x70c, 0x710, 0x714, 0x718] {
            assert_eq!(reset.word(at), Some(0));
        }
    }
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
