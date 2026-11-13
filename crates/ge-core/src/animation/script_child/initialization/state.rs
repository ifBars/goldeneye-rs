use super::*;
impl Header {
    pub(in super::super) fn visit_initialization(&self, emit: &mut impl FnMut(u32)) {
        emit(u32::from(self.initialization.is_some()));
        if let Some(t) = &self.initialization {
            emit(t.inputs.component_speed_bits);
            emit(t.inputs.scaled_step_bits);
            emit(((t.inputs.maximum_entries as u64) >> 32) as u32);
            emit(t.inputs.maximum_entries as u32);
            for value in [
                t.delta_bits,
                t.duration_raw,
                t.frame_bits,
                t.selected_offset,
                t.selected_start.map(|v| v as i32 as u32),
                t.compared_reference,
                t.pending_file,
                t.initial_index_raw.map(u32::from),
                t.initial_index_clamped.map(u32::from),
                t.frame_range.map(|v| v as u32),
                t.flags_raw.map(u32::from),
            ] {
                emit(u32::from(value.is_some()));
                if let Some(value) = value {
                    emit(value);
                }
            }
            emit(t.entries_visited as u32);
            emit(u32::from(t.requested.is_some()));
            if let Some(counts) = t.requested {
                for count in counts {
                    emit(u32::from(count));
                }
            }
            for value in t.rows_visited {
                emit(value as u32);
            }
            for value in t.bound {
                emit(u32::from(value));
            }
            emit(u32::from(t.setup_returned));
        }
        match &self.clips {
            None => emit(0),
            Some(Storage::Inline { file }) => {
                emit(1);
                visit_file(*file, emit);
            }
            Some(Storage::Heap(records)) => {
                emit(2);
                emit(records.len() as u32);
                for record in records {
                    visit_file(record.file, emit);
                    for byte in record.bytes {
                        emit(u32::from(byte.is_some()));
                        if let Some(byte) = byte {
                            emit(u32::from(byte));
                        }
                    }
                }
            }
        }
    }
}
fn visit_file(file: Option<u32>, emit: &mut impl FnMut(u32)) {
    emit(u32::from(file.is_some()));
    if let Some(file) = file {
        emit(file);
    }
}
