use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::animation::{
    nodes::Kind,
    script_child::{self, Stage},
};
use serde::Serialize;
mod phase_zero;
mod position;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChildHeader {
    pub native_header_bytes: usize,
    pub resource_reused: bool,
    pub context_bound: bool,
    pub generation_observed: u32,
    pub cache_generation: u32,
    pub metadata_cached: bool,
    pub descriptor_index: Option<usize>,
    pub body_bound: bool,
    pub phase_zero: Option<phase_zero::PhaseZero>,
    pub phase_zero_initialized: bool,
    pub initializer_returned: bool,
    pub factory_returned: bool,
    pub layer_inserted: bool,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<Option<ChildHeader>> {
    let command = player
        .action
        .as_ref()
        .context("TP action absent")?
        .state
        .command();
    let Some(command) = command else {
        return Ok(None);
    };
    let Some(id) = command.factory_node() else {
        return Ok(None);
    };
    let request = command.child().context("TP child factory request absent")?;
    let animator = player
        .components
        .preparation
        .animator()
        .context("TP child animator absent")?;
    let header = animator
        .script_children()
        .get(id)
        .context("retained TP child header absent")?;
    let binding = animator.component().context("TP current resource absent")?;
    let returned = matches!(
        header.stage(),
        Stage::ReturnedWithoutBody | Stage::ReturnedWithoutPlay | Stage::ReturnedInitialized
    );
    let phase_zero = phase_zero::build(header)?;
    let expected_flags = if let Some(flags) = header.initialization().and_then(|t| t.flags_raw) {
        if flags & 1 != 0 {
            u32::from(request.effective_flags) | 0x80
        } else {
            u32::from(request.effective_flags) & !0x80
        }
    } else {
        u32::from(request.effective_flags)
    };
    ensure!(
        (returned || header.stage() == Stage::AwaitingInitializer || phase_zero.is_some())
            && header.owner().get() == animator.identity().id().get()
            && animator.node_ids().kind(id) == Some(Kind::Script)
            && header.file() == binding.file
            && header.reference() == request.reference
            && header.slot() == Some(request.slot)
            && header.word(0x20) == Some(expected_flags)
            && header.word(0x1c).is_none()
            && header.word(0).is_none()
            && header.word(4).is_none()
            && binding.reference == request.reference
            && animator.word(0x20) == Some(request.reference)
            && animator.word(0x24) == Some(u32::MAX)
            && animator.script_children().len() == 1
            && animator
                .layers()
                .context("TP layers absent")?
                .slots()
                .iter()
                .all(|slot| !slot.nodes_head().contains(&id)),
        "TP child factory prefix or ownership differs"
    );
    Ok(Some(ChildHeader {
        native_header_bytes: script_child::HEADER_BYTES,
        resource_reused: true,
        context_bound: true,
        generation_observed: header
            .observed_generation()
            .context("TP child generation read absent")?,
        cache_generation: header
            .word(0x14)
            .context("TP child cache generation unknown")?,
        metadata_cached: header.cached().is_some(),
        descriptor_index: header.cached().map(|binding| binding.directory_index),
        body_bound: header
            .body_bound()
            .context("TP script body binding absent")?,
        phase_zero,
        phase_zero_initialized: matches!(
            header.stage(),
            Stage::ReturnedWithoutPlay | Stage::ReturnedInitialized
        ),
        initializer_returned: returned,
        factory_returned: returned,
        layer_inserted: false,
        state_sha256: super::super::words(b"ge_player_tp_class04_header_v3\0", |emit| {
            header.visit_state_words(emit)
        }),
    }))
}
