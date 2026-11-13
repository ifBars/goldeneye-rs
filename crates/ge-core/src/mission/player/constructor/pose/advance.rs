use super::*;
impl Pending {
    fn enable(&mut self, index: usize, preparation: &mut Preparation, item: &Item) -> Result<bool> {
        if !self.receiver_searches[index] {
            let found = preparation.item_animator_by_file(
                item,
                self.player,
                [0x0102_0017, 0x0102_0016][index],
            )?;
            if let Some(identity) = found {
                let reference = preparation
                    .mode_animator(identity)?
                    .word(0x20)
                    .context("pose current reference unknown")?;
                self.enable_requests[index] = Some((identity, reference));
            }
            self.receiver_searches[index] = true;
        }
        let Some((identity, reference)) = self.enable_requests[index] else {
            return Ok(true);
        };
        if matches!((reference >> 24) & 0x7f, 2 | 0x0e) {
            self.wait(Dependency::SkeletonEnable);
            return Ok(false);
        }
        let enabled = if index == 0 {
            self.tp_enabled
        } else {
            self.fp_enabled
        }
        .context("pose enable argument absent")?;
        self.enable_writes[index] =
            Some(preparation.apply_pose_enable(identity, reference, enabled)?);
        Ok(true)
    }

    pub fn advance(
        &mut self,
        base: &mut base_handler::Owner,
        reset: &mut PendingReset,
        preparation: &mut Preparation,
        item: &mut Item,
        source: &mut impl Source,
    ) -> Result<()> {
        ensure!(
            base.player() == self.player && reset.player() == self.player,
            "pose player owner differs"
        );
        if self.dependency.is_some() || self.stage == Stage::Returned {
            return Ok(());
        }
        loop {
            match self.stage {
                Stage::Entry => {
                    let value = base.byte(0x702).context("pose entry flag unknown")?;
                    self.entry_702 = Some(value);
                    if value != 0 {
                        self.stage = Stage::Returned;
                        return Ok(());
                    }
                    self.stage = Stage::BlendGate;
                }
                Stage::BlendGate => {
                    if self.blend_ba8.is_none() {
                        self.blend_ba8 =
                            Some(reset.byte(0xba8).context("pose blend gate unknown")?);
                    }
                    if !self.bab_written {
                        reset.store_pose_input(self.input_raw)?;
                        self.bab_written = true;
                    }
                    if self.blend_ba8 != Some(0) {
                        self.wait(Dependency::BlendThreshold);
                        return Ok(());
                    }
                    self.stage = Stage::EnableArguments;
                }
                Stage::EnableArguments => {
                    let tp = self.input_signed() == 0;
                    let fp = if tp {
                        false
                    } else {
                        base.byte(0x618).context("pose FP flag unknown")? != 0
                    };
                    self.tp_enabled = Some(tp);
                    self.fp_enabled = Some(fp);
                    self.stage = Stage::ThirdPersonEnable;
                }
                Stage::ThirdPersonEnable => {
                    ensure!(
                        base.item() == Some(item.id()) && item.handler() == Some(self.player),
                        "pose actual item differs"
                    );
                    if !self.enable(0, preparation, item)? {
                        return Ok(());
                    }
                    self.stage = Stage::FirstPersonEnable;
                }
                Stage::FirstPersonEnable => {
                    if !self.enable(1, preparation, item)? {
                        return Ok(());
                    }
                    self.stage = Stage::CharacterBinding;
                }
                Stage::CharacterBinding => {
                    self.character_binding =
                        Some(reset.character().context("pose character binding absent")?);
                    self.stage = Stage::Character10;
                }
                Stage::Character10 => {
                    self.character_scalars[0] = Some(source.character_scalar(
                        self.character_binding.context("pose character absent")?,
                        CharacterField::At10,
                    )?);
                    self.stage = Stage::Character14;
                }
                Stage::Character14 => {
                    self.character_scalars[1] = Some(source.character_scalar(
                        self.character_binding.context("pose character absent")?,
                        CharacterField::At14,
                    )?);
                    let first =
                        self.character_scalars[0].context("pose first character scalar absent")?;
                    self.character_vector = Some([
                        first,
                        self.character_scalars[1].expect("stored scalar"),
                        first,
                        0,
                    ]);
                    self.stage = Stage::SessionScale;
                }
                Stage::SessionScale => {
                    let multiple = source.multiple_local_users_in_mode_three()?;
                    self.session_scale_multiple = Some(multiple);
                    if multiple {
                        self.wait(Dependency::MultiplayerScale);
                        return Ok(());
                    }
                    self.stage = Stage::Scale;
                }
                Stage::Scale => {
                    if self.scale_bits.is_none() {
                        self.scale_bits = Some(if self.fp_enabled == Some(true) {
                            let ones = source.ones_vector()?;
                            [ones[0], ones[1], ones[2], 0]
                        } else {
                            self.character_vector
                                .context("pose character vector absent")?
                        });
                    }
                    item.store_constructor_scale(self.scale_bits.expect("prepared scale"));
                    self.scale_stored = true;
                    self.stage = Stage::ScaleInvalidate;
                }
                Stage::ScaleInvalidate => {
                    item.invalidate_constructor_pose()?;
                    self.scale_invalidated = true;
                    self.stage = Stage::Dirty;
                }
                Stage::Dirty => {
                    let (before, after) = item.clear_constructor_scale_dirty();
                    self.dirty_before = Some(before);
                    self.dirty_after = Some(after);
                    self.stage = Stage::WeaponMode;
                }
                Stage::WeaponMode => {
                    if !self.ca4_written {
                        let secondary = reset
                            .word(0x710)
                            .context("pose secondary history unknown")?;
                        if matches!(
                            secondary,
                            0x0900_0a17 | 0x0900_0a18 | 0x0900_05db | 0x0900_0c3d
                        ) {
                            self.wait(Dependency::WeaponMode);
                            return Ok(());
                        }
                        reset.store_pose_weapon_mode()?;
                        self.ca4_written = true;
                    }
                    let absent = reset.word(0xafc).context("pose mode attachment unknown")? == 0;
                    self.mode_afc_absent = Some(absent);
                    if !absent {
                        self.wait(Dependency::AttachmentOwner);
                        return Ok(());
                    }
                    self.stage = Stage::ForceComponents;
                }
                Stage::ForceComponents => {
                    while let Some(id) = item.components().get(self.force_nodes_checked).copied() {
                        if reset
                            .word(0xafc)
                            .context("pose Force component link unknown")?
                            != 0
                            || reset.word(0xb00).context("pose Force body link unknown")? != 0
                        {
                            self.wait(Dependency::AttachmentOwner);
                            return Ok(());
                        }
                        // This is the actual registered Animator owner, whose
                        // native RTTI is Anim and does not derive from Force.
                        preparation.item_animator(item, self.player, id)?;
                        self.force_nodes_checked += 1;
                    }
                    self.stage = Stage::UserTarget;
                }
                Stage::UserTarget => {
                    if base
                        .word(0x98c)
                        .context("pose alternate player target unknown")?
                        != 0
                    {
                        self.wait(Dependency::UserTarget);
                        return Ok(());
                    }
                    self.user_target = Some(self.player);
                    self.stage = Stage::UserLookup;
                }
                Stage::UserLookup => {
                    let user = source.local_user_for_player(
                        self.user_target.context("pose lookup target absent")?,
                    )?;
                    self.user_lookup = Some(user.map_or(UserLookup::NotFound, UserLookup::Matched));
                    if user.is_some() {
                        self.wait(Dependency::UserMatched);
                        return Ok(());
                    }
                    self.stage = Stage::UserFalseArm;
                }
                Stage::UserFalseArm => {
                    let absent = reset
                        .word(0xb00)
                        .context("pose user false-arm body unknown")?
                        == 0;
                    self.false_arm_b00_absent = Some(absent);
                    if !absent {
                        self.wait(Dependency::AttachmentOwner);
                        return Ok(());
                    }
                    self.stage = Stage::BacklinkComponents;
                }
                Stage::BacklinkComponents => {
                    while let Some(id) = item.components().get(self.backlink_nodes_checked).copied()
                    {
                        // node() checks the actual +8 item backlink. Equality
                        // skips the later owner/RTTI/enable provider path.
                        let identity = preparation.item_animator(item, self.player, id)?;
                        if preparation.mode_animator(identity)?.item() != Some(item.id()) {
                            self.wait(Dependency::ComponentBacklink);
                            return Ok(());
                        }
                        self.backlink_nodes_checked += 1;
                    }
                    self.stage = Stage::Attachment;
                }
                Stage::Attachment => {
                    let absent = reset
                        .word(0xafc)
                        .context("pose attachment call receiver unknown")?
                        == 0;
                    self.attachment_absent = Some(absent);
                    if !absent {
                        self.wait(Dependency::AttachmentOwner);
                        return Ok(());
                    }
                    self.stage = Stage::GlobalByte;
                }
                Stage::GlobalByte => {
                    self.weapon_global_byte = source.weapon_global_byte()?;
                    self.weapon_global_read = true;
                    if let Some(value) = self.weapon_global_byte {
                        self.global_argument = Some(GlobalArgument::Known(if value == 0 {
                            self.input_signed()
                        } else {
                            0
                        }));
                    }
                    self.stage = Stage::GlobalAttachment;
                }
                Stage::GlobalAttachment => {
                    let absent = reset
                        .word(0xafc)
                        .context("pose global-controlled receiver unknown")?
                        == 0;
                    self.global_attachment_absent = Some(absent);
                    if !absent {
                        self.wait(Dependency::GlobalArgument);
                        return Ok(());
                    }
                    if self.weapon_global_byte.is_none() {
                        self.global_argument =
                            Some(GlobalArgument::ArgumentUnreadBecauseReceiverAbsent);
                    }
                    self.stage = Stage::FadeInputs;
                }
                Stage::FadeInputs => {
                    if self.fade_cfc_bits.is_none() {
                        self.fade_cfc_bits =
                            Some(reset.word(0xcfc).context("pose fade scalar unknown")?);
                    }
                    let absent = reset.word(0xafc).context("pose fade receiver unknown")? == 0;
                    self.fade_attachment_absent = Some(absent);
                    self.fade_argument_bits = Some(
                        (1f32 - f32::from_bits(self.fade_cfc_bits.expect("captured fade")))
                            .to_bits(),
                    );
                    if !absent {
                        self.wait(Dependency::FadeAttachment);
                        return Ok(());
                    }
                    self.stage = Stage::FinalSession;
                }
                Stage::FinalSession => {
                    let multiple = source.multiple_local_users_in_mode_three()?;
                    self.final_session_multiple = Some(multiple);
                    if multiple {
                        self.wait(Dependency::FinalMultiplayer);
                        return Ok(());
                    }
                    self.stage = Stage::Returned;
                    return Ok(());
                }
                Stage::Returned => return Ok(()),
            }
        }
    }
}
