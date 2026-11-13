//! Reached derived reset writes before the shared scratch-capacity operation.
//! This owns only known bytes; the rest of the player remains uninitialized.
mod action;
mod callback_tail;
mod components;
mod constructor_tail;
mod pose;
mod selection;
mod tail;
use super::super::{base_handler, identity::PlayerId};
use anyhow::{Result, ensure};
pub use selection::CharacterBinding;
use std::collections::BTreeMap;
pub use tail::TailProvider;

#[derive(Debug)]
pub struct PendingReset {
    player: PlayerId,
    bytes: BTreeMap<u16, u8>,
    stage: Stage,
    character: Option<CharacterBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    AwaitingScratch,
    SelectingCharacter,
    CharacterBound,
    SelectingInventory,
    InventorySelected,
    TailInProgress,
    TailFailed,
    Complete,
}

/// Proof that this retained player's supported virtual reset reached its return.
/// It does not complete the enclosing PlayerNew constructor.
#[derive(Debug)]
pub struct ResetCompletion {
    player: PlayerId,
}
impl ResetCompletion {
    pub fn player(&self) -> PlayerId {
        self.player
    }
}

impl PendingReset {
    pub fn after_base(base: &base_handler::Owner) -> Result<Self> {
        ensure!(
            base.stage() == base_handler::Stage::RuntimeResetComplete,
            "derived reset requires the actual complete runtime base reset"
        );
        let mut reset = Self {
            player: base.player(),
            bytes: BTreeMap::new(),
            stage: Stage::AwaitingScratch,
            character: None,
        };
        // The explicit C74 memset covers C74..FDB, not the whole player.
        for offset in 0xc74..0xfdc {
            reset.bytes.insert(offset, 0);
        }
        for offset in [0x1158, 0x1154] {
            reset.word_store(offset, 0);
        }
        for offset in [0xc7d, 0xbab, 0xbac] {
            reset.bytes.insert(offset, 1);
        }
        for offset in [0xeec, 0xbbf, 0xbbe, 0xbbd, 0xbc0, 0xbbc, 0xbb9, 0xbba] {
            reset.bytes.insert(offset, 0);
        }
        reset.word_store(0xbc4, 0);
        for offset in [0xc74, 0xc78, 0x99c, 0x9a0, 0x9a4] {
            reset.word_store(offset, 0);
        }
        reset.bytes.insert(0xc7c, 1);
        Ok(reset)
    }

    fn word_store(&mut self, offset: u16, word: u32) {
        for (delta, byte) in word.to_be_bytes().into_iter().enumerate() {
            self.bytes.insert(offset + delta as u16, byte);
        }
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn character(&self) -> Option<CharacterBinding> {
        self.character
    }
    pub fn completion(&self) -> Result<ResetCompletion> {
        ensure!(
            self.stage == Stage::Complete,
            "derived reset has not completed"
        );
        Ok(ResetCompletion {
            player: self.player,
        })
    }
    pub fn halfword(&self, offset: u16) -> Option<u16> {
        Some(u16::from_be_bytes([
            self.byte(offset)?,
            self.byte(offset.checked_add(1)?)?,
        ]))
    }
    pub fn byte(&self, offset: u16) -> Option<u8> {
        self.bytes.get(&offset).copied()
    }
    pub fn word(&self, offset: u16) -> Option<u32> {
        let mut bytes = [0; 4];
        for (delta, byte) in bytes.iter_mut().enumerate() {
            *byte = self.byte(offset.checked_add(delta as u16)?)?;
        }
        Some(u32::from_be_bytes(bytes))
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(2);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(match self.stage {
            Stage::AwaitingScratch => 0,
            Stage::SelectingCharacter => 1,
            Stage::CharacterBound => 2,
            Stage::SelectingInventory => 3,
            Stage::InventorySelected => 4,
            Stage::TailInProgress => 5,
            Stage::TailFailed => 6,
            Stage::Complete => 7,
        });
        emit(u32::from(self.character.is_some()));
        if let Some(character) = self.character {
            emit(character.registry_resource);
            emit(character.key);
            emit((character.source_index as u64 >> 32) as u32);
            emit(character.source_index as u32);
        }
        emit(self.bytes.len() as u32);
        for (&offset, &byte) in &self.bytes {
            emit(u32::from(offset));
            emit(u32::from(byte));
        }
    }
}
