use super::super::super::Owned;
use super::{BindRefresh, SkeletonBinding};
use crate::game::world_hooks;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    components::{FirstPersonStage, PoseCallback, Registry, ResourceBinding},
    world::World,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FirstPersonBinding {
    pub helper_returned: bool,
    pub identity_registered: bool,
    pub item_list_ordinal: usize,
    pub ordered_links_bound: bool,
    pub masks_configured: bool,
    pub table_shared: bool,
    pub pose_target_assigned: bool,
    pub visual_postbind_skipped: bool,
    pub skeleton: SkeletonBinding,
    pub bind_refresh: BindRefresh,
    pub state_sha256: String,
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    identities: &Registry,
    hook: &world_hooks::Attempt,
) -> Result<FirstPersonBinding> {
    let preparation = &player.components.preparation;
    let fp = preparation.first_person().context("FP owner is absent")?;
    let animator = fp.animator();
    let tp = preparation.animator().context("TP owner is absent")?;
    let runtime = animator.skeleton().context("FP skeleton is absent")?;
    let refresh = fp.refresh().context("FP bind return is absent")?;
    let source = player
        .components
        .first_person
        .skin_source
        .as_ref()
        .context("FP skin wrapper is absent")?;
    let component = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("FP component wrapper is absent")?;
    let skin = player
        .components
        .first_person
        .skin
        .as_ref()
        .context("FP skin is absent")?;
    let item = world.item(player.base.item().context("FP item is absent")?)?;
    let primary = ResourceBinding {
        file: source.file(),
        reference: 0x0d00_016f,
    };
    let shared = hook.animation_callbacks()?;
    ensure!(
        fp.stage() == FirstPersonStage::Returned
            && source.file() == player.loadout.selected_character().fp_skin_file()
            && fp.skin_file() == source.file()
            && component.file() == 0x0102_0016
            && animator
                .component()
                .is_some_and(|b| b.file == component.file()
                    && (player
                        .action
                        .as_ref()
                        .and_then(|a| a.state.first_person_dispatch())
                        .is_some()
                        || b.reference == 0))
            && animator.identity() != tp.identity()
            && animator.player() == player.base.player()
            && identities.resolve(animator.identity().id()) == Some(animator.identity())
            && item.components() == [tp.identity().id(), animator.identity().id()]
            && animator.item() == Some(item.id())
            && animator.previous_component() == Some(tp.identity())
            && tp.next_component() == Some(animator.identity())
            && animator.next_component().is_none()
            && animator.word(0).is_none()
            && animator.word(4) == Some(0)
            && animator.byte(0x8e) == Some(0)
            && animator.byte(0x8c) == Some(0x11)
            && animator.byte(0x130)
                == Some(animator.layers().map_or(1, |layers| layers.active_count()))
            && animator.byte(0x131) == Some(0xfb)
            && animator.byte(0x136) == Some(0xfb)
            && animator
                .callback_table()
                .is_some_and(|table| Arc::ptr_eq(table, &shared))
            && runtime.primary() == primary
            && runtime.secondary() == Some(primary)
            && runtime.animator() == Some(animator.identity())
            && runtime.frame_stamp_bits().is_err()
            && runtime.pose_callback() == Some(PoseCallback::PlayerNewPostprocess)
            && runtime.skeleton().bones().len() == skin.bone_count()
            && runtime.auxiliary().len() == source.auxiliary_words(skin)?
            && runtime.auxiliary().iter().all(|&word| word == 0),
        "FP retained resources, bindings or item ownership differ"
    );
    ensure!(
        animator.configured_header_flags() == Some([1; 4]),
        "FP configured header receipt differs"
    );
    for (slot, start) in [0x138, 0x158, 0x178, 0x198].into_iter().enumerate() {
        let flags = animator
            .byte(start + 8)
            .zip(animator.byte(start + 9))
            .map(|(a, b)| u16::from_be_bytes([a, b]));
        ensure!(
            flags
                == Some(
                    animator
                        .layers()
                        .map_or(1, |layers| layers.slots()[slot].flags_raw())
                )
                && animator.byte(start + 10) == Some(3),
            "FP header configuration differs"
        );
    }
    let state_sha256 = super::super::words(b"ge_player_fp_helper_v1\0", |emit| {
        fp.visit_state_words(emit)
    });
    Ok(FirstPersonBinding {
        helper_returned: true,
        identity_registered: true,
        item_list_ordinal: 1,
        ordered_links_bound: true,
        masks_configured: true,
        table_shared: true,
        pose_target_assigned: true,
        visual_postbind_skipped: true,
        skeleton: SkeletonBinding {
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
            empty_refresh_complete: true,
            retained_skeleton_file_bytes: source.file_bytes(),
            retained_component_file_bytes: component.file_bytes(),
            state_sha256: super::super::words(b"ge_player_fp_skeleton_v1\0", |emit| {
                runtime.visit_state_words(emit)
            }),
        },
        bind_refresh: BindRefresh {
            arena_bytes: ge_core::animation::scratch::ARENA_BYTES,
            reserved_bytes: 0x2200,
            reservation_start: refresh.reservation_start(),
            cursor_after: refresh.cursor_after(),
            live_reservations_after: refresh.live_after(),
            refresh_ordinal: refresh.ordinal(),
            allocation_ordinal: refresh.allocation_ordinal(),
            regions_bound: 4,
            flags_after: 0,
            scope_released: true,
            state_sha256: super::super::words(b"ge_player_fp_refresh_v1\0", |emit| {
                refresh.visit_state_words(emit)
            }),
        },
        state_sha256,
    })
}
