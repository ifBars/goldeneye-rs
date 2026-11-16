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
    /// Additive marker for owners whose source destructor completed.
    retired: Vec<bool>,
    counter: u64,
    limit: usize,
}
impl Registry {
    pub fn from_constructor(limit: usize) -> Self {
        Self {
            identities: Vec::new(),
            retired: Vec::new(),
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
        self.retired.try_reserve(1)?;
        self.identities.push(identity);
        self.retired.push(false);
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
    /// Marks a destroyed owner. Identities are never reused or removed, so
    /// `resolve` still returns this historical identity.
    pub fn retire(&mut self, identity: Identity) -> Result<()> {
        ensure!(
            self.resolve(identity.id) == Some(identity),
            "retired identity does not belong to this registry"
        );
        let index = (identity.id.get() - 1) as usize;
        ensure!(
            !self.retired[index],
            "component identity is already retired"
        );
        self.retired[index] = true;
        Ok(())
    }
    pub fn is_retired(&self, identity: Identity) -> bool {
        self.resolve(identity.id) == Some(identity)
            && self.retired[(identity.id.get() - 1) as usize]
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
