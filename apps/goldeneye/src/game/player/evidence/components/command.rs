use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::animation::layers::command::Stage;
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandBody {
    pub stage: &'static str,
    pub opcode: u32,
    pub set_reference: Option<u32>,
    pub set_index: Option<usize>,
    pub set_groups: Option<usize>,
    pub groups_visited: usize,
    pub weight_writes: usize,
    pub pending_group: Option<usize>,
    pub pending_slot: Option<u8>,
    pub child_reference: Option<u32>,
    pub factory_class: Option<u8>,
    pub child_flags: Option<u16>,
    pub child_divisor: Option<u32>,
    pub mask: Option<u16>,
    pub children_created: usize,
    pub command_returned: bool,
    pub cleanup_complete: bool,
    pub selector_acknowledged: bool,
    pub state_sha256: String,
    pub layers_state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<Option<CommandBody>> {
    let action = player.action.as_ref().context("TP command action absent")?;
    let Some(command) = action.state.command() else {
        ensure!(
            action.tp_command.is_none() && action.tp_command_stream.is_none(),
            "TP command body was skipped"
        );
        return Ok(None);
    };
    ensure!(
        action
            .tp_command
            .is_some_and(|location| location.opcode() == command.opcode())
            && action.tp_command_stream.is_some(),
        "TP command source binding differs"
    );
    let stage = match command.stage() {
        Stage::ChildFactoryPending => "child_factory_pending",
        Stage::CleanupPending => "cleanup_pending",
        Stage::MaskControlPending => "mask_control_pending",
        Stage::MaskClearPending => "mask_clear_pending",
        Stage::OpcodePending => "opcode_pending",
        _ => anyhow::bail!("TP command has no reportable effect boundary"),
    };
    let owner = player
        .components
        .preparation
        .animator()
        .context("TP command animator absent")?;
    let layers = owner.layers().context("TP command layer owner absent")?;
    ensure!(
        !layers.failed()
            && owner.controls().len() == 4
            && layers.active_count() == 1
            && layers
                .slots()
                .iter()
                .enumerate()
                .all(|(i, slot)| owner.word(0x144 + i * 0x20) == Some(slot.weight_bits())),
        "TP command weight prefix differs from actual owner"
    );
    let child = command.child();
    Ok(Some(CommandBody {
        stage,
        opcode: command.opcode(),
        set_reference: command.set_reference(),
        set_index: command.set_index(),
        set_groups: command.group_count(),
        groups_visited: command.groups().len(),
        weight_writes: command.weight_writes(),
        pending_group: child.map(|child| child.group),
        pending_slot: child.map(|child| child.slot),
        child_reference: child.map(|child| child.reference),
        factory_class: child.map(|child| ((child.reference >> 24) & 0x7f) as u8),
        child_flags: child.map(|child| child.effective_flags),
        child_divisor: child.map(|child| child.divisor),
        mask: command.mask(),
        children_created: 0,
        command_returned: false,
        cleanup_complete: false,
        selector_acknowledged: false,
        state_sha256: super::super::words(b"ge_player_tp_command_prefix_v1\0", |emit| {
            command.visit_state_words(emit)
        }),
        layers_state_sha256: super::super::words(b"ge_player_tp_command_layers_v1\0", |emit| {
            layers.visit_state_words(emit)
        }),
    }))
}
