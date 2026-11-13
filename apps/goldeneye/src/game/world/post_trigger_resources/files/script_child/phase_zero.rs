use super::*;
use ge_core::animation::script_child::initialization::{Entry, FileLookup, Source};
pub(crate) struct PhaseSource<'a> {
    file: u32,
    script: ge_assets::frontend_script::Script<'a>,
}
impl FileBinding {
    pub(crate) fn phase_zero_source(&self, binding: Binding) -> Result<PhaseSource<'_>> {
        Ok(PhaseSource {
            file: self.file(),
            script: self
                .source
                .base_animation_script_body(binding.directory_index)?
                .context("retained script body disappeared")?,
        })
    }
}
impl Source for PhaseSource<'_> {
    fn file(&self) -> u32 {
        self.file
    }
    fn duration(&mut self) -> Result<u32> {
        self.script.duration_raw()
    }
    fn entry(&mut self, offset: u32) -> Result<Entry> {
        let value = self.script.selection_entry(offset)?;
        Ok(Entry {
            start: value.start,
            opcode: value.opcode,
            next: value.next,
        })
    }
    fn counts(&mut self, offset: u32) -> Result<[u8; 2]> {
        self.script.clip_counts(offset)
    }
    fn file_token(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32> {
        self.script.clip_file(offset, secondary, row, normal)
    }
    fn lookup_file(&mut self, _: u32) -> Result<FileLookup> {
        // No independent file reload and no fabricated registry miss. The
        // shared file-only registry must supply this operation before resuming.
        Ok(FileLookup::Pending)
    }
    fn reference(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32> {
        self.script.clip_reference(offset, secondary, row, normal)
    }
    fn blend(&mut self, offset: u32, row: usize) -> Result<u32> {
        self.script.clip_blend(offset, row)
    }
    fn start(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16> {
        self.script.clip_start(offset, secondary, row, normal)
    }
    fn end(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16> {
        self.script.clip_end(offset, secondary, row, normal)
    }
    fn initial_index(&mut self, offset: u32) -> Result<u8> {
        self.script.play_initial_index(offset)
    }
    fn flags(&mut self, offset: u32) -> Result<u8> {
        self.script.play_flags(offset)
    }
}
