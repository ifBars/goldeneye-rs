use super::*;
impl Pending {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        fn wide(emit: &mut impl FnMut(u32), value: usize) {
            emit(((value as u64) >> 32) as u32);
            emit(value as u32);
        }
        fn optional(emit: &mut impl FnMut(u32), value: Option<u32>) {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        emit(3);
        emit(self.opcode);
        emit(self.stage as u32);
        optional(&mut emit, self.set_reference);
        for value in [self.set_index, self.group_count] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                wide(&mut emit, value);
            }
        }
        optional(&mut emit, self.options.map(|value| value as u16 as u32));
        optional(&mut emit, self.divisor);
        optional(&mut emit, self.flags.map(u32::from));
        wide(&mut emit, self.groups.len());
        for group in &self.groups {
            emit(u32::from(group.source_slot));
            emit(group.effective_slot);
            optional(&mut emit, group.weight_write);
            optional(
                &mut emit,
                group.child_count.map(|value| value as u16 as u32),
            );
        }
        emit(u32::from(self.child.is_some()));
        if let Some(child) = self.child {
            wide(&mut emit, child.group);
            wide(&mut emit, child.ordinal);
            emit(u32::from(child.slot));
            emit(child.reference);
            emit(u32::from(child.effective_flags));
            emit(child.divisor);
        }
        optional(&mut emit, self.mask.map(u32::from));
        emit(u32::from(self.flags_or));
        emit(u32::from(self.factory_started));
        emit(u32::from(self.factory_node.is_some()));
        if let Some(id) = self.factory_node {
            emit((id.get() >> 32) as u32);
            emit(id.get() as u32);
        }
        wide(&mut emit, self.effects.len());
        for effect in &self.effects {
            emit((effect.node.get() >> 32) as u32);
            emit(effect.node.get() as u32);
            wide(&mut emit, effect.request.group);
            wide(&mut emit, effect.request.ordinal);
            emit(u32::from(effect.request.slot));
            emit(effect.request.reference);
            emit(u32::from(effect.request.effective_flags));
            emit(effect.request.divisor);
            emit(effect.stage as u32);
            emit(u32::from(effect.inserted));
            emit(u32::from(effect.copied_head.is_some()));
            if let Some(id) = effect.copied_head {
                emit((id.get() >> 32) as u32);
                emit(id.get() as u32);
            }
            optional(&mut emit, effect.copied_bits);
            optional(&mut emit, effect.post_word);
        }
    }
}
