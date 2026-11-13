use super::super::position::Binding;
use super::*;
impl Header {
    pub fn clip_file(&self, index: usize) -> Option<u32> {
        match self.clips.as_ref()? {
            Storage::Inline { file, .. } if index == 0 => *file,
            Storage::Inline { .. } => None,
            Storage::Heap(records) => records.get(index)?.file,
        }
    }
    pub fn clip_cached(&self, index: usize) -> Option<Binding> {
        match self.clips.as_ref()? {
            Storage::Inline { cached, .. } if index == 0 => *cached,
            Storage::Inline { .. } => None,
            Storage::Heap(records) => records.get(index)?.cached,
        }
    }
    pub(in super::super) fn set_clip_cache(
        &mut self,
        index: usize,
        binding: Option<Binding>,
    ) -> Result<()> {
        match self.clips.as_mut().context("clip storage absent")? {
            Storage::Inline { cached, .. } => {
                ensure!(index == 0, "inline clip cache index differs");
                *cached = binding;
                self.bytes[0x4c..0x50].fill(None);
            }
            Storage::Heap(records) => {
                let record = records
                    .get_mut(index)
                    .context("clip cache index exceeds allocation")?;
                record.cached = binding;
                record.bytes[0xc..0x10].fill(None);
            }
        }
        if binding.is_none() {
            self.clip_write(index, 0xc, &0u32.to_be_bytes())?;
        }
        Ok(())
    }
}
