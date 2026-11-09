use super::*;
use crate::mission::player::base_handler;

struct Providers;
impl base_handler::Provider for Providers {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        Ok(7)
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

#[test]
fn derived_clear_requires_complete_base_and_preserves_unwritten_boundaries() {
    let mut base = base_handler::Owner::new(player(29));
    let mut provider = Providers;
    assert!(PendingReset::after_base(&base).is_err());
    base.initialize_generic(&mut provider).unwrap();
    base.initialize_basic().unwrap();
    base.initialize_player_basic_prefix().unwrap();
    base.apply_reset(0x4244_0000, &mut provider).unwrap();
    let basic = base.finish_basic().unwrap();
    let prelude = PendingPrelude::after_basic(&basic);
    assert!(PendingReset::after_base(&base).is_err());
    base.apply_reset(0x4244_0000, &mut provider).unwrap();
    base.finish_reset_prefix().unwrap();
    assert!(PendingReset::after_base(&base).is_err());
    base.finish_runtime_reset(&mut provider).unwrap();
    let reset = PendingReset::after_base(&base).unwrap();
    assert_eq!(reset.player(), prelude.player());
    assert_eq!(reset.word(0xc74), Some(0));
    assert_eq!(reset.word(0xc7c), Some(0x0101_0000));
    assert_eq!(reset.word(0xd10), Some(0));
    assert_eq!(reset.byte(0xfdb), Some(0));
    assert_eq!(reset.byte(0xfdc), None);
    assert_eq!(reset.word(0x1154), Some(0));
    assert_eq!(reset.word(0x1158), Some(0));
    assert_eq!(reset.byte(0xbab), Some(1));
    assert_eq!(reset.byte(0xbac), Some(1));
    assert_eq!(reset.byte(0xc54), None);
    assert_eq!(reset.word(u16::MAX), None);
    assert!(prelude.inventory().records().is_empty());
    assert_eq!(base.completed_resets(), 2);
}
