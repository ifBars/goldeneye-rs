use super::*;
use ge_assets::animation_modes::{ClassFourReference, ExtractionBudget};

impl FileBinding {
    pub(crate) fn preload_class_four_reference(
        &self,
        mapped: u32,
    ) -> Result<Option<ClassFourReference>> {
        self.source
            .base_animation_modes()?
            .first_preload_class_four(
                mapped,
                ExtractionBudget {
                    transitions: 4096,
                    commands: 16384,
                    groups: 4096,
                    children: 16384,
                },
            )
    }
}
