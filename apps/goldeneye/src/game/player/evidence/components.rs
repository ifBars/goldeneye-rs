use super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    components::{Stage, TextureOutcome},
    world::World,
};
use serde::Serialize;
mod attachment;
mod refresh;
mod skeleton;
pub use attachment::AttachmentBinding;
pub use refresh::BindRefresh;
pub use skeleton::SkeletonBinding;

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
    pub texture_directory_outcome: &'static str,
    pub texture_requests: usize,
    pub texture_targets_resident: usize,
    pub texture_residency_complete: bool,
    pub texture_requests_state_sha256: String,
    pub component_binding_complete: bool,
    pub preparation_state_sha256: String,
    pub target_state_sha256: String,
    pub skeleton_binding: SkeletonBinding,
    pub bind_refresh: BindRefresh,
    pub attachment: AttachmentBinding,
    pub scope: &'static str,
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    identities: &ge_core::mission::player::components::Registry,
) -> Result<Option<ComponentInitialization>> {
    if player.flag.is_none() {
        return Ok(None);
    }
    let preparation = &player.components.preparation;
    ensure!(
        preparation.stage() == Stage::AwaitingVisualPostbind,
        "component preparation has not reached its actual attachment return"
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
    let textures = preparation
        .textures()
        .context("TP texture enumeration is absent")?;
    ensure!(
        preparation.texture_requests() == textures.hashes.len()
            && player.components.textures.len() == textures.hashes.len(),
        "TP texture residency prefix is incomplete"
    );
    for (owner, &hash) in player.components.textures.iter().zip(&textures.hashes) {
        ensure!(
            target.same_source(owner) && owner.target() == hash,
            "TP texture target order or source owner differs"
        );
    }
    let mut texture_hash = sha2::Sha256::new();
    use sha2::Digest;
    texture_hash.update(b"ge_player_texture_target_requests_v1\0");
    for owner in &player.components.textures {
        texture_hash.update(owner.fingerprint().as_bytes());
    }
    ensure!(
        owner.player() == player.base.player()
            && target.file() == visual.file()
            && target.target() == visual.skin()
            && world.item(receipt.item())?.components() == [owner.identity().id()]
            && preparation.textures().is_some(),
        "TP owner, source or attachment boundary differs"
    );
    Ok(Some(ComponentInitialization {
        stage: "tp_visual_postbind_pending",
        pending_dependency: "tp_visual_postbind",
        native_component_bytes: 0x1b8,
        constructed_slots: 4,
        active_slots: usize::from(owner.byte(0x130).context("TP active count unknown")?),
        retained_file_bytes: target.file_bytes(),
        retained_section_bytes: target.section_bytes(),
        third_person_constructed: true,
        third_person_attached: true,
        first_person_constructed: false,
        skin_request_resolved: true,
        texture_dependencies_enumerated: textures.outcome == TextureOutcome::Enumerated,
        texture_directory_outcome: match textures.outcome {
            TextureOutcome::Enumerated => "enumerated",
            TextureOutcome::EntityDirectoryAbsent => "entity_directory_absent",
            TextureOutcome::TextureDirectoryAbsent => "texture_directory_absent",
        },
        texture_requests: preparation.texture_requests(),
        texture_targets_resident: player.components.textures.len(),
        texture_residency_complete: true,
        texture_requests_state_sha256: format!("{:x}", texture_hash.finalize()),
        component_binding_complete: true,
        preparation_state_sha256: super::words(b"ge_player_component_preparation_v1\0", |emit| {
            preparation.visit_state_words(emit)
        }),
        target_state_sha256: target.fingerprint(),
        skeleton_binding: skeleton::build(player)?,
        bind_refresh: refresh::build(player)?,
        attachment: attachment::build(player, world, identities)?,
        scope: "The retained TP animator has a distinct host component identity shared by its skeletal backlink, bind return and actual item membership. Attachment writes its item backlink and clears byte8e before appending the first component to the item list. No activation or callback is inferred from membership. The prior zero-time callback uses the controller's shared scratch and releases its scope before bind success. Visual postbind, FP construction, player completion, sampled pose and camera remain incomplete. Host identities do not establish native allocation/SDK registry history; native timing, streaming and lifetime parity remain unverified.",
    }))
}
