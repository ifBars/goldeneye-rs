//! Six-bone providers reuse the actual temporary primary-skin wrapper and the
//! retained supplied DATA window. Only the reached hash/tag fields are read.
use super::*;
use ge_assets::supplied_data::BoneTagTable;
use ge_core::mission::player::{
    components::{
        ResourceBinding,
        temporary::{SkinBody, SkinDescriptor, SkinLookup, SkinSource},
    },
    constructor::preload::bone_outputs::{BoneTagSource, TagWalk},
};

pub(crate) struct BoneOutputSource<'a> {
    skin: &'a FileBinding,
    hashes: BoneTagTable,
}
impl FileBinding {
    pub(crate) fn bone_output_source(&self, hashes: BoneTagTable) -> BoneOutputSource<'_> {
        BoneOutputSource { skin: self, hashes }
    }
}
impl SkinSource for BoneOutputSource<'_> {
    fn lookup(&mut self, primary: ResourceBinding) -> Result<SkinLookup> {
        self.skin.evaluation_skin().lookup(primary)
    }
    fn generation(&mut self, primary: ResourceBinding) -> Result<u32> {
        self.skin.evaluation_skin().generation(primary)
    }
    fn body(&mut self, descriptor: SkinDescriptor) -> Result<SkinBody> {
        self.skin.evaluation_skin().body(descriptor)
    }
}
impl BoneTagSource for BoneOutputSource<'_> {
    fn hash(&mut self, index: u8) -> Result<u32> {
        self.hashes.hash(index)
    }
    fn tag(&mut self, descriptor: SkinDescriptor, hash: u32) -> Result<TagWalk> {
        ensure!(
            descriptor.file == self.skin.file()
                && descriptor.generation == self.skin.source.generation,
            "bone tag descriptor wrapper or generation changed"
        );
        if (descriptor.reference >> 24) & 0x7f != 0x0d {
            return Ok(TagWalk::Pending);
        }
        let binding = self
            .skin
            .source
            .base_skin_reference(descriptor.reference)?
            .context("bone tag skin descriptor disappeared")?;
        if binding.raw_section() & 0x7fff != 0 {
            return Ok(TagWalk::Pending);
        }
        let body = self
            .skin
            .source
            .base_skin_body(descriptor.reference)?
            .context("bone tag skin body disappeared")?;
        ensure!(
            body.file() == descriptor.file
                && body.reference() == descriptor.reference
                && body.directory_index() == binding.directory_index()
                && body.stored_hash() == binding.hash(),
            "bone tag body identity changed"
        );
        Ok(match self.skin.source.base_skin_tag(&body, hash, 4096)? {
            Some(tag) => TagWalk::Found {
                bone: tag.bone_index,
            },
            None => TagWalk::Sentinel,
        })
    }
}
