use super::*;
use ge_core::animation::script_child::{Binding, Provider};
mod forward_tick;
mod phase_zero;
mod position;
impl Provider for FileBinding {
    fn file(&self) -> u32 {
        self.source.header.hash
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(self.source.generation)
    }
    fn resolve_script(&mut self, reference: u32) -> Result<Option<Binding>> {
        Ok(self
            .source
            .base_animation_script_reference(reference)?
            .map(|descriptor| Binding {
                directory_index: descriptor.directory_index,
                hash: descriptor.hash,
            }))
    }
    fn bind_body(&mut self, binding: Binding) -> Result<bool> {
        Ok(self
            .source
            .base_animation_script_body(binding.directory_index)?
            .is_some())
    }
    fn body_state(
        &mut self,
        binding: Binding,
    ) -> Result<ge_core::animation::script_child::BodyState> {
        use ge_core::animation::script_child::BodyState;
        ensure!(
            binding.directory_index < 32767,
            "script body directory index exceeds native bound"
        );
        let descriptor = self
            .source
            .base_animation_script_reference(0x8400_0000 | u32::try_from(binding.directory_index)?)?
            .context("cached script descriptor disappeared")?;
        ensure!(
            descriptor.hash == binding.hash,
            "cached script descriptor identity changed"
        );
        if descriptor.section & 0x7fff != 0 {
            // A retained full file does not establish this logical section's
            // publication or the native descriptor's runtime body pointer.
            // The getter itself requests no load, so retain that dependency.
            return Ok(BodyState::Pending);
        }
        Ok(if self.bind_body(binding)? {
            BodyState::Resident
        } else {
            BodyState::Absent
        })
    }
}
impl FileBinding {
    pub(crate) fn shared_script_source(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
        }
    }
}
