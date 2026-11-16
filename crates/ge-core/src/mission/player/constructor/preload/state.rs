use super::*;
fn optional(value: Option<u32>, emit: &mut impl FnMut(u32)) {
    emit(u32::from(value.is_some()));
    if let Some(value) = value {
        emit(value);
    }
}
impl Request {
    pub fn visit_state_words(self, mut emit: impl FnMut(u32)) {
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        self.receiver.visit_state_words(&mut emit);
        for n in [self.ordinal, self.source_ordinal] {
            emit((n as u64 >> 32) as u32);
            emit(n as u32);
        }
        emit(self.weapon);
        emit(u32::from(self.alternate));
        emit(u32::from(self.variant));
    }
}
impl Call {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.request.visit_state_words(&mut emit);
        emit(self.stage as u32);
        optional(self.mapping.map(|m| m.output), &mut emit);
        optional(self.mapping.map(|m| u32::from(m.accepted)), &mut emit);
        emit(u32::from(self.raw_cursor));
        optional(self.raw_flag.map(u32::from), &mut emit);
        for n in [
            self.raw_flags_read,
            self.raw_keys_read,
            self.entry_keys_read,
        ] {
            emit(n as u32);
        }
        optional(self.raw_index.map(u32::from), &mut emit);
        emit(u32::from(self.entry_cursor));
        optional(self.entry_index.map(u32::from), &mut emit);
        optional(self.selected_key, &mut emit);
        for b in [
            self.raw_indices_written,
            self.occupied_written,
            self.weapon_written,
            self.mapping_written,
            self.invalidated,
            self.extraction_returned,
        ] {
            emit(u32::from(b));
        }
        optional(self.reference, &mut emit);
        optional(self.return_word, &mut emit);
        optional(self.return_reason.map(|r| r as u32), &mut emit);
        emit(u32::from(self.temporary_completed.is_some()));
        if let Some(identity) = self.temporary_completed {
            identity.visit_state_words(&mut emit);
        }
    }
}
impl Pending {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit((self.lease.sequence >> 32) as u32);
        emit(self.lease.sequence as u32);
        self.receiver.visit_state_words(&mut emit);
        emit(self.stage as u32);
        optional(self.entry_flag.map(u32::from), &mut emit);
        emit(u32::from(self.raw_flags_written));
        emit(u32::from(self.entry_keys_written));
        emit(u32::from(self.registry_count.is_some()));
        if let Some(count) = self.registry_count {
            emit(count as u32);
        }
        for n in [self.row, self.enabled_rows, self.row_flags.len()] {
            emit(n as u32);
        }
        for flag in &self.row_flags {
            emit(u32::from(*flag));
        }
        optional(self.weapon, &mut emit);
        emit(u32::from(self.call_in_row));
        emit(self.completed.len() as u32);
        for call in &self.completed {
            call.visit_state_words(&mut emit);
        }
        emit(u32::from(self.current.is_some()));
        if let Some(call) = &self.current {
            call.visit_state_words(&mut emit);
        }
        emit(self.weapon_reads as u32);
        emit(u32::from(self.flag_cleared));
    }
}
