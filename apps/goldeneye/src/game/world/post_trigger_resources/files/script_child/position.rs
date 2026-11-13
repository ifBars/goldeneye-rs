use super::*;
use ge_assets::skeletal_animation::live::{BodyState, LiveBody};
use ge_core::animation::script_child::position::{Binding as AnimationBinding, Body, Source};
pub(crate) struct PositionSource<'a> {
    owner: &'a LoadedFile,
    script: ge_assets::frontend_script::Script<'a>,
    body: Option<(AnimationBinding, LiveBody<'a>)>,
}
impl FileBinding {
    pub(crate) fn script_position_source(&self, binding: Binding) -> Result<PositionSource<'_>> {
        Ok(PositionSource {
            owner: &self.source,
            script: self
                .source
                .base_animation_script_body(binding.directory_index)?
                .context("position script body disappeared")?,
            body: None,
        })
    }
}
impl Source for PositionSource<'_> {
    fn file(&self) -> u32 {
        self.owner.header.hash
    }
    fn generation(&mut self, file: u32) -> Result<Option<u32>> {
        Ok((file == self.owner.header.hash).then_some(self.owner.generation))
    }
    fn resolve(&mut self, file: u32, reference: u32) -> Result<Option<AnimationBinding>> {
        ensure!(
            file == self.owner.header.hash,
            "animation metadata file owner differs"
        );
        Ok(self
            .owner
            .base_animation_binding(reference)?
            .map(|b| AnimationBinding {
                directory_index: b.directory_index,
            }))
    }
    fn body(&mut self, file: u32, binding: AnimationBinding) -> Result<Body> {
        ensure!(
            file == self.owner.header.hash,
            "animation body file owner differs"
        );
        Ok(
            match self.owner.base_animation_body(binding.directory_index)? {
                BodyState::Absent => Body::Absent,
                BodyState::NotResident => Body::Pending,
                BodyState::Resident(body) => {
                    self.body = Some((binding, body));
                    Body::Resident
                }
            },
        )
    }
    fn timing_flags(&mut self, offset: u32) -> Result<u32> {
        self.script.play_timing_flags(offset)
    }
    fn animation_fps(&mut self, file: u32, binding: AnimationBinding) -> Result<u8> {
        let (cached, body) = self.body.context("animation FPS precedes body binding")?;
        ensure!(
            file == self.owner.header.hash && cached == binding,
            "animation FPS owner differs"
        );
        body.frames_per_second()
    }
    fn script_fps(&mut self) -> Result<u32> {
        self.script.frames_per_second_raw()
    }
}
