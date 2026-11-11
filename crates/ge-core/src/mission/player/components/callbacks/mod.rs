//! Constructor-owned shared callback table and the supported PlayerNew receiver
//! routes. Table binding is distinct from animation scheduling or pose sampling.
mod dispatch;
use anyhow::{Result, ensure};
pub use dispatch::{Event, EventCall, EventOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handler {
    Event,
    Noop,
    Acknowledge,
}
#[derive(Debug)]
pub struct Table {
    owner: u64,
    handlers: [Handler; 4],
}
impl Table {
    /// Actual pre-main initializer's four slots, with a host semantic identity.
    /// No source address, original executable or imported runtime history lives here.
    pub fn from_constructor(owner: u64) -> Result<Self> {
        ensure!(owner != 0, "animation callback table identity is zero");
        Ok(Self {
            owner,
            handlers: [
                Handler::Event,
                Handler::Noop,
                Handler::Noop,
                Handler::Acknowledge,
            ],
        })
    }
    pub fn owner(&self) -> u64 {
        self.owner
    }
    pub fn handlers(&self) -> &[Handler; 4] {
        &self.handlers
    }
    pub fn noop(&self, slot: usize) -> Result<()> {
        ensure!(
            self.handlers.get(slot) == Some(&Handler::Noop),
            "callback slot is not a source no-op"
        );
        Ok(())
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.owner >> 32) as u32);
        emit(self.owner as u32);
        for handler in self.handlers {
            emit(match handler {
                Handler::Event => 1,
                Handler::Noop => 2,
                Handler::Acknowledge => 3,
            });
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
