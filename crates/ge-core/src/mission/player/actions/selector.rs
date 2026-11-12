use super::*;
use crate::animation::mode::{
    self,
    runtime::{Cursor, Invocation, Provider},
};
impl Pending {
    pub fn begin_third_person_selector(
        &mut self,
        base: &mut base_handler::Owner,
        reset: &PendingReset,
        preparation: &Preparation,
        fp_available: bool,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonSelector
                && self.selector.is_none()
                && self.player == base.player()
                && self.player == reset.player(),
            "TP selector entry order or player differs"
        );
        self.stage = Stage::SelectingThirdPerson;
        let tracks = !fp_available;
        self.tracks_player_modes = Some(tracks);
        if tracks {
            base.begin_primary_mode_update()?;
            let tp = preparation
                .animator()
                .context("TP primary-head owner is absent")?;
            ensure!(
                tp.player() == self.player
                    && tp.identity()
                        == self
                            .resolved
                            .get(1)
                            .context("TP mapping absent")?
                            .request
                            .component,
                "TP primary-head identity differs"
            );
            let head = tp.layers().context("TP primary layers are absent")?.slots()[0]
                .nodes_head()
                .first()
                .copied();
            self.primary_head = Some((head, 0));
            if let Some(head) = head {
                // The reached snapshot-control type is outside the mode-child
                // subtype chain; its comparison word stays zero. Later child
                // types require their real type/word provider here.
                let control = tp
                    .controls()
                    .get(head)
                    .context("TP primary head type provider is absent")?;
                ensure!(
                    control.component().get() == tp.identity().id().get(),
                    "TP primary head owner differs"
                );
            }
        }
        let previous = reset
            .word(if tracks { 0x714 } else { 0x70c })
            .context("TP selector previous mode is unknown")?;
        let requested = self
            .resolved
            .get(1)
            .context("TP mode mapping is absent")?
            .output;
        self.selector = Some(Cursor::new(
            mode::Inputs {
                requested,
                previous,
                fallback: self.fallback.context("TP selector fallback is unknown")?,
                resource_available: true,
            },
            mode::Budget {
                lookups: 3,
                transition_rows: 65536,
                commands: 4096,
                groups: 4096,
                children: 16384,
            },
        ));
        Ok(())
    }
    pub fn selector(&self) -> Option<&Cursor> {
        self.selector.as_ref()
    }
    pub fn tracks_player_modes(&self) -> Option<bool> {
        self.tracks_player_modes
    }
    pub fn primary_head(&self) -> Option<(Option<crate::animation::layers::NodeId>, u32)> {
        self.primary_head
    }
    pub fn advance_third_person_selector(
        &mut self,
        source: &mut impl Provider,
    ) -> Result<Option<Invocation>> {
        ensure!(
            self.stage == Stage::SelectingThirdPerson,
            "TP live selector advance is out of source order"
        );
        let result = self
            .selector
            .as_mut()
            .context("TP live selector is absent")?
            .next_command(source)?;
        self.stage = if result.is_some() {
            Stage::AwaitingThirdPersonCommand
        } else {
            Stage::AwaitingThirdPersonSetterTail
        };
        Ok(result)
    }
    pub fn third_person_command_executed(&mut self, invocation: Invocation) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonCommand,
            "TP command return is out of source order"
        );
        self.selector
            .as_mut()
            .context("TP live selector is absent")?
            .executed(invocation)?;
        self.stage = Stage::SelectingThirdPerson;
        Ok(())
    }
    pub fn third_person_command_failed(&mut self, invocation: Invocation) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonCommand,
            "TP failed command differs from caller stage"
        );
        self.selector
            .as_mut()
            .context("TP live selector is absent")?
            .failed(invocation)?;
        self.stage = Stage::ThirdPersonCommandFailed;
        Ok(())
    }
}
