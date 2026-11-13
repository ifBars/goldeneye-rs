use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    player::{
        constructor::pose::{self, GlobalArgument, Stage, UserLookup},
        world::World,
    },
    session::local_users::{Origin, State},
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnableWrite {
    pub component_file: u32,
    pub reference: u32,
    pub enabled: bool,
    pub reference_matched: bool,
    pub flags_before: Option<u16>,
    pub flags_after: Option<u16>,
    pub invalidated_8e: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalUserLookup {
    pub visited: usize,
    pub matched: bool,
    pub association_state: &'static str,
    pub association_origin: &'static str,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PoseSetup {
    pub stage: &'static str,
    pub pending_dependency: Option<&'static str>,
    pub input_raw: u8,
    pub input_signed: i8,
    pub helper_returned: bool,
    pub entry_702: Option<u8>,
    pub blend_ba8: Option<u8>,
    pub bab_written: bool,
    pub tp_enabled: Option<bool>,
    pub fp_enabled: Option<bool>,
    pub receiver_searches: [bool; 2],
    pub enable_writes: [Option<EnableWrite>; 2],
    pub character_bound: bool,
    pub character_scalars_read: usize,
    pub character_vector_read: bool,
    pub session_scale_multiple: Option<bool>,
    pub scale_source: Option<&'static str>,
    pub scale_stored: bool,
    pub scale_invalidated: bool,
    pub dirty_before: Option<u8>,
    pub dirty_after: Option<u8>,
    pub ca4_written: bool,
    pub mode_afc_absent: Option<bool>,
    pub force_nodes_checked: usize,
    pub user_target_bound: bool,
    pub user_lookup: Option<LocalUserLookup>,
    pub false_arm_b00_absent: Option<bool>,
    pub backlink_nodes_checked: usize,
    pub attachment_absent: Option<bool>,
    pub weapon_global_read: bool,
    pub weapon_global_byte: Option<u8>,
    pub global_argument: Option<i8>,
    pub global_equivalence_used: bool,
    pub global_attachment_absent: Option<bool>,
    pub fade_inputs_read: bool,
    pub fade_argument_ready: bool,
    pub fade_attachment_absent: Option<bool>,
    pub final_session_multiple: Option<bool>,
    pub state_sha256: String,
}
fn receipt(player: &Owned, state: &pose::Pending, state_sha256: String) -> Result<PoseSetup> {
    let owner = player.pose_setup.as_ref().context("pose owner absent")?;
    let user_lookup = if let Some(result) = state.user_lookup() {
        let lookup = owner
            .sessionlookups
            .last()
            .context("pose local-user result lacks its real session receipt")?;
        ensure!(
            lookup.session == player.request.session()
                && lookup.target == state.user_target().context("pose user target absent")?
                && lookup.association.session() == Some(lookup.session)
                && lookup.association.user() == player.request.user()
                && lookup.visited == 1
                && lookup.matched
                    == match lookup.association.state() {
                        State::KnownNull => None,
                        State::Bound(target) =>
                            (target == lookup.target).then_some(lookup.association.user()),
                        State::Unavailable =>
                            anyhow::bail!("pose lookup cannot exhaust an unavailable association"),
                    }
                && match result {
                    UserLookup::NotFound => lookup.matched.is_none(),
                    UserLookup::Matched(user) => lookup.matched == Some(user),
                },
            "pose local-user query changed its session, target or association owner"
        );
        Some(LocalUserLookup {
            visited: lookup.visited,
            matched: lookup.matched.is_some(),
            association_state: match lookup.association.state() {
                State::KnownNull => "known_null",
                State::Bound(_) => "bound",
                State::Unavailable => {
                    anyhow::bail!("unavailable association cannot prove a lookup result")
                }
            },
            association_origin: match lookup.association.origin() {
                Origin::LocalUserConstructor => "local_user_constructor",
                Origin::SuppliedSnapshot => "supplied_snapshot",
                Origin::Unavailable => {
                    anyhow::bail!("unavailable association has no source origin")
                }
            },
            state_sha256: super::super::words(
                b"ge_player_pose_local_user_lookup_v1\0",
                |mut emit| {
                    lookup.association.visit_state_words(&mut emit);
                    emit((lookup.session.get() >> 32) as u32);
                    emit(lookup.session.get() as u32);
                    emit((lookup.target.get() >> 32) as u32);
                    emit(lookup.target.get() as u32);
                    emit(lookup.visited as u32);
                    emit(u32::from(lookup.matched.is_some()));
                    if let Some(user) = lookup.matched {
                        emit((user.get() >> 32) as u32);
                        emit(user.get() as u32);
                    }
                },
            ),
        })
    } else {
        ensure!(
            owner.sessionlookups.is_empty(),
            "pose retained an unconsumed user query"
        );
        None
    };
    let stage = match state.stage() {
        Stage::Entry => "entry",
        Stage::BlendGate => "blend_gate",
        Stage::EnableArguments => "enable_arguments",
        Stage::ThirdPersonEnable => "third_person_enable",
        Stage::FirstPersonEnable => "first_person_enable",
        Stage::CharacterBinding => "character_binding",
        Stage::Character10 => "character_10",
        Stage::Character14 => "character_14",
        Stage::SessionScale => "session_scale",
        Stage::Scale => "scale",
        Stage::ScaleInvalidate => "scale_invalidate",
        Stage::Dirty => "dirty",
        Stage::WeaponMode => "weapon_mode",
        Stage::ForceComponents => "force_components",
        Stage::UserTarget => "user_target",
        Stage::UserLookup => "user_lookup",
        Stage::UserFalseArm => "user_false_arm",
        Stage::BacklinkComponents => "backlink_components",
        Stage::Attachment => "attachment",
        Stage::GlobalByte => "global_byte",
        Stage::GlobalAttachment => "global_attachment",
        Stage::FadeInputs => "fade_inputs",
        Stage::FinalSession => "final_session",
        Stage::Returned => "returned",
    };
    Ok(PoseSetup {
        stage,
        pending_dependency: state.dependency().map(|d| d.as_str()),
        input_raw: state.input_raw(),
        input_signed: state.input_signed(),
        helper_returned: state.helper_returned(),
        entry_702: state.entry_702(),
        blend_ba8: state.blend_ba8(),
        bab_written: state.bab_written(),
        tp_enabled: state.tp_enabled(),
        fp_enabled: state.fp_enabled(),
        receiver_searches: state.receiver_searches(),
        enable_writes: std::array::from_fn(|i| {
            state.enable_writes()[i].map(|w| EnableWrite {
                component_file: [0x0102_0017, 0x0102_0016][i],
                reference: w.reference,
                enabled: w.enabled,
                reference_matched: w.reference_matched,
                flags_before: w.flags_before,
                flags_after: w.flags_after,
                invalidated_8e: w.invalidated_8e,
            })
        }),
        character_bound: state.character_binding().is_some(),
        character_scalars_read: state
            .character_scalars()
            .iter()
            .filter(|v| v.is_some())
            .count(),
        character_vector_read: state.character_vector().is_some(),
        session_scale_multiple: state.session_scale_multiple(),
        scale_source: state.scale_bits().map(|_| {
            if state.fp_enabled() == Some(true) {
                "startup_ones"
            } else {
                "character"
            }
        }),
        scale_stored: state.scale_stored(),
        scale_invalidated: state.scale_invalidated(),
        dirty_before: state.dirty_before(),
        dirty_after: state.dirty_after(),
        ca4_written: state.ca4_written(),
        mode_afc_absent: state.mode_afc_absent(),
        force_nodes_checked: state.force_nodes_checked(),
        user_target_bound: state.user_target().is_some(),
        user_lookup,
        false_arm_b00_absent: state.false_arm_b00_absent(),
        backlink_nodes_checked: state.backlink_nodes_checked(),
        attachment_absent: state.attachment_absent(),
        weapon_global_read: state.weapon_global_read(),
        weapon_global_byte: state.weapon_global_byte(),
        global_argument: match state.global_argument() {
            Some(GlobalArgument::Known(v)) => Some(v),
            _ => None,
        },
        global_equivalence_used: state.global_equivalence_used(),
        global_attachment_absent: state.global_attachment_absent(),
        fade_inputs_read: state.fade_cfc_bits().is_some(),
        fade_argument_ready: state.fade_argument_bits().is_some(),
        fade_attachment_absent: state.fade_attachment_absent(),
        final_session_multiple: state.final_session_multiple(),
        state_sha256,
    })
}
pub(super) fn build(player: &Owned, world: &World) -> Result<Option<PoseSetup>> {
    let Some(owner) = &player.pose_setup else {
        return Ok(None);
    };
    let state = &owner.state;
    ensure!(
        state.player() == player.base.player()
            && player
                .completed_constructor_evidence
                .as_ref()
                .is_some_and(|t| t.helper_return == Some(1) && t.stage == "awaiting_pose")
            && player
                .completed_repeat_evidence
                .as_ref()
                .is_some_and(|r| r.action_invocation_returned),
        "pose helper changed its player or lacks prior immutable receipts"
    );
    if let Some(snapshot) = &player.completed_pose_evidence {
        ensure!(
            state.helper_returned()
                && *snapshot == receipt(player, state, snapshot.state_sha256.clone())?,
            "pose snapshot differs from its actual returned helper receipt"
        );
        return Ok(Some(snapshot.clone()));
    }
    let item = world.item(player.base.item().context("pose item absent")?)?;
    if let Some(entry) = state.entry_702() {
        ensure!(
            player.base.byte(0x702) == Some(entry),
            "pose entry byte differs"
        );
    }
    if let Some(blend) = state.blend_ba8() {
        ensure!(
            player.reset.byte(0xba8) == Some(blend),
            "pose blend byte differs"
        );
    }
    if state.bab_written() {
        ensure!(
            player.reset.byte(0xbab) == Some(state.input_raw()),
            "pose bab byte store differs"
        );
    }
    for (i, write) in state.enable_writes().iter().enumerate() {
        if let Some(write) = write {
            let animator = player
                .components
                .preparation
                .mode_animator(write.receiver)?;
            ensure!(
                animator.player() == state.player()
                    && animator.item() == Some(item.id())
                    && item.components().contains(&animator.identity().id())
                    && animator
                        .component()
                        .is_some_and(|b| b.file == [0x0102_0017, 0x0102_0016][i])
                    && animator.word(0x20) == Some(write.reference),
                "pose enable receiver or current reference differs"
            );
            if let Some(after) = write.flags_after {
                ensure!(
                    animator
                        .byte(0xc)
                        .zip(animator.byte(0xd))
                        .map(|(a, b)| u16::from_be_bytes([a, b]))
                        == Some(after),
                    "pose semantic enable flags differ"
                );
            }
            if write.invalidated_8e {
                ensure!(
                    animator.byte(0x8e) == Some(0),
                    "pose enable invalidation differs"
                );
            }
        }
    }
    if let Some(binding) = state.character_binding() {
        let character = player.loadout.selected_character();
        ensure!(
            player.reset.character() == Some(binding)
                && binding.registry_resource == 0x0109_0013
                && binding.key == character.key()
                && binding.source_index == character.source_index(),
            "pose semantic character binding differs"
        );
        for (i, scalar) in state.character_scalars().into_iter().enumerate() {
            if let Some(bits) = scalar {
                ensure!(
                    character.words_raw()[4 + i] == bits,
                    "pose scalar differs from the reached retained character field"
                );
            }
        }
        if let Some(vector) = state.character_vector() {
            ensure!(
                state.character_scalars() == [Some(vector[0]), Some(vector[1])]
                    && vector[2] == vector[0]
                    && vector[3] == 0,
                "pose character vector differs from source scalar ordering"
            );
        }
    }
    if state.scale_stored() {
        ensure!(
            state.scale_bits() == Some(item.scale_bits()),
            "pose item scale differs"
        );
    }
    if state.scale_invalidated() {
        ensure!(
            item.factory().is_some_and(|f| f.byte_8e() == 0),
            "pose item scale invalidation differs"
        );
    }
    if let Some(after) = state.dirty_after() {
        ensure!(item.byte_8c() == after, "pose item dirty byte differs");
    }
    if state.ca4_written() {
        ensure!(
            player.reset.word(0xca4) == Some(0x0e00_0070),
            "pose weapon mode word differs"
        );
    }
    for absent in [
        state.mode_afc_absent(),
        state.attachment_absent(),
        state.global_attachment_absent(),
        state.fade_attachment_absent(),
    ]
    .into_iter()
    .flatten()
    {
        ensure!(
            player.reset.word(0xafc).map(|w| w == 0) == Some(absent),
            "pose attachment NULL branch differs"
        );
    }
    if let Some(absent) = state.false_arm_b00_absent() {
        ensure!(
            player.reset.word(0xb00).map(|w| w == 0) == Some(absent),
            "pose user false-arm b00 branch differs"
        );
    }
    if let Some(target) = state.user_target() {
        ensure!(
            target == state.player() && player.base.word(0x98c) == Some(0),
            "pose selected a foreign user lookup target"
        );
    }
    if let Some(cfc) = state.fade_cfc_bits() {
        ensure!(
            player.reset.word(0xcfc) == Some(cfc),
            "pose fade input differs"
        );
    }
    if let Some(argument) = state.fade_argument_bits() {
        ensure!(
            state
                .fade_cfc_bits()
                .is_some_and(|v| (1f32 - f32::from_bits(v)).to_bits() == argument),
            "pose ordered fade arithmetic differs"
        );
    }
    let hash = super::super::words(b"ge_player_constructor_pose_v1\0", |mut emit| {
        state.visit_state_words(&mut emit);
        player.base.visit_state_words(&mut emit);
        player.reset.visit_state_words(&mut emit);
        item.visit_state_words(&mut emit);
        player.components.preparation.visit_state_words(&mut emit);
    });
    Ok(Some(receipt(player, state, hash)?))
}
