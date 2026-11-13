//! Sparse source constructor/reset state. Unwritten fields remain unknown.
//! Shared RNG, global and resource mutations execute through retained providers;
//! no independent stream/store or completed player is manufactured here.
mod animation;
mod basic;
mod callback_tail;
mod mode_setter;
pub use animation::AnimationInterface;
mod constructor_flag;
mod constructor_tail;
mod generic;
mod reset;
mod runtime_reset;
mod types;
mod virtual_init;
mod virtual_suffix;
mod visual;
pub use constructor_flag::{ConstructorFlagProvider, ConstructorFlagReceipt};
pub use runtime_reset::RuntimeResetProvider;
pub use types::{BasicReceipt, Binding, Failure, Provider, ResetReceipt, Stage};
pub use virtual_suffix::{VirtualBasicReceipt, VirtualSessionProvider};
pub use visual::VisualOverrides;

use super::identity::{ItemId, PlayerId};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Owner {
    player: PlayerId,
    stage: Stage,
    failure: Option<Failure>,
    words: BTreeMap<u16, u32>,
    halves: BTreeMap<u16, u16>,
    bytes: BTreeMap<u16, u8>,
    bindings: BTreeMap<u16, Binding>,
    reset_calls: usize,
    completed_resets: usize,
    nested_initialized: bool,
    reset_string: Option<String>,
    item: Option<ItemId>,
    visual_skeleton: Option<super::components::Identity>,
}
impl Owner {
    pub fn new(player: PlayerId) -> Self {
        Self {
            player,
            stage: Stage::Fresh,
            failure: None,
            words: BTreeMap::new(),
            halves: BTreeMap::new(),
            bytes: BTreeMap::new(),
            bindings: BTreeMap::new(),
            reset_calls: 0,
            completed_resets: 0,
            nested_initialized: false,
            reset_string: None,
            item: None,
            visual_skeleton: None,
        }
    }
    fn require(&self, stage: Stage) -> Result<()> {
        ensure!(
            self.failure.is_none(),
            "base-handler retains a failed provider operation"
        );
        ensure!(
            self.stage == stage,
            "base-handler operation is out of source order"
        );
        Ok(())
    }
    fn w(&mut self, offset: u16, value: u32) {
        self.words.insert(offset, value);
    }
    fn b(&mut self, offset: u16, value: u8) {
        self.bytes.insert(offset, value);
    }
    fn h(&mut self, offset: u16, value: u16) {
        self.halves.insert(offset, value);
    }
    fn zero_words(&mut self, start: u16, end: u16) {
        for offset in (start..=end).step_by(4) {
            self.w(offset, 0);
        }
    }
    fn zero_bytes(&mut self, start: u16, end: u16) {
        for offset in start..=end {
            self.b(offset, 0);
        }
    }
    fn container(&mut self, offset: u16, growth: u32) {
        for delta in [0, 4, 12, 16] {
            self.w(offset + delta, 0);
        }
        self.w(offset + 8, growth);
        self.bindings.insert(offset + 20, Binding::Container);
    }
    pub const fn player(&self) -> PlayerId {
        self.player
    }
    pub const fn item(&self) -> Option<ItemId> {
        self.item
    }
    pub const fn stage(&self) -> Stage {
        self.stage
    }
    pub const fn failure(&self) -> Option<Failure> {
        self.failure
    }
    pub fn word(&self, offset: u16) -> Option<u32> {
        self.words.get(&offset).copied()
    }
    pub fn halfword(&self, offset: u16) -> Option<u16> {
        self.halves.get(&offset).copied()
    }
    pub fn byte(&self, offset: u16) -> Option<u8> {
        self.bytes.get(&offset).copied()
    }
    pub fn known_words(&self) -> impl Iterator<Item = (u16, u32)> + '_ {
        self.words.iter().map(|(&offset, &bits)| (offset, bits))
    }
    pub const fn reset_calls(&self) -> usize {
        self.reset_calls
    }
    pub const fn completed_resets(&self) -> usize {
        self.completed_resets
    }
    pub const fn nested_initialized(&self) -> bool {
        self.nested_initialized
    }

    /// Versioned canonical scalar visitation, not a serialized native image.
    /// Width-specific maps preserve unknown padding and semantic bindings.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(3);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(match self.stage {
            Stage::Fresh => 0,
            Stage::GenericPrefix => 1,
            Stage::GenericComplete => 2,
            Stage::BaseComplete => 3,
            Stage::PlayerBasicPrefix => 4,
            Stage::ResetPrefix => 5,
            Stage::ResetComplete => 6,
            Stage::BasicComplete => 7,
            Stage::RuntimeResetPrefix => 8,
            Stage::RuntimeResetTail => 9,
            Stage::RuntimeResetComplete => 10,
            Stage::ConstructorFlagPrefix => 11,
            Stage::ConstructorFlagComplete => 12,
            Stage::VirtualItemBound => 13,
            Stage::VirtualMotionComplete => 14,
            Stage::VirtualBasicSuffix => 15,
            Stage::VirtualBasicComplete => 16,
        });
        emit(match self.failure {
            None => 0,
            Some(Failure::RandomSeed) => 1,
            Some(Failure::GlobalReset) => 2,
            Some(Failure::ResourceReset) => 3,
            Some(Failure::RuntimeDependency) => 4,
            Some(Failure::RuntimeGlobalFloat) => 5,
            Some(Failure::RuntimeDescriptor) => 6,
            Some(Failure::RuntimeGlobalByte) => 7,
            Some(Failure::ConstructorFlag) => 8,
            Some(Failure::VirtualSession) => 9,
        });
        for value in [self.reset_calls, self.completed_resets] {
            emit(((value as u64) >> 32) as u32);
            emit(value as u32);
        }
        emit(u32::from(self.nested_initialized));
        emit(u32::from(self.reset_string.is_some()));
        if let Some(value) = &self.reset_string {
            emit(value.len() as u32);
        }
        emit(self.words.len() as u32);
        for (&offset, &bits) in &self.words {
            emit(u32::from(offset));
            emit(bits);
        }
        emit(self.halves.len() as u32);
        for (&offset, &bits) in &self.halves {
            emit(u32::from(offset));
            emit(u32::from(bits));
        }
        emit(self.bytes.len() as u32);
        for (&offset, &bits) in &self.bytes {
            emit(u32::from(offset));
            emit(u32::from(bits));
        }
        emit(self.bindings.len() as u32);
        for (&offset, &binding) in &self.bindings {
            emit(u32::from(offset));
            emit(binding.raw());
        }
        emit(u32::from(self.item.is_some()));
        if let Some(item) = self.item {
            for id in [item.world().get(), item.get()] {
                emit((id >> 32) as u32);
                emit(id as u32);
            }
        }
        emit(u32::from(self.visual_skeleton.is_some()));
        if let Some(identity) = self.visual_skeleton {
            identity.visit_state_words(emit);
        }
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/callback_fixture.rs"]
pub(crate) mod callback_fixture;
