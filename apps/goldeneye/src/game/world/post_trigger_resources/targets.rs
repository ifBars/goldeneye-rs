//! Later player requests reuse the same actual wrapper and logical section
//! records. Retained targets keep both owners alive across worker cancellation.
use super::{Owned, Resolution, Section, resolve};
use anyhow::{Context, Result};
use ge_assets::{edb::Edb, resource::GameAssets};
use ge_core::mission::{
    player::components::{TextureEnumeration, TextureOutcome},
    post_trigger::Request,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) struct Target {
    pub(super) source: Arc<Edb>,
    section: Option<Arc<Section>>,
    resolution: Resolution,
    request: Request,
}
impl Owned {
    pub(crate) fn request_player_target(
        &mut self,
        file: u32,
        target: u32,
        assets: &mut GameAssets,
    ) -> Result<Target> {
        self.request_target_with(file, target, |hash| {
            crate::mission::resource::load(assets, hash, 64 * 1024 * 1024)
        })
    }
    fn request_target_with(
        &mut self,
        file: u32,
        target: u32,
        mut load: impl FnMut(u32) -> Result<Edb>,
    ) -> Result<Target> {
        let request = Request {
            ordinal: 0,
            file,
            target,
        };
        let resolution = resolve(request, &mut self.files, &mut load)?;
        let retained = self
            .files
            .get(&file)
            .context("requested player wrapper is absent")?;
        let section = if resolution.base {
            None
        } else {
            Some(Arc::clone(
                retained
                    .sections
                    .iter()
                    .find(|s| s.index == resolution.section_index)
                    .context("requested player logical section record is absent")?,
            ))
        };
        Ok(Target {
            source: Arc::clone(&retained.edb),
            section,
            resolution,
            request,
        })
    }
}
impl Target {
    pub(crate) fn same_source(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
    }
    pub(crate) fn file(&self) -> u32 {
        self.source.header.hash
    }
    pub(crate) fn target(&self) -> u32 {
        self.request.target
    }
    pub(crate) fn file_bytes(&self) -> usize {
        self.source.header.payload_bytes as usize
    }
    pub(crate) fn section_bytes(&self) -> usize {
        self.section.as_ref().map_or(0, |s| s.end - s.start)
    }
    /// Traverse the requested skin's geometry through this same retained file
    /// owner. Directory absence remains its own outcome; errors stay errors.
    pub(crate) fn skin_texture_dependencies(&self) -> Result<TextureEnumeration> {
        use ge_assets::skin::{TextureBudget, TextureDependencies};
        let dependencies = self
            .source
            .skin_texture_dependencies(self.target(), TextureBudget::default())?
            .context("requested player skin reference is absent")?;
        Ok(match dependencies {
            TextureDependencies::EntityDirectoryAbsent => TextureEnumeration {
                outcome: TextureOutcome::EntityDirectoryAbsent,
                hashes: Vec::new(),
            },
            TextureDependencies::TextureDirectoryAbsent => TextureEnumeration {
                outcome: TextureOutcome::TextureDirectoryAbsent,
                hashes: Vec::new(),
            },
            TextureDependencies::Enumerated(value) => TextureEnumeration {
                outcome: TextureOutcome::Enumerated,
                hashes: value.hashes(),
            },
        })
    }
    pub(crate) fn fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"ge_player_target_residency_v1\0");
        for value in [
            self.file(),
            self.target(),
            self.resolution.stored_hash,
            self.resolution.key,
            self.source.header.payload_bytes,
        ] {
            hash.update(value.to_be_bytes());
        }
        hash.update((self.resolution.section_index as u64).to_be_bytes());
        hash.update([u8::from(self.resolution.base)]);
        if let Some(section) = &self.section {
            hash.update(section.sha256);
        }
        format!("{:x}", hash.finalize())
    }
}

#[cfg(test)]
#[path = "tests/targets.rs"]
mod tests;
