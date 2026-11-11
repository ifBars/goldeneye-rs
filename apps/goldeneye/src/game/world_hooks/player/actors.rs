//! Persistent objects behind the source global item registry and actor lists.
//! This lock is separate from manager state, so reached provider writes do not
//! recursively acquire the same mutex. Cancelled objects retain real owners.
use super::super::Attempt;
use anyhow::Result;
use ge_core::mission::player::{
    identity::{ItemId, PlayerId, WorldId},
    item_registry::Registry,
    world::{Budget, World},
};
use std::collections::BTreeMap;

pub(in crate::game) struct Actors {
    pub world: World,
    pub registry: Registry,
    pub players: BTreeMap<PlayerId, crate::game::player::Owned>,
    pub components: ge_core::mission::player::components::Registry,
}
impl Actors {
    pub fn from_constructor() -> Self {
        Self {
            world: World::from_constructor(
                WorldId::new(1).expect("nonzero global world identity"),
                1,
                10,
                Budget {
                    items: 65_536,
                    memberships: 32,
                },
            )
            .expect("bounded global world constructor"),
            registry: Registry::from_constructor(),
            players: BTreeMap::new(),
            components: ge_core::mission::player::components::Registry::from_constructor(65_536),
        }
    }
}
impl Attempt {
    pub fn with_player_actors<T>(
        &self,
        operation: impl FnOnce(&mut Actors) -> Result<T>,
    ) -> Result<T> {
        self.ensure_current()?;
        let mut actors = self
            .shared
            .0
            .actors
            .lock()
            .map_err(|_| anyhow::anyhow!("player actor arena lock is poisoned"))?;
        self.ensure_current()?;
        let result = operation(&mut actors);
        self.ensure_current()?;
        result
    }

    pub fn player_registration_mask(&self, item: ItemId) -> Result<u32> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player registration owner lock is poisoned"))?;
        self.ensure_current()?;
        let flags = state.player.config.current_flags()?;
        // Actual PlayerNew ancestry does not take the Restart bit-2 branch.
        let mut mask = 0x21;
        if flags.config_raw != 0 {
            mask |= 8;
        }
        if flags.game_raw != 0 {
            mask |= 4;
            state.player.game_item = Some(item);
        }
        Ok(mask)
    }
}

#[cfg(test)]
#[path = "tests/actors.rs"]
mod tests;
