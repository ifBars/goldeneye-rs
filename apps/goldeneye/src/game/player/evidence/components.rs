use super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{components::Stage, world::World};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComponentInitialization {
    pub stage: &'static str,
    pub pending_dependency: &'static str,
    pub native_component_bytes: usize,
    pub constructed_slots: usize,
    pub active_slots: usize,
    pub retained_file_bytes: usize,
    pub retained_section_bytes: usize,
    pub third_person_constructed: bool,
    pub third_person_attached: bool,
    pub first_person_constructed: bool,
    pub skin_request_resolved: bool,
    pub texture_dependencies_enumerated: bool,
    pub component_binding_complete: bool,
    pub preparation_state_sha256: String,
    pub target_state_sha256: String,
    pub scope: &'static str,
}
pub(super) fn build(player: &Owned, world: &World) -> Result<Option<ComponentInitialization>> {
    if player.flag.is_none() {
        return Ok(None);
    }
    let preparation = &player.components.preparation;
    ensure!(
        preparation.stage() == Stage::AwaitingTextures,
        "component preparation has not reached its actual skin request"
    );
    let owner = preparation
        .animator()
        .context("TP constructor owner is absent")?;
    let target = player
        .components
        .target
        .as_ref()
        .context("TP skin residency owner is absent")?;
    let visual = preparation.visual().context("TP visual plan is absent")?;
    let receipt = player
        .virtual_init
        .completion()
        .context("TP basic receipt is absent")?;
    ensure!(
        owner.player() == player.base.player()
            && target.file() == visual.file()
            && target.target() == visual.skin()
            && world.item(receipt.item())?.components().is_empty(),
        "TP owner, source or attachment boundary differs"
    );
    Ok(Some(ComponentInitialization {
        stage: "tp_texture_dependencies_pending",
        pending_dependency: "skin_texture_dependencies",
        native_component_bytes: 0x1b8,
        constructed_slots: 4,
        active_slots: usize::from(owner.byte(0x130).context("TP active count unknown")?),
        retained_file_bytes: target.file_bytes(),
        retained_section_bytes: target.section_bytes(),
        third_person_constructed: true,
        third_person_attached: false,
        first_person_constructed: false,
        skin_request_resolved: true,
        texture_dependencies_enumerated: false,
        component_binding_complete: false,
        preparation_state_sha256: super::words(b"ge_player_component_preparation_v1\0", |emit| {
            preparation.visit_state_words(emit)
        }),
        target_state_sha256: target.fingerprint(),
        scope: "Actual basic return precedes the repeated clear, fresh cleanup, same-inventory slot transition and TP allocation. Direct visual lookup uses retained primary records without consuming a group cursor or RNG. The actual skin request shares its resource wrapper and logical section record; both remain owned across cancellation. MAX texture token requires geometry/material dependency enumeration, still pending. No TP attachment, FP construction, component bind, player completion, sampled pose or camera is claimed. Native streaming/history and lifetime parity remain unverified.",
    }))
}
