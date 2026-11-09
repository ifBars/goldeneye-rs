use super::*;
use crate::mission::player::{constructor, inventory};

struct Source;
impl Provider for Source {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        Ok(5)
    }
    fn reset_base_global_byte(&mut self) -> Result<()> {
        Ok(())
    }
    fn reset_resource_state(&mut self, _: u32, _: u32) -> Result<()> {
        Ok(())
    }
}
impl RuntimeResetProvider for Source {
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
impl constructor::TailProvider for Source {
    fn default_quad_bits(&mut self) -> Result<[u32; 4]> {
        Ok([2.0_f32.to_bits(); 4])
    }
    fn current_game_field_328_bits(&mut self) -> Result<u32> {
        Ok(21.0_f32.to_bits())
    }
    fn default_vector_bits(&mut self) -> Result<[u32; 3]> {
        Ok([3.0_f32.to_bits(); 3])
    }
}
impl inventory::FlagProvider for Source {
    fn interface_bound(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn clear_interface_flags(&mut self, _: u32) -> Result<()> {
        unreachable!("authored absent interface")
    }
}

// Obtain the unforgeable completion through the real pure reset pipeline.
// Its catalog/pose inputs are independently authored synthetic metadata.
fn completed_reset(id: u64) -> (Owner, constructor::ResetCompletion) {
    let mut source = Source;
    let mut owner = Owner::new(PlayerId::new(id).unwrap());
    owner.initialize_generic(&mut source).unwrap();
    owner.initialize_basic().unwrap();
    owner.initialize_player_basic_prefix().unwrap();
    owner.apply_reset(21.0_f32.to_bits(), &mut source).unwrap();
    let mut prelude = constructor::PendingPrelude::after_basic(&owner.finish_basic().unwrap());
    owner.apply_reset(21.0_f32.to_bits(), &mut source).unwrap();
    owner.finish_reset_prefix().unwrap();
    owner.finish_runtime_reset(&mut source).unwrap();
    let mut reset = constructor::PendingReset::after_base(&owner).unwrap();
    let receipt = constructor::Scratch::from_constructor()
        .prepare_reset(constructor::ScratchBudget {
            native_bytes: 8192,
            growth_steps: 4,
        })
        .unwrap();
    reset.after_scratch(&receipt).unwrap();
    reset
        .bind_character(constructor::CharacterBinding {
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
            &mut source,
            budget,
        )
        .unwrap();
    reset.record_inventory(prelude.inventory()).unwrap();
    reset.finish_tail(&mut prelude, &mut source).unwrap();
    (owner, reset.completion().unwrap())
}

#[derive(Default)]
struct Notification {
    calls: Vec<u32>,
    fail: bool,
}
impl ConstructorFlagProvider for Notification {
    fn mark_constructor_flag_dirty(&mut self, channel: u32) -> Result<()> {
        self.calls.push(channel);
        ensure!(!self.fail, "authored notification failure");
        Ok(())
    }
}

#[test]
fn equal_raw_flag_skips_even_an_unavailable_interface_and_failing_provider() {
    for raw in [0, 0x80, 0xff] {
        let (mut owner, proof) = completed_reset(30);
        owner.b(0x994, raw);
        owner.words.remove(&0x4f8);
        let mut provider = Notification {
            fail: true,
            ..Notification::default()
        };
        let receipt = owner
            .apply_constructor_flag(&proof, raw, 0, &mut provider)
            .unwrap();
        assert_eq!(
            (
                receipt.player(),
                receipt.raw(),
                receipt.changed(),
                receipt.interface_callbacks()
            ),
            (owner.player(), raw, false, 0)
        );
        assert_eq!(owner.byte(0x994), Some(raw));
        assert_eq!(owner.word(0x4f8), None);
        assert!(provider.calls.is_empty());
        assert_eq!(owner.stage(), Stage::ConstructorFlagComplete);
        assert_eq!(owner.failure(), None);
    }
}

#[test]
fn changed_high_bit_bytes_and_signed_nonzero_suppression_preserve_raw_values() {
    for (raw, suppress) in [(0x80, -128), (0xff, -1), (0x7f, 1)] {
        let (mut owner, proof) = completed_reset(31);
        owner.words.remove(&0x4f8);
        let mut provider = Notification::default();
        let receipt = owner
            .apply_constructor_flag(&proof, raw, suppress, &mut provider)
            .unwrap();
        assert!(receipt.changed());
        assert_eq!(receipt.raw(), raw);
        assert_eq!(receipt.interface_callbacks(), 0);
        assert_eq!(owner.byte(0x994), Some(raw));
        assert!(provider.calls.is_empty());
    }
}

#[test]
fn actual_null_interface_skips_notification_but_bound_interface_calls_channel_two() {
    for bound in [false, true] {
        let (mut owner, proof) = completed_reset(32);
        owner.w(0x4f8, if bound { 7 } else { 0 });
        let mut provider = Notification::default();
        let receipt = owner
            .apply_constructor_flag(&proof, 0xff, 0, &mut provider)
            .unwrap();
        assert_eq!(owner.byte(0x994), Some(0xff));
        assert_eq!(receipt.interface_callbacks(), usize::from(bound));
        assert_eq!(provider.calls, if bound { vec![2] } else { vec![] });
        assert_eq!(owner.stage(), Stage::ConstructorFlagComplete);
    }
}

#[test]
fn callback_failure_retains_the_prior_write_and_blocks_replay_and_completion() {
    let (mut owner, proof) = completed_reset(33);
    owner.w(0x4f8, 9);
    let mut provider = Notification {
        fail: true,
        ..Notification::default()
    };
    assert!(
        owner
            .apply_constructor_flag(&proof, 0x80, 0, &mut provider)
            .is_err()
    );
    assert_eq!(owner.byte(0x994), Some(0x80));
    assert_eq!(owner.stage(), Stage::ConstructorFlagPrefix);
    assert_eq!(owner.failure(), Some(Failure::ConstructorFlag));
    provider.fail = false;
    assert!(
        owner
            .apply_constructor_flag(&proof, 0x80, 0, &mut provider)
            .is_err()
    );
    assert_eq!(provider.calls, [2]);
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    assert_eq!((words[3], words[4]), (11, 8));
}

#[test]
fn missing_interface_is_not_null_and_missing_old_byte_is_not_zero() {
    let (mut owner, proof) = completed_reset(34);
    owner.words.remove(&0x4f8);
    let mut provider = Notification::default();
    assert!(
        owner
            .apply_constructor_flag(&proof, 0xff, 0, &mut provider)
            .is_err()
    );
    assert_eq!(owner.byte(0x994), Some(0xff));
    assert_eq!(owner.failure(), Some(Failure::ConstructorFlag));
    assert!(provider.calls.is_empty());

    let (mut owner, proof) = completed_reset(35);
    owner.bytes.remove(&0x994);
    assert!(
        owner
            .apply_constructor_flag(&proof, 0, 0, &mut provider)
            .is_err()
    );
    assert_eq!(owner.byte(0x994), None);
    assert_eq!(owner.stage(), Stage::ConstructorFlagPrefix);
    assert_eq!(owner.failure(), Some(Failure::ConstructorFlag));
    assert!(provider.calls.is_empty());
}

#[test]
fn reset_proof_player_and_base_stage_are_checked_before_mutation() {
    let (mut owner, _) = completed_reset(36);
    let (_, wrong) = completed_reset(37);
    let mut provider = Notification::default();
    assert!(
        owner
            .apply_constructor_flag(&wrong, 0x80, 0, &mut provider)
            .is_err()
    );
    assert_eq!(owner.stage(), Stage::RuntimeResetComplete);
    assert_eq!(owner.failure(), None);
    assert_eq!(owner.byte(0x994), Some(0));

    let (mut owner, proof) = completed_reset(38);
    owner.stage = Stage::RuntimeResetPrefix;
    assert!(
        owner
            .apply_constructor_flag(&proof, 0x80, 0, &mut provider)
            .is_err()
    );
    assert_eq!(owner.stage(), Stage::RuntimeResetPrefix);
    assert_eq!(owner.failure(), None);
    assert_eq!(owner.byte(0x994), Some(0));
    assert!(provider.calls.is_empty());
}

#[test]
fn completed_consumer_cannot_write_or_notify_a_second_time() {
    let (mut owner, proof) = completed_reset(39);
    owner.w(0x4f8, 5);
    let mut provider = Notification::default();
    owner
        .apply_constructor_flag(&proof, 0x80, 0, &mut provider)
        .unwrap();
    assert!(
        owner
            .apply_constructor_flag(&proof, 0xff, 0, &mut provider)
            .is_err()
    );
    assert_eq!(owner.byte(0x994), Some(0x80));
    assert_eq!(owner.stage(), Stage::ConstructorFlagComplete);
    assert_eq!(provider.calls, [2]);
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    assert_eq!((words[3], words[4]), (12, 0));
}
