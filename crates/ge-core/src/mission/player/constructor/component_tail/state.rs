use super::*;
impl Pending {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.stage as u32);
        emit(self.dependency.map_or(0, |v| v as u32 + 1));
        for pair in self.rotation_reads {
            emit(u32::from(pair.is_some()));
            if let Some(pair) = pair {
                for word in pair {
                    emit(word);
                }
            }
        }
        for stored in self.rotation_stores {
            emit(u32::from(stored));
        }
        emit(u32::from(self.rotation_invalidated));
        match self.character {
            None => emit(0),
            Some(CharacterLookup::KnownAbsent) => emit(1),
            Some(CharacterLookup::Bound(binding)) => {
                emit(2);
                emit(binding.registry_resource);
                emit(binding.key);
                emit((binding.source_index as u64 >> 32) as u32);
                emit(binding.source_index as u32);
            }
        }
        for word in self.character_scalars {
            emit(u32::from(word.is_some()));
            if let Some(word) = word {
                emit(word);
            }
        }
        emit(u32::from(self.scale_bits.is_some()));
        if let Some(bits) = self.scale_bits {
            for word in bits {
                emit(word);
            }
        }
        for value in [
            self.session_multiple.map(u32::from),
            self.flags_before.map(u32::from),
            self.flags_after.map(u32::from),
            self.helper_return,
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        for stored in self.scale_stores {
            emit(u32::from(stored));
        }
        emit(u32::from(self.scale_invalidated));
    }
}
