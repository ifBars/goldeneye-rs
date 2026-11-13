//! The same controller-lived arena/context serves reached animation callbacks.
//! Its mutex is separate from actor ownership and survives canceled attempts.
use super::{Attempt, State};
use anyhow::Result;
use ge_core::mission::player::components::Preparation;
mod repeat;

impl Attempt {
    pub fn finish_initial_fp_mask(
        &self,
        action: &mut ge_core::mission::player::actions::FirstPersonDispatch,
        preparation: &mut Preparation,
    ) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("FP mask scratch owner lock is poisoned"))?;
        self.ensure_current()?;
        let State {
            animation_context,
            animation_scratch,
            ..
        } = &mut *state;
        action.finish_mask_command(preparation, animation_context, animation_scratch)?;
        self.ensure_current()?;
        Ok(())
    }
    pub fn begin_initial_fp_dispatch(
        &self,
        action: &mut ge_core::mission::player::actions::Pending,
        base: &mut ge_core::mission::player::base_handler::Owner,
        reset: &ge_core::mission::player::constructor::PendingReset,
        preparation: &mut Preparation,
    ) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("FP fallback owner lock is poisoned"))?;
        self.ensure_current()?;
        action.begin_first_person_dispatch(
            base,
            reset,
            preparation,
            &mut state.animation_fallback,
        )?;
        self.ensure_current()?;
        Ok(())
    }
    pub fn animation_rate_inputs(
        &self,
        current_file_data_available: bool,
    ) -> Result<ge_core::animation::script_child::rate::Inputs> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation rate owner lock is poisoned"))?;
        self.ensure_current()?;
        let mut inputs = state.animation_rate_globals;
        inputs.current_file_data_available = current_file_data_available;
        inputs.baseline = state.clock.baseline();
        Ok(inputs)
    }
    pub fn animation_step(&self) -> Result<u32> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation step owner lock is poisoned"))?;
        self.ensure_current()?;
        Ok(state.clock.scaled_step_bits())
    }
    pub fn animation_slot_override(&self) -> Result<ge_core::animation::layers::SlotOverride> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation slot override owner lock is poisoned"))?;
        self.ensure_current()?;
        Ok(state.animation_slot_override)
    }
    pub fn capture_initial_tp(
        &self,
        action: &mut ge_core::mission::player::actions::Pending,
        preparation: &mut Preparation,
    ) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("TP capture shared owner lock is poisoned"))?;
        self.ensure_current()?;
        let State {
            animation_context,
            animation_scratch,
            animation_fallback,
            ..
        } = &mut *state;
        action.capture_third_person(
            preparation,
            animation_context,
            animation_scratch,
            animation_fallback,
        )?;
        self.ensure_current()?;
        Ok(())
    }
    pub fn animation_fallback(&self) -> Result<(u32, u64)> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation fallback owner lock is poisoned"))?;
        self.ensure_current()?;
        Ok((
            state.animation_fallback.value()?,
            state.animation_fallback.writes(),
        ))
    }
    pub fn finish_fp_bind(&self, preparation: &mut Preparation) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("FP animation scratch owner lock is poisoned"))?;
        self.ensure_current()?;
        let State {
            animation_context,
            animation_scratch,
            ..
        } = &mut *state;
        preparation
            .first_person_mut()?
            .finish_bind_zero_time(animation_context, animation_scratch)?;
        self.ensure_current()?;
        Ok(())
    }
    pub fn animation_callbacks(
        &self,
    ) -> Result<std::sync::Arc<ge_core::mission::player::components::callbacks::Table>> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation callback owner lock is poisoned"))?;
        self.ensure_current()?;
        Ok(std::sync::Arc::clone(&state.animation_callbacks))
    }
    pub fn finish_tp_bind(&self, preparation: &mut Preparation) -> Result<()> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("animation scratch owner lock is poisoned"))?;
        self.ensure_current()?;
        let State {
            animation_context,
            animation_scratch,
            ..
        } = &mut *state;
        preparation.finish_bind_zero_time(animation_context, animation_scratch)?;
        // Reached shared mutations and the real bind return remain retained if
        // cancellation wins here. No later attachment is dispatched by this call.
        self.ensure_current()?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/animation.rs"]
mod tests;
