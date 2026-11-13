//! Fresh single-player admission with semantic host identities.
//!
//! This owns no provider, native handle, world or player. Host callbacks report
//! actual operation outcomes; request construction alone never activates a
//! session. Live world/catalog completion remains outside this module.
pub mod local_users;
pub mod member_snapshot;
pub mod player_load;
mod readiness;
mod readiness_load;
pub mod records;

use std::num::NonZeroU64;

use anyhow::{Result, ensure};

use super::Difficulty;
pub use readiness::{FlagsDelta, PeerSnapshot, UserSnapshot, receive_delta};
pub use readiness_load::{
    Event26Receipt, FlagAction, FlagActionReceipt, LoadBinding, LoadReadinessSnapshot,
    LocalEvent26, Phase19Receipt,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGameRequest {
    difficulty: Difficulty,
}

impl NewGameRequest {
    pub const fn new(difficulty: Difficulty) -> Self {
        Self { difficulty }
    }

    pub const fn difficulty(self) -> Difficulty {
        self.difficulty
    }

    pub const fn level_hash(self) -> u32 {
        0x0181_016b
    }

    pub const fn start_point_id(self) -> u32 {
        1
    }

    pub const fn local_user_count(self) -> u32 {
        1
    }
}

/// Host-owned identity, never a provider handle or original memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(NonZeroU64);

impl SessionId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(NonZeroU64::new(value).ok_or_else(|| {
            anyhow::anyhow!("session identity must be nonzero")
        })?))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Host-local identity binds the one admitted user and readiness events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(NonZeroU64);

impl UserId {
    pub fn new(value: u64) -> Result<Self> {
        Ok(Self(NonZeroU64::new(value).ok_or_else(|| {
            anyhow::anyhow!("user identity must be nonzero")
        })?))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    ModePending,
    AwaitingCreate,
    Creating,
    Admitting,
    Active,
    Failed(Failure),
    TornDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Mode,
    Create,
    Admission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationResult {
    Pending,
    Succeeded,
    Failed,
}

/// Only source-proven descriptor fields used by this bounded fresh path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    pub word_2c_raw: u32,
    pub word_30_raw: u32,
    pub local_user_count: u32,
    pub level_hashes: [u32; 3],
}

/// Source-created local manager state. No world readiness is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Managers {
    pub local_user_count: u32,
    pub readiness_flags_raw: u32,
    pub user: UserSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    request: NewGameRequest,
    session_id: SessionId,
    user_id: UserId,
    stage: Stage,
    mode: u32,
    backend: u32,
    descriptor: Option<Descriptor>,
    managers: Option<Managers>,
    member_records: Option<records::Owner>,
    pending: Option<SessionId>,
    active: Option<SessionId>,
    source_create_counter: u32,
    lifecycle_byte_40: u8,
    lifecycle_byte_41: u8,
    readiness_load: Option<readiness_load::Owner>,
    player_load: Option<player_load::PlayerAttempt>,
}

impl Session {
    /// Explicit fresh construction; this does not represent arbitrary provider
    /// history or create a source provider/account identity.
    pub fn begin(request: NewGameRequest, session_id: SessionId, user_id: UserId) -> Self {
        Self {
            request,
            session_id,
            user_id,
            stage: Stage::ModePending,
            mode: 0,
            backend: 0,
            descriptor: None,
            managers: None,
            member_records: None,
            pending: None,
            active: None,
            source_create_counter: 8,
            lifecycle_byte_40: 0,
            lifecycle_byte_41: 0,
            readiness_load: None,
            player_load: None,
        }
    }

    pub fn mode_result(&mut self, result: OperationResult) -> Result<()> {
        ensure!(
            self.stage == Stage::ModePending,
            "mode result out of sequence"
        );
        match result {
            OperationResult::Pending => {}
            OperationResult::Succeeded => {
                self.backend = 2;
                self.mode = 3;
                self.stage = Stage::AwaitingCreate;
            }
            OperationResult::Failed => self.stage = Stage::Failed(Failure::Mode),
        }
        Ok(())
    }

