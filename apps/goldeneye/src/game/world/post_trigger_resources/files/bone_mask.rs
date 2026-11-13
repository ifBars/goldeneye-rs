use super::*;
use ge_assets::skeletal_animation::live::BodyState;
use ge_core::mission::player::actions::suffix::{MaskLookup, MaskValue};

impl FileBinding {
    pub(crate) fn animation_bone_mask(&self, reference: u32) -> Result<MaskLookup> {
        let Some(binding) = self.source.base_animation_binding(reference)? else {
            return Ok(MaskLookup::MissingDescriptor);
        };
        Ok(
            match self.source.base_animation_body(binding.directory_index)? {
                BodyState::Absent => MaskLookup::MissingBody,
                BodyState::NotResident => MaskLookup::Pending,
                BodyState::Resident(body) => {
                    let mask = body.bone_mask()?;
                    MaskLookup::Resident(MaskValue::new(
                        binding.directory_index,
                        mask.attribute_present,
                        mask.words,
                    ))
                }
            },
        )
    }
}
