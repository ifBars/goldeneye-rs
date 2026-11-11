use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::components::ResourceBinding;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkeletonBinding {
    pub native_runtime_bytes: usize,
    pub native_storage_bytes: usize,
    pub bones: usize,
    pub roots: usize,
    pub auxiliary_words: usize,
    pub primary_bound: bool,
    pub secondary_bound: bool,
    pub animator_backlink_bound: bool,
    pub component_resource_bound: bool,
    pub frame_stamp_known: bool,
    pub empty_refresh_complete: bool,
    pub retained_skeleton_file_bytes: usize,
    pub retained_component_file_bytes: usize,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<SkeletonBinding> {
    let animator = player
        .components
        .preparation
        .animator()
        .context("TP animator is absent")?;
    let runtime = animator
        .skeleton()
        .context("TP skeletal runtime is absent")?;
    let source = player
        .components
        .skeleton_source
        .as_ref()
        .context("default skeleton wrapper is absent")?;
    let component = player
        .components
        .component_source
        .as_ref()
        .context("TP component wrapper is absent")?;
    let skin = player
        .components
        .skin
        .as_ref()
        .context("retained default skin is absent")?;
    let primary = ResourceBinding {
        file: source.file(),
        reference: 0x0d00_0001,
    };
    ensure!(
        source.file() == 0x010a_0056
            && component.file() == 0x0102_0017
            && runtime.primary() == primary
            && runtime.secondary()
                == player
                    .components
                    .preparation
                    .visual_postbind()
                    .map(|postbind| postbind.secondary())
            && runtime.animator() == Some(animator.identity())
            && animator.player() == player.base.player()
            && animator.component()
                == Some(ResourceBinding {
                    file: component.file(),
                    reference: 0
                })
            && runtime.skeleton().bones().len() == skin.bone_count()
            && runtime.auxiliary().len() == source.auxiliary_words(skin)?
            && runtime.auxiliary().iter().all(|&word| word == 0)
            && runtime.frame_stamp_bits().is_err(),
        "TP skeletal owners or reached initialization differ"
    );
    Ok(SkeletonBinding {
        native_runtime_bytes: 0x164,
        native_storage_bytes: runtime.native_storage_bytes(),
        bones: runtime.skeleton().bones().len(),
        roots: runtime.skeleton().roots().len(),
        auxiliary_words: runtime.auxiliary().len(),
        primary_bound: true,
        secondary_bound: true,
        animator_backlink_bound: true,
        component_resource_bound: true,
        frame_stamp_known: false,
        empty_refresh_complete: player.components.preparation.bind_return().is_some(),
        retained_skeleton_file_bytes: source.file_bytes(),
        retained_component_file_bytes: component.file_bytes(),
        state_sha256: super::super::words(b"ge_player_tp_skeleton_binding_v1\0", |emit| {
            runtime.visit_state_words(emit)
        }),
    })
}
