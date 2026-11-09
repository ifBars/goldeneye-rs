//! The narrow same-user XNetSessionMember field used as PlayerNew's incoming
//! flag. This is distinct from readiness flags+28 and the player's own byte994.
//! It does not model a complete native object, registry, RTTI or SDK dispatch.
use super::{Session, SessionId, Stage, UserId};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    FreshConstructor,
    SuppliedSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberSnapshot {
    session: SessionId,
    user: UserId,
    source_flag_raw: Option<u8>,
    origin: Origin,
}

impl MemberSnapshot {
    /// The actual metadata constructor writes byte+26=0. No other bytes of its
    /// native allocation are invented, nor is original subsequent history known.
    pub const fn from_constructor(session: SessionId, user: UserId) -> Self {
        Self {
            session,
            user,
            source_flag_raw: Some(0),
            origin: Origin::FreshConstructor,
        }
    }

    /// Explicitly supplied narrow field state. None means an unknown field on
    /// a present matching snapshot, not zero or an absent session member.
    pub const fn from_supplied(
        session: SessionId,
        user: UserId,
        source_flag_raw: Option<u8>,
    ) -> Self {
        Self {
            session,
            user,
            source_flag_raw,
            origin: Origin::SuppliedSnapshot,
        }
    }

    pub const fn session(self) -> SessionId {
        self.session
    }
    pub const fn user(self) -> UserId {
        self.user
    }
    pub const fn source_flag_raw(self) -> Option<u8> {
        self.source_flag_raw
    }
    pub const fn origin(self) -> Origin {
        self.origin
    }
}

impl Session {
    fn require_member_identity(&self, session: SessionId, user: UserId) -> Result<()> {
        ensure!(
            matches!(self.stage, Stage::Admitting | Stage::Active),
            "member metadata requires the created session manager"
        );
        ensure!(
            session == self.session_id && user == self.user_id,
            "member metadata belongs to a different session or user"
        );
        let managers = self
            .managers
            .as_ref()
            .context("member metadata manager absent")?;
        ensure!(
            managers.user.identity == user,
            "member metadata differs from the actual manager user"
        );
        Ok(())
    }

    /// Unknown field state is returned as a present MemberSnapshot containing
    /// None. Missing metadata, foreign identities and invalid lifecycle are errors.
    pub fn member_snapshot(&self, session: SessionId, user: UserId) -> Result<&MemberSnapshot> {
        self.require_member_identity(session, user)?;
        let snapshot = self
            .member_snapshot
            .as_ref()
            .context("matching member metadata absent")?;
        ensure!(
            snapshot.session == session && snapshot.user == user,
            "retained member metadata identity differs"
        );
        Ok(snapshot)
    }

    /// Bind an explicit supplied field snapshot to the same existing member.
    /// This does NOT execute 80212100's dirty-channel/associated-player callbacks
    /// or claim that a native transport update occurred. Already captured player
    /// requests remain immutable even when this retained snapshot is replaced.
    pub fn replace_member_snapshot(&mut self, snapshot: MemberSnapshot) -> Result<()> {
        self.require_member_identity(snapshot.session, snapshot.user)?;
        ensure!(
            self.member_snapshot.is_some(),
            "matching member metadata absent"
        );
        self.member_snapshot = Some(snapshot);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
