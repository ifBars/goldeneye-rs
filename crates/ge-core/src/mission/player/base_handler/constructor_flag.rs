//! The source constructor's raw flag consumer after the derived reset returns.
//! The captured metadata argument is separate from the earlier reset byte994.
use super::{Failure, Owner, Stage};
use crate::mission::player::{constructor::ResetCompletion, identity::PlayerId};
use anyhow::{Context, Result, ensure};

pub trait ConstructorFlagProvider {
    /// Notify the actual bound +4f8 interface through its dirty channel.
    /// Interface presence comes from Owner, never from a caller readiness flag.
    fn mark_constructor_flag_dirty(&mut self, channel: u32) -> Result<()>;
}

/// Reached flag-consumer outcome, not completed PlayerNew or player readiness.
#[derive(Debug)]
pub struct ConstructorFlagReceipt {
    player: PlayerId,
    raw: u8,
    changed: bool,
    interface_callbacks: usize,
}
impl ConstructorFlagReceipt {
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn raw(&self) -> u8 {
        self.raw
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn interface_callbacks(&self) -> usize {
        self.interface_callbacks
    }
}

impl Owner {
    pub fn apply_constructor_flag(
        &mut self,
        reset: &ResetCompletion,
        raw: u8,
        suppress_notification: i8,
        provider: &mut impl ConstructorFlagProvider,
    ) -> Result<ConstructorFlagReceipt> {
        self.require(Stage::RuntimeResetComplete)?;
        ensure!(
            reset.player() == self.player,
            "constructor flag reset proof belongs to another player"
        );
        self.stage = Stage::ConstructorFlagPrefix;
        match self.consume_constructor_flag(raw, suppress_notification, provider) {
            Ok(receipt) => {
                self.stage = Stage::ConstructorFlagComplete;
                Ok(receipt)
            }
            Err(error) => {
                self.failure = Some(Failure::ConstructorFlag);
                Err(error)
            }
        }
    }

    fn consume_constructor_flag(
        &mut self,
        raw: u8,
        suppress_notification: i8,
        provider: &mut impl ConstructorFlagProvider,
    ) -> Result<ConstructorFlagReceipt> {
        let previous = self
            .byte(0x994)
            .context("constructor flag byte994 is unavailable")?;
        // The native signed-byte comparison is equal exactly when raw bytes
        // are equal. Equality performs no write and does not even read +4f8.
        let changed = previous != raw;
        let mut interface_callbacks = 0;
        if changed {
            self.b(0x994, raw);
            if suppress_notification == 0 {
                let interface = self
                    .word(0x4f8)
                    .context("constructor flag interface binding is unavailable")?;
                if interface != 0 {
                    provider
                        .mark_constructor_flag_dirty(2)
                        .context("constructor flag dirty-channel notification failed")?;
                    interface_callbacks = 1;
                }
            }
        }
        Ok(ConstructorFlagReceipt {
            player: self.player,
            raw,
            changed,
            interface_callbacks,
        })
    }
}
