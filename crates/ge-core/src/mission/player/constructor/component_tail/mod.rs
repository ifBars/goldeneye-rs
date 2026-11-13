//! The component helper's final rotation/scale fragment after the pose helper.
//! Initializer cache work and player publication are separate continuations.
use super::{
    PendingReset,
    pose::{self, CharacterField, CharacterLookup, Source},
};
use crate::mission::player::{base_handler, identity::PlayerId, item::Item};
use anyhow::{Context, Result, ensure};
mod state;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    RotationSecond,
    RotationFirst,
    StoreRotationFirst,
    StoreRotationSecond,
    RotationInvalidate,
    Character,
    Character10,
    Character14,
    SessionScale,
    ScaleFirst,
    ScaleSecond,
    ScaleInvalidate,
    Flags,
    Returned,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    MultiplayerScale,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        "component_tail_multiplayer_scale"
    }
}
#[derive(Debug)]
pub struct Pending {
    player: PlayerId,
    stage: Stage,
    dependency: Option<Dependency>,
    rotation_reads: [Option<[u32; 2]>; 2],
    rotation_stores: [bool; 2],
    rotation_invalidated: bool,
    character: Option<CharacterLookup>,
    character_scalars: [Option<u32>; 2],
    scale_bits: Option<[u32; 4]>,
    session_multiple: Option<bool>,
    scale_stores: [bool; 2],
    scale_invalidated: bool,
    flags_before: Option<u8>,
    flags_after: Option<u8>,
    helper_return: Option<u32>,
}
impl Pending {
    pub fn begin(pose: &pose::Pending) -> Result<Self> {
        ensure!(
            pose.helper_returned(),
            "component tail precedes pose helper return"
        );
        Ok(Self::new(pose.player()))
    }
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            stage: Stage::RotationSecond,
            dependency: None,
            rotation_reads: [None; 2],
            rotation_stores: [false; 2],
            rotation_invalidated: false,
            character: None,
            character_scalars: [None; 2],
            scale_bits: None,
            session_multiple: None,
            scale_stores: [false; 2],
            scale_invalidated: false,
            flags_before: None,
            flags_after: None,
            helper_return: None,
        }
    }
    fn returned(&mut self) {
        self.helper_return = Some(1);
        self.stage = Stage::Returned;
    }
    pub fn advance(
        &mut self,
        base: &base_handler::Owner,
        reset: &PendingReset,
        item: &mut Item,
        source: &mut impl Source,
    ) -> Result<()> {
        ensure!(
            base.player() == self.player
                && reset.player() == self.player
                && base.item() == Some(item.id())
                && item.handler() == Some(self.player),
            "component tail owner/item differs"
        );
        if self.dependency.is_some() || self.stage == Stage::Returned {
            return Ok(());
        }
        loop {
            match self.stage {
                Stage::RotationSecond => {
                    self.rotation_reads[1] = Some(item.constructor_rotation_pair(true));
                    self.stage = Stage::RotationFirst;
                }
                Stage::RotationFirst => {
                    self.rotation_reads[0] = Some(item.constructor_rotation_pair(false));
                    self.stage = Stage::StoreRotationFirst;
                }
                Stage::StoreRotationFirst => {
                    item.store_constructor_rotation_pair(
                        false,
                        self.rotation_reads[0].context("component first rotation pair absent")?,
                    );
                    self.rotation_stores[0] = true;
                    self.stage = Stage::StoreRotationSecond;
                }
                Stage::StoreRotationSecond => {
                    item.store_constructor_rotation_pair(
                        true,
                        self.rotation_reads[1].context("component second rotation pair absent")?,
                    );
                    self.rotation_stores[1] = true;
                    self.stage = Stage::RotationInvalidate;
                }
                Stage::RotationInvalidate => {
                    item.invalidate_constructor_pose()?;
                    self.rotation_invalidated = true;
                    self.stage = Stage::Character;
                }
                Stage::Character => {
                    let lookup = source.constructor_character(reset.character())?;
                    self.character = Some(lookup);
                    if lookup == CharacterLookup::KnownAbsent {
                        self.returned();
                        return Ok(());
                    }
                    self.stage = Stage::Character10;
                }
                Stage::Character10 => {
                    let Some(CharacterLookup::Bound(binding)) = self.character else {
                        anyhow::bail!("component character binding absent");
                    };
                    self.character_scalars[0] =
                        Some(source.character_scalar(binding, CharacterField::At10)?);
                    self.stage = Stage::Character14;
                }
                Stage::Character14 => {
                    let Some(CharacterLookup::Bound(binding)) = self.character else {
                        anyhow::bail!("component character binding absent");
                    };
                    self.character_scalars[1] =
                        Some(source.character_scalar(binding, CharacterField::At14)?);
                    let first = self.character_scalars[0]
                        .context("component first character scalar absent")?;
                    self.scale_bits = Some([
                        first,
                        self.character_scalars[1].expect("captured scalar"),
                        first,
                        0,
                    ]);
                    self.stage = Stage::SessionScale;
                }
                Stage::SessionScale => {
                    let multiple = source.multiple_local_users_in_mode_three()?;
                    self.session_multiple = Some(multiple);
                    if multiple {
                        self.dependency = Some(Dependency::MultiplayerScale);
                        return Ok(());
                    }
                    self.stage = Stage::ScaleFirst;
                }
                Stage::ScaleFirst => {
                    let bits = self.scale_bits.context("component final scale absent")?;
                    item.store_constructor_scale_pair(false, [bits[0], bits[1]]);
                    self.scale_stores[0] = true;
                    self.stage = Stage::ScaleSecond;
                }
                Stage::ScaleSecond => {
                    let bits = self.scale_bits.context("component final scale absent")?;
                    item.store_constructor_scale_pair(true, [bits[2], bits[3]]);
                    self.scale_stores[1] = true;
                    self.stage = Stage::ScaleInvalidate;
                }
                Stage::ScaleInvalidate => {
                    item.invalidate_constructor_pose()?;
                    self.scale_invalidated = true;
                    self.stage = Stage::Flags;
                }
                Stage::Flags => {
                    let (before, after) = item.mark_constructor_component_pose();
                    self.flags_before = Some(before);
                    self.flags_after = Some(after);
                    self.returned();
                    return Ok(());
                }
                Stage::Returned => return Ok(()),
            }
        }
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
    pub fn helper_return(&self) -> Option<u32> {
        self.helper_return
    }
    pub fn helper_returned(&self) -> bool {
        self.stage == Stage::Returned
    }
    pub fn rotation_reads(&self) -> [Option<[u32; 2]>; 2] {
        self.rotation_reads
    }
    pub fn rotation_stores(&self) -> [bool; 2] {
        self.rotation_stores
    }
    pub fn rotation_invalidated(&self) -> bool {
        self.rotation_invalidated
    }
    pub fn character_lookup(&self) -> Option<CharacterLookup> {
        self.character
    }
    pub fn character_scalars(&self) -> [Option<u32>; 2] {
        self.character_scalars
    }
    pub fn scale_bits(&self) -> Option<[u32; 4]> {
        self.scale_bits
    }
    pub fn session_multiple(&self) -> Option<bool> {
        self.session_multiple
    }
    pub fn scale_stores(&self) -> [bool; 2] {
        self.scale_stores
    }
    pub fn scale_invalidated(&self) -> bool {
        self.scale_invalidated
    }
    pub fn flags_before(&self) -> Option<u8> {
        self.flags_before
    }
    pub fn flags_after(&self) -> Option<u8> {
        self.flags_after
    }
}

#[cfg(test)]
mod tests;
