//! File-only lookups share the existing resource registry. A supplied-data load
//! is a host adaptation, not proof of the native registry's preload history.
use super::{File, LoadedFile, Owned};
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets, skin, target_section};
use std::{collections::BTreeMap, sync::Arc};
mod bone_mask;
mod modes;
mod script_child;

pub(crate) struct FileBinding {
    source: Arc<LoadedFile>,
}
pub(super) fn retain(
    files: &mut BTreeMap<u32, File>,
    file: u32,
    load: &mut impl FnMut(u32) -> Result<Edb>,
) -> Result<FileBinding> {
    ensure!(file >> 24 == 1, "resource file prefix rejected");
    if let std::collections::btree_map::Entry::Vacant(entry) = files.entry(file) {
        let edb = load(file)?;
        ensure!(edb.header.hash == file, "resource file owner differs");
        entry.insert(File {
            edb: Arc::new(LoadedFile {
                edb,
                generation: u32::MAX.wrapping_add(1),
            }),
            sections: Vec::new(),
        });
    }
    Ok(FileBinding {
        source: Arc::clone(
            &files
                .get(&file)
                .context("retained resource file is absent")?
                .edb,
        ),
    })
}
impl Owned {
    pub(crate) fn lookup_player_file(
        &mut self,
        file: u32,
        assets: &mut GameAssets,
    ) -> Result<FileBinding> {
        retain(&mut self.files, file, &mut |hash| {
            crate::mission::resource::load(assets, hash, 64 * 1024 * 1024)
        })
    }
}
impl FileBinding {
    pub(crate) fn same_target(&self, target: &super::Target) -> bool {
        Arc::ptr_eq(&self.source, &target.source)
    }
    pub(crate) fn skin_body(&self, reference: u32) -> Result<skin::Body> {
        let target = self
            .source
            .target_section(reference, target_section::Budget::default())?
            .context("visual skin descriptor is absent")?;
        ensure!(
            target.is_base(),
            "visual body lookup requires non-base residency provider"
        );
        let body = self
            .source
            .skin_body(reference, 4096)?
            .context("visual skin body is absent")?;
        ensure!(
            body.address()
                .checked_add(4)
                .is_some_and(|end| end <= self.source.header.base_bytes as usize),
            "visual body header is outside the logically resident base"
        );
        Ok(body)
    }
    pub(crate) fn file(&self) -> u32 {
        self.source.header.hash
    }
    pub(crate) fn file_bytes(&self) -> usize {
        self.source.header.payload_bytes as usize
    }
    pub(crate) fn skin(&self, reference: u32) -> Result<skin::Skin> {
        let target = self
            .source
            .target_section(reference, target_section::Budget::default())?
            .context("default skeleton descriptor is absent")?;
        // File lookup does not issue a target-residency request. The supported
        // default lives in the base; another section needs its actual provider.
        ensure!(
            target.is_base(),
            "default skeleton requires non-base residency provider"
        );
        self.source
            .skin(
                reference,
                skin::Budget {
                    skins: 4096,
                    bones: 128,
                },
            )?
            .context("default skeleton skin is absent")
    }
    pub(crate) fn auxiliary_words(&self, skin: &skin::Skin) -> Result<usize> {
        Ok(usize::try_from(skin.auxiliary_count(&self.source)?)?)
    }
}

#[cfg(test)]
#[path = "tests/files.rs"]
mod tests;
