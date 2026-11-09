use super::*;
use ge_core::mission::player::{
    constructor::*,
    identity::{PlayerId, WorldId},
    inventory::*,
};

struct Providers;
impl base_handler::Provider for Providers {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        Ok(13)
    }
    fn reset_base_global_byte(&mut self) -> Result<()> {
        Ok(())
    }
    fn reset_resource_state(&mut self, _: u32, _: u32) -> Result<()> {
        Ok(())
    }
}
impl base_handler::RuntimeResetProvider for Providers {
    fn reset_runtime_global_float(&mut self, _: u32) -> Result<()> {
        Ok(())
    }
    fn session_descriptor_present(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn session_descriptor_mode(&mut self) -> Result<u32> {
        Ok(1)
    }
    fn set_runtime_multiplayer_byte(&mut self, _: u8) -> Result<()> {
        Ok(())
    }
}
impl TailProvider for Providers {
    fn default_quad_bits(&mut self) -> Result<[u32; 4]> {
        Ok([0; 4])
    }
    fn current_game_field_328_bits(&mut self) -> Result<u32> {
        Ok(49f32.to_bits())
    }
    fn default_vector_bits(&mut self) -> Result<[u32; 3]> {
        Ok([1f32.to_bits(); 3])
    }
}
impl FlagProvider for Providers {
    fn interface_bound(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn clear_interface_flags(&mut self, _: u32) -> Result<()> {
        anyhow::bail!("unexpected flags callback")
    }
}
impl base_handler::ConstructorFlagProvider for Providers {
    fn mark_constructor_flag_dirty(&mut self, _: u32) -> Result<()> {
        anyhow::bail!("equal flag queried a provider")
    }
}
fn player_state(id: u64) -> (base_handler::Owner, PendingPrelude, PendingReset) {
    let mut p = Providers;
    let mut base = base_handler::Owner::new(PlayerId::new(id).unwrap());
    base.initialize_generic(&mut p).unwrap();
    base.initialize_basic().unwrap();
    base.initialize_player_basic_prefix().unwrap();
    base.apply_reset(49f32.to_bits(), &mut p).unwrap();
    let basic = base.finish_basic().unwrap();
    let mut prelude = PendingPrelude::after_basic(&basic);
    base.apply_reset(49f32.to_bits(), &mut p).unwrap();
    base.finish_reset_prefix().unwrap();
    base.finish_runtime_reset(&mut p).unwrap();
    let mut reset = PendingReset::after_base(&base).unwrap();
    let receipt = Scratch::from_constructor()
        .prepare_reset(ScratchBudget {
            native_bytes: 4736,
            growth_steps: 4,
        })
        .unwrap();
    reset.after_scratch(&receipt).unwrap();
    reset
        .bind_character(CharacterBinding {
            registry_resource: 0x0109_0013,
            key: 0x4d00_0307,
            source_index: 0,
        })
        .unwrap();
    reset.begin_inventory().unwrap();
    let metadata = Metadata {
        inventory_key: 0x0200_0830,
        primary_key: 0x4b00_0042,
        type_key: 0x4b20_0000,
        category_key: 0x4b10_0001,
        byte_8_raw: 0,
        byte_9_raw: 0,
        ammunition_raw: [0; 2],
        source_index: 0,
        native_sorted_index: 1,
        normal_values_bits: [0; 10],
        modifier_bits: [1f32.to_bits(); 4],
    };
    let budget = ge_core::mission::player::inventory::Budget {
        records: 16,
        order_words: 16,
    };
    prelude
        .inventory_mut()
        .admit_default(metadata, budget)
        .unwrap();
    prelude
        .inventory_mut()
        .select_default(
            &[0x0200_0830],
            &[],
            &metadata,
            PoseInputs {
                blend_bits: 0,
                c54_raw: None,
                player_flags_raw: 0,
                flag_5ee_raw: 0,
                flag_95c_raw: 0,
                multiplayer_raw: 0,
            },
            &mut p,
            budget,
        )
        .unwrap();
    reset.record_inventory(prelude.inventory()).unwrap();
    reset.finish_tail(&mut prelude, &mut p).unwrap();
    base.apply_constructor_flag(&reset.completion().unwrap(), 0, 0, &mut p)
        .unwrap();
    (base, prelude, reset)
}
fn base(id: u64) -> base_handler::Owner {
    player_state(id).0
}
fn world() -> World {
    World::from_constructor(
        WorldId::new(81).unwrap(),
        1,
        10,
        world::Budget {
            items: 16,
            memberships: 10,
        },
    )
    .unwrap()
}
fn zone(position: [u32; 4]) -> Result<zone::Context> {
    assert_eq!(position, [0; 4]);
    Ok(zone::Context {
        loaded_ordinal: Some(0),
        zone_leaf: Some(3),
        callback_zone: 0x103,
        packed: 0x0103_0000,
        nodes_visited: 2,
    })
}
#[derive(Default)]
struct Environment {
    fail_zone: bool,
    fail_mask: bool,
    fail_count: bool,
    calls: Vec<&'static str>,
}
impl base_handler::VirtualSessionProvider for Environment {
    fn session_present(&mut self) -> Result<bool> {
        self.calls.push("session");
        Ok(true)
    }
    fn session_user_count(&mut self) -> Result<u32> {
        self.calls.push("count");
        ensure!(!self.fail_count, "actual record owner is unavailable");
        Ok(1)
    }
}
impl super::Environment for Environment {
    fn origin_zone(&mut self, position: [u32; 4]) -> Result<zone::Context> {
        self.calls.push("zone");
        ensure!(!self.fail_zone, "zone provider unavailable");
        zone(position)
    }
    fn registration_mask(&mut self, _: ItemId) -> Result<u32> {
        self.calls.push("mask");
        ensure!(!self.fail_mask, "config field is unavailable");
        Ok(0x2d)
    }
}
#[test]
fn actual_factory_activation_and_basic_return_preserve_deferred_scalar() {
    let mut base = base(7);
    let mut world = world();
    let mut registry = Registry::from_constructor();
    let mut init = Owned::new();
    let mut environment = Environment::default();
    init.initialize(&mut base, &mut world, &mut registry, &mut environment)
        .unwrap();
    assert_eq!(init.stage(), Stage::BasicComplete);
    assert_eq!(environment.calls, ["zone", "mask", "session", "count"]);
    let id = init.item().unwrap();
    let item = world.item(id).unwrap();
    assert_eq!(registry.records()[0].item(), id);
    assert_eq!(item.factory().unwrap().numeric_id(), 1);
    assert_eq!(world.actors().unwrap(), [id]);
    assert_eq!(base.item(), Some(id));
    assert_eq!(item.handler(), Some(base.player()));
    assert_eq!(item.physics().unwrap().get(), base.player().get());
    assert_eq!(
        (
            item.flags(),
            item.membership_mask(),
            item.priority(),
            item.zone()
        ),
        (0x2b, 0x3d, 0xff, 0x103)
    );
    assert_eq!(item.position_bits(), [0; 4]);
    assert_eq!(base.word(0x224), None);
    let physics = init.physics().unwrap();
    assert_eq!(physics.item(), Some(id.get()));
    assert_eq!(physics.item_identity(), Some(id));
    assert_eq!(physics.word(0x160), Some(30f32.to_bits()));
    assert_eq!(physics.word(0x1d0), Some(0.825f32.to_bits()));
    assert_eq!(physics.word(0x1cc), Some(0.5f32.to_bits()));
    assert_eq!(physics.word(0x1c8), Some(1));
    assert_eq!(physics.byte(0x1ff), Some(1));
    assert_eq!(physics.half(8), Some(0x4000));
    assert_eq!(physics.word(0x180), Some(2f32.to_bits()));
    assert_eq!(physics.word(0x168), None);
    assert!(
        physics
            .handler_scalar_bits()
            .unwrap_err()
            .is::<physics::MissingHandlerKind>()
    );
    let pending = physics.deferred_handler_scalar().unwrap();
    assert_eq!(pending.item_identity(), Some(id));
    assert_eq!(pending.half_height_bits(), 0.825f32.to_bits());
    assert_eq!(base.stage(), base_handler::Stage::VirtualBasicComplete);
    assert_eq!(base.byte(0x618), Some(1));
    assert_eq!(init.completion().unwrap().item(), id);
    assert_eq!(init.completion().unwrap().player(), base.player());
    assert!(
        init.initialize(&mut base, &mut world, &mut registry, &mut environment)
            .is_err()
    );
    assert_eq!(environment.calls, ["zone", "mask", "session", "count"]);
    assert_eq!(
        (world.item_count(), registry.count(), registry.counter()),
        (1, 1, 1)
    );
}
#[test]
fn failed_origin_or_registration_provider_retains_reached_real_objects() {
    for fail_zone in [true, false] {
        let mut base = base(9);
        let mut world = world();
        let mut registry = Registry::from_constructor();
        let mut init = Owned::new();
        let mut environment = Environment {
            fail_zone,
            fail_mask: true,
            ..Environment::default()
        };
        assert!(
            init.initialize(&mut base, &mut world, &mut registry, &mut environment)
                .is_err()
        );
        assert_eq!(init.stage(), Stage::Failed);
        let id = init.item().unwrap();
        assert_eq!(base.item(), Some(id));
        assert_eq!(world.item(id).unwrap().handler(), Some(base.player()));
        assert!(world.item(id).unwrap().factory().is_some());
        assert_eq!(registry.records()[0].item(), id);
        assert!(world.actors().unwrap().is_empty());
        assert!(init.physics().is_none());
        assert_eq!(base.stage(), base_handler::Stage::VirtualItemBound);
        assert_eq!(init.origin().is_some(), !fail_zone);
    }
}

#[test]
fn count_failure_retains_actual_activation_but_does_not_publish_basic_return() {
    let mut base = base(10);
    let mut world = world();
    let mut registry = Registry::from_constructor();
    let mut init = Owned::new();
    let mut environment = Environment {
        fail_count: true,
        ..Environment::default()
    };
    assert!(
        init.initialize(&mut base, &mut world, &mut registry, &mut environment)
            .is_err()
    );
    assert_eq!(init.stage(), Stage::Failed);
    assert!(init.completion().is_none());
    assert_eq!(base.stage(), base_handler::Stage::VirtualBasicSuffix);
    assert_eq!(
        world.item(init.item().unwrap()).unwrap().membership_mask(),
        0x3d
    );
    assert_eq!(init.physics().unwrap().byte(0x1ff), Some(1));
    assert!(init.physics().unwrap().scalar_pending());
    assert_eq!(base.word(0x574), Some(0));
    let before = environment.calls.clone();
    assert!(
        init.initialize(&mut base, &mut world, &mut registry, &mut environment)
            .is_err()
    );
    assert_eq!(environment.calls, before);
}

#[path = "component_preparation.rs"]
mod component_preparation;
