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
    extra_skins: [u32; 2],
    extra_textures: [u32; 2],
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
            extra_skins: [words[8], words[9]],
            extra_textures: [words[11], words[12]],
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
    /// The callback validates the full top byte and nonzero low16, independently
    /// of the later raw primary dependency request.
    pub fn callback_skin(&self) -> u32 {
        if self.skin >> 24 == 0x0d && self.skin & 0xffff != 0 {
            self.skin
        } else {
            0x0d00_0001
        }
    }
    pub fn extra_skins(&self) -> [u32; 2] {
        self.extra_skins
    }
    pub fn override_textures(&self) -> [u32; 3] {
        [
            self.texture_raw,
            self.extra_textures[0],
            self.extra_textures[1],
        ]
    }
    pub fn texture_request(&self) -> TextureRequest {
        if self.texture_raw == u32::MAX {
            TextureRequest::EnumerateSkin
        } else {
            TextureRequest::Explicit(self.texture_raw)
        }
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(2);
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
        for word in self.extra_skins.into_iter().chain(self.extra_textures) {
            emit(word);
        }
    }
}

#[cfg(test)]
#[path = "tests/visual.rs"]
mod tests;
