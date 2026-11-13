use super::*;
impl Header {
    pub(in crate::animation::script_child) fn resolve_position_animation(
        &mut self,
        source: &mut impl Source,
        t: &mut Trace,
    ) -> Result<bool> {
        let index = usize::from(t.index);
        if let Some(file) = self.clip_file(index) {
            t.file = Some(file);
            let Some(generation) = source.generation(file)? else {
                self.stage = Stage::AwaitingAnimationOwner;
                return Ok(false);
            };
            t.generation = Some(generation);
            if self.clip_word(index, 8) != Some(generation) {
                let reference = self.clip_word(index, 4).context("clip reference unknown")?;
                let binding = source.resolve(file, reference)?;
                self.set_clip_cache(index, binding)?;
                if binding.is_some() {
                    self.clip_write(index, 8, &generation.to_be_bytes())?;
                }
            }
            t.binding = self.clip_cached(index);
            if t.binding.is_none() && self.clip_word(index, 0xc) != Some(0) {
                self.stage = Stage::AwaitingClipCache;
                return Ok(false);
            }
        } else if self.clip_word(index, 0) == Some(0) {
            t.file = Some(self.file);
            let mut reference = self
                .clip_word(index, 0x10)
                .context("local clip reference unknown")?;
            if reference == u32::MAX {
                reference = self
                    .clip_word(index, 4)
                    .context("serialized local clip reference unknown")?;
                self.clip_write(index, 0x10, &reference.to_be_bytes())?;
                if reference != u32::MAX
                    && reference & 0x8000_0000 != 0
                    && let Some(binding) = source.resolve(self.file, reference)?
                {
                    reference = (reference & 0x7f00_0000)
                        | 0x8000_0000
                        | u32::try_from(binding.directory_index)?;
                    self.clip_write(index, 0x10, &reference.to_be_bytes())?;
                }
            }
            t.binding = source.resolve(self.file, reference)?;
        } else {
            self.stage = Stage::AwaitingClipCache;
            return Ok(false);
        }
        let Some(binding) = t.binding else {
            t.body_bound = Some(false);
            return Ok(true);
        };
        match source.body(t.file.context("animation file absent")?, binding)? {
            Body::Absent => t.body_bound = Some(false),
            Body::Resident => t.body_bound = Some(true),
            Body::Pending => {
                self.stage = Stage::AwaitingAnimationBody;
                return Ok(false);
            }
        }
        Ok(true)
    }
}
