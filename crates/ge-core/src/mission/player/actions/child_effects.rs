use super::*;
use crate::animation::layers::{SlotOverride, command};
impl Pending {
    pub fn finish_third_person_child_effects(
        &mut self,
        preparation: &mut Preparation,
        source: &mut impl command::Source,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonCommand
                && preparation
                    .animator()
                    .is_some_and(|owner| owner.player() == self.player
                        && self
                            .resolved
                            .get(1)
                            .is_some_and(|r| r.request.component == owner.identity())),
            "TP child effect caller identity/order differs"
        );
        let command = self
            .command
            .as_mut()
            .context("TP child effect command absent")?;
        let result = preparation.finish_third_person_child_effects(command, source, slot_override);
        if result.is_err() {
            let invocation = self
                .selector
                .as_ref()
                .context("TP selector absent")?
                .pending()
                .context("TP invocation absent")?;
            self.third_person_command_failed(invocation)?;
        }
        result
    }
}
