use super::super::constructor::CharacterBinding;
use crate::mission::actor_definitions::{Record, RecordId};
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureRequest {
    EnumerateSkin,
    Explicit(u32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visual {
    character: CharacterBinding,
    record: RecordId,
    source_index: usize,
    file: u32,
    skin: u32,
    texture_raw: u32,
}
impl Visual {
    pub fn from_record(character: CharacterBinding, record: &Record) -> Result<Self> {
        ensure!(
            character.registry_resource == 0x0109_0013 && character.key == record.key(),
            "component visual record differs from the actual character key"
        );
        let words = record.words();
        Ok(Self {
            character,
            record: record.id(),
            source_index: record.source_index(),
            file: words[6],
            skin: words[7],
            texture_raw: words[10],
        })
    }
    pub fn character(&self) -> CharacterBinding {
        self.character
    }
    pub fn file(&self) -> u32 {
        self.file
    }
    pub fn skin(&self) -> u32 {
        self.skin
    }
    pub fn texture_request(&self) -> TextureRequest {
        if self.texture_raw == u32::MAX {
            TextureRequest::EnumerateSkin
        } else {
            TextureRequest::Explicit(self.texture_raw)
        }
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.character.registry_resource);
        emit(self.character.key);
        for value in [
            self.character.source_index as u64,
            self.record.get(),
            self.source_index as u64,
        ] {
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        emit(self.file);
        emit(self.skin);
        emit(self.texture_raw);
    }
}
