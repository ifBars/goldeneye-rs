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
        use super::super::publication::ScriptBody;
        Ok(match self.source.script_body(binding)? {
            ScriptBody::Resident(_) => true,
            ScriptBody::Absent => false,
            ScriptBody::Pending => anyhow::bail!("script runtime body publication is pending"),
        })
    }
    fn body_state(
        &mut self,
        binding: Binding,
    ) -> Result<ge_core::animation::script_child::BodyState> {
        use super::super::publication::ScriptBody;
        use ge_core::animation::script_child::BodyState;
        Ok(match self.source.script_body(binding)? {
            ScriptBody::Resident(_) => BodyState::Resident,
            ScriptBody::Absent => BodyState::Absent,
            ScriptBody::Pending => BodyState::Pending,
        })
    }
}
impl FileBinding {
    pub(crate) fn script_publication_evidence(
        &self,
        binding: Binding,
    ) -> Result<Option<super::super::ScriptPublication>> {
        self.source.script_publication_evidence(binding)
    }

    pub(crate) fn shared_script_source(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
        }
    }
}
