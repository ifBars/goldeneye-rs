//! One secondary-intent setter call. Every invocation owns its cursor, command
//! history and return; callers retain earlier calls across later mutations.
use super::*;
use crate::animation::{
    layers::{SlotOverride, command},
    mode::{
        self,
        runtime::{Cursor, Invocation, Provider},
    },
    script_child,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Selecting,
    CommandPending,
    SetterTail,
    Returned,
    Failed,
}

#[derive(Debug, Clone, Copy)]
pub struct Inputs {
    pub target: Target,
    pub receiver: Identity,
    pub generic: u32,
    pub requested: u32,
    pub parameter_bits: u32,
    pub tracks: bool,
    pub fallback: (u32, u64),
}
pub struct Call {
    inputs: Inputs,
    previous: u32,
    stage: Stage,
    cursor: Cursor,
    command: Option<command::Pending>,
    completed: Vec<command::Pending>,
    setter: Option<SetterReturn>,
}
impl Call {
    pub fn begin(
        inputs: Inputs,
        base: &mut base_handler::Owner,
        reset: &PendingReset,
        preparation: &Preparation,
    ) -> Result<Self> {
        ensure!(
            inputs.receiver.player() == base.player() && reset.player() == base.player(),
            "secondary call player differs"
        );
        let owner = preparation.mode_animator(inputs.receiver)?;
        let file = if inputs.target == Target::FirstPerson {
            0x0102_0016
        } else {
            0x0102_0017
        };
        ensure!(
            owner
                .component()
                .is_some_and(|binding| binding.file == file)
                && (inputs.target != Target::FirstPerson || inputs.tracks),
            "secondary call receiver or direct FP qualification differs"
        );
        let previous = super::secondary_setter::enter(base, reset, inputs.tracks)?;
        Ok(Self {
            inputs,
            previous,
            stage: Stage::Selecting,
            cursor: Cursor::new(
                mode::Inputs {
                    requested: inputs.requested,
                    previous,
                    fallback: inputs.fallback.0,
                    resource_available: true,
                },
                mode::Budget {
                    lookups: 3,
                    transition_rows: 65536,
                    commands: 4096,
                    groups: 4096,
                    children: 16384,
                },
            ),
            command: None,
            completed: Vec::new(),
            setter: None,
        })
    }
    pub fn inputs(&self) -> Inputs {
        self.inputs
    }
    pub fn previous(&self) -> u32 {
        self.previous
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn selector(&self) -> &Cursor {
        &self.cursor
    }
    pub fn setter(&self) -> Option<SetterReturn> {
        self.setter
    }
    pub fn command(&self) -> Option<&command::Pending> {
        self.command.as_ref().or_else(|| {
            matches!(
                self.stage,
                Stage::SetterTail | Stage::Returned | Stage::Failed
            )
            .then(|| self.completed.last())
            .flatten()
        })
    }
    pub fn has_pending_command(&self) -> bool {
        self.command.is_some()
    }
    pub fn completed_commands(&self) -> &[command::Pending] {
        &self.completed
    }
    pub fn advance(&mut self, source: &mut impl Provider) -> Result<Option<Invocation>> {
        ensure!(
            self.stage == Stage::Selecting,
            "secondary cursor advance out of order"
        );
        match self.cursor.next_command(source) {
            Ok(next) => {
                self.stage = if next.is_some() {
                    Stage::CommandPending
                } else {
                    Stage::SetterTail
                };
                Ok(next)
            }
            Err(error) => {
                self.stage = Stage::Failed;
                Err(error)
            }
        }
    }
    fn with_command(
        &mut self,
        preparation: &mut Preparation,
        run: impl FnOnce(Identity, &mut command::Pending, &mut Preparation) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::CommandPending,
            "secondary body continuation out of order"
        );
        let result = run(
            self.inputs.receiver,
            self.command.as_mut().context("secondary command absent")?,
            preparation,
        );
        if result.is_err() {
            self.fail()?;
        }
        result
    }
    pub fn prepare_command(
        &mut self,
        preparation: &mut Preparation,
        opcode: u32,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::CommandPending && self.command.is_none(),
            "secondary command prefix replays"
        );
        self.command = Some(command::Pending::new(opcode));
        self.with_command(preparation, |receiver, body, prep| {
            prep.prepare_mode_command(receiver, body, source, slot_override)
        })
    }
    pub fn finish_mask(
        &mut self,
        preparation: &mut Preparation,
        context: &mut crate::animation::scratch::Context,
        arena: &mut crate::animation::scratch::Arena,
    ) -> Result<()> {
        self.with_command(preparation, |receiver, body, prep| {
            prep.finish_mode_mask_command(receiver, body, context, arena)
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
    pub fn acknowledge(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::CommandPending
                && self
                    .command
                    .as_ref()
                    .is_some_and(|c| c.stage() == command::Stage::Returned),
            "secondary acknowledgement precedes body return"
        );
        self.completed.try_reserve(1)?;
        self.cursor.executed(
            self.cursor
                .pending()
                .context("secondary invocation absent")?,
        )?;
        self.completed
            .push(self.command.take().expect("checked returned body"));
        self.stage = Stage::Selecting;
        Ok(())
    }
    pub fn finish(&mut self, reset: &mut PendingReset) -> Result<()> {
        ensure!(
            self.stage == Stage::SetterTail && self.setter.is_none(),
            "secondary setter tail out of order or replays"
        );
        self.setter = Some(SetterReturn {
            return_word: self
                .cursor
                .return_word()
                .context("secondary selection has not returned")?,
            history_updated: false,
            primary_changed: None,
        });
        let result = super::secondary_setter::commit(
            super::secondary_setter::Inputs {
                player: self.inputs.receiver.player(),
                generic: self.inputs.generic,
                mapped: self.inputs.requested,
                tracks: self.inputs.tracks,
            },
            reset,
            self.setter.as_mut().expect("retained setter"),
        );
        self.stage = if result.is_ok() {
            Stage::Returned
        } else {
            Stage::Failed
        };
        result
    }
    pub fn fail(&mut self) -> Result<()> {
        if let Some(invocation) = self.cursor.pending() {
            self.cursor.failed(invocation)?;
        }
        self.stage = Stage::Failed;
        Ok(())
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(self.stage as u32);
        emit(self.inputs.target as u32);
        self.inputs.receiver.visit_state_words(&mut emit);
        for value in [
            self.inputs.generic,
            self.inputs.requested,
            self.inputs.parameter_bits,
            u32::from(self.inputs.tracks),
            self.previous,
            self.inputs.fallback.0,
        ] {
            emit(value);
        }
        emit((self.inputs.fallback.1 >> 32) as u32);
        emit(self.inputs.fallback.1 as u32);
        self.cursor.visit_state_words(&mut emit);
        emit(u32::from(self.command.is_some()));
        if let Some(command) = &self.command {
            command.visit_state_words(&mut emit);
        }
        emit(self.completed.len() as u32);
        for command in &self.completed {
            command.visit_state_words(&mut emit);
        }
        emit(u32::from(self.setter.is_some()));
        if let Some(setter) = self.setter {
            emit(setter.return_word);
            emit(u32::from(setter.history_updated));
        }
    }
}

#[cfg(test)]
mod tests;
