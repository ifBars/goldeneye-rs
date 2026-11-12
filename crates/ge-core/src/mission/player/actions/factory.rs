use super::*;
use crate::animation::{layers::command, script_child};
impl Pending {
    pub fn initialize_third_person_child_prefix(
        &mut self,
        preparation: &mut Preparation,
        source: &mut impl script_child::Provider,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonCommand,
            "TP child factory is out of caller order"
        );
        ensure!(
            preparation
                .animator()
                .is_some_and(|owner| owner.player() == self.player
                    && self
                        .resolved
                        .get(1)
                        .is_some_and(|resolved| resolved.request.component == owner.identity())),
            "TP child factory component identity differs"
        );
        let command = self.command.as_mut().context("TP command absent")?;
        ensure!(
            command.stage() == command::Stage::ChildFactoryPending
                && command.factory_node().is_none(),
            "TP child factory differs or replays"
        );
        let result = preparation.initialize_third_person_child_prefix(command, source);
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
