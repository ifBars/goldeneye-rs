//! Source-ordered animation preload over the existing constructor allocations.
//! A reached temporary dependency never commits its key or clears the flag.
use super::{Lease, ResetStage, Shared, initializer_prefix};
use crate::mission::player::{components::Identity, identity::PlayerId, transform_cache};
use anyhow::{Context, Result, ensure};
mod call;
pub mod post_bind;
mod state;
pub use call::{Call, CallStage, ReturnReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub player: PlayerId,
    pub receiver: Identity,
    pub ordinal: usize,
    pub source_ordinal: usize,
    pub weapon: u32,
    pub alternate: bool,
    pub variant: u8,
}
impl Request {
    pub fn generic(self) -> u32 {
        if self.alternate {
            0x0900_07a7
        } else {
            0x0900_0765
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    pub output: u32,
    pub accepted: bool,
}
pub trait Source {
    fn registry_count(&mut self) -> Result<usize>;
    fn enabled_byte(&mut self, source_ordinal: usize) -> Result<u8>;
    fn weapon(&mut self, source_ordinal: usize) -> Result<u32>;
    fn remap(&mut self, request: Request) -> Result<Mapping>;
    fn extract_reference(&mut self, receiver: Identity, mapped: u32) -> Result<Option<u32>>;
}
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub registry_rows: usize,
    pub calls: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            registry_rows: 1024,
            calls: 6144,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Entry,
    RawFlags,
    EntryKeys,
    RegistryCount,
    Row,
    Weapon,
    Call,
    AwaitingTemporary,
    FinalFlag,
    Returned,
}

#[derive(Debug)]
pub struct Pending {
    player: PlayerId,
    lease: Lease,
    receiver: Identity,
    stage: Stage,
    entry_flag: Option<u8>,
    raw_flags_written: u8,
    entry_keys_written: u8,
    registry_count: Option<usize>,
    row: usize,
    row_flags: Vec<u8>,
    enabled_rows: usize,
    weapon: Option<u32>,
    call_in_row: u8,
    completed: Vec<Call>,
    current: Option<Call>,
    flag_cleared: bool,
}
impl Pending {
    pub fn begin(prefix: &initializer_prefix::Pending, lease: Lease) -> Result<Self> {
        ensure!(
            prefix.stage() == initializer_prefix::Stage::AwaitingPreload
                && prefix.cache_enabled() == Some(1)
                && prefix.player() == lease.player(),
            "preload precedes actual initializer flag gate or changes player"
        );
        let receiver = prefix.receivers()[0].context("preload actual FP receiver absent")?;
        ensure!(
            receiver.player() == prefix.player(),
            "preload receiver player differs"
        );
        Ok(Self::new(lease, receiver))
    }
    fn new(lease: Lease, receiver: Identity) -> Self {
        Self {
            player: lease.player(),
            lease,
            receiver,
            stage: Stage::Entry,
            entry_flag: None,
            raw_flags_written: 0,
            entry_keys_written: 0,
            registry_count: None,
            row: 0,
            row_flags: Vec::new(),
            enabled_rows: 0,
            weapon: None,
            call_in_row: 0,
            completed: Vec::new(),
            current: None,
            flag_cleared: false,
        }
    }
    pub fn advance(
        &mut self,
        shared: &mut Shared,
        caches: &mut transform_cache::Owner,
        source: &mut impl Source,
        budget: Budget,
    ) -> Result<()> {
        ensure!(
            self.lease.player() == self.player
                && self.receiver.player() == self.player
                && shared.reset_stage(self.lease)? == ResetStage::Complete,
            "preload owner or constructor lease differs"
        );
        loop {
            match self.stage {
                Stage::Entry => {
                    let flag = shared.cache_enabled();
                    self.entry_flag = Some(flag);
                    if flag == 0 {
                        self.stage = Stage::Returned;
                        return Ok(());
                    }
                    self.stage = Stage::RawFlags;
                }
                Stage::RawFlags => {
                    while self.raw_flags_written < 32 {
                        shared.clear_preload_raw_flag(self.lease, self.raw_flags_written)?;
                        self.raw_flags_written += 1;
                    }
                    self.stage = Stage::EntryKeys;
                }
                Stage::EntryKeys => {
                    while self.entry_keys_written < 34 {
                        shared.clear_preload_entry_key(self.lease, self.entry_keys_written)?;
                        self.entry_keys_written += 1;
                    }
                    self.stage = Stage::RegistryCount;
                }
                Stage::RegistryCount => {
                    let count = match self.registry_count {
                        Some(count) => count,
                        None => {
                            let count = source.registry_count()?;
                            self.registry_count = Some(count);
                            count
                        }
                    };
                    ensure!(
                        count <= budget.registry_rows,
                        "preload registry row budget exhausted"
                    );
                    self.row_flags.try_reserve_exact(count)?;
                    self.stage = Stage::Row;
                }
                Stage::Row => {
                    if self.row
                        == self
                            .registry_count
                            .context("preload registry count absent")?
                    {
                        self.stage = Stage::FinalFlag;
                        continue;
                    }
                    let flag = source.enabled_byte(self.row)?;
                    self.row_flags.push(flag);
                    if flag == 0 {
                        self.row += 1;
                    } else {
                        self.enabled_rows += 1;
                        self.stage = Stage::Weapon;
                    }
                }
                Stage::Weapon => {
                    let weapon = source.weapon(self.row)?;
                    ensure!(
                        weapon != 0,
                        "preload reached an invalid terminated registry row"
                    );
                    self.weapon = Some(weapon);
                    self.call_in_row = 0;
                    self.stage = Stage::Call;
                }
                Stage::Call => {
                    if self.current.is_none() {
                        ensure!(
                            self.completed.len() < budget.calls,
                            "preload call budget exhausted"
                        );
                        self.completed.try_reserve(1)?;
                        self.current = Some(Call::begin(Request {
                            player: self.player,
                            receiver: self.receiver,
                            ordinal: self.completed.len(),
                            source_ordinal: self.row,
                            weapon: self.weapon.context("preload row weapon absent")?,
                            alternate: !self.call_in_row.is_multiple_of(2),
                            variant: self.call_in_row / 2,
                        })?);
                    }
                    let call = self.current.as_mut().expect("retained preload call");
                    call.advance(self.lease, shared, caches, source)?;
                    if call.stage() == CallStage::AwaitingTemporary {
                        self.stage = Stage::AwaitingTemporary;
                        return Ok(());
                    }
                    ensure!(
                        call.stage() == CallStage::Returned,
                        "preload call has not returned"
                    );
                    self.completed
                        .push(self.current.take().expect("completed preload call"));
                    self.call_in_row += 1;
                    if self.call_in_row == 6 {
                        self.row += 1;
                        self.weapon = None;
                        self.call_in_row = 0;
                        self.stage = Stage::Row;
                    }
                }
                Stage::AwaitingTemporary | Stage::Returned => return Ok(()),
                Stage::FinalFlag => {
                    // This source write is reachable only after actual registry
                    // exhaustion and all six calls on every enabled row return.
                    ensure!(
                        self.current.is_none()
                            && self.row
                                == self
                                    .registry_count
                                    .context("preload registry count absent")?,
                        "preload flag clear precedes all actual child returns"
                    );
                    shared.cache_enabled = 0;
                    self.flag_cleared = true;
                    self.stage = Stage::Returned;
                    return Ok(());
                }
            }
        }
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn lease(&self) -> Lease {
        self.lease
    }
    pub fn receiver(&self) -> Identity {
        self.receiver
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn entry_flag(&self) -> Option<u8> {
        self.entry_flag
    }
    pub fn raw_flags_written(&self) -> u8 {
        self.raw_flags_written
    }
    pub fn entry_keys_written(&self) -> u8 {
        self.entry_keys_written
    }
    pub fn registry_count(&self) -> Option<usize> {
        self.registry_count
    }
    pub fn source_row(&self) -> usize {
        self.row
    }
    pub fn row_flags(&self) -> &[u8] {
        &self.row_flags
    }
    pub fn enabled_rows(&self) -> usize {
        self.enabled_rows
    }
    pub fn current_weapon(&self) -> Option<u32> {
        self.weapon
    }
    pub fn call_in_row(&self) -> u8 {
        self.call_in_row
    }
    pub fn completed_calls(&self) -> &[Call] {
        &self.completed
    }
    pub fn current_call(&self) -> Option<&Call> {
        self.current.as_ref()
    }
    pub fn flag_cleared(&self) -> bool {
        self.flag_cleared
    }
    pub fn helper_returned(&self) -> bool {
        self.stage == Stage::Returned
    }
    pub fn dependency(&self) -> &'static str {
        match self.stage {
            Stage::AwaitingTemporary => "preload_temporary_bind",
            Stage::Returned => "player_initializer_post_preload",
            _ => "player_animation_preload",
        }
    }
}

#[cfg(test)]
mod tests;
