//! The second invocation owns its own maps and setter returns. Earlier command
//! receipts describe retained traces, rather than rebuilding their old heads.
use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::{
    animation::layers::command::{self, EffectStage},
    mission::player::{
        actions::{self, Target, mode_call, repeat},
        components::Animator,
    },
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandTrace {
    pub stage: &'static str,
    pub opcode: u32,
    pub set_reference: Option<u32>,
    pub set_index: Option<usize>,
    pub set_groups: Option<usize>,
    pub groups_visited: usize,
    pub weight_writes: usize,
    pub mask: Option<u16>,
    pub pending_slot: Option<u8>,
    pub child_reference: Option<u32>,
    pub children_created: usize,
    pub children_completed: usize,
    pub children_inserted: usize,
    pub head_copies: usize,
    pub effective_flags_or: u16,
    pub command_returned: bool,
    pub selector_acknowledged: bool,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModeCall {
    pub target: &'static str,
    pub component_file: u32,
    pub generic_request: u32,
    pub requested: u32,
    pub parameter_bits: u32,
    pub tracks_player_modes: bool,
    pub fallback: u32,
    pub fallback_writes: u64,
    pub previous: u32,
    pub stage: &'static str,
    pub selector: super::ThirdPersonSelector,
    pub completed_commands: Vec<CommandTrace>,
    pub pending_command: Option<CommandTrace>,
    pub setter_return_word: Option<u32>,
    pub history_updated: bool,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepeatDispatch {
    pub phase: &'static str,
    pub captured_position_bits: u32,
    pub captured_head_present: bool,
    pub captured_matched: bool,
    pub tp_coordinate_written: Option<bool>,
    pub seek_bits: Option<u32>,
    pub seek_node_present: bool,
    pub seek_returned: bool,
    pub override_writes: usize,
    pub fp_coordinate_bits: Option<u32>,
    pub fp_coordinate_written: Option<bool>,
    pub tp_active_slots: u8,
    pub fp_active_slots: u8,
    pub calls: Vec<ModeCall>,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepeatAction {
    pub stage: &'static str,
    pub generic_request: u32,
    pub parameter_bits: u32,
    pub weapon_key: u32,
    pub variant: u32,
    pub remap_calls: usize,
    pub map_rows_read: usize,
    pub fp_mapped: u32,
    pub tp_mapped: u32,
    pub tp_secondary_mapped: u32,
    pub mapping_state_sha256: String,
    pub dispatch: Option<RepeatDispatch>,
    pub suffix: Option<super::suffix::ActionSuffix>,
    pub action_invocation_returned: bool,
    pub callback_returned: bool,
    pub state_sha256: String,
}
fn trace(body: &command::Pending, acknowledged: bool) -> Result<CommandTrace> {
    let stage = match body.stage() {
        command::Stage::Entry => "entry",
        command::Stage::ChildFactoryPending => "child_factory_pending",
        command::Stage::CleanupPending => "cleanup_pending",
        command::Stage::MaskControlPending => "mask_control_pending",
        command::Stage::MaskClearPending => "mask_clear_pending",
        command::Stage::MaskRefreshPending => "mask_refresh_pending",
        command::Stage::MaskBackingPending => "mask_backing_pending",
        command::Stage::MaskEffectsPending => "mask_effects_pending",
        command::Stage::MaskDestroyPending => "mask_destroy_pending",
        command::Stage::OpcodePending => "opcode_pending",
        command::Stage::ChildWrapperPending => "child_wrapper_pending",
        command::Stage::ChildContinuationPending => "child_continuation_pending",
        command::Stage::Returned => "returned",
        command::Stage::Failed => anyhow::bail!("failed repeat command cannot publish progress"),
    };
    ensure!(
        !acknowledged || body.stage() == command::Stage::Returned,
        "repeat command acknowledged before return"
    );
    Ok(CommandTrace {
        stage,
        opcode: body.opcode(),
        set_reference: body.set_reference(),
        set_index: body.set_index(),
        set_groups: body.group_count(),
        groups_visited: body.groups().len(),
        weight_writes: body.weight_writes(),
        mask: body.mask(),
        pending_slot: body.child().map(|c| c.slot),
        child_reference: body.child().map(|c| c.reference),
        children_created: body.effects().len(),
        children_completed: body
            .effects()
            .iter()
            .filter(|e| e.stage == EffectStage::Complete)
            .count(),
        children_inserted: body.effects().iter().filter(|e| e.inserted).count(),
        head_copies: body
            .effects()
            .iter()
            .filter(|e| e.copied_bits.is_some())
            .count(),
        effective_flags_or: body.flags_or(),
        command_returned: body.stage() == command::Stage::Returned,
        selector_acknowledged: acknowledged,
        state_sha256: super::super::words(b"ge_player_repeat_command_trace_v1\0", |emit| {
            body.visit_state_words(emit)
        }),
    })
}
fn coherent(owner: &Animator) -> Result<u8> {
    let layers = owner.layers().context("repeat receiver layers absent")?;
    ensure!(
        !layers.failed() && owner.byte(0x130) == Some(layers.active_count()),
        "repeat receiver active count differs"
    );
    for (slot, layer) in layers.slots().iter().enumerate() {
        let start = 0x138 + slot * 0x20;
        ensure!(
            owner.word(start + 0xc) == Some(layer.weight_bits())
                && owner
                    .byte(start + 8)
                    .zip(owner.byte(start + 9))
                    .map(|(a, b)| u16::from_be_bytes([a, b]))
                    == Some(layer.flags_raw()),
            "repeat receiver raw/typed headers differ"
        );
        if !layer.nodes_head().is_empty() {
            ensure!(
                owner.word(start).is_none() && owner.word(start + 4).is_none(),
                "repeat typed list contradicts raw NULL"
            );
        }
    }
    Ok(layers.active_count())
}
fn call(player: &Owned, call: &mode_call::Call, ordinal: usize) -> Result<ModeCall> {
    let inputs = call.inputs();
    let owner = player
        .components
        .preparation
        .mode_animator(inputs.receiver)?;
    let component_file = owner
        .component()
        .context("repeat call component binding absent")?
        .file;
    ensure!(
        inputs.receiver.player() == player.base.player()
            && component_file
                == if inputs.target == Target::FirstPerson {
                    0x0102_0016
                } else {
                    0x0102_0017
                },
        "repeat call receiver differs"
    );
    let stage = match call.stage() {
        mode_call::Stage::Selecting => "selecting",
        mode_call::Stage::CommandPending => "command_pending",
        mode_call::Stage::SetterTail => "setter_tail_pending",
        mode_call::Stage::Returned => "returned",
        mode_call::Stage::Failed => anyhow::bail!("failed repeat setter cannot publish progress"),
    };
    let binding = player
        .repeat_action
        .as_ref()
        .context("repeat owner absent")?
        .commands
        .get(ordinal);
    if let Some(body) = call.command() {
        ensure!(
            binding
                .is_some_and(|b| b.location.is_some_and(|l| l.opcode() == body.opcode())
                    && b.stream.is_some()),
            "repeat call command source binding differs"
        );
    }
    let completed_commands = call
        .completed_commands()
        .iter()
        .map(|b| trace(b, true))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        call.selector().completed_commands() == completed_commands.len(),
        "repeat acknowledged command history differs"
    );
    let pending_command = if call.stage() == mode_call::Stage::CommandPending {
        call.command().map(|b| trace(b, false)).transpose()?
    } else {
        None
    };
    let selector_stage = if call.stage() == mode_call::Stage::CommandPending {
        "command_pending"
    } else if call.selector().return_word().is_some() {
        "returned"
    } else {
        "selecting"
    };
    let setter = call.setter();
    Ok(ModeCall {
        target: match inputs.target {
            Target::FirstPerson => "first_person",
            Target::ThirdPerson => "third_person",
            Target::ThirdPersonSecondary => "third_person_secondary",
        },
        component_file,
        generic_request: inputs.generic,
        requested: inputs.requested,
        parameter_bits: inputs.parameter_bits,
        tracks_player_modes: inputs.tracks,
        fallback: inputs.fallback.0,
        fallback_writes: inputs.fallback.1,
        previous: call.previous(),
        stage,
        selector: super::selector::report(
            call.selector(),
            inputs.tracks,
            selector_stage,
            call.selector()
                .pending()
                .and_then(|_| binding.and_then(|b| b.location).map(|l| l.opcode())),
        ),
        completed_commands,
        pending_command,
        setter_return_word: setter.map(|s| s.return_word),
        history_updated: setter.is_some_and(|s| s.history_updated),
        state_sha256: super::super::words(b"ge_player_repeat_mode_call_v1\0", |emit| {
            call.visit_state_words(emit)
        }),
    })
}
pub(super) fn build(player: &Owned) -> Result<Option<RepeatAction>> {
    let Some(repeated) = &player.repeat_action else {
        return Ok(None);
    };
    let first = player
        .action
        .as_ref()
        .and_then(|a| a.completed_evidence.as_ref())
        .context("repeat action has no immutable first invocation snapshot")?;
    let state = &repeated.state;
    ensure!(
        first.action_invocation_returned
            && !first.callback_returned
            && state.player() == player.base.player()
            && state.resolved().len() == 3
            && repeated.mappings.len() == 3
            && first
                .suffix
                .as_ref()
                .is_some_and(|s| s.repeat_generic == Some(state.generic())
                    && s.repeat_parameter_bits == Some(state.parameter_bits())),
        "repeat action differs from its actual prepared request"
    );
    let mut mapping_hash = Sha256::new();
    mapping_hash.update(b"ge_player_repeat_maps_v1\0");
    mapping_hash.update(player.loadout.provenance().state_sha256.as_bytes());
    for ((resolved, mapped), target) in state.resolved().iter().zip(&repeated.mappings).zip([
        Target::FirstPerson,
        Target::ThirdPerson,
        Target::ThirdPersonSecondary,
    ]) {
        ensure!(
            resolved.request.target == target
                && resolved.request.generic == mapped.generic
                && Some(mapped.source_weapon) == state.weapon()
                && Some(mapped.variant) == state.variant()
                && resolved.output == mapped.output
                && resolved.mapped == mapped.mapped,
            "repeat mapping retained source differs"
        );
        for word in [
            mapped.source_weapon,
            mapped.generic,
            mapped.variant,
            mapped.output,
            u32::from(mapped.mapped),
            mapped.map_rows_read as u32,
        ] {
            mapping_hash.update(word.to_be_bytes());
        }
        mapping_hash.update((mapped.steps.len() as u64).to_be_bytes());
        for step in &mapped.steps {
            mapping_hash.update(step.weapon.to_be_bytes());
            mapping_hash.update(step.candidate_raw.to_be_bytes());
            for value in [step.record_source_index, step.matched_source_ordinal] {
                mapping_hash.update([u8::from(value.is_some())]);
                if let Some(value) = value {
                    mapping_hash.update((value as u64).to_be_bytes());
                }
            }
            mapping_hash.update([match step.available {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            }]);
        }
    }
    let dispatch = repeated
        .dispatch
        .as_ref()
        .map(|d| -> Result<_> {
            let tp = player
                .components
                .preparation
                .animator()
                .context("repeat TP absent")?;
            let fp = player
                .components
                .preparation
                .first_person()
                .context("repeat FP absent")?
                .animator();
            ensure!(
                d.player() == player.base.player(),
                "repeat dispatch player differs"
            );
            if let Some(latest) = d
                .calls()
                .iter()
                .rev()
                .find(|call| call.setter().is_some_and(|setter| setter.history_updated))
            {
                let inputs = latest.inputs();
                ensure!(
                    player.reset.word(0x710) == Some(inputs.generic)
                        && player.reset.word(0x718) == Some(inputs.requested),
                    "repeat secondary history differs from the latest actual setter"
                );
            }
            let phase = match d.stage() {
                repeat::Stage::FirstThirdPerson => "first_third_person",
                repeat::Stage::ThirdPersonCoordinate => "third_person_coordinate",
                repeat::Stage::ThirdPersonSeek => "third_person_seek",
                repeat::Stage::EnablingOverride => "enabling_override",
                repeat::Stage::SecondThirdPerson => "second_third_person",
                repeat::Stage::DisablingOverride => "disabling_override",
                repeat::Stage::FirstPerson => "first_person",
                repeat::Stage::FirstPersonCoordinate => "first_person_coordinate",
                repeat::Stage::Returned => "returned",
            };
            let captured = d.captured();
            Ok(RepeatDispatch {
                phase,
                captured_position_bits: captured.bits,
                captured_head_present: captured.head.is_some(),
                captured_matched: captured.matched,
                tp_coordinate_written: d.tp_coordinate_written(),
                seek_bits: d.seek_bits(),
                seek_node_present: d.seek_node().is_some(),
                seek_returned: d.seek_returned(),
                override_writes: d.override_writes(),
                fp_coordinate_bits: d.fp_coordinate_bits(),
                fp_coordinate_written: d.fp_coordinate_written(),
                tp_active_slots: coherent(tp)?,
                fp_active_slots: coherent(fp)?,
                calls: d
                    .calls()
                    .iter()
                    .enumerate()
                    .map(|(i, c)| call(player, c, i))
                    .collect::<Result<Vec<_>>>()?,
                state_sha256: super::super::words(b"ge_player_repeat_dispatch_v1\0", |mut emit| {
                    d.visit_state_words(&mut emit);
                    tp.visit_state_words(&mut emit);
                    fp.visit_state_words(&mut emit);
                }),
            })
        })
        .transpose()?;
    let stage = match state.stage() {
        actions::Stage::AwaitingThirdPersonDispatch | actions::Stage::RepeatedDispatch => {
            "dispatch_pending"
        }
        actions::Stage::AwaitingActionSuffix | actions::Stage::ApplyingActionSuffix => {
            "suffix_pending"
        }
        actions::Stage::Returned => "returned",
        _ => anyhow::bail!("repeat action has no reportable reached boundary"),
    };
    Ok(Some(RepeatAction {
        stage,
        generic_request: state.generic(),
        parameter_bits: state.parameter_bits(),
        weapon_key: state.weapon().context("repeat weapon unknown")?,
        variant: state.variant().context("repeat variant unknown")?,
        remap_calls: repeated.mappings.len(),
        map_rows_read: repeated.mappings.iter().map(|m| m.map_rows_read).sum(),
        fp_mapped: state.resolved()[0].output,
        tp_mapped: state.resolved()[1].output,
        tp_secondary_mapped: state.resolved()[2].output,
        mapping_state_sha256: format!("{:x}", mapping_hash.finalize()),
        dispatch,
        suffix: super::suffix::build_for(player, state)?,
        action_invocation_returned: state.stage() == actions::Stage::Returned,
        callback_returned: false,
        state_sha256: super::super::words(b"ge_player_repeat_action_v1\0", |emit| {
            state.visit_state_words(emit)
        }),
    }))
}
