//! The same controller-lived arena/context serves reached animation callbacks.
//! Its mutex is separate from actor ownership and survives canceled attempts.
use super::{Attempt, State};
use anyhow::Result;
use ge_core::mission::player::components::Preparation;

impl Attempt {
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
