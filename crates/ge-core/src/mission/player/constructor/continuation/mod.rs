//! Actual constructor continuation after the initial virtual callback returns.
//! Nonempty cleanup owners and the following pose helper remain explicit inputs.
use super::{PendingPrelude, PendingReset};
use crate::mission::player::{
    actions::callback_tail::CallbackTail, base_handler, identity::PlayerId,
};
use anyhow::{Context, Result, ensure};
mod state;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackEntry {
    player: PlayerId,
    saved_state_704: u32,
}
impl CallbackEntry {
    /// Capture at the actual component-helper return, immediately before the
    /// initial virtual callback. A later diagnostic read is not this proof.
    pub fn capture(base: &base_handler::Owner) -> Result<Self> {
        ensure!(
            base.stage() == base_handler::Stage::VirtualBasicComplete,
            "constructor callback entry precedes basic return"
        );
        Ok(Self {
            player: base.player(),
            saved_state_704: base
                .word(0x704)
                .context("constructor callback incoming state unknown")?,
        })
    }
    pub fn player(self) -> PlayerId {
        self.player
    }
    pub fn saved_state_704(self) -> u32 {
        self.saved_state_704
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataField {
    Type,
    InventoryKey,
    PrimaryKey,
}
pub trait MetadataSource {
    /// Resolve this nonnegative native sorted index from a real retained row.
    /// An unavailable row or field is an error, never the default weapon row.
    fn word(&mut self, slot: u8, field: MetadataField) -> Result<u32>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataRead {
    pub slot: u8,
    pub field: MetadataField,
    pub value: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverrideRemoval {
    pub key: u32,
    pub count_raw: u32,
    pub removed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    RestoreState,
    SelectScript,
    CurrentRecord,
    Variant,
    CleanupKey,
    FirstComponent,
    PrimaryKey,
    BodyCleanup,
    ReadCleanupLink,
    ChildCleanup,
    Marker,
    ActorGate,
    FirstOverride,
    SecondOverride,
    FirstEffect,
    SecondEffect,
    HelperReturn,
    PoseInput,
    AwaitingPose,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    ScriptSelection,
    AlternateVariant,
    NormalComponentBinding,
    ComponentCleanup,
    SpecialPrimaryCleanup,
    BodyCleanup,
    ChildCleanup,
    ActorCleanup,
    VisualOverrides,
    FirstEffectCleanup,
    SecondEffectCleanup,
    Pose,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScriptSelection => "constructor_script_selection",
            Self::AlternateVariant => "constructor_alternate_variant",
            Self::NormalComponentBinding => "constructor_component_binding",
            Self::ComponentCleanup => "constructor_component_cleanup",
            Self::SpecialPrimaryCleanup => "constructor_special_primary_cleanup",
            Self::BodyCleanup => "constructor_body_cleanup",
            Self::ChildCleanup => "constructor_child_cleanup",
            Self::ActorCleanup => "constructor_actor_cleanup",
            Self::VisualOverrides => "constructor_visual_overrides",
            Self::FirstEffectCleanup => "constructor_first_effect_cleanup",
            Self::SecondEffectCleanup => "constructor_second_effect_cleanup",
            Self::Pose => "constructor_pose",
        }
    }
}

#[derive(Debug)]
pub struct Pending {
    player: PlayerId,
    saved_state_704: u32,
    stage: Stage,
    dependency: Option<Dependency>,
    state_restored: bool,
    script_reference: Option<u32>,
    metadata_reads: [Option<MetadataRead>; 3],
    current_slot: Option<i8>,
    current_record_present: Option<bool>,
    current_inventory_key: Option<u32>,
    current_flags: Option<u16>,
    variant: Option<u8>,
    cleanup_checks: [Option<bool>; 5],
    b84_written: bool,
    bd1_written: bool,
    actor_absent: Option<bool>,
    override_removals: [Option<OverrideRemoval>; 2],
    helper_return: Option<u32>,
    pose_input_raw: Option<u8>,
}

impl Pending {
    pub fn begin(callback: &CallbackTail, entry: CallbackEntry) -> Result<Self> {
        ensure!(
            callback.callback_returned() && callback.player() == entry.player,
            "constructor continuation precedes callback return or changes player"
        );
        Ok(Self::new(entry))
    }
    fn new(entry: CallbackEntry) -> Self {
        Self {
            player: entry.player,
            saved_state_704: entry.saved_state_704,
            stage: Stage::RestoreState,
            dependency: None,
            state_restored: false,
            script_reference: None,
            metadata_reads: [None; 3],
            current_slot: None,
            current_record_present: None,
            current_inventory_key: None,
            current_flags: None,
            variant: None,
            cleanup_checks: [None; 5],
            b84_written: false,
            bd1_written: false,
            actor_absent: None,
            override_removals: [None; 2],
            helper_return: None,
            pose_input_raw: None,
        }
    }
    fn wait(&mut self, dependency: Dependency) {
        self.dependency = Some(dependency);
    }
    fn metadata(
        &mut self,
        reset: &PendingReset,
        source: &mut impl MetadataSource,
        index: usize,
        field: MetadataField,
    ) -> Result<Option<u32>> {
        let slot = reset
            .byte(0xc98)
            .context("constructor metadata slot unknown")? as i8;
        if slot < 0 {
            return Ok(None);
        }
        let value = source.word(slot as u8, field)?;
        self.metadata_reads[index] = Some(MetadataRead {
            slot: slot as u8,
            field,
            value,
        });
        Ok(Some(value))
    }
    fn link(&mut self, reset: &PendingReset, index: usize, at: u16) -> Result<bool> {
        let absent = reset.word(at).context("constructor cleanup link unknown")? == 0;
        self.cleanup_checks[index] = Some(absent);
        Ok(absent)
    }

    /// Advance only reached reads/stores. Unsupported live branches freeze at
    /// their dependency; failed input reads retain earlier successful prefixes.
    pub fn advance(
        &mut self,
        base: &mut base_handler::Owner,
        reset: &mut PendingReset,
        prelude: &PendingPrelude,
        source: &mut impl MetadataSource,
    ) -> Result<()> {
        ensure!(
            base.player() == self.player
                && reset.player() == self.player
                && prelude.player() == self.player,
            "constructor continuation owner differs"
        );
        if self.dependency.is_some() {
            return Ok(());
        }
        loop {
            match self.stage {
                Stage::RestoreState => {
                    if self.saved_state_704 == 5 {
                        base.restore_constructor_state_five()?;
                        self.state_restored = true;
                    }
                    self.stage = Stage::SelectScript;
                }
                Stage::SelectScript => {
                    let kind = self.metadata(reset, source, 0, MetadataField::Type)?;
                    if matches!(kind, Some(0x4b20_0001 | 0x4b20_0009)) {
                        self.wait(Dependency::ScriptSelection);
                        return Ok(());
                    }
                    self.script_reference = Some(0x0400_1dac);
                    self.stage = Stage::CurrentRecord;
                }
                Stage::CurrentRecord => {
                    let slot = reset
                        .byte(0xc96)
                        .context("constructor inventory slot unknown")?
                        as i8;
                    let record = if slot < 0 {
                        None
                    } else {
                        prelude.inventory().records().get(slot as usize)
                    };
                    self.current_slot = Some(slot);
                    self.current_record_present = Some(record.is_some());
                    self.current_inventory_key = Some(record.map_or(0, |r| r.key));
                    self.stage = Stage::Variant;
                }
                Stage::Variant => {
                    if base
                        .byte(0x5ed)
                        .context("constructor alternate variant unknown")?
                        != 0
                    {
                        self.wait(Dependency::AlternateVariant);
                        return Ok(());
                    }
                    let slot = self
                        .current_slot
                        .context("constructor current record locator absent")?;
                    self.current_flags = if slot < 0 {
                        None
                    } else {
                        prelude
                            .inventory()
                            .records()
                            .get(slot as usize)
                            .map(|record| record.flags)
                    };
                    self.variant =
                        Some(u8::from(self.current_flags.is_some_and(|v| v & 0x100 != 0)));
                    self.stage = Stage::CleanupKey;
                }
                Stage::CleanupKey => {
                    if !matches!(
                        self.metadata(reset, source, 1, MetadataField::InventoryKey)?,
                        Some(0x0200_0830 | 0x0200_1e4d)
                    ) {
                        self.wait(Dependency::NormalComponentBinding);
                        return Ok(());
                    }
                    self.stage = Stage::FirstComponent;
                }
                Stage::FirstComponent => {
                    if !self.link(reset, 0, 0xafc)? {
                        self.wait(Dependency::ComponentCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::PrimaryKey;
                }
                Stage::PrimaryKey => {
                    if self.metadata(reset, source, 2, MetadataField::PrimaryKey)?
                        == Some(0x4b00_001b)
                    {
                        self.wait(Dependency::SpecialPrimaryCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::BodyCleanup;
                }
                Stage::BodyCleanup => {
                    if !self.link(reset, 1, 0xb00)? {
                        self.wait(Dependency::BodyCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::ReadCleanupLink;
                }
                Stage::ReadCleanupLink => {
                    self.link(reset, 2, 0xaf8)?;
                    self.stage = Stage::ChildCleanup;
                }
                Stage::ChildCleanup => {
                    reset.clear_constructor_effect_word()?;
                    self.b84_written = true;
                    if self.cleanup_checks[2] != Some(true) {
                        self.wait(Dependency::ChildCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::Marker;
                }
                Stage::Marker => {
                    reset.clear_constructor_marker()?;
                    self.bd1_written = true;
                    self.stage = Stage::ActorGate;
                }
                Stage::ActorGate => {
                    let absent = base.word(0x570).context("constructor actor gate unknown")? == 0;
                    self.actor_absent = Some(absent);
                    if !absent {
                        self.wait(Dependency::ActorCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::FirstOverride;
                }
                Stage::FirstOverride | Stage::SecondOverride => {
                    let index = usize::from(self.stage == Stage::SecondOverride);
                    let count = base.constructor_override_count()?;
                    if (count as i32) > 0 {
                        self.wait(Dependency::VisualOverrides);
                        return Ok(());
                    }
                    self.override_removals[index] = Some(OverrideRemoval {
                        key: [0x0600_056d, 0x0600_067b][index],
                        count_raw: count,
                        removed: false,
                    });
                    self.stage = if index == 0 {
                        Stage::SecondOverride
                    } else {
                        Stage::FirstEffect
                    };
                }
                Stage::FirstEffect => {
                    if !self.link(reset, 3, 0xbb0)? {
                        self.wait(Dependency::FirstEffectCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::SecondEffect;
                }
                Stage::SecondEffect => {
                    if !self.link(reset, 4, 0xbb4)? {
                        self.wait(Dependency::SecondEffectCleanup);
                        return Ok(());
                    }
                    self.stage = Stage::HelperReturn;
                }
                Stage::HelperReturn => {
                    self.helper_return = Some(1);
                    self.stage = Stage::PoseInput;
                }
                Stage::PoseInput => {
                    self.pose_input_raw = Some(
                        reset
                            .byte(0xc7d)
                            .context("constructor pose input unknown")?,
                    );
                    self.stage = Stage::AwaitingPose;
                    self.wait(Dependency::Pose);
                    return Ok(());
                }
                Stage::AwaitingPose => return Ok(()),
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
    pub fn saved_state_704(&self) -> u32 {
        self.saved_state_704
    }
    pub fn state_restored(&self) -> bool {
        self.state_restored
    }
    pub fn script_reference(&self) -> Option<u32> {
        self.script_reference
    }
    pub fn metadata_reads(&self) -> &[Option<MetadataRead>; 3] {
        &self.metadata_reads
    }
    pub fn current_slot(&self) -> Option<i8> {
        self.current_slot
    }
    pub fn current_record_present(&self) -> Option<bool> {
        self.current_record_present
    }
    pub fn current_inventory_key(&self) -> Option<u32> {
        self.current_inventory_key
    }
    pub fn current_flags(&self) -> Option<u16> {
        self.current_flags
    }
    pub fn variant(&self) -> Option<u8> {
        self.variant
    }
    /// Source order: afc, b00, af8, bb0, bb4. Some(true) is known NULL.
    pub fn cleanup_checks(&self) -> &[Option<bool>; 5] {
        &self.cleanup_checks
    }
    pub fn b84_written(&self) -> bool {
        self.b84_written
    }
    pub fn bd1_written(&self) -> bool {
        self.bd1_written
    }
    pub fn actor_absent(&self) -> Option<bool> {
        self.actor_absent
    }
    pub fn override_removals(&self) -> &[Option<OverrideRemoval>; 2] {
        &self.override_removals
    }
    pub fn helper_return(&self) -> Option<u32> {
        self.helper_return
    }
    pub fn pose_input_raw(&self) -> Option<u8> {
        self.pose_input_raw
    }
    pub fn pose_input_signed(&self) -> Option<i8> {
        self.pose_input_raw.map(|v| v as i8)
    }
}

#[cfg(test)]
mod tests;
