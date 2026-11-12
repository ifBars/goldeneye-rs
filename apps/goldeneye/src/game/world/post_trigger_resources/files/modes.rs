//! Live mode reads stay inside the existing logically resident base owner.
use super::*;
use ge_core::animation::mode::{
    Mode,
    runtime::{Invocation, Provider},
};
pub(crate) struct ModeSource<'a> {
    view: ge_assets::animation_modes::View<'a>,
}
impl Provider for ModeSource<'_> {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>> {
        self.view.resolve_mode(reference)
    }
    fn mode(&mut self, index: usize) -> Result<Mode> {
        let descriptor = self.view.mode(index)?;
        Ok(Mode {
            hash: descriptor.hash,
            transition_count: usize::try_from(descriptor.count)?,
        })
    }
    fn transition_target(&mut self, index: usize, row: usize) -> Result<u32> {
        self.view.transition_target(index, row)
    }
    fn command_count(&mut self, index: usize, row: usize) -> Result<usize> {
        self.view.command_count(index, row, 4096)
    }
}
impl FileBinding {
    pub(crate) fn mode_available(&self, reference: u32) -> Result<bool> {
        Ok(self
            .source
            .base_animation_modes()?
            .resolve_mode(reference)?
            .is_some())
    }
    pub(crate) fn mode_source(&self) -> Result<ModeSource<'_>> {
        Ok(ModeSource {
            view: self.source.base_animation_modes()?,
        })
    }
    pub(crate) fn mode_command_location(
        &self,
        command: Invocation,
    ) -> Result<ge_assets::animation_modes::CommandLocation> {
        self.source.base_animation_modes()?.command_location(
            command.mode,
            command.transition,
            command.ordinal,
            4096,
        )
    }
}
