use super::*;
impl FirstPersonDispatch {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.receiver.visit_state_words(&mut emit);
        emit(self.stage as u32);
        emit(self.generic);
        emit(self.requested);
        emit(u32::from(self.initial_heads_empty));
        emit(u32::from(self.fallback.is_some()));
        if let Some((word, writes)) = self.fallback {
            emit(word);
            emit((writes >> 32) as u32);
            emit(writes as u32);
        }
        emit(u32::from(self.primary_head.is_some()));
        if let Some((head, word)) = self.primary_head {
            emit(u32::from(head.is_some()));
            if let Some(id) = head {
                emit((id.get() >> 32) as u32);
                emit(id.get() as u32);
            }
            emit(word);
        }
        emit(u32::from(self.selector.is_some()));
        if let Some(c) = &self.selector {
            c.visit_state_words(&mut emit);
        }
        emit(u32::from(self.command.is_some()));
        if let Some(c) = &self.command {
            c.visit_state_words(&mut emit);
        }
        emit(self.completed_commands.len() as u32);
        for c in &self.completed_commands {
            c.visit_state_words(&mut emit);
        }
        emit(u32::from(self.setter.is_some()));
        if let Some(s) = self.setter {
            emit(s.return_word);
            emit(u32::from(s.history_updated));
            emit(s.primary_changed.map_or(0, |v| 1 + u32::from(v)));
        }
        emit(self.coordinate_written.map_or(0, |v| 1 + u32::from(v)));
    }
}
