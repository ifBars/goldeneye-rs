use super::*;
impl Pending {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.saved_state_704);
        emit(self.stage as u32);
        emit(self.dependency.map_or(0, |v| 1 + v as u32));
        emit(u32::from(self.state_restored));
        for value in [
            self.script_reference,
            self.current_slot.map(|v| v as i32 as u32),
            self.current_record_present.map(u32::from),
            self.current_inventory_key,
            self.current_flags.map(u32::from),
            self.variant.map(u32::from),
            self.actor_absent.map(u32::from),
            self.helper_return,
            self.pose_input_raw.map(u32::from),
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        for read in self.metadata_reads {
            emit(u32::from(read.is_some()));
            if let Some(read) = read {
                emit(u32::from(read.slot));
                emit(read.field as u32);
                emit(read.value);
            }
        }
        for absent in self.cleanup_checks {
            emit(absent.map_or(0, |v| 1 + u32::from(v)));
        }
        emit(u32::from(self.b84_written));
        emit(u32::from(self.bd1_written));
        for removal in self.override_removals {
            emit(u32::from(removal.is_some()));
            if let Some(removal) = removal {
                emit(removal.key);
                emit(removal.count_raw);
                emit(u32::from(removal.removed));
            }
        }
    }
}
