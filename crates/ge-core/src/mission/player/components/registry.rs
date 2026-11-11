//! Host identities for retained component owners. These ordinals are not native
//! addresses or a reconstruction of the game's allocator/SDK registry history.
use crate::mission::player::identity::{ComponentId, PlayerId};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    id: ComponentId,
    player: PlayerId,
}
pub struct Registry {
    identities: Vec<Identity>,
    counter: u64,
    limit: usize,
}
impl Registry {
    pub fn from_constructor(limit: usize) -> Self {
        Self {
            identities: Vec::new(),
            counter: 0,
            limit,
        }
    }
    pub fn allocate(&mut self, player: PlayerId) -> Result<Identity> {
        ensure!(
            self.identities.len() < self.limit,
            "component identity budget exhausted"
        );
        let next = self
            .counter
            .checked_add(1)
            .context("component identity counter overflow")?;
        let identity = Identity {
            id: ComponentId::new(next)?,
            player,
        };
        self.identities.try_reserve(1)?;
        self.identities.push(identity);
        self.counter = next;
        Ok(identity)
    }
    pub fn resolve(&self, id: ComponentId) -> Option<Identity> {
        let index = usize::try_from(id.get().checked_sub(1)?).ok()?;
        self.identities
            .get(index)
            .copied()
            .filter(|identity| identity.id == id)
    }
    pub fn len(&self) -> usize {
        self.identities.len()
    }
    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}
impl Identity {
    pub fn id(self) -> ComponentId {
        self.id
    }
    pub fn player(self) -> PlayerId {
        self.player
    }
    pub fn visit_state_words(self, mut emit: impl FnMut(u32)) {
        emit(1);
        for id in [self.id.get(), self.player.get()] {
            emit((id >> 32) as u32);
            emit(id as u32);
        }
    }
}

#[cfg(test)]
#[path = "tests/registry.rs"]
mod tests;
