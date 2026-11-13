use super::*;
impl Header {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(4);
        self.owner.visit_state_words(&mut emit);
        emit(self.file);
        emit(self.reference);
        emit(self.stage as u32);
        emit(u32::from(self.slot.is_some()));
        if let Some(slot) = self.slot {
            emit(u32::from(slot));
        }
        for byte in &self.bytes {
            emit(u32::from(byte.is_some()));
            if let Some(byte) = byte {
                emit(u32::from(*byte));
            }
        }
        emit(u32::from(self.observed_generation.is_some()));
        if let Some(generation) = self.observed_generation {
            emit(generation);
        }
        emit(u32::from(self.cached.is_some()));
        if let Some(binding) = self.cached {
            emit(((binding.directory_index as u64) >> 32) as u32);
            emit(binding.directory_index as u32);
            emit(binding.hash);
        }
        emit(match self.body_bound {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
        // Typed pointer roles: owner resource; transition/rate self storage;
        // embedded clip reference and phase storage. No native addresses.
        for role in [1, 0x34, 0x3c, 0x40, 0x64] {
            emit(role);
        }
        self.visit_initialization(&mut emit);
        self.visit_position(&mut emit);
        emit((self.update_count >> 32) as u32);
        emit(self.update_count as u32);
        emit(self.updates.len() as u32);
        for update in &self.updates {
            update.visit_state_words(&mut emit);
        }
    }
}
impl Store {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.owner.visit_state_words(&mut emit);
        emit(((self.maximum as u64) >> 32) as u32);
        emit(self.maximum as u32);
        emit(self.headers.len() as u32);
        for (id, header) in &self.headers {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
            header.visit_state_words(&mut emit);
        }
    }
}
