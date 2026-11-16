use super::*;
impl CallBinding {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        self.request.visit_state_words(&mut emit);
        emit(u32::from(self.entry));
        emit(self.mapped);
        emit(self.reference);
        self.temporary.visit_state_words(&mut emit);
    }
}
fn stage_word(stage: Stage) -> u32 {
    match stage {
        Stage::Hash => 0,
        Stage::Lookup => 1,
        Stage::Body => 2,
        Stage::Tag => 3,
        Stage::Store(step) => 0x10 + step as u32,
        Stage::Done => 4,
        Stage::Failed => 5,
        Stage::Aborted => 6,
    }
}
impl BoneOutputs {
    /// Counts and outcomes only; copied lanes stay in the Array34 owner.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.binding.visit_state_words(&mut emit);
        emit(stage_word(self.stage));
        emit(u32::from(self.iteration));
        emit(u32::from(self.hash_cursor));
        for count in [
            self.hash_reads,
            self.lookups,
            self.body_reads,
            self.tag_walks,
            self.stores,
        ] {
            emit(count);
        }
        // Internal hash inputs: the captured supplied hash, the primary-bound
        // descriptor (file/reference/generation) and the decided bone. These
        // distinguish paused states; public evidence must not expose them raw.
        emit(u32::from(self.current.is_some()));
        if let Some(lookup) = self.current {
            emit(lookup.hash);
            emit(u32::from(lookup.descriptor.is_some()));
            if let Some(descriptor) = lookup.descriptor {
                emit(descriptor.file);
                emit(descriptor.reference);
                emit(descriptor.generation);
            }
            emit(u32::from(lookup.paused));
        }
        emit(u32::from(self.found.is_some()));
        if let Some(bone) = self.found {
            emit(u32::from(bone));
        }
        for outcome in self.outcomes {
            match outcome {
                None => emit(0),
                Some(Outcome::Missing(missing)) => emit(1 + missing as u32),
                Some(Outcome::Found { bone }) => {
                    emit(8);
                    emit(u32::from(bone));
                }
            }
        }
    }
}
