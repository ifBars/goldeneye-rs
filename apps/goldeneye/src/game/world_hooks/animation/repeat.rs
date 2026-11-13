use super::*;
use anyhow::ensure;
use ge_core::mission::player::actions::{mode_call, secondary_parameters::Globals};
impl Attempt {
    pub fn store_animation_fallback(&self, value: u32) -> Result<(u32, u64)> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("repeat fallback owner poisoned"))?;
        self.ensure_current()?;
        state.animation_fallback.store(value)?;
        Ok((
            state.animation_fallback.value()?,
            state.animation_fallback.writes(),
        ))
    }
    pub fn store_animation_slot_override(&self, enabled: u8, slot: u32) -> Result<()> {
        ensure!(enabled <= 1 && slot < 4, "unsupported repeat slot override");
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("repeat slot override owner poisoned"))?;
        self.ensure_current()?;
        state.animation_slot_override.enabled_raw = enabled;
        state.animation_slot_override.slot_raw = slot;
        Ok(())
    }
    pub fn animation_parameter_globals(&self) -> Result<Globals> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation parameter owner poisoned"))?;
        self.ensure_current()?;
        Ok(state.animation_parameter_globals)
    }
    pub fn finish_repeat_mask(
        &self,
        call: &mut mode_call::Call,
        preparation: &mut Preparation,
    ) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("repeat mask scratch owner poisoned"))?;
        self.ensure_current()?;
        let State {
            animation_context,
            animation_scratch,
            ..
        } = &mut *state;
        call.finish_mask(preparation, animation_context, animation_scratch)?;
        self.ensure_current()?;
        Ok(())
    }
}
