//! Virtual forward tick reads the script's actual duration and rate, even at dt0.
use super::*;
use ge_core::animation::script_child::forward_tick::Source;

impl Source for FileBinding {
    fn duration(&mut self, binding: Binding) -> Result<u32> {
        self.source
            .base_animation_script_body(binding.directory_index)?
            .context("forward tick script body disappeared")?
            .duration_raw()
    }

    fn fps_bits(&mut self, binding: Binding) -> Result<u32> {
        self.source
            .base_animation_script_body(binding.directory_index)?
            .context("forward tick script body disappeared")?
            .frames_per_second_raw()
    }
}