    /// Represents starting the queued create operation after mode completion.
    /// Its request counter +0x58 adds -1; it is not the local-user count.
    pub fn begin_create(&mut self) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingCreate,
            "create start out of sequence"
        );
        self.source_create_counter -= 1;
        self.stage = Stage::Creating;
        Ok(())
    }

    pub fn create_result(&mut self, result: OperationResult) -> Result<()> {
        ensure!(
            self.stage == Stage::Creating,
            "create result out of sequence"
        );
        match result {
            OperationResult::Pending => {}
            OperationResult::Succeeded => {
                self.descriptor = Some(Descriptor {
                    word_2c_raw: 1,
                    word_30_raw: 0,
                    local_user_count: self.request.local_user_count(),
                    level_hashes: [self.request.level_hash(); 3],
                });
                self.managers = Some(Managers {
                    local_user_count: 1,
                    readiness_flags_raw: 0,
                    user: UserSnapshot {
                        identity: self.user_id,
                        flags_raw: 0,
                        created_byte_raw: 0,
                        player_association: local_users::Association::from_constructor(
                            self.session_id,
                            self.user_id,
                        ),
                    },
                });
                let mut records = records::Owner::from_constructor(self.session_id);
                records.register_member(
                    member_snapshot::MemberSnapshot::from_constructor(
                        self.session_id,
                        self.user_id,
                    ),
                    records::Budget {
                        records: 1,
                        visits: 1,
                    },
                )?;
                self.member_records = Some(records);
                self.stage = Stage::Admitting;
            }
            OperationResult::Failed => self.stage = Stage::Failed(Failure::Create),
        }
        Ok(())
    }

    /// Fresh storage contains exactly the created session. Native lookup picks
    /// the first exact handle; an unknown identity clears pending selection.
    pub fn select_pending(&mut self, identity: SessionId) -> Result<bool> {
        ensure!(
            self.stage == Stage::Admitting,
            "session selection out of sequence"
        );
        self.pending = (identity == self.session_id).then_some(self.session_id);
        Ok(self.pending.is_some())
    }

    pub fn admission_result(&mut self, result: OperationResult) -> Result<()> {
        ensure!(
            self.stage == Stage::Admitting,
            "admission result out of sequence"
        );
        match result {
            OperationResult::Pending => {}
            OperationResult::Succeeded => {
                ensure!(
                    self.pending == Some(self.session_id),
                    "created session not selected"
                );
                self.active = self.pending.take();
                self.stage = Stage::Active;
            }
            OperationResult::Failed => {
                self.pending = None;
                self.stage = Stage::Failed(Failure::Admission);
            }
        }
        Ok(())
    }

    /// Source final teardown bytes are 40=0,41=1. Dropping semantic owned
    /// objects here is a host lifetime boundary, not native reference counting.
    pub fn teardown(&mut self) -> Result<()> {
        ensure!(self.stage != Stage::TornDown, "session already torn down");
        self.pending = None;
        self.active = None;
        self.descriptor = None;
        self.managers = None;
        self.member_records = None;
        self.readiness_load = None;
        self.player_load = None;
        self.lifecycle_byte_40 = 0;
        self.lifecycle_byte_41 = 1;
        self.stage = Stage::TornDown;
        Ok(())
    }

    pub fn request(&self) -> NewGameRequest {
        self.request
    }

    pub fn identity(&self) -> SessionId {
        self.session_id
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn mode(&self) -> u32 {
        self.mode
    }

    pub fn backend(&self) -> u32 {
        self.backend
    }

    pub fn descriptor(&self) -> Option<&Descriptor> {
        self.descriptor.as_ref()
    }

    pub fn managers(&self) -> Option<&Managers> {
        self.managers.as_ref()
    }

    pub fn pending(&self) -> Option<SessionId> {
        self.pending
    }

    pub fn active(&self) -> Option<SessionId> {
        self.active
    }

    pub fn source_create_counter(&self) -> u32 {
        self.source_create_counter
    }

    pub fn lifecycle_bytes(&self) -> [u8; 2] {
        [self.lifecycle_byte_40, self.lifecycle_byte_41]
    }

    /// Source800c0ac0 resolves active then pending descriptor; the fresh owned
    /// one-user count makes this false without a caller-supplied scale flag.
    pub fn multiple_local_users_in_mode_three(&self) -> bool {
        (self.active.is_some() || self.pending.is_some())
            && self.mode == 3
            && self
                .descriptor
                .is_some_and(|descriptor| descriptor.local_user_count > 1)
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
