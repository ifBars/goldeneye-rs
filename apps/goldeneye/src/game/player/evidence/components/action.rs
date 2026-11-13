use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::actions::{Stage, Target};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitialAction {
    pub stage: &'static str,
    pub generic_request: u32,
    pub parameter_bits: u32,
    pub direction_raw: u8,
    pub weapon_key: u32,
    pub variant: u32,
    pub remap_calls: usize,
    pub map_rows_read: usize,
    pub fp_mapped: u32,
    pub tp_mapped: u32,
    pub tp_secondary_mapped: u32,
    pub source_tables_reused: bool,
    pub mode_controls_created: usize,
    pub mode_history_updated: bool,
    pub callback_returned: bool,
    pub action_invocation_returned: bool,
    pub state_sha256: String,
    pub mapping_state_sha256: String,
    pub tp_capture: super::ThirdPersonCapture,
    pub tp_selector: super::ThirdPersonSelector,
    pub tp_command: Option<super::CommandBody>,
    pub tp_dispatch: super::dispatch::ThirdPersonDispatch,
    pub fp_dispatch: Option<super::fp_dispatch::FirstPersonDispatch>,
    pub suffix: Option<super::suffix::ActionSuffix>,
}
pub(super) fn build(
    player: &Owned,
    hook: &crate::game::world_hooks::Attempt,
) -> Result<InitialAction> {
    let owner = player
        .action
        .as_ref()
        .context("initial action owner is absent")?;
    let state = &owner.state;
    if let Some(snapshot) = &owner.completed_evidence {
        ensure!(
            state.stage() == Stage::Returned
                && state.player() == player.base.player()
                && state.generic() == snapshot.generic_request
                && snapshot.action_invocation_returned
                && !snapshot.callback_returned
                && snapshot.stage == "action_dispatch_repeat_pending",
            "initial action snapshot differs from its completed invocation"
        );
        return Ok(snapshot.clone());
    }
    let tp = player
        .components
        .preparation
        .animator()
        .context("initial action TP owner is absent")?;
    let fp = player
        .components
        .preparation
        .first_person()
        .context("initial action FP owner is absent")?
        .animator();
    ensure!(
        state.first_person_dispatch().is_some()
            || [0x138, 0x158, 0x178, 0x198]
                .into_iter()
                .all(|at| fp.word(at) == Some(0) && fp.word(at + 4) == Some(0)),
        "FP controls were reached before TP selector return"
    );
    ensure!(
        matches!(
            state.stage(),
            Stage::AwaitingThirdPersonCommand
                | Stage::AwaitingThirdPersonSetterTail
                | Stage::AwaitingThirdPersonDispatchTail
                | Stage::AwaitingFirstPersonDispatch
                | Stage::FirstPersonDispatch
                | Stage::AwaitingActionSuffix
                | Stage::ApplyingActionSuffix
                | Stage::Returned
        ) && state.player() == player.base.player()
            && state.resolved().len() == 3
            && owner.mappings.len() == 3
            && state.generic() == 0x0900_071e
            && player.reset.byte(0xc7c) == Some(0xff)
            && player.base.word(0x704) == Some(0),
        "initial action caller boundary differs"
    );
    let mut hash = Sha256::new();
    hash.update(b"ge_player_initial_action_maps_v1\0");
    hash.update(player.loadout.provenance().state_sha256.as_bytes());
    for ((resolved, mapped), expected) in state.resolved().iter().zip(&owner.mappings).zip([
        Target::FirstPerson,
        Target::ThirdPerson,
        Target::ThirdPersonSecondary,
    ]) {
        ensure!(
            resolved.request.target == expected
                && resolved.request.generic == mapped.generic
                && Some(mapped.source_weapon) == state.weapon()
                && Some(mapped.variant) == state.variant()
                && resolved.output == mapped.output
                && resolved.mapped == mapped.mapped,
            "initial action remap order or retained table result differs"
        );
        for word in [
            mapped.source_weapon,
            mapped.generic,
            mapped.variant,
            mapped.output,
            u32::from(mapped.mapped),
            mapped.map_rows_read as u32,
        ] {
            hash.update(word.to_be_bytes());
        }
        hash.update((mapped.steps.len() as u64).to_be_bytes());
        for step in &mapped.steps {
            hash.update(step.weapon.to_be_bytes());
            hash.update(step.candidate_raw.to_be_bytes());
            for value in [step.record_source_index, step.matched_source_ordinal] {
                hash.update([u8::from(value.is_some())]);
                if let Some(value) = value {
                    hash.update((value as u64).to_be_bytes());
                }
            }
            hash.update([match step.available {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            }]);
        }
    }
    let tp_dispatch = super::dispatch::build(player)?;
    let fp_dispatch = super::fp_dispatch::build(player)?;
    if let Some(fp) = state
        .first_person_dispatch()
        .filter(|fp| fp.setter().is_some_and(|s| s.history_updated))
    {
        let setter = fp.setter().expect("checked history receipt");
        ensure!(
            player.reset.word(0x70c) == Some(fp.generic())
                && player.reset.word(0x714) == Some(fp.requested())
                && (setter.primary_changed != Some(true) || player.reset.word(0x710) == Some(0)),
            "FP setter history differs from actual player writes"
        );
    } else if tp_dispatch.history_updated {
        ensure!(
            player.reset.word(0x70c) == Some(state.generic())
                && player.reset.word(0x714) == Some(state.resolved()[1].output)
                && (tp_dispatch.primary_changed != Some(true)
                    || player.reset.word(0x710) == Some(0)),
            "TP setter history differs from actual player writes"
        );
    } else {
        ensure!(
            [0x70c, 0x710, 0x714, 0x718]
                .into_iter()
                .all(|at| player.reset.word(at) == Some(0)),
            "fresh TP path changed history without a setter write"
        );
    }
    Ok(InitialAction {
        stage: super::selector::boundary(state)?.0,
        generic_request: state.generic(),
        parameter_bits: 0,
        direction_raw: 0xff,
        weapon_key: state.weapon().context("initial weapon is unknown")?,
        variant: state.variant().context("initial variant is unknown")?,
        remap_calls: owner.mappings.len(),
        map_rows_read: owner
            .mappings
            .iter()
            .map(|result| result.map_rows_read)
            .sum(),
        fp_mapped: state.resolved()[0].output,
        tp_mapped: state.resolved()[1].output,
        tp_secondary_mapped: state.resolved()[2].output,
        source_tables_reused: true,
        mode_controls_created: tp.controls().len(),
        mode_history_updated: tp_dispatch.history_updated
            || fp_dispatch.as_ref().is_some_and(|fp| fp.history_updated),
        callback_returned: false,
        action_invocation_returned: state.stage() == Stage::Returned,
        state_sha256: super::super::words(b"ge_player_initial_action_v2\0", |emit| {
            state.visit_state_words(emit)
        }),
        mapping_state_sha256: format!("{:x}", hash.finalize()),
        tp_capture: super::control_capture::build(player, hook)?,
        tp_selector: super::selector::build(player)?,
        tp_command: super::command::build(player)?,
        tp_dispatch,
        fp_dispatch,
        suffix: super::suffix::build(player)?,
    })
}
