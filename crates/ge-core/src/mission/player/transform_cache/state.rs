use super::*;

pub(super) fn u64_words(value: u64, emit: &mut impl FnMut(u32)) {
    emit((value >> 32) as u32);
    emit(value as u32);
}
pub(super) fn usize_words(value: usize, emit: &mut impl FnMut(u32)) {
    u64_words(value as u64, emit);
}
fn slot(value: Option<Slot>, emit: &mut impl FnMut(u32)) {
    emit(value.map_or(0, |slot| 1 + slot as u32));
}
fn identity(value: Option<super::super::components::Identity>, emit: &mut impl FnMut(u32)) {
    emit(u32::from(value.is_some()));
    if let Some(value) = value {
        value.visit_state_words(emit);
    }
}
fn ids(values: &[BackingId], emit: &mut impl FnMut(u32)) {
    usize_words(values.len(), emit);
    for value in values {
        u64_words(value.get(), emit);
    }
}

impl BuildTrace {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(self.slot as u32);
        identity(Some(self.receiver), &mut emit);
        emit(self.stage as u32);
        emit(u32::from(self.bone_count.is_some()));
        if let Some(count) = self.bone_count {
            emit(count);
        }
        for count in [
            self.roots_completed,
            self.descendant_visits,
            self.descendant_bytes,
            self.offsets_written,
        ] {
            usize_words(count, &mut emit);
        }
        ids(&self.allocations, &mut emit);
        ids(&self.releases, &mut emit);
        for flag in [self.waiting_skeleton, self.failed, self.returned] {
            emit(u32::from(flag));
        }
    }
}
impl Trace {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        u64_words(self.ordinal, &mut emit);
        emit(self.stage as u32);
        for receiver in self.receivers {
            identity(receiver, &mut emit);
        }
        slot(self.selected_before, &mut emit);
        for build in &self.builds {
            emit(u32::from(build.is_some()));
            if let Some(build) = build {
                build.visit_state_words(&mut emit);
            }
        }
        for flag in [
            self.missing_receiver_skip,
            self.reused_selected,
            self.selection_returned,
        ] {
            emit(u32::from(flag));
        }
        slot(self.selected_after, &mut emit);
        emit(u32::from(self.failed));
        emit(u32::from(self.returned));
    }
}
impl Owner {
    /// Typed state only. In particular no unwritten allocation byte is read.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        slot(self.selected, &mut emit);
        u64_words(self.next_backing, &mut emit);
        u64_words(self.invocation_count, &mut emit);
        for header in &self.headers {
            emit(header.count);
            emit(u32::from(header.association.is_some()));
            if let Some(player) = header.association {
                u64_words(player.get(), &mut emit);
            }
            u64_words(header.invalidation_count_reads, &mut emit);
            u64_words(header.invalidation_pointer_reads, &mut emit);
            for backing in header.backings() {
                emit(u32::from(backing.is_some()));
                if let Some(backing) = backing {
                    backing.visit(&mut emit);
                }
            }
        }
        emit(u32::from(self.trace.is_some()));
        if let Some(trace) = &self.trace {
            trace.visit_state_words(&mut emit);
        }
    }
}

impl Snapshot {
    /// Hash the captured receipt, independently of the owner's later changes.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        slot(self.selected, &mut emit);
        for count in self.counts {
            emit(count);
        }
        for backing in self.matrix_backings {
            emit(u32::from(backing.is_some()));
            if let Some(backing) = backing {
                u64_words(backing.get(), &mut emit);
            }
        }
        for association in self.associations {
            emit(u32::from(association.is_some()));
            if let Some(association) = association {
                u64_words(association.get(), &mut emit);
            }
        }
        usize_words(self.native_bytes, &mut emit);
        usize_words(self.allocations, &mut emit);
        u64_words(self.invocation_count, &mut emit);
    }
}
