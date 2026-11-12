use super::*;
use ge_core::animation::script_child::{Binding, Provider};
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
}
impl FileBinding {
    pub(crate) fn shared_script_source(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
        }
    }
}
