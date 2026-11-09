use super::super::{SessionId, UserId};
use crate::mission::{
    loader::phase::AttemptId,
    spawn,
    triggers::{MapContext, TriggerId},
    world::InstanceId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerGate {
    Session41,
    Session40,
    ExternalOperation0b,
    FrontendLevel,
    ManagerAbsent,
    ManagerNotReady,
    UserAlreadyCreated,
    UserNotEligible,
    CatalogNotReady,
    StartPointMissing,
}

/// An explicit continuation boundary, never a claim that PlayerNew returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDependency {
    OwnerInitialization,
    PhysicsInitialization,
    ComponentInitialization,
    CompletionRegistry,
    PostPublicationInitialization,
    SourceUserFlag,
}

/// Retained source selection and session identity. No completion object, player
/// publication, created byte, camera or native provider completion is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerAttempt {
    pub(super) attempt: AttemptId,
    pub(super) session: SessionId,
    pub(super) user: UserId,
    pub(super) trigger: TriggerId,
    pub(super) instance: InstanceId,
    pub(super) map_selector: i8,
    pub(super) ordinal: u16,
    pub(super) selection: spawn::Selection,
    pub(super) pose: spawn::Pose,
    pub(super) dependency: PlayerDependency,
}
impl PlayerAttempt {
    pub const fn attempt(self) -> AttemptId {
        self.attempt
    }
    pub const fn session(self) -> SessionId {
        self.session
    }
    pub const fn user(self) -> UserId {
        self.user
    }
    pub const fn trigger(self) -> TriggerId {
        self.trigger
    }
    pub const fn map(self) -> MapContext {
        MapContext {
            instance: self.instance,
            map_selector: self.map_selector,
        }
    }
    pub const fn ordinal(self) -> u16 {
        self.ordinal
    }
    pub const fn selection(self) -> spawn::Selection {
        self.selection
    }
    pub const fn pose(self) -> spawn::Pose {
        self.pose
    }
    pub const fn dependency(self) -> PlayerDependency {
        self.dependency
    }
    /// The per-user metadata interface +0x26 producer remains unbound. Known
    /// earlier EXItem/physics prefixes may execute before this input is read.
    pub const fn source_flag_raw(self) -> Option<u8> {
        None
    }
}

/// One-shot dispatch token. No public constructor, Clone or serialization.
#[derive(Debug)]
pub struct PlayerRequest(pub(super) PlayerAttempt);
impl PlayerRequest {
    pub const fn attempt(&self) -> AttemptId {
        self.0.attempt
    }
    pub const fn session(&self) -> SessionId {
        self.0.session
    }
    pub const fn user(&self) -> UserId {
        self.0.user
    }
    pub const fn trigger(&self) -> TriggerId {
        self.0.trigger
    }
    pub const fn map(&self) -> MapContext {
        self.0.map()
    }
    pub const fn ordinal(&self) -> u16 {
        self.0.ordinal
    }
    pub const fn selection(&self) -> spawn::Selection {
        self.0.selection
    }
    pub const fn pose(&self) -> spawn::Pose {
        self.0.pose
    }
    pub const fn source_flag_raw(&self) -> Option<u8> {
        None
    }
}

#[derive(Debug)]
pub enum PlayerAttemptOutcome {
    Gated(PlayerGate),
    Started(PlayerRequest),
    InProgress(PlayerAttempt),
}
