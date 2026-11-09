use super::*;
use crate::mission::player::{base_handler, identity::PlayerId};
use std::collections::BTreeMap;

struct BaseProvider;
impl base_handler::Provider for BaseProvider {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        Ok(3)
    }
    fn reset_base_global_byte(&mut self) -> Result<()> {
        Ok(())
    }
    fn reset_resource_state(&mut self, _: u32, _: u32) -> Result<()> {
        Ok(())
    }
}
impl base_handler::RuntimeResetProvider for BaseProvider {
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

fn prelude(id: u64) -> PendingPrelude {
    let mut base = base_handler::Owner::new(PlayerId::new(id).unwrap());
    let mut provider = BaseProvider;
    base.initialize_generic(&mut provider).unwrap();
    base.initialize_basic().unwrap();
    base.initialize_player_basic_prefix().unwrap();
    base.apply_reset(17.0_f32.to_bits(), &mut provider).unwrap();
    PendingPrelude::after_basic(&base.finish_basic().unwrap())
}

// An authored tail-entry fixture isolates the continuation from independent
// inventory/scratch tests. Unwritten bytes deliberately remain unknown.
fn entry(id: u64) -> PendingReset {
    let mut reset = PendingReset {
        player: PlayerId::new(id).unwrap(),
        bytes: BTreeMap::new(),
        stage: Stage::InventorySelected,
        character: None,
    };
    reset.bytes.insert(0xc54, 7);
    reset.word_store(0x71c, 11.0_f32.to_bits());
    reset.word_store(0x720, 13.0_f32.to_bits());
    reset.word_store(0xc2c, 19);
    reset
}

#[derive(Default)]
struct Provider {
    calls: Vec<&'static str>,
    quads: u32,
    fail: Option<usize>,
}
impl Provider {
    fn call(&mut self, name: &'static str) -> Result<()> {
        self.calls.push(name);
        ensure!(
            self.fail != Some(self.calls.len()),
            "authored provider failure"
        );
        Ok(())
    }
}
impl TailProvider for Provider {
    fn default_quad_bits(&mut self) -> Result<[u32; 4]> {
        self.call("quad")?;
        self.quads += 1;
        Ok(std::array::from_fn(|index| {
            0x3e00_0000 + self.quads * 16 + index as u32
        }))
    }
    fn current_game_field_328_bits(&mut self) -> Result<u32> {
        self.call("game")?;
        Ok(37.0_f32.to_bits())
    }
    fn default_vector_bits(&mut self) -> Result<[u32; 3]> {
        self.call("vector")?;
        Ok([0x8000_0000, 0x3e12_3456, 0xbf23_4567])
    }
}

#[test]
fn tail_uses_each_reached_provider_read_and_retains_distinct_inputs() {
    let mut reset = entry(5);
    let mut prelude = prelude(5);
    let mut provider = Provider::default();
    assert_eq!(reset.byte(0xc54), Some(7));
    assert!(reset.completion().is_err());
    reset.finish_tail(&mut prelude, &mut provider).unwrap();
    assert_eq!(
        provider.calls,
        ["quad", "quad", "game", "vector", "quad", "quad"]
    );
    for (ordinal, offset) in [0xb88, 0xb98, 0xedc, 0xc30].into_iter().enumerate() {
        for lane in 0..4 {
            assert_eq!(
                reset.word(offset + lane * 4),
                Some(0x3e00_0000 + (ordinal as u32 + 1) * 16 + u32::from(lane))
            );
        }
    }
    assert_eq!(reset.word(0x71c), Some(37.0_f32.to_bits()));
    assert_eq!(reset.word(0x720), Some(13.0_f32.to_bits()));
    assert_eq!(reset.word(0xeb0), Some(0x8000_0000));
    assert_eq!(reset.word(0xeb4), Some(0x3e12_3456));
    assert_eq!(reset.word(0xeb8), Some(0xbf23_4567));
    assert_eq!(reset.byte(0xc54), Some(0));
    assert_eq!(reset.completion().unwrap().player(), prelude.player());
    assert_eq!(reset.stage(), Stage::Complete);
}

#[test]
fn tail_sets_supported_vectors_scalars_and_leaves_sparse_holes_unknown() {
    let mut reset = entry(6);
    reset
        .finish_tail(&mut prelude(6), &mut Provider::default())
        .unwrap();
    for offset in [0xc08, 0xc0c, 0xc10, 0xc14, 0xec0, 0xec4, 0xed0] {
        assert_eq!(reset.word(offset), Some(1.0_f32.to_bits()));
    }
    for offset in [0xde4, 0xe04] {
        assert_eq!(
            [
                reset.word(offset),
                reset.word(offset + 4),
                reset.word(offset + 8),
                reset.word(offset + 12)
            ],
            [Some(0), Some(0), Some(0), Some(1.0_f32.to_bits())]
        );
    }
    assert_eq!(reset.word(0xf5c), Some(0));
    assert_eq!(reset.word(0xf60), Some(0.825_f32.to_bits()));
    assert_eq!(reset.word(0xf64), Some(0));
    assert_eq!(reset.word(0xf68), Some(0));
    assert_eq!(reset.word(0xf6c), Some(0.5_f32.to_bits()));
    assert_eq!(reset.word(0xf70), Some(0.825_f32.to_bits()));
    assert_eq!(reset.word(0x3a8), Some(100.0_f32.to_bits()));
    assert_eq!(reset.word(0x3ac), Some(100.0_f32.to_bits()));
    assert_eq!(reset.word(0xe5c), Some(2.75_f32.to_bits()));
    assert_eq!(reset.word(0xca4), Some(0x0e00_0070));
    for offset in [0xd28, 0xd94, 0xc6c] {
        assert_eq!(reset.word(offset), Some(u32::MAX));
    }
    for offset in [0xbae, 0xbaf, 0xc1c, 0xfdc, 0xfeb, 0x1150] {
        assert_eq!(reset.byte(offset), None);
    }
}

#[test]
fn actual_local_owner_is_cleared_before_last_read_and_fec_range_is_exact() {
    let mut reset = entry(7);
    let mut prelude = prelude(7);
    let original_growth = prelude.local_buffer().growth();
    let mut provider = Provider {
        fail: Some(6),
        ..Provider::default()
    };
    assert!(reset.finish_tail(&mut prelude, &mut provider).is_err());
    assert_eq!(reset.stage(), Stage::TailFailed);
    let buffer = prelude.local_buffer();
    assert_eq!(
        (
            buffer.capacity(),
            buffer.count(),
            buffer.has_backing(),
            buffer.growth()
        ),
        (0, 0, false, original_growth)
    );
    assert_eq!(reset.word(0xc18), Some(0));
    assert_eq!(reset.word(0xc20), Some(0));
    assert_eq!(reset.word(0xc24), Some(0));
    assert_eq!(reset.byte(0xfeb), None);
    assert!((0xfec..0x1150).all(|offset| reset.byte(offset) == Some(0)));
    assert_eq!(reset.byte(0x1150), None);
    assert_eq!(reset.word(0xc30), None);
    assert_eq!(reset.word(0xc2c), Some(19));
    assert!(reset.completion().is_err());
}

#[test]
fn each_failed_provider_latches_reached_writes_without_replaying_or_completing() {
    for failure in 1..=6 {
        let mut reset = entry(8);
        let mut prelude = prelude(8);
        let mut provider = Provider {
            fail: Some(failure),
            ..Provider::default()
        };
        assert!(reset.finish_tail(&mut prelude, &mut provider).is_err());
        assert_eq!(reset.stage(), Stage::TailFailed);
        assert_eq!(provider.calls.len(), failure);
        assert_eq!(reset.byte(0xca0), Some(0));
        assert_eq!(reset.byte(0xc54), Some(if failure <= 2 { 7 } else { 0 }));
        assert_eq!(reset.word(0xb88).is_some(), failure > 1);
        assert_eq!(reset.word(0xb98).is_some(), failure > 2);
        assert_eq!(
            reset.word(0x71c),
            Some(if failure <= 3 {
                11.0_f32.to_bits()
            } else {
                37.0_f32.to_bits()
            })
        );
        assert_eq!(reset.word(0xeb0).is_some(), failure > 4);
        assert_eq!(reset.word(0xedc).is_some(), failure > 5);
        assert!(reset.completion().is_err());
        assert!(reset.finish_tail(&mut prelude, &mut provider).is_err());
        assert_eq!(provider.calls.len(), failure);
    }
}

#[test]
fn wrong_stage_or_player_preserves_state_and_does_not_touch_provider() {
    let mut reset = entry(9);
    let mut provider = Provider::default();
    assert!(reset.finish_tail(&mut prelude(10), &mut provider).is_err());
    assert_eq!(reset.stage(), Stage::InventorySelected);
    assert_eq!(reset.byte(0xc54), Some(7));
    reset.stage = Stage::AwaitingScratch;
    assert!(reset.finish_tail(&mut prelude(9), &mut provider).is_err());
    assert_eq!(reset.stage(), Stage::AwaitingScratch);
    assert!(provider.calls.is_empty());
}

#[test]
fn completed_tail_cannot_run_again_or_obtain_another_provider_snapshot() {
    let mut reset = entry(11);
    let mut prelude = prelude(11);
    let mut provider = Provider::default();
    reset.finish_tail(&mut prelude, &mut provider).unwrap();
    assert!(reset.finish_tail(&mut prelude, &mut provider).is_err());
    assert_eq!(reset.stage(), Stage::Complete);
    assert_eq!(provider.calls.len(), 6);
    assert_eq!(
        reset.completion().unwrap().player(),
        PlayerId::new(11).unwrap()
    );
}

#[test]
fn actual_base_scratch_character_and_inventory_progress_into_tail_without_early_c54_clear() {
    use crate::mission::player::{
        constructor::{CharacterBinding, Scratch, ScratchBudget},
        inventory,
    };
    struct Flags;
    impl inventory::FlagProvider for Flags {
        fn interface_bound(&mut self) -> Result<bool> {
            Ok(false)
        }
        fn clear_interface_flags(&mut self, _: u32) -> Result<()> {
            unreachable!("unbound actual interface")
        }
    }
    let mut base = base_handler::Owner::new(PlayerId::new(12).unwrap());
    let mut basic_provider = BaseProvider;
    base.initialize_generic(&mut basic_provider).unwrap();
    base.initialize_basic().unwrap();
    base.initialize_player_basic_prefix().unwrap();
    base.apply_reset(17.0_f32.to_bits(), &mut basic_provider)
        .unwrap();
    let mut prelude = PendingPrelude::after_basic(&base.finish_basic().unwrap());
    base.apply_reset(17.0_f32.to_bits(), &mut basic_provider)
        .unwrap();
    base.finish_reset_prefix().unwrap();
    base.finish_runtime_reset(&mut basic_provider).unwrap();
    let mut reset = PendingReset::after_base(&base).unwrap();
    let mut provider = Provider::default();
    assert!(reset.finish_tail(&mut prelude, &mut provider).is_err());
    assert_eq!(reset.byte(0xc54), None);
    let receipt = Scratch::from_constructor()
        .prepare_reset(ScratchBudget {
            native_bytes: 8192,
            growth_steps: 4,
        })
        .unwrap();
    reset.after_scratch(&receipt).unwrap();
    reset
        .bind_character(CharacterBinding {
            registry_resource: 0x0109_0013,
            key: 0x4d00_000a,
            source_index: 2,
        })
        .unwrap();
    reset.begin_inventory().unwrap();
    let metadata = inventory::Metadata {
        inventory_key: 0x0200_0830,
        primary_key: 0x4b00_0021,
        type_key: 0x4b20_0000,
        category_key: 0x4b10_0001,
        byte_8_raw: 0,
        byte_9_raw: 0,
        ammunition_raw: [0; 2],
        source_index: 4,
        native_sorted_index: 2,
        normal_values_bits: [0; 10],
        modifier_bits: [1.0_f32.to_bits(); 4],
    };
    let budget = inventory::Budget {
        records: 16,
        order_words: 4,
    };
    prelude
        .inventory_mut()
        .admit_default(metadata, budget)
        .unwrap();
    prelude
        .inventory_mut()
        .select_default(
            &[metadata.inventory_key],
            &[],
            &metadata,
            inventory::PoseInputs {
                blend_bits: 0,
                c54_raw: reset.byte(0xc54),
                player_flags_raw: 0,
                flag_5ee_raw: 0,
                flag_95c_raw: 0,
                multiplayer_raw: 0,
            },
            &mut Flags,
            budget,
        )
        .unwrap();
    reset.record_inventory(prelude.inventory()).unwrap();
    assert_eq!(reset.stage(), Stage::InventorySelected);
    assert_eq!(reset.byte(0xc54), None);
    assert!(provider.calls.is_empty());
    reset.finish_tail(&mut prelude, &mut provider).unwrap();
    assert_eq!(reset.byte(0xc54), Some(0));
    assert_eq!(reset.stage(), Stage::Complete);
    assert_eq!(prelude.inventory().records().len(), 1);
    assert_eq!(prelude.inventory().growth(), 16);
}
