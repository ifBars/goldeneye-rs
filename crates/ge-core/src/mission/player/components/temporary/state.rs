use super::*;
impl Pending {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.stage() as u32);
        self.source_receiver.visit_state_words(&mut emit);
        for binding in [self.component, self.skin] {
            emit(binding.file);
            emit(binding.reference);
        }
        self.animator.visit_state_words(&mut emit);
        for id in [self.factory_node, self.head] {
            emit(u32::from(id.is_some()));
            if let Some(id) = id {
                emit((id.get() >> 32) as u32);
                emit(id.get() as u32);
            }
        }
        emit(u32::from(self.factory_returned));
        emit(u32::from(self.forward.is_some()));
        if let Some(trace) = &self.forward {
            trace.visit_state_words(&mut emit);
        }
        emit(u32::from(self.maintenance_skipped));
        emit(u32::from(self.reverse_skipped));
        emit(u32::from(self.refresh.is_some()));
        if let Some(refresh) = &self.refresh {
            refresh.visit_state_words(&mut emit);
        }
        emit(u32::from(self.evaluation.is_some()));
        if let Some(evaluation) = &self.evaluation {
            evaluation.visit_state_words(&mut emit);
        }
        emit(self.prepares.len() as u32);
        for prepare in &self.prepares {
            prepare.visit_state_words(&mut emit);
        }
        emit(self.guards.len() as u32);
        for guard in &self.guards {
            guard.visit_state_words(&mut emit);
        }
    }
}
