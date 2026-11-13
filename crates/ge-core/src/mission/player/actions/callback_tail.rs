//! Return tail of the initial virtual callback, after both action invocations.
//! The following constructor helpers remain separate dependencies.
use super::*;
use crate::mission::player::{identity::PhysicsId, physics};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Logging,
    DerivedTimer,
    State704,
    Physics,
    FinalFlags,
    Returned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsOutcome {
    Absent,
    Cleared {
        identity: PhysicsId,
        flags_before: u16,
        flags_after: u16,
    },
}

#[derive(Debug)]
pub struct CallbackTail {
    player: PlayerId,
    stage: Stage,
    logging_noop_returned: bool,
    e59_raw: Option<u8>,
    initial_state_704: Option<u32>,
    state_704: Option<u32>,
    physics_outcome: Option<PhysicsOutcome>,
    word_708: Option<u32>,
    flags_220_before: Option<u32>,
    flags_220_after: Option<u32>,
}

impl CallbackTail {
    pub fn begin(first: &Pending, second: &Pending, dispatch: &repeat::Dispatch) -> Result<Self> {
        ensure!(
            first.stage() == super::Stage::Returned
                && first.generic() == 0x0900_071e
                && first.parameter_bits() == 0
                && first
                    .action_suffix()
                    .is_some_and(|s| s.stage() == suffix::SuffixStage::Returned)
                && first.action_suffix().and_then(|s| s.repeat_request())
                    == Some(suffix::RepeatRequest {
                        generic: 0x0900_0765,
                        parameter_bits: f32::MAX.to_bits(),
                    }),
            "callback tail precedes the first action return and repeat request"
        );
        ensure!(
            second.player() == first.player()
                && second.stage() == super::Stage::Returned
                && second.generic() == 0x0900_0765
                && second.parameter_bits() == f32::MAX.to_bits()
                && second
                    .action_suffix()
                    .is_some_and(|s| s.stage() == suffix::SuffixStage::Returned)
                && dispatch.player() == first.player()
                && dispatch.stage() == repeat::Stage::Returned,
            "callback tail precedes the repeated action return or changes owner"
        );
        Ok(Self::new(first.player()))
    }

    fn new(player: PlayerId) -> Self {
        Self {
            player,
            stage: Stage::Logging,
            logging_noop_returned: false,
            e59_raw: None,
            initial_state_704: None,
            state_704: None,
            physics_outcome: None,
            word_708: None,
            flags_220_before: None,
            flags_220_after: None,
        }
    }

    /// Run each reached prefix once. A missing owner/field leaves its stage
    /// pending and preserves all earlier stores for a later continuation.
    pub fn advance(
        &mut self,
        base: &mut base_handler::Owner,
        reset: &mut PendingReset,
        item: &Item,
        mut physics: Option<&mut physics::Owner>,
    ) -> Result<()> {
        ensure!(
            self.stage != Stage::Returned,
            "callback tail return replays"
        );
        ensure!(
            base.player() == self.player
                && reset.player() == self.player
                && base.item() == Some(item.id())
                && item.handler() == Some(self.player),
            "callback tail player/item ownership differs"
        );
        loop {
            match self.stage {
                Stage::Logging => {
                    // This build's complete logging callee only spills varargs
                    // into its own stack and returns; it reads no format data.
                    self.logging_noop_returned = true;
                    self.stage = Stage::DerivedTimer;
                }
                Stage::DerivedTimer => {
                    self.e59_raw = Some(reset.clear_callback_timer(self.player)?);
                    self.stage = Stage::State704;
                }
                Stage::State704 => {
                    let next = if self.e59_raw.context("callback branch absent")? as i8 == 0 {
                        2
                    } else {
                        3
                    };
                    self.initial_state_704 = base.write_callback_state(next)?;
                    self.state_704 = Some(next);
                    self.stage = Stage::Physics;
                }
                Stage::Physics => {
                    self.physics_outcome = Some(if let Some(identity) = item.physics() {
                        ensure!(
                            identity.get() == self.player.get(),
                            "callback physics identity differs"
                        );
                        let owner = physics
                            .take()
                            .context("callback physics owner unavailable")?;
                        let (flags_before, flags_after) = owner.clear_callback_motion(item.id())?;
                        PhysicsOutcome::Cleared {
                            identity,
                            flags_before,
                            flags_after,
                        }
                    } else {
                        PhysicsOutcome::Absent
                    });
                    self.stage = Stage::FinalFlags;
                }
                Stage::FinalFlags => {
                    let (before, after) = base.finish_callback_flags()?;
                    self.flags_220_before = Some(before);
                    self.word_708 = Some(1);
                    self.flags_220_after = Some(after);
                    self.stage = Stage::Returned;
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
    pub fn callback_returned(&self) -> bool {
        self.stage == Stage::Returned
    }
    pub fn logging_noop_returned(&self) -> bool {
        self.logging_noop_returned
    }
    pub fn e59_raw(&self) -> Option<u8> {
        self.e59_raw
    }
    pub fn initial_state_704(&self) -> Option<u32> {
        self.initial_state_704
    }
    pub fn state_704(&self) -> Option<u32> {
        self.state_704
    }
    pub fn physics_outcome(&self) -> Option<PhysicsOutcome> {
        self.physics_outcome
    }
    pub fn word_708(&self) -> Option<u32> {
        self.word_708
    }
    pub fn flags_220_before(&self) -> Option<u32> {
        self.flags_220_before
    }
    pub fn flags_220_after(&self) -> Option<u32> {
        self.flags_220_after
    }

    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        emit(self.stage as u32);
        emit(u32::from(self.logging_noop_returned));
        for value in [
            self.e59_raw.map(u32::from),
            self.initial_state_704,
            self.state_704,
            self.word_708,
            self.flags_220_before,
            self.flags_220_after,
        ] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        match self.physics_outcome {
            None => emit(0),
            Some(PhysicsOutcome::Absent) => emit(1),
            Some(PhysicsOutcome::Cleared {
                identity,
                flags_before,
                flags_after,
            }) => {
                emit(2);
                emit((identity.get() >> 32) as u32);
                emit(identity.get() as u32);
                emit(u32::from(flags_before));
                emit(u32::from(flags_after));
            }
        }
    }
}

#[cfg(test)]
mod tests;
