use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::{animation::mode::runtime, mission::player::actions::Stage};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThirdPersonSelector {
    pub stage: &'static str,
    pub requested: u32,
    pub previous: u32,
    pub fallback: u32,
    pub tracks_player_modes: bool,
    pub target_index: Option<usize>,
    pub attempts: usize,
    pub lookups: usize,
    pub transition_rows: usize,
    pub commands_issued: usize,
    pub commands_executed: usize,
    pub pending_ordinal: Option<usize>,
    pub pending_opcode: Option<u32>,
    pub return_word: Option<u32>,
    pub base_only: bool,
    pub command_effects_complete: bool,
    pub state_sha256: String,
}
pub(super) fn boundary(stage: Stage) -> Result<(&'static str, &'static str)> {
    match stage {
        Stage::AwaitingThirdPersonCommand => Ok(("tp_command_pending", "tp_animation_command")),
        Stage::AwaitingThirdPersonSetterTail => {
            Ok(("tp_setter_tail_pending", "tp_mode_setter_tail"))
        }
        _ => anyhow::bail!("TP selector has no reportable pending boundary"),
    }
}
pub(super) fn build(player: &Owned) -> Result<ThirdPersonSelector> {
    let owner = player
        .action
        .as_ref()
        .context("TP action owner is absent")?;
    let cursor = owner
        .state
        .selector()
        .context("TP live selector is absent")?;
    let pending = cursor.pending();
    let returned = cursor.return_word();
    let command = owner.tp_command;
    let stage = match owner.state.stage() {
        Stage::AwaitingThirdPersonCommand => {
            ensure!(
                cursor.stage() == runtime::Stage::CommandPending
                    && pending.is_some()
                    && command.is_some()
                    && returned.is_none(),
                "TP pending command and live cursor differ"
            );
            "command_pending"
        }
        Stage::AwaitingThirdPersonSetterTail => {
            ensure!(
                cursor.stage() == runtime::Stage::Complete
                    && pending.is_none()
                    && (command.is_none() || owner.state.command_acknowledged())
                    && returned.is_some(),
                "TP returned selector and setter boundary differ"
            );
            "returned"
        }
        _ => anyhow::bail!("TP selector has not reached a reportable boundary"),
    };
    ensure!(
        cursor.completed_commands() == owner.state.completed_commands().len(),
        "TP command returns differ from actual acknowledged bodies"
    );
    let inputs = cursor.inputs();
    ensure!(
        inputs.resource_available
            && inputs.requested == owner.state.resolved()[1].output
            && Some(inputs.fallback) == owner.state.fallback(),
        "TP live selector inputs differ"
    );
    let tracks = owner
        .state
        .tracks_player_modes()
        .context("TP qualification is absent")?;
    let primary = player
        .components
        .preparation
        .animator()
        .context("TP primary owner absent")?
        .layers()
        .context("TP primary layers absent")?
        .slots()[0]
        .nodes_head()
        .first()
        .copied();
    if tracks {
        let saved = owner
            .state
            .primary_head()
            .context("TP saved primary comparison absent")?;
        ensure!(
            saved.1 == 0
                && saved.0.is_some_and(|id| player
                    .components
                    .preparation
                    .animator()
                    .is_some_and(|a| a.controls().get(id).is_some()))
                && (cursor.completed_commands() > 0 || saved.0 == primary),
            "TP saved primary comparison differs from entry capture"
        );
    } else {
        ensure!(
            owner.state.primary_head().is_none(),
            "untracked TP path has a primary comparison"
        );
    }
    ensure!(
        Some(inputs.previous) == player.reset.word(if tracks { 0x714 } else { 0x70c }),
        "TP previous mode differs from actual player history"
    );
    let work = cursor.work();
    Ok(ThirdPersonSelector {
        stage,
        requested: inputs.requested,
        previous: inputs.previous,
        fallback: inputs.fallback,
        tracks_player_modes: tracks,
        target_index: cursor.target(),
        attempts: cursor.attempts().len(),
        lookups: work.lookups,
        transition_rows: work.transition_rows,
        commands_issued: work.commands,
        commands_executed: cursor.completed_commands(),
        pending_ordinal: pending.map(|value| value.ordinal),
        pending_opcode: pending.and_then(|_| command.map(|value| value.opcode())),
        return_word: returned,
        base_only: true,
        command_effects_complete: cursor.stage() == runtime::Stage::Complete
            && cursor.completed_commands() > 0,
        state_sha256: super::super::words(b"ge_player_tp_live_selector_v1\0", |mut emit| {
            cursor.visit_state_words(&mut emit);
            emit(u32::from(tracks));
            emit(u32::from(command.is_some()));
            if let Some(command) = command {
                emit(command.opcode());
            }
        }),
    })
}
