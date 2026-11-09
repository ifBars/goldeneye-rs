//! Reached derived reset writes before the shared scratch-capacity operation.
//! This owns only known bytes; the rest of the player remains uninitialized.
use super::super::{base_handler, identity::PlayerId};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct PendingReset {
    player: PlayerId,
    bytes: BTreeMap<u16, u8>,
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
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.bytes.len() as u32);
        for (&offset, &byte) in &self.bytes {
            emit(u32::from(offset));
            emit(u32::from(byte));
        }
    }
}
