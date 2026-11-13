use super::*;
impl Pending {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(u32::from(self.input_raw));
        emit(self.stage as u32);
        emit(self.dependency.map_or(0, |value| value as u32 + 1));
        for value in [
            self.entry_702.map(u32::from),
            self.blend_ba8.map(u32::from),
            self.tp_enabled.map(u32::from),
            self.fp_enabled.map(u32::from),
            self.session_scale_multiple.map(u32::from),
            self.dirty_before.map(u32::from),
            self.dirty_after.map(u32::from),
            self.mode_afc_absent.map(u32::from),
            self.false_arm_b00_absent.map(u32::from),
            self.attachment_absent.map(u32::from),
            self.weapon_global_byte.map(u32::from),
            self.global_attachment_absent.map(u32::from),
            self.fade_cfc_bits,
            self.fade_argument_bits,
            self.fade_attachment_absent.map(u32::from),
            self.final_session_multiple.map(u32::from),
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        for value in [
            self.bab_written,
            self.scale_stored,
            self.scale_invalidated,
            self.ca4_written,
            self.weapon_global_read,
        ] {
            emit(u32::from(value));
        }
        for (searched, write) in self.receiver_searches.into_iter().zip(self.enable_writes) {
            emit(u32::from(searched));
            emit(u32::from(write.is_some()));
            if let Some(write) = write {
                write.receiver.visit_state_words(&mut emit);
                emit(write.reference);
                emit(u32::from(write.enabled));
                emit(u32::from(write.reference_matched));
                for flags in [write.flags_before, write.flags_after] {
                    emit(u32::from(flags.is_some()));
                    if let Some(flags) = flags {
                        emit(u32::from(flags));
                    }
                }
                emit(u32::from(write.invalidated_8e));
            }
        }
        for request in self.enable_requests {
            emit(u32::from(request.is_some()));
            if let Some((identity, reference)) = request {
                identity.visit_state_words(&mut emit);
                emit(reference);
            }
        }
        emit(u32::from(self.character_binding.is_some()));
        if let Some(binding) = self.character_binding {
            emit(binding.registry_resource);
            emit(binding.key);
            emit((binding.source_index as u64 >> 32) as u32);
            emit(binding.source_index as u32);
        }
        for value in self.character_scalars {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        for value in [self.character_vector, self.scale_bits] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                for word in value {
                    emit(word);
                }
            }
        }
        for value in [self.force_nodes_checked, self.backlink_nodes_checked] {
            emit((value as u64 >> 32) as u32);
            emit(value as u32);
        }
        emit(u32::from(self.user_target.is_some()));
        if let Some(target) = self.user_target {
            emit((target.get() >> 32) as u32);
            emit(target.get() as u32);
        }
        match self.user_lookup {
            None => emit(0),
            Some(UserLookup::NotFound) => emit(1),
            Some(UserLookup::Matched(user)) => {
                emit(2);
                emit((user.get() >> 32) as u32);
                emit(user.get() as u32);
            }
        }
        match self.global_argument {
            None => emit(0),
            Some(GlobalArgument::Known(value)) => {
                emit(1);
                emit(value as i32 as u32);
            }
            Some(GlobalArgument::ArgumentUnreadBecauseReceiverAbsent) => emit(2),
        }
    }
}
