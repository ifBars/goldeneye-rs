use super::*;
fn head_words(head: PostBindHead, emit: &mut impl FnMut(u32)) {
    let (kind, node) = match head {
        PostBindHead::Null => (0, None),
        PostBindHead::ClassFour(node) => (1, Some(node)),
        PostBindHead::NonMatching(node) => (2, Some(node)),
        PostBindHead::Unknown => (3, None),
    };
    emit(kind);
    if let Some(node) = node {
        emit((node.get() >> 32) as u32);
        emit(node.get() as u32);
    }
}
impl PostBind {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.request.visit_state_words(&mut emit);
        self.temporary.visit_state_words(&mut emit);
        emit(self.stage() as u32);
        emit(u32::from(self.head.is_some()));
        if let Some(head) = self.head {
            head_words(head, &mut emit);
        }
        emit(self.head_reads);
        emit(u32::from(self.parameter.is_some()));
        if let Some(parameter) = &self.parameter {
            parameter.visit_state_words(&mut emit);
        }
        emit(u32::from(self.capture.is_some()));
        if let Some(capture) = &self.capture {
            capture.visit_state_words(&mut emit);
        }
        emit(u32::from(self.phase.is_some()));
        if let Some(phase) = &self.phase {
            phase.visit_state_words(&mut emit);
        }
        match self.assignment {
            None => emit(0),
            Some(PhaseAssignment::Deferred) => emit(1),
            Some(PhaseAssignment::Known(bits)) => {
                emit(2);
                emit(bits);
            }
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
