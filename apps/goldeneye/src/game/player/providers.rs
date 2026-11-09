//! Source providers bind the retained worker and controller owners.
use super::super::{actors::Definitions, resource_states, world_hooks};
use anyhow::{Context, Result};
use ge_core::mission::{
    player::{base_handler, inventory::FlagProvider},
    session::Session,
};

pub(super) struct Provider<'a> {
    pub(super) definitions: &'a mut Definitions,
    pub(super) resources: &'a mut resource_states::Attempt,
    pub(super) hook: &'a world_hooks::Attempt,
    pub(super) session: &'a Session,
    pub(super) rng_reads: usize,
    pub(super) resource_resets: usize,
}
impl base_handler::RuntimeResetProvider for Provider<'_> {
    fn reset_runtime_global_float(&mut self, bits: u32) -> Result<()> {
        self.hook.reset_player_runtime_float(bits)
    }
    fn session_descriptor_present(&mut self) -> Result<bool> {
        self.hook.ensure_current()?;
        Ok(self.session.descriptor().is_some())
    }
    fn session_descriptor_mode(&mut self) -> Result<u32> {
        self.hook.ensure_current()?;
        Ok(self
            .session
            .descriptor()
            .ok_or_else(|| anyhow::anyhow!("player reset session descriptor disappeared"))?
            .word_2c_raw)
    }
    fn set_runtime_multiplayer_byte(&mut self, value: u8) -> Result<()> {
        self.hook.set_player_multiplayer_byte(value)
    }
}
impl base_handler::Provider for Provider<'_> {
    fn draw_handler_seed(&mut self) -> Result<u16> {
        self.hook.ensure_current()?;
        let seed = self.definitions.draw_handler_seed()?;
        self.rng_reads += 1;
        Ok(seed)
    }
    fn reset_base_global_byte(&mut self) -> Result<()> {
        self.hook.reset_player_base_byte()
    }
    fn reset_resource_state(&mut self, token: u32, value: u32) -> Result<()> {
        self.resources
            .set(token, value, || self.hook.ensure_current())?;
        self.resource_resets += 1;
        Ok(())
    }
}

pub(super) struct InventoryFlags<'a> {
    pub base: &'a base_handler::Owner,
    pub hook: &'a world_hooks::Attempt,
}
impl FlagProvider for InventoryFlags<'_> {
    fn interface_bound(&mut self) -> Result<bool> {
        self.hook.ensure_current()?;
        let pointer = self
            .base
            .word(0x4f8)
            .context("player interface construction is unbound")?;
        anyhow::ensure!(
            pointer == 0,
            "bound player interface needs its actual flag provider"
        );
        Ok(false)
    }
    fn clear_interface_flags(&mut self, _: u32) -> Result<()> {
        anyhow::bail!("player interface flag callback is unavailable")
    }
}

pub(super) struct Tail<'a> {
    pub hook: &'a world_hooks::Attempt,
    pub sources: &'a super::loadout::Sources,
}
impl ge_core::mission::player::constructor::TailProvider for Tail<'_> {
    fn default_quad_bits(&mut self) -> Result<[u32; 4]> {
        self.hook.player_reset_quad_bits()
    }
    fn current_game_field_328_bits(&mut self) -> Result<u32> {
        self.hook.player_reset_input()
    }
    fn default_vector_bits(&mut self) -> Result<[u32; 3]> {
        self.hook.ensure_current()?;
        Ok(self
            .sources
            .supplied_data()
            .player_reset_defaults()?
            .vector_bits())
    }
}
