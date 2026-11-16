//! The reached primary-skin lookup over the same wrapper that bound the
//! temporary skeleton. No target request, topology or cached-tail access.
use super::*;
use ge_core::mission::player::components::{
    ResourceBinding,
    temporary::prepare::{SkinBody, SkinDescriptor, SkinLookup, SkinSource},
};

pub(crate) struct EvaluationSkin<'a> {
    source: &'a FileBinding,
}
impl FileBinding {
    pub(crate) fn evaluation_skin(&self) -> EvaluationSkin<'_> {
        EvaluationSkin { source: self }
    }
}
impl EvaluationSkin<'_> {
    fn validate_owner(&self, primary: ResourceBinding) -> Result<()> {
        ensure!(
            primary.file == self.source.file(),
            "evaluation primary skin wrapper differs"
        );
        Ok(())
    }
}
impl SkinSource for EvaluationSkin<'_> {
    fn lookup(&mut self, primary: ResourceBinding) -> Result<SkinLookup> {
        self.validate_owner(primary)?;
        if (primary.reference >> 24) & 0x7f != 0x0d {
            return Ok(SkinLookup::Pending); // real remapping provider required
        }
        Ok(
            match self.source.source.base_skin_reference(primary.reference)? {
                None => SkinLookup::Absent,
                Some(_) => SkinLookup::Present(SkinDescriptor {
                    file: primary.file,
                    reference: primary.reference,
                    generation: self.source.source.generation,
                }),
            },
        )
    }
    fn generation(&mut self, primary: ResourceBinding) -> Result<u32> {
        self.validate_owner(primary)?;
        Ok(self.source.source.generation)
    }
    fn body(&mut self, descriptor: SkinDescriptor) -> Result<SkinBody> {
        self.validate_owner(ResourceBinding {
            file: descriptor.file,
            reference: descriptor.reference,
        })?;
        ensure!(
            descriptor.generation == self.source.source.generation,
            "evaluation skin generation changed"
        );
        let binding = self
            .source
            .source
            .base_skin_reference(descriptor.reference)?
            .context("evaluation skin descriptor disappeared")?;
        if binding.raw_section() & 0x7fff != 0 {
            return Ok(SkinBody::Pending);
        }
        let body = self
            .source
            .source
            .base_skin_body(descriptor.reference)?
            .context("evaluation skin body disappeared")?;
        ensure!(
            body.file() == descriptor.file
                && body.reference() == descriptor.reference
                && body.directory_index() == binding.directory_index()
                && body.stored_hash() == binding.hash(),
            "evaluation skin body binding changed"
        );
        Ok(SkinBody::Resident)
    }
}

#[cfg(test)]
#[path = "skin_evaluation/tests.rs"]
mod tests;
