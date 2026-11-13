use super::*;
pub(in super::super) fn visit_binding(binding: Option<Binding>, emit: &mut impl FnMut(u32)) {
    emit(u32::from(binding.is_some()));
    if let Some(binding) = binding {
        emit(((binding.directory_index as u64) >> 32) as u32);
        emit(binding.directory_index as u32);
    }
}
impl Header {
    pub(in super::super) fn visit_position(&self, emit: &mut impl FnMut(u32)) {
        emit(u32::from(self.position.is_some()));
        if let Some(t) = &self.position {
            emit(u32::from(t.index));
            for value in [
                t.file,
                t.generation,
                t.timing_flags,
                t.animation_fps.map(u32::from),
                t.script_fps_bits,
                t.frame_bits,
                t.position_bits,
                t.previous_position_bits,
            ] {
                emit(u32::from(value.is_some()));
                if let Some(value) = value {
                    emit(value);
                }
            }
            visit_binding(t.binding, emit);
            emit(match t.body_bound {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            });
            emit(u32::from(t.root_motion_pending));
        }
    }
}
