use super::super::{FlagsDelta, SessionId, UserId};
use crate::mission::loader::phase::AttemptId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    pub session: SessionId,
    pub user: UserId,
    pub attempt: AttemptId,
    pub level: u32,
}

/// Actual admission/loader association, not a serialized completion receipt.
/// Root must retain this unique binding with the same worker and loaded owners.
#[derive(Debug)]
pub struct LoadBinding {
    pub(super) identity: Identity,
}
impl LoadBinding {
    pub const fn session(&self) -> SessionId {
        self.identity.session
    }
    pub const fn user(&self) -> UserId {
        self.identity.user
    }
    pub const fn attempt(&self) -> AttemptId {
        self.identity.attempt
    }
    pub const fn level_hash(&self) -> u32 {
        self.identity.level
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagAction {
    Clear(u32),
    Set(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalEvent26 {
    pub session: SessionId,
    pub source: UserId,
    pub delta: FlagsDelta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event26Receipt {
    pub event: LocalEvent26,
    pub peer_updated: bool,
    pub user_updated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagActionReceipt {
    pub action: FlagAction,
    pub owner_before: u32,
    pub owner_after: u32,
    pub local_before: u32,
    pub local_after: u32,
    pub local_store: bool,
    pub event: Option<Event26Receipt>,
}

/// Typed result of this invocation. No asset payloads, native addresses or JSON
/// authority; the app publishes only its deliberately scoped scalar evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase19Receipt {
    pub actions: [FlagActionReceipt; 4],
    pub changed_events: usize,
    pub peer_updates: usize,
    pub user_updates: usize,
    pub local_mirror_stores: usize,
    pub local_observer_callbacks: usize,
    pub user_observer_callbacks: usize,
    pub owner_flags_raw: u32,
    pub peer_flags_raw: u32,
    pub user_flags_raw: u32,
    pub local_mirror_flags_raw: u32,
    pub created_byte_raw: u8,
    pub native_timing_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadReadinessSnapshot {
    pub owner_flags_raw: u32,
    pub peer_flags_raw: Option<u32>,
    pub user_flags_raw: u32,
    pub local_mirror_flags_raw: u32,
    pub created_byte_raw: u8,
    pub invocations: usize,
    pub changed_events: usize,
    pub local_mirror_stores: usize,
    pub delivery_failed: bool,
}
