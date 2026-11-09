//! Ordered semantic registration of actual session member records. Source
//! outer kind+44 is derived from the metadata interface+4; metadata+44 is a
//! different platform field. SDK slots, handles and transport are not modeled.
use super::{Session, SessionId, Stage, UserId, member_snapshot::MemberSnapshot};
use anyhow::{Context, Result, ensure};
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordId {
    session: SessionId,
    sequence: NonZeroU64,
}
impl RecordId {
    pub const fn session(self) -> SessionId {
        self.session
    }
    /// Host semantic sequence, never an SDK slot or source handle.
    pub const fn get(self) -> u64 {
        self.sequence.get()
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub records: usize,
    pub visits: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    Member(MemberSnapshot),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    id: RecordId,
    kind_raw: u32,
    payload: Payload,
}
impl Record {
    pub const fn id(&self) -> RecordId {
        self.id
    }
    pub const fn kind_raw(&self) -> u32 {
        self.kind_raw
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    pub fn member(&self) -> &MemberSnapshot {
        match &self.payload {
            Payload::Member(member) => member,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    session: SessionId,
    records: Vec<Record>,
    next_sequence: u64,
}
impl Owner {
    /// Explicit fresh semantic owner, not arbitrary original registry history.
    pub fn from_constructor(session: SessionId) -> Self {
        Self {
            session,
            records: Vec::new(),
            next_sequence: 1,
        }
    }
    pub const fn session(&self) -> SessionId {
        self.session
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    pub fn register_member(
        &mut self,
        mut member: MemberSnapshot,
        budget: Budget,
    ) -> Result<RecordId> {
        ensure!(
            member.session() == self.session,
            "member registration session differs"
        );
        let kind = member
            .interface_kind_raw()
            .context("member interface kind is not constructor-bound")?;
        ensure!(
            kind == 2 && member.record_id().is_none(),
            "unsupported or already registered member interface"
        );
        ensure!(
            self.records.len() < budget.records && self.records.len() <= budget.visits,
            "session record registration budget exhausted"
        );
        ensure!(
            !self
                .records
                .iter()
                .any(|row| row.member().user() == member.user()),
            "repeated member registration requires an unsupported replacement lifecycle"
        );
        let sequence =
            NonZeroU64::new(self.next_sequence).context("session record identity is zero")?;
        let next = self
            .next_sequence
            .checked_add(1)
            .context("session record identity exhausted")?;
        self.records
            .try_reserve(1)
            .context("session record allocation failed")?;
        let id = RecordId {
            session: self.session,
            sequence,
        };
        member.bind_record(id)?;
        // Actual constructor kind, outer payload and metadata backlink precede
        // publication. Vec order is a semantic adaptation, not SDK bucket order.
        self.records.push(Record {
            id,
            kind_raw: kind,
            payload: Payload::Member(member),
        });
        self.next_sequence = next;
        Ok(id)
    }

    pub fn member_snapshot(&self, session: SessionId, user: UserId) -> Result<&MemberSnapshot> {
        ensure!(
            session == self.session,
            "session record owner identity differs"
        );
        let row = self
            .records
            .iter()
            .find(|row| row.kind_raw == 2 && row.member().user() == user)
            .context("matching registered member metadata absent")?;
        let member = row.member();
        ensure!(
            member.session() == session
                && member.record_id() == Some(row.id)
                && member.interface_kind_raw() == Some(row.kind_raw),
            "member payload/backlink identity differs"
        );
        Ok(member)
    }

    pub fn replace_member_snapshot(&mut self, snapshot: MemberSnapshot) -> Result<()> {
        self.member_snapshot(snapshot.session(), snapshot.user())?;
        let row = self
            .records
            .iter_mut()
            .find(|row| row.kind_raw == 2 && row.member().user() == snapshot.user())
            .context("matching registered member metadata absent")?;
        match &mut row.payload {
            Payload::Member(member) => member.replace_field_snapshot(snapshot),
        }
    }

    /// Execute the source kind2 predicate over retained rows. No request count,
    /// descriptor local-user count, metadata flag or cached total is authority.
    pub fn count_members(&self) -> Result<u32> {
        let mut count = 0_u32;
        for row in &self.records {
            if row.kind_raw != 2 {
                continue;
            }
            let member = row.member();
            ensure!(
                row.id.session() == self.session
                    && member.session() == self.session
                    && member.record_id() == Some(row.id)
                    && member.interface_kind_raw() == Some(2),
                "registered member kind/payload/backlink is not bound"
            );
            count = count
                .checked_add(1)
                .context("session member count overflow")?;
        }
        Ok(count)
    }
}

impl Session {
    pub fn member_records(&self) -> Option<&Owner> {
        self.member_records.as_ref()
    }

    /// Only the actual active descriptor/selector0 branch of 801e7830. Other
    /// descriptors and selector inputs require their distinct source branch.
    pub fn active_member_record_count(&self) -> Result<u32> {
        ensure!(
            self.stage == Stage::Active && self.active == Some(self.session_id),
            "member count requires the actual active session descriptor"
        );
        let descriptor = self
            .descriptor
            .as_ref()
            .context("active descriptor owner absent")?;
        ensure!(
            descriptor.level_hashes == [self.request.level_hash(); 3],
            "active descriptor origin differs from this session"
        );
        self.member_snapshot(self.session_id, self.user_id)?;
        self.member_records
            .as_ref()
            .context("registered session record owner absent")?
            .count_members()
    }
}

#[cfg(test)]
mod tests;
