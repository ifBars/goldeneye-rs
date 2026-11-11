//! The supported PlayerNew embedded animation receiver, distinct from a raw
//! pointer to its root handler. Other receiver classes need their own dispatch.
use super::{Binding, Owner, Stage};
use crate::mission::player::identity::PlayerId;
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationInterface {
    player: PlayerId,
}
impl Owner {
    pub fn animation_interface(&self) -> Result<AnimationInterface> {
        ensure!(
            self.stage == Stage::VirtualBasicComplete
                && self.bindings.get(&0x4fc) == Some(&Binding::EmbeddedInterface),
            "animation interface requires the actual initialized player member"
        );
        Ok(AnimationInterface {
            player: self.player,
        })
    }
}
impl AnimationInterface {
    pub fn player(self) -> PlayerId {
        self.player
    }
    pub fn visit_state_words(self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
    }
}
