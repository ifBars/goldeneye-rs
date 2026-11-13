use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::{animation::mode::runtime, mission::player::actions::FirstPersonStage};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FirstPersonDispatch {
    pub stage: &'static str,
    pub component_file: u32,
    pub qualification_skipped: bool,
    /// Dispatcher-entry capture only; later mask commands retain their own proof.
    pub capture_performed: bool,
    pub initial_heads_empty: bool,
    pub fallback: u32,
    pub fallback_writes: u64,
    pub selector: super::ThirdPersonSelector,
    pub command: Option<super::CommandBody>,
    pub mask_capture: Option<super::mask::MaskEffects>,
    pub mask_commands_returned: usize,
    pub setter_return_word: Option<u32>,
    pub history_updated: bool,
    pub primary_changed: Option<bool>,
    pub coordinate_written: Option<bool>,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<Option<FirstPersonDispatch>> {
    let action = player.action.as_ref().context("FP action absent")?;
    let Some(fp) = action.state.first_person_dispatch() else {
        return Ok(None);
    };
    let owner = player
        .components
        .preparation
        .first_person()
        .context("FP animator owner absent")?
        .animator();
    let source = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("FP component source absent")?;
    let cursor = fp.selector().context("FP cursor absent")?;
    let (fallback, writes) = fp.fallback().context("FP fallback snapshot absent")?;
    ensure!(
        owner.identity() == fp.identity()
            && owner.player() == player.base.player()
            && source.file() == 0x0102_0016
            && owner.component().is_some_and(|b| b.file == source.file())
            && fp.generic() == action.state.generic()
            && fp.requested() == action.state.resolved()[0].output
            && fp.initial_heads_empty()
            && fp.primary_head() == Some((None, 0))
            && fallback == 0x0900_065e
            && action.state.fallback_writes().is_some_and(|v| writes > v)
            && cursor.inputs().requested == fp.requested()
            && cursor.inputs().previous == 0
            && cursor.inputs().fallback == fallback
            && cursor.inputs().resource_available
            && cursor.completed_commands() == fp.completed_commands().len(),
        "FP receiver, entry snapshot or selector inputs differ"
    );
    let stage = match fp.stage() {
        FirstPersonStage::CommandPending => {
            ensure!(
                cursor.stage() == runtime::Stage::CommandPending
                    && cursor.pending().is_some()
                    && cursor.return_word().is_none(),
                "FP pending command differs from selector"
            );
            "command_pending"
        }
        FirstPersonStage::SetterTail => "setter_tail_pending",
        FirstPersonStage::DispatchTail => "dispatch_tail_pending",
        FirstPersonStage::Returned => "returned",
        _ => anyhow::bail!("FP dispatcher has no reportable boundary"),
    };
    if stage != "command_pending" {
        ensure!(
            cursor.stage() == runtime::Stage::Complete
                && cursor.pending().is_none()
                && cursor.return_word().is_some(),
            "FP returned selector differs from dispatcher"
        );
    }
    let command = if let Some(command) = fp.command() {
        ensure!(
            action
                .fp_command
                .is_some_and(|location| location.opcode() == command.opcode())
                && action.fp_command_stream.is_some(),
            "FP command source binding differs"
        );
        Some(super::command::build_for(
            owner,
            command,
            fp.command_acknowledged(),
        )?)
    } else {
        ensure!(
            action.fp_command.is_none()
                && action.fp_command_stream.is_none()
                && cursor.completed_commands() == 0,
            "FP command receipt disappeared"
        );
        None
    };
    let mask_commands_returned = fp
        .completed_commands()
        .iter()
        .filter(|c| c.opcode() == 0x0c00_0005)
        .count();
    let mask_capture = fp
        .completed_commands()
        .iter()
        .rev()
        .find(|c| c.opcode() == 0x0c00_0005 && c.mask().is_some_and(|mask| mask != 0))
        .map(|command| super::mask::build(owner, command))
        .transpose()?
        .flatten();
    let setter = fp.setter();
    ensure!(
        match fp.stage() {
            FirstPersonStage::CommandPending | FirstPersonStage::SetterTail =>
                setter.is_none() && fp.coordinate_written().is_none(),
            FirstPersonStage::DispatchTail => setter.is_some() && fp.coordinate_written().is_none(),
            FirstPersonStage::Returned => setter.is_some() && fp.coordinate_written().is_some(),
            _ => false,
        },
        "FP setter and dispatch helper order differs"
    );
    Ok(Some(FirstPersonDispatch {
        stage,
        component_file: source.file(),
        qualification_skipped: true,
        capture_performed: false,
        initial_heads_empty: fp.initial_heads_empty(),
        fallback,
        fallback_writes: writes,
        selector: super::selector::report(
            cursor,
            true,
            if cursor.stage() == runtime::Stage::CommandPending {
                "command_pending"
            } else {
                "returned"
            },
            action.fp_command.map(|c| c.opcode()),
        ),
        command,
        mask_capture,
        mask_commands_returned,
        setter_return_word: setter.map(|s| s.return_word),
        history_updated: setter.is_some_and(|s| s.history_updated),
        primary_changed: setter.and_then(|s| s.primary_changed),
        coordinate_written: fp.coordinate_written(),
        state_sha256: super::super::words(b"ge_player_fp_dispatch_v1\0", |emit| {
            fp.visit_state_words(emit)
        }),
    }))
}
