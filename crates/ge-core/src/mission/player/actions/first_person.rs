//! Initial FP dispatch on its actual receiver. TP receipts remain retained while
//! shared fallback/history advance through the later call.
use super::*;
use crate::animation::{
    dispatch::Fallback,
    layers::{NodeId, SlotOverride, command},
    mode::{
        self,
        runtime::{Cursor, Invocation, Provider},
    },
    script_child,
};
mod commands;
mod state;
#[cfg(test)]
#[path = "first_person/tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPersonStage {
    Entering,
    Selecting,
    CommandPending,
    SetterTail,
    DispatchTail,
    Returned,
    Failed,
}
pub struct FirstPersonDispatch {
    player: PlayerId,
    receiver: Identity,
    generic: u32,
    requested: u32,
    stage: FirstPersonStage,
    fallback: Option<(u32, u64)>,
    primary_head: Option<(Option<NodeId>, u32)>,
    initial_heads_empty: bool,
    selector: Option<Cursor>,
    command: Option<command::Pending>,
    completed_commands: Vec<command::Pending>,
    setter: Option<SetterReturn>,
    coordinate_written: Option<bool>,
}
impl Pending {
    pub fn first_person_dispatch(&self) -> Option<&FirstPersonDispatch> {
        self.first_person.as_ref()
    }
    pub fn first_person_dispatch_mut(&mut self) -> Result<&mut FirstPersonDispatch> {
        ensure!(
            self.stage == Stage::FirstPersonDispatch,
            "FP dispatcher is not current"
        );
        self.first_person.as_mut().context("FP dispatcher absent")
    }
    pub fn begin_first_person_dispatch(
        &mut self,
        base: &mut base_handler::Owner,
        reset: &PendingReset,
        preparation: &mut Preparation,
        fallback: &mut Fallback,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingFirstPersonDispatch
                && self.first_person.is_none()
                && base.player() == self.player
                && reset.player() == self.player
                && self
                    .dispatch
                    .as_ref()
                    .is_some_and(|d| d.stage() == DispatchStage::Returned),
            "FP dispatch entry owner/order differs"
        );
        let resolved = self.resolved.first().context("FP mapped request absent")?;
        self.first_person = Some(FirstPersonDispatch {
            player: self.player,
            receiver: resolved.request.component,
            generic: self.generic,
            requested: resolved.output,
            stage: FirstPersonStage::Entering,
            fallback: None,
            primary_head: None,
            initial_heads_empty: false,
            selector: None,
            command: None,
            completed_commands: Vec::new(),
            setter: None,
            coordinate_written: None,
        });
        self.stage = Stage::FirstPersonDispatch;
        let fp = self.first_person.as_mut().expect("retained FP dispatch");
        let result = fp.begin(base, reset, preparation, fallback);
        if result.is_err() {
            fp.stage = FirstPersonStage::Failed;
        }
        result
    }
    pub fn complete_first_person_dispatch(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::FirstPersonDispatch
                && self
                    .first_person
                    .as_ref()
                    .is_some_and(|fp| fp.stage == FirstPersonStage::Returned),
            "FP dispatcher has not returned"
        );
        self.stage = Stage::AwaitingActionSuffix;
        Ok(())
    }
}
impl FirstPersonDispatch {
    fn begin(
        &mut self,
        base: &mut base_handler::Owner,
        reset: &PendingReset,
        preparation: &mut Preparation,
        fallback: &mut Fallback,
    ) -> Result<()> {
        fallback.store(0x0900_000b)?;
        ensure!(
            self.generic == 0x0900_071e,
            "unsupported initial FP dispatch case"
        );
        fallback.store(0x0900_065e)?;
        self.fallback = Some((fallback.value()?, fallback.writes()));
        let owner = preparation.mode_animator(self.receiver)?;
        ensure!(
            owner.player() == self.player
                && owner.component().is_some_and(|r| r.file == 0x0102_0016),
            "FP qualifier receiver/resource differs"
        );
        // This receiver's resource qualifies directly. No separate availability
        // query or control capture precedes the tracked primary setter.
        base.begin_primary_mode_update()?;
        preparation.bind_empty_mode_layers(self.receiver)?;
        let owner = preparation.mode_animator(self.receiver)?;
        let layers = owner.layers().context("FP actual layers absent")?;
        ensure!(
            layers.slots().iter().all(|s| s.nodes_head().is_empty()),
            "fresh initial FP heads are not empty"
        );
        self.initial_heads_empty = true;
        self.primary_head = Some((None, 0));
        let previous = reset
            .word(0x714)
            .context("FP previous tracked mode unknown")?;
        self.selector = Some(Cursor::new(
            mode::Inputs {
                requested: self.requested,
                previous,
                fallback: self.fallback.expect("bound fallback").0,
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
        self.stage = FirstPersonStage::Selecting;
        Ok(())
    }
    pub fn identity(&self) -> Identity {
        self.receiver
    }
    pub fn stage(&self) -> FirstPersonStage {
        self.stage
    }
    pub fn generic(&self) -> u32 {
        self.generic
    }
    pub fn requested(&self) -> u32 {
        self.requested
    }
    pub fn fallback(&self) -> Option<(u32, u64)> {
        self.fallback
    }
    pub fn initial_heads_empty(&self) -> bool {
        self.initial_heads_empty
    }
    pub fn primary_head(&self) -> Option<(Option<NodeId>, u32)> {
        self.primary_head
    }
    pub fn selector(&self) -> Option<&Cursor> {
        self.selector.as_ref()
    }
    pub fn setter(&self) -> Option<SetterReturn> {
        self.setter
    }
    pub fn coordinate_written(&self) -> Option<bool> {
        self.coordinate_written
    }
    pub fn completed_commands(&self) -> &[command::Pending] {
        &self.completed_commands
    }
    pub fn command(&self) -> Option<&command::Pending> {
        self.command.as_ref().or_else(|| {
            matches!(
                self.stage,
                FirstPersonStage::SetterTail
                    | FirstPersonStage::DispatchTail
                    | FirstPersonStage::Returned
            )
            .then(|| self.completed_commands.last())
            .flatten()
        })
    }
    pub fn command_acknowledged(&self) -> bool {
        self.command.is_none() && !self.completed_commands.is_empty()
    }
    pub fn finish_setter(
        &mut self,
        reset: &mut PendingReset,
        preparation: &Preparation,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::SetterTail
                && self.setter.is_none()
                && reset.player() == self.player,
            "FP setter return owner/order differs"
        );
        let word = self
            .selector
            .as_ref()
            .context("FP selector absent")?
            .return_word()
            .context("FP selector has not returned")?;
        self.setter = Some(SetterReturn {
            return_word: word,
            history_updated: false,
            primary_changed: None,
        });
        let result = super::setter::finish_tracked(
            super::setter::Inputs {
                player: self.player,
                receiver: self.receiver,
                saved: self.primary_head,
                generic: self.generic,
                mapped: self.requested,
            },
            reset,
            preparation,
            self.setter.as_mut().expect("retained setter"),
        );
        self.stage = if result.is_ok() {
            FirstPersonStage::DispatchTail
        } else {
            FirstPersonStage::Failed
        };
        result
    }
    pub fn finish_dispatch_tail(
        &mut self,
        reset: &PendingReset,
        preparation: &mut Preparation,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::DispatchTail
                && self.coordinate_written.is_none()
                && reset.player() == self.player,
            "FP tail owner/order differs"
        );
        let bits = reset.word(0x99c).context("FP slot1 blend input unknown")?;
        match preparation.write_mode_dispatch_scalar(self.receiver, 1, bits) {
            Ok(written) => {
                self.coordinate_written = Some(written);
                self.stage = FirstPersonStage::Returned;
                Ok(())
            }
            Err(e) => {
                self.stage = FirstPersonStage::Failed;
                Err(e)
            }
        }
    }
}
