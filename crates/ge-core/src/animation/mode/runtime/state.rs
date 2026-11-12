use super::*;
impl Cursor {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(emit: &mut impl FnMut(u32), value: usize) {
            emit(((value as u64) >> 32) as u32);
            emit(value as u32);
        }
        fn optional(emit: &mut impl FnMut(u32), value: Option<usize>) {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                wide(emit, value);
            }
        }
        emit(1);
        emit(self.phase as u32);
        for value in [
            self.inputs.requested,
            self.inputs.previous,
            self.inputs.fallback,
            u32::from(self.inputs.resource_available),
        ] {
            emit(value);
        }
        for value in [
            self.budget.lookups,
            self.budget.transition_rows,
            self.budget.commands,
            self.budget.groups,
            self.budget.children,
            self.work.lookups,
            self.work.transition_rows,
            self.work.commands,
            self.work.groups,
            self.work.children,
            self.scan,
            self.transitions,
        ] {
            wide(&mut emit, value);
        }
        optional(&mut emit, self.target);
        wide(&mut emit, self.attempts.len());
        for attempt in &self.attempts {
            emit(match attempt.kind {
                AttemptKind::Previous => 0,
                AttemptKind::Fallback => 1,
            });
            optional(&mut emit, attempt.source_index);
            emit(u32::from(attempt.source_hash.is_some()));
            if let Some(hash) = attempt.source_hash {
                emit(hash);
            }
            optional(&mut emit, attempt.matched_transition);
            optional(&mut emit, attempt.command_count);
            wide(&mut emit, attempt.completed);
        }
        emit(u32::from(self.pending.is_some()));
        if let Some(command) = self.pending {
            for value in [
                command.attempt,
                command.mode,
                command.transition,
                command.ordinal,
            ] {
                wide(&mut emit, value);
            }
        }
        emit(u32::from(self.returned.is_some()));
        if let Some(value) = self.returned {
            emit(value);
        }
    }
}
