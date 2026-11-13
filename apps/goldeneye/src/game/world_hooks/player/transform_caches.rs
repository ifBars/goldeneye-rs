//! The shared cache headers outlive individual players and cancelled attempts.
use super::super::Attempt;
use anyhow::Result;
use ge_core::mission::player::{constructor::Shared, transform_cache::Owner};

impl Attempt {
    pub(in crate::game) fn with_player_transform_caches<T>(
        &self,
        operation: impl FnOnce(&mut Owner, &Shared) -> Result<T>,
    ) -> Result<T> {
        self.ensure_current()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player transform-cache lock is poisoned"))?;
        self.ensure_current()?;
        let player = &mut state.player;
        let result = operation(&mut player.transform_caches, &player.constructor);
        self.ensure_current()?;
        result
    }
}

#[cfg(test)]
#[path = "tests/transform_caches.rs"]
mod tests;
