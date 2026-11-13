//! Source constructor pose helper. A return here does not return the enclosing
//! constructor, publish a player, sample animation or establish a camera.
use super::{CharacterBinding, PendingReset, continuation};
use crate::mission::player::{
    base_handler,
    components::{EnableWrite, Identity, Preparation},
    identity::PlayerId,
    item::Item,
};
use crate::mission::session::UserId;
use anyhow::{Context, Result, ensure};
mod advance;
mod state;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterField {
    At10,
    At14,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterLookup {
    Bound(CharacterBinding),
    KnownAbsent,
}
pub trait Source {
    /// A sparse missing binding is unknown. Only a real provider may supply the
    /// explicit native NULL outcome needed by the following component tail.
    fn constructor_character(
        &mut self,
        binding: Option<CharacterBinding>,
    ) -> Result<CharacterLookup> {
        Ok(CharacterLookup::Bound(
            binding.context("constructor character ownership unknown")?,
        ))
    }
    fn character_scalar(&mut self, binding: CharacterBinding, field: CharacterField)
    -> Result<u32>;
    fn multiple_local_users_in_mode_three(&mut self) -> Result<bool>;
    fn ones_vector(&mut self) -> Result<[u32; 3]>;
    fn local_user_for_player(&mut self, player: PlayerId) -> Result<Option<UserId>>;
    /// None retains the reached live input as unknown, never as value zero.
    fn weapon_global_byte(&mut self) -> Result<Option<u8>>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Entry,
    BlendGate,
    EnableArguments,
    ThirdPersonEnable,
    FirstPersonEnable,
    CharacterBinding,
    Character10,
    Character14,
    SessionScale,
    Scale,
    ScaleInvalidate,
    Dirty,
    WeaponMode,
    ForceComponents,
    UserTarget,
    UserLookup,
    UserFalseArm,
    BacklinkComponents,
    Attachment,
    GlobalByte,
    GlobalAttachment,
    FadeInputs,
    FinalSession,
    Returned,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    BlendThreshold,
    SkeletonEnable,
    MultiplayerScale,
    WeaponMode,
    AttachmentOwner,
    UserTarget,
    UserMatched,
    ComponentBacklink,
    GlobalArgument,
    FadeAttachment,
    FinalMultiplayer,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BlendThreshold => "pose_blend_threshold",
            Self::SkeletonEnable => "pose_skeleton_enable",
            Self::MultiplayerScale => "pose_multiplayer_scale",
            Self::WeaponMode => "pose_weapon_mode",
            Self::AttachmentOwner => "pose_attachment_owner",
            Self::UserTarget => "pose_user_target",
            Self::UserMatched => "pose_matched_user",
            Self::ComponentBacklink => "pose_component_backlink",
            Self::GlobalArgument => "pose_global_argument",
            Self::FadeAttachment => "pose_fade_attachment",
            Self::FinalMultiplayer => "pose_final_multiplayer",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserLookup {
    NotFound,
    Matched(UserId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalArgument {
    Known(i8),
    ArgumentUnreadBecauseReceiverAbsent,
}

#[derive(Debug)]
pub struct Pending {
    player: PlayerId,
    input_raw: u8,
    stage: Stage,
    dependency: Option<Dependency>,
    entry_702: Option<u8>,
    blend_ba8: Option<u8>,
    bab_written: bool,
    tp_enabled: Option<bool>,
    fp_enabled: Option<bool>,
    receiver_searches: [bool; 2],
    enable_writes: [Option<EnableWrite>; 2],
    enable_requests: [Option<(Identity, u32)>; 2],
    character_binding: Option<CharacterBinding>,
    character_scalars: [Option<u32>; 2],
    character_vector: Option<[u32; 4]>,
    session_scale_multiple: Option<bool>,
    scale_bits: Option<[u32; 4]>,
    scale_stored: bool,
    scale_invalidated: bool,
    dirty_before: Option<u8>,
    dirty_after: Option<u8>,
    ca4_written: bool,
    mode_afc_absent: Option<bool>,
    force_nodes_checked: usize,
    user_target: Option<PlayerId>,
    user_lookup: Option<UserLookup>,
    false_arm_b00_absent: Option<bool>,
    backlink_nodes_checked: usize,
    attachment_absent: Option<bool>,
    weapon_global_read: bool,
    weapon_global_byte: Option<u8>,
    global_argument: Option<GlobalArgument>,
    global_attachment_absent: Option<bool>,
    fade_cfc_bits: Option<u32>,
    fade_argument_bits: Option<u32>,
    fade_attachment_absent: Option<bool>,
    final_session_multiple: Option<bool>,
}
impl Pending {
    pub fn begin(continuation: &continuation::Pending) -> Result<Self> {
        ensure!(
            continuation.stage() == continuation::Stage::AwaitingPose
                && continuation.helper_return() == Some(1),
            "pose entry precedes constructor helper return"
        );
        Ok(Self::new(
            continuation.player(),
            continuation
                .pose_input_raw()
                .context("pose input capture absent")?,
        ))
    }
    fn new(player: PlayerId, input_raw: u8) -> Self {
        Self {
            player,
            input_raw,
            stage: Stage::Entry,
            dependency: None,
            entry_702: None,
            blend_ba8: None,
            bab_written: false,
            tp_enabled: None,
            fp_enabled: None,
            receiver_searches: [false; 2],
            enable_writes: [None; 2],
            enable_requests: [None; 2],
            character_binding: None,
            character_scalars: [None; 2],
            character_vector: None,
            session_scale_multiple: None,
            scale_bits: None,
            scale_stored: false,
            scale_invalidated: false,
            dirty_before: None,
            dirty_after: None,
            ca4_written: false,
            mode_afc_absent: None,
            force_nodes_checked: 0,
            user_target: None,
            user_lookup: None,
            false_arm_b00_absent: None,
            backlink_nodes_checked: 0,
            attachment_absent: None,
            weapon_global_read: false,
            weapon_global_byte: None,
            global_argument: None,
            global_attachment_absent: None,
            fade_cfc_bits: None,
            fade_argument_bits: None,
            fade_attachment_absent: None,
            final_session_multiple: None,
        }
    }
    fn wait(&mut self, dependency: Dependency) {
        self.dependency = Some(dependency);
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn dependency(&self) -> Option<Dependency> {
        self.dependency
    }
    pub fn helper_returned(&self) -> bool {
        self.stage == Stage::Returned
    }
    pub fn input_raw(&self) -> u8 {
        self.input_raw
    }
    pub fn input_signed(&self) -> i8 {
        self.input_raw as i8
    }
    pub fn entry_702(&self) -> Option<u8> {
        self.entry_702
    }
    pub fn blend_ba8(&self) -> Option<u8> {
        self.blend_ba8
    }
    pub fn bab_written(&self) -> bool {
        self.bab_written
    }
    pub fn tp_enabled(&self) -> Option<bool> {
        self.tp_enabled
    }
    pub fn fp_enabled(&self) -> Option<bool> {
        self.fp_enabled
    }
    pub fn receiver_searches(&self) -> [bool; 2] {
        self.receiver_searches
    }
    pub fn enable_writes(&self) -> &[Option<EnableWrite>; 2] {
        &self.enable_writes
    }
    pub fn character_binding(&self) -> Option<CharacterBinding> {
        self.character_binding
    }
    pub fn character_scalars(&self) -> [Option<u32>; 2] {
        self.character_scalars
    }
    pub fn character_vector(&self) -> Option<[u32; 4]> {
        self.character_vector
    }
    pub fn session_scale_multiple(&self) -> Option<bool> {
        self.session_scale_multiple
    }
    pub fn scale_bits(&self) -> Option<[u32; 4]> {
        self.scale_bits
    }
    pub fn scale_stored(&self) -> bool {
        self.scale_stored
    }
    pub fn scale_invalidated(&self) -> bool {
        self.scale_invalidated
    }
    pub fn dirty_before(&self) -> Option<u8> {
        self.dirty_before
    }
    pub fn dirty_after(&self) -> Option<u8> {
        self.dirty_after
    }
    pub fn ca4_written(&self) -> bool {
        self.ca4_written
    }
    pub fn mode_afc_absent(&self) -> Option<bool> {
        self.mode_afc_absent
    }
    pub fn force_nodes_checked(&self) -> usize {
        self.force_nodes_checked
    }
    pub fn user_target(&self) -> Option<PlayerId> {
        self.user_target
    }
    pub fn user_lookup(&self) -> Option<UserLookup> {
        self.user_lookup
    }
    pub fn false_arm_b00_absent(&self) -> Option<bool> {
        self.false_arm_b00_absent
    }
    pub fn backlink_nodes_checked(&self) -> usize {
        self.backlink_nodes_checked
    }
    pub fn attachment_absent(&self) -> Option<bool> {
        self.attachment_absent
    }
    pub fn weapon_global_read(&self) -> bool {
        self.weapon_global_read
    }
    pub fn weapon_global_byte(&self) -> Option<u8> {
        self.weapon_global_byte
    }
    pub fn global_argument(&self) -> Option<GlobalArgument> {
        self.global_argument
    }
    pub fn global_equivalence_used(&self) -> bool {
        self.global_argument == Some(GlobalArgument::ArgumentUnreadBecauseReceiverAbsent)
    }
    pub fn global_attachment_absent(&self) -> Option<bool> {
        self.global_attachment_absent
    }
    pub fn fade_cfc_bits(&self) -> Option<u32> {
        self.fade_cfc_bits
    }
    pub fn fade_argument_bits(&self) -> Option<u32> {
        self.fade_argument_bits
    }
    pub fn fade_attachment_absent(&self) -> Option<bool> {
        self.fade_attachment_absent
    }
    pub fn final_session_multiple(&self) -> Option<bool> {
        self.final_session_multiple
    }
}

#[cfg(test)]
pub(crate) mod tests;
