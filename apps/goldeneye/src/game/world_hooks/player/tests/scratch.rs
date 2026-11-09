use super::*;
use crate::game::world_hooks::Shared;
use ge_core::mission::player::{base_handler, identity::PlayerId};

struct Provider;
impl base_handler::Provider for Provider {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        Ok(9)
    }
    fn reset_base_global_byte(&mut self) -> Result<()> {
        Ok(())
    }
    fn reset_resource_state(&mut self, _: u32, _: u32) -> Result<()> {
        Ok(())
    }
}
impl base_handler::RuntimeResetProvider for Provider {
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
fn reset(id: u64) -> PendingReset {
    let mut base = base_handler::Owner::new(PlayerId::new(id).unwrap());
    let mut provider = Provider;
    base.initialize_generic(&mut provider).unwrap();
    base.initialize_basic().unwrap();
    base.initialize_player_basic_prefix().unwrap();
    base.apply_reset(0x4244_0000, &mut provider).unwrap();
    base.finish_basic().unwrap();
    base.apply_reset(0x4244_0000, &mut provider).unwrap();
    base.finish_reset_prefix().unwrap();
    base.finish_runtime_reset(&mut provider).unwrap();
    PendingReset::after_base(&base).unwrap()
}
#[test]
fn scratch_reentry_reuses_actual_backing_and_rejects_stale_or_replayed_steps() {
    let shared = Shared::from_constructor();
    let old = shared.begin(1).unwrap();
    assert!(old.player_multiplayer_byte().is_err());
    let first = old
        .begin_player_constructor(PlayerId::new(1).unwrap())
        .unwrap();
    let mut pending = reset(1);
    old.prepare_player_scratch(first, &mut pending).unwrap();
    assert_eq!(pending.stage(), DerivedResetStage::SelectingCharacter);
    assert_eq!(pending.byte(0xe58), Some(1));
    let before = old.player_scratch_snapshot().unwrap();
    assert_eq!((before.capacity, before.growth, before.count), (32, 16, 0));
    assert_eq!((before.native_bytes, before.reallocations), (4736, 1));
    assert!(old.prepare_player_scratch(first, &mut pending).is_err());
    shared.invalidate();
    let current = shared.begin(2).unwrap();
    assert!(old.player_scratch_snapshot().is_err());
    let second = current
        .begin_player_constructor(PlayerId::new(2).unwrap())
        .unwrap();
    let mut next = reset(2);
    assert!(current.prepare_player_scratch(first, &mut next).is_err());
    current.prepare_player_scratch(second, &mut next).unwrap();
    let after = current.player_scratch_snapshot().unwrap();
    assert_eq!(after.host_capacity_bytes, before.host_capacity_bytes);
    assert_eq!(after.state_sha256, before.state_sha256);
    assert_eq!((after.native_bytes, after.reallocations), (4736, 1));
}
