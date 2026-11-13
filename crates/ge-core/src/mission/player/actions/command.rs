use super::*;
use crate::animation::layers::{SlotOverride, command};
impl Pending {
    pub fn command(&self) -> Option<&command::Pending> {
        self.command.as_ref().or_else(|| {
            if self.stage == Stage::AwaitingThirdPersonSetterTail {
                self.completed_commands.last()
            } else {
                None
            }
        })
    }
    pub fn completed_commands(&self) -> &[command::Pending] {
        &self.completed_commands
    }
    pub fn command_acknowledged(&self) -> bool {
        self.command.is_none() && !self.completed_commands.is_empty()
    }
    pub fn acknowledge_third_person_command(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonCommand
                && self
                    .command
                    .as_ref()
                    .is_some_and(|c| c.stage() == command::Stage::Returned),
            "TP command acknowledgement precedes effect return or replays"
        );
        let cursor = self
            .selector
            .as_mut()
            .context("TP command acknowledgement cursor absent")?;
        let invocation = cursor
            .pending()
            .context("TP command acknowledgement invocation absent")?;
        self.completed_commands.try_reserve(1)?;
        cursor.executed(invocation)?;
        self.completed_commands
            .push(self.command.take().expect("validated returned body"));
        self.stage = Stage::SelectingThirdPerson;
        Ok(())
    }
    pub fn prepare_third_person_command(
        &mut self,
        preparation: &mut Preparation,
        opcode: u32,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonCommand && self.command.is_none(),
            "TP command body is out of source order or replays"
        );
        let invocation = self
            .selector
            .as_ref()
            .context("TP selector absent")?
            .pending()
            .context("TP command invocation absent")?;
        ensure!(
            preparation
                .animator()
                .is_some_and(|owner| owner.player() == self.player
                    && self
                        .resolved
                        .get(1)
                        .is_some_and(|resolved| resolved.request.component == owner.identity())),
            "TP command component identity differs"
        );
        self.command = Some(command::Pending::new(opcode));
        let result = preparation.prepare_third_person_command(
            self.command.as_mut().expect("retained command"),
            source,
            slot_override,
        );
        if result.is_err() {
            self.third_person_command_failed(invocation)?;
        }
        result
    }
}
