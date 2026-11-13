use super::*;
use ge_assets::animation_modes::{CommandStream, View};
use ge_core::animation::layers::command::Source;
pub(crate) struct CommandSource<'a> {
    view: View<'a>,
    stream: &'a mut CommandStream,
}
impl FileBinding {
    pub(crate) fn mode_command_source<'a>(
        &'a self,
        stream: &'a mut CommandStream,
    ) -> Result<CommandSource<'a>> {
        Ok(CommandSource {
            view: self.source.base_animation_modes()?,
            stream,
        })
    }
}
impl Source for CommandSource<'_> {
    fn set_reference(&mut self) -> Result<u32> {
        self.stream.set_reference(&self.view)
    }
    fn resolve_set(&mut self, reference: u32) -> Result<Option<usize>> {
        self.stream.resolve_set(&self.view, reference)
    }
    fn options(&mut self) -> Result<i16> {
        self.stream.options(&self.view)
    }
    fn divisor_override(&mut self) -> Result<u32> {
        self.stream.divisor_override(&self.view)
    }
    fn flags_override(&mut self) -> Result<u16> {
        self.stream.flags_override(&self.view)
    }
    fn begin_set(&mut self, index: usize) -> Result<usize> {
        self.stream.begin_set(&self.view, index)
    }
    fn group_slot(&mut self) -> Result<u16> {
        self.stream.group_slot(&self.view)
    }
    fn group_weight(&mut self) -> Result<u32> {
        self.stream.group_weight(&self.view)
    }
    fn child_count(&mut self) -> Result<i16> {
        self.stream.child_count(&self.view)
    }
    fn child_divisor(&mut self, ordinal: usize) -> Result<u16> {
        self.stream.child_divisor(&self.view, ordinal)
    }
    fn child_flags(&mut self, ordinal: usize) -> Result<u16> {
        self.stream.child_flags(&self.view, ordinal)
    }
    fn child_reference(&mut self, ordinal: usize) -> Result<u32> {
        self.stream.child_reference(&self.view, ordinal)
    }
    fn child_word(&mut self, ordinal: usize) -> Result<u32> {
        self.stream.child_word(&self.view, ordinal)
    }
    fn next_group(&mut self, children: u16) -> Result<()> {
        self.stream.next_group(children)
    }
}
