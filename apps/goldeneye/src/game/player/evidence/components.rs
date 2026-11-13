use super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    components::{Stage, TextureOutcome},
    world::World,
};
use serde::Serialize;
mod action;
mod attachment;
mod callback_tail;
mod callbacks;
mod child;
mod command;
mod component_tail;
mod constructor_tail;
mod control_capture;
mod dispatch;
mod first_person;
mod fp_dispatch;
mod initializer_prefix;
mod mask;
mod pose_setup;
mod postbind;
mod refresh;
mod repeat;
mod selector;
mod skeleton;
mod suffix;
pub use action::InitialAction;
pub use attachment::AttachmentBinding;
pub use callback_tail::CallbackTail;
pub use callbacks::CallbackBinding;
pub use child::ChildHeader;
pub use command::CommandBody;
pub use component_tail::ComponentTail;
pub use constructor_tail::ConstructorTail;
pub use control_capture::ThirdPersonCapture;
pub use first_person::FirstPersonBinding;
pub use pose_setup::PoseSetup;
pub use postbind::VisualPostbind;
pub use refresh::BindRefresh;
pub use repeat::RepeatAction;
pub use selector::ThirdPersonSelector;
pub use skeleton::SkeletonBinding;

pub(super) fn capture_initial_action(
    player: &Owned,
    hook: &crate::game::world_hooks::Attempt,
) -> Result<InitialAction> {
    action::build(player, hook)
}
pub(super) fn capture_callback_tail(player: &Owned, world: &World) -> Result<CallbackTail> {
    callback_tail::build(player, world)?.context("returned callback tail evidence absent")
}
pub(super) fn capture_constructor_entry(player: &Owned) -> Result<(RepeatAction, ConstructorTail)> {
    Ok((
        repeat::build(player)?.context("returned repeat receipt absent")?,
        constructor_tail::build(player)?.context("constructor helper receipt absent")?,
    ))
}
pub(super) fn capture_pose_setup(player: &Owned, world: &World) -> Result<PoseSetup> {
    pose_setup::build(player, world)?.context("returned pose helper evidence absent")
}
pub(super) fn capture_component_tail(player: &Owned, world: &World) -> Result<ComponentTail> {
    component_tail::build(player, world)?.context("returned outer component helper evidence absent")
}

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
    pub visual_postbind: VisualPostbind,
    pub callbacks: CallbackBinding,
    pub first_person: FirstPersonBinding,
    pub initial_action: InitialAction,
    pub repeat_action: Option<repeat::RepeatAction>,
    pub callback_tail: Option<callback_tail::CallbackTail>,
    pub constructor_tail: Option<constructor_tail::ConstructorTail>,
    pub pose_setup: Option<PoseSetup>,
    pub component_tail: Option<component_tail::ComponentTail>,
    pub initializer_prefix: Option<initializer_prefix::InitializerPrefix>,
    pub scope: &'static str,
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    identities: &ge_core::mission::player::components::Registry,
    hook: &crate::game::world_hooks::Attempt,
) -> Result<Option<ComponentInitialization>> {
    if player.flag.is_none() {
        return Ok(None);
    }
    let preparation = &player.components.preparation;
    ensure!(
        preparation.stage() == Stage::AwaitingInitialModes,
        "component preparation has not reached the TP/FP helper returns and FP masks"
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
            && world.item(receipt.item())?.components().first() == Some(&owner.identity().id())
            && preparation.textures().is_some(),
        "TP owner, source or attachment boundary differs"
    );
    let action = action::build(player, hook)?;
    let repeat_action = repeat::build(player)?;
    let callback_tail = callback_tail::build(player, world)?;
    let constructor_tail = constructor_tail::build(player)?;
    let pose_setup = pose_setup::build(player, world)?;
    let component_tail = component_tail::build(player, world)?;
    let initializer_prefix = initializer_prefix::build(player, world, hook)?;
    let (stage, pending_dependency) = if let Some(prefix) = &initializer_prefix {
        match prefix.stage {
            "awaiting_preload" => (
                "player_animation_preload_pending",
                "player_animation_preload",
            ),
            "caches" => ("player_transform_caches_pending", "player_transform_caches"),
            "awaiting_tail" => ("player_initializer_tail_pending", "player_initializer_tail"),
            _ => ("player_initializer_pending", "player_initializer"),
        }
    } else if component_tail.as_ref().is_some_and(|t| t.helper_returned) {
        ("player_initializer_pending", "player_initializer")
    } else if pose_setup.as_ref().is_some_and(|p| p.helper_returned) {
        ("player_component_tail_pending", "player_component_tail")
    } else if constructor_tail
        .as_ref()
        .is_some_and(|t| t.stage == "awaiting_pose")
    {
        ("player_pose_setup_pending", "player_pose_setup")
    } else if callback_tail.as_ref().is_some_and(|t| t.callback_returned) {
        ("player_constructor_tail_pending", "player_constructor_tail")
    } else if repeat_action
        .as_ref()
        .is_some_and(|r| r.action_invocation_returned)
    {
        ("player_callback_tail_pending", "player_callback_tail")
    } else {
        selector::boundary(&player.action.as_ref().context("TP action absent")?.state)?
    };
    Ok(Some(ComponentInitialization {
        stage,
        pending_dependency,
        native_component_bytes: 0x1b8,
        constructed_slots: 4,
        active_slots: usize::from(owner.byte(0x130).context("TP active count unknown")?),
        retained_file_bytes: target.file_bytes(),
        retained_section_bytes: target.section_bytes(),
        third_person_constructed: true,
        third_person_attached: true,
        first_person_constructed: true,
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
        visual_postbind: postbind::build(player, world)?,
        callbacks: callbacks::build(player, world, hook)?,
        first_person: first_person::build(player, world, identities, hook)?,
        initial_action: action,
        repeat_action,
        callback_tail,
        constructor_tail,
        pose_setup,
        component_tail,
        initializer_prefix,
        scope: "TP selection and command effects use the same resident component base. Returned class-four factories copy the old head position, insert typed list members and apply scalar/post-word writes in source order. Only returned bodies are acknowledged before selector continuation. The setter preserves its actual tracked/untracked history gates. TP dispatch applies reached blend-coordinate writes and re-enters existing scripts with initflag1, retaining their file/cache/clip owners and separate update traces. Other factory classes, wrapper allocation and cleanup remain explicit dependencies when reached. The first action receipt stays immutable while the repeated action records its own setters, captured seek, override stores and second suffix. Callback return comes only from its separate tail receipt. Constructor continuation, pose postprocessing, scheduling, completed player and camera remain incomplete.",
    }))
}
