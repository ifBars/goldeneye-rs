use super::*;
impl FirstPersonDispatch {
    pub fn finish_mask_command(
        &mut self,
        preparation: &mut Preparation,
        context: &mut crate::animation::scratch::Context,
        arena: &mut crate::animation::scratch::Arena,
    ) -> Result<()> {
        self.with_command(preparation, |receiver, body, prep| {
            prep.finish_mode_mask_command(receiver, body, context, arena)
        })
    }
    pub fn advance(&mut self, source: &mut impl Provider) -> Result<Option<Invocation>> {
        ensure!(
            self.stage == FirstPersonStage::Selecting,
            "FP selector advance out of order"
        );
        match self
            .selector
            .as_mut()
            .context("FP selector absent")?
            .next_command(source)
        {
            Ok(inv) => {
                self.stage = if inv.is_some() {
                    FirstPersonStage::CommandPending
                } else {
                    FirstPersonStage::SetterTail
                };
                Ok(inv)
            }
            Err(e) => {
                self.stage = FirstPersonStage::Failed;
                Err(e)
            }
        }
    }
    pub fn fail(&mut self) -> Result<()> {
        ensure!(
            self.stage != FirstPersonStage::Failed,
            "failed FP dispatch replays"
        );
        if let Some(inv) = self.selector.as_ref().and_then(|c| c.pending()) {
            self.selector
                .as_mut()
                .expect("retained cursor")
                .failed(inv)?;
        }
        self.stage = FirstPersonStage::Failed;
        Ok(())
    }
    pub fn prepare_command(
        &mut self,
        preparation: &mut Preparation,
        opcode: u32,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::CommandPending && self.command.is_none(),
            "FP command prefix replays or precedes selection"
        );
        self.command = Some(command::Pending::new(opcode));
        self.with_command(preparation, |receiver, body, prep| {
            prep.prepare_mode_command(receiver, body, source, slot_override)
        })
    }
    pub fn initialize_child_prefix(
        &mut self,
        preparation: &mut Preparation,
        source: &mut impl script_child::Provider,
    ) -> Result<()> {
        self.with_command(preparation, |receiver, body, prep| {
            prep.initialize_mode_child_prefix(receiver, body, source)
        })
    }
    pub fn initialize_child_phase_zero(
        &mut self,
        preparation: &mut Preparation,
        source: &mut impl script_child::initialization::Source,
        step: u32,
    ) -> Result<()> {
        self.with_command(preparation, |receiver, body, prep| {
            prep.initialize_mode_child_phase_zero(receiver, body, source, step, 65536)
        })
    }
    pub fn prepare_child_position(
        &mut self,
        preparation: &mut Preparation,
        source: &mut impl script_child::position::Source,
    ) -> Result<()> {
        self.with_command(preparation, |receiver, body, prep| {
            prep.prepare_mode_child_position(receiver, body, source)
        })
    }
    pub fn finish_child_effects(
        &mut self,
        preparation: &mut Preparation,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        self.with_command(preparation, |receiver, body, prep| {
            prep.finish_mode_child_effects(receiver, body, source, slot_override)
        })
    }
    fn with_command(
        &mut self,
        preparation: &mut Preparation,
        run: impl FnOnce(Identity, &mut command::Pending, &mut Preparation) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::CommandPending,
            "FP command continuation order differs"
        );
        let result = run(
            self.receiver,
            self.command.as_mut().context("FP command absent")?,
            preparation,
        );
        if result.is_err() {
            self.fail()?;
        }
        result
    }
    pub fn acknowledge_command(&mut self) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::CommandPending
                && self
                    .command
                    .as_ref()
                    .is_some_and(|c| c.stage() == command::Stage::Returned),
            "FP command acknowledgement precedes return or replays"
        );
        self.completed_commands.try_reserve(1)?;
        let cursor = self.selector.as_mut().context("FP cursor absent")?;
        cursor.executed(cursor.pending().context("FP invocation absent")?)?;
        self.completed_commands
            .push(self.command.take().expect("validated returned command"));
        self.stage = FirstPersonStage::Selecting;
        Ok(())
    }
}
