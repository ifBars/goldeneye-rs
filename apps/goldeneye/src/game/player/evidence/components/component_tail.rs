use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    constructor::{
        component_tail::{Pending, Stage},
        pose::CharacterLookup,
    },
    world::World,
};
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentTail {
    pub stage: &'static str,
    pub pending_dependency: Option<&'static str>,
    pub helper_return: Option<u32>,
    pub helper_returned: bool,
    pub rotation_reads: [bool; 2],
    pub rotation_stores: [bool; 2],
    pub rotation_invalidated: bool,
    pub character_outcome: Option<&'static str>,
    pub character_scalars_read: usize,
    pub character_scale_ready: bool,
    pub session_multiple: Option<bool>,
    pub scale_stores: [bool; 2],
    pub scale_invalidated: bool,
    pub flags_before: Option<u8>,
    pub flags_after: Option<u8>,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned, world: &World) -> Result<Option<ComponentTail>> {
    let Some(tail) = &player.component_tail else {
        return Ok(None);
    };
    ensure!(
        tail.player() == player.base.player()
            && player
                .completed_pose_evidence
                .as_ref()
                .is_some_and(|p| p.helper_returned),
        "component tail lacks its actual pose-helper return snapshot"
    );
    if let Some(snapshot) = &player.completed_component_evidence {
        ensure!(
            tail.helper_returned() && *snapshot == receipt(tail, snapshot.state_sha256.clone()),
            "component snapshot differs from its actual returned helper receipt"
        );
        return Ok(Some(snapshot.clone()));
    }
    let item = world.item(player.base.item().context("component-tail item absent")?)?;
    let rotation = item.rotation_bits();
    for i in 0..2 {
        if tail.rotation_stores()[i] {
            ensure!(
                tail.rotation_reads()[i] == Some([rotation[i * 2], rotation[i * 2 + 1]]),
                "component-tail actual rotation self-copy differs"
            );
        }
    }
    if tail.rotation_invalidated() || tail.scale_invalidated() {
        ensure!(
            item.factory().is_some_and(|f| f.byte_8e() == 0),
            "component-tail item invalidation differs"
        );
    }
    match tail.character_lookup() {
        None => {}
        Some(CharacterLookup::KnownAbsent) => {
            ensure!(
                player.reset.character().is_none(),
                "component tail ignored a bound character"
            );
        }
        Some(CharacterLookup::Bound(binding)) => {
            let row = player.loadout.selected_character();
            ensure!(
                player.reset.character() == Some(binding)
                    && binding.registry_resource == 0x0109_0013
                    && binding.key == row.key()
                    && binding.source_index == row.source_index(),
                "component-tail character differs"
            );
            for (i, value) in tail.character_scalars().into_iter().enumerate() {
                if let Some(value) = value {
                    ensure!(
                        row.words_raw()[4 + i] == value,
                        "component-tail retained scalar differs"
                    );
                }
            }
            if let Some(bits) = tail.scale_bits() {
                ensure!(
                    tail.character_scalars() == [Some(bits[0]), Some(bits[1])]
                        && bits[2] == bits[0]
                        && bits[3] == 0,
                    "component-tail character scale differs"
                );
            }
        }
    }
    let scale = item.scale_bits();
    for i in 0..2 {
        if tail.scale_stores()[i] {
            ensure!(
                tail.scale_bits()
                    .is_some_and(|bits| bits[i * 2..i * 2 + 2] == scale[i * 2..i * 2 + 2]),
                "component-tail final scale pair differs"
            );
        }
    }
    if let Some(after) = tail.flags_after() {
        ensure!(
            item.byte_8c() == after
                && tail
                    .flags_before()
                    .is_some_and(|before| before | 3 == after),
            "component-tail actual byte flag OR differs"
        );
    }
    Ok(Some(receipt(
        tail,
        super::super::words(b"ge_player_constructor_component_tail_v1\0", |mut emit| {
            tail.visit_state_words(&mut emit);
            item.visit_state_words(&mut emit);
            player.reset.visit_state_words(&mut emit);
        }),
    )))
}
fn receipt(tail: &Pending, state_sha256: String) -> ComponentTail {
    let stage = match tail.stage() {
        Stage::RotationSecond => "rotation_second",
        Stage::RotationFirst => "rotation_first",
        Stage::StoreRotationFirst => "store_rotation_first",
        Stage::StoreRotationSecond => "store_rotation_second",
        Stage::RotationInvalidate => "rotation_invalidate",
        Stage::Character => "character",
        Stage::Character10 => "character_10",
        Stage::Character14 => "character_14",
        Stage::SessionScale => "session_scale",
        Stage::ScaleFirst => "scale_first",
        Stage::ScaleSecond => "scale_second",
        Stage::ScaleInvalidate => "scale_invalidate",
        Stage::Flags => "flags",
        Stage::Returned => "returned",
    };
    ComponentTail {
        stage,
        pending_dependency: tail.dependency().map(|d| d.as_str()),
        helper_return: tail.helper_return(),
        helper_returned: tail.helper_returned(),
        rotation_reads: tail.rotation_reads().map(|v| v.is_some()),
        rotation_stores: tail.rotation_stores(),
        rotation_invalidated: tail.rotation_invalidated(),
        character_outcome: tail.character_lookup().map(|v| match v {
            CharacterLookup::KnownAbsent => "known_absent",
            CharacterLookup::Bound(_) => "bound",
        }),
        character_scalars_read: tail
            .character_scalars()
            .iter()
            .filter(|v| v.is_some())
            .count(),
        character_scale_ready: tail.scale_bits().is_some(),
        session_multiple: tail.session_multiple(),
        scale_stores: tail.scale_stores(),
        scale_invalidated: tail.scale_invalidated(),
        flags_before: tail.flags_before(),
        flags_after: tail.flags_after(),
        state_sha256,
    }
}
