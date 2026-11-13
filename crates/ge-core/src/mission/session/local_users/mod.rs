//! The actual admitted local user's player link. Readiness and created bytes
//! neither initialize this link nor substitute for its source-owned value.
use super::{Session, SessionId, Stage, UserId};
use crate::mission::player::identity::PlayerId;
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Unavailable,
    KnownNull,
    Bound(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Unavailable,
    LocalUserConstructor,
    SuppliedSnapshot,
}

/// A field on the same local user as its readiness snapshot. The owning session
/// and user prevent a NULL captured from a different lifetime satisfying lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Association {
    session: Option<SessionId>,
    user: UserId,
    state: State,
    origin: Origin,
}
impl Association {
    pub(crate) fn from_constructor(session: SessionId, user: UserId) -> Self {
        // The local-user constructor explicitly stores NULL before collection
        // insertion. Player publication occurs after the full player return.
        Self {
            session: Some(session),
            user,
            state: State::KnownNull,
            origin: Origin::LocalUserConstructor,
        }
    }

    pub fn unavailable(user: UserId) -> Self {
        Self {
            session: None,
            user,
            state: State::Unavailable,
            origin: Origin::Unavailable,
        }
    }

    /// An explicit supplied value is separate from constructor history. This
    /// does not publish a newly constructed player or change a Session owner.
    pub fn from_supplied(session: SessionId, user: UserId, state: State) -> Self {
        Self {
            session: Some(session),
            user,
            state,
            origin: Origin::SuppliedSnapshot,
        }
    }

    pub fn session(self) -> Option<SessionId> {
        self.session
    }
    pub fn user(self) -> UserId {
        self.user
    }
    pub fn state(self) -> State {
        self.state
    }
    pub fn origin(self) -> Origin {
        self.origin
    }

    pub fn visit_state_words(self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit(u32::from(self.session.is_some()));
        for value in [self.session.map_or(0, SessionId::get), self.user.get()] {
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        emit(self.origin as u32);
        match self.state {
            State::Unavailable => emit(0),
            State::KnownNull => emit(1),
            State::Bound(player) => {
                emit(2);
                emit((player.get() >> 32) as u32);
                emit(player.get() as u32);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lookup {
    pub session: SessionId,
    pub target: PlayerId,
    pub visited: usize,
    pub matched: Option<UserId>,
    pub association: Association,
}

impl Session {
    /// Visit the actual supported one-user collection in ordinal order. A
    /// missing collection or unknown association cannot prove a lookup miss.
    pub fn lookup_local_user_for_player(&self, target: PlayerId) -> Result<Lookup> {
        ensure!(
            self.stage == Stage::Active && self.active == Some(self.session_id),
            "local-user lookup requires the actual admitted session"
        );
        let managers = self
            .managers
            .as_ref()
            .context("local-user collection unavailable")?;
        ensure!(
            managers.local_user_count == 1,
            "local-user collection requires its actual supported backing"
        );
        let user = &managers.user;
        let association = user.player_association;
        ensure!(
            user.identity == self.user_id
                && association.user == user.identity
                && association.session == Some(self.session_id),
            "local-user association has a stale or foreign owner"
        );
        let matched = match association.state {
            State::Unavailable => anyhow::bail!("local-user player association unavailable"),
            State::KnownNull => None,
            State::Bound(player) => (player == target).then_some(user.identity),
        };
        Ok(Lookup {
            session: self.session_id,
            target,
            visited: 1,
            matched,
            association,
        })
    }
}

#[cfg(test)]
mod tests;
