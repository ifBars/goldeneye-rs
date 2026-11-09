//! Sparse registration gates from explicit constructor-compatible ownership.
//! This is neither original global history nor the full config application.
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    ConstructorSnapshot,
    SuppliedSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub master_2a_raw: Option<u8>,
    pub requested_50_raw: Option<u8>,
    pub cached_51_raw: Option<u8>,
    pub game_175_raw: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistrationFlags {
    pub config_raw: u8,
    pub game_raw: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyContext {
    /// The actual leading provider lookup returned NULL. No writes are reached.
    Absent,
    /// The adapter actually completed earlier operations in 800d33c0 and
    /// reached 800d3954. This enum cannot stand in for those operations.
    MembershipStageReached,
}

/// Actual source users are resolved in storage order. Their retained-reference
/// lifecycle and actor identity belong to the provider, not a fabricated list.
pub trait ActorProvider {
    type Actor: Copy;
    /// Source signed count is reloaded at every loop test, including termination.
    fn actor_count(&mut self) -> Result<i32>;
    fn actor_at(&mut self, ordinal: usize) -> Result<Option<Self::Actor>>;
    fn membership(&mut self, actor: Self::Actor) -> Result<u32>;
    /// Must invoke the owned world's real membership operation. Errors preserve
    /// reached provider writes; this module never rolls them back.
    fn update_membership(&mut self, actor: Self::Actor, desired: u32) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub actor_visits: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyStage {
    Unapplied,
    ProviderAbsent,
    Unchanged,
    Updating { ordinal: usize },
    Failed { ordinal: usize },
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Receipt {
    pub stage: ApplyStage,
    pub user_visits: usize,
    pub membership_updates: usize,
    pub cached_store: bool,
}

#[derive(Debug)]
pub struct Owner {
    fields: Snapshot,
    origin: Origin,
    stage: ApplyStage,
}

impl Owner {
    pub fn from_constructor() -> Self {
        Self {
            fields: Snapshot {
                master_2a_raw: Some(1),
                requested_50_raw: Some(0),
                cached_51_raw: Some(1),
                game_175_raw: Some(1),
            },
            origin: Origin::ConstructorSnapshot,
            stage: ApplyStage::Unapplied,
        }
    }
    /// Explicit retained input, including unknown fields. No inferred history.
    pub fn from_snapshot(fields: Snapshot) -> Self {
        Self {
            fields,
            origin: Origin::SuppliedSnapshot,
            stage: ApplyStage::Unapplied,
        }
    }
    pub fn origin(&self) -> Origin {
        self.origin
    }
    pub fn snapshot(&self) -> Snapshot {
        self.fields
    }
    pub fn stage(&self) -> ApplyStage {
        self.stage
    }
    pub fn current_flags(&self) -> Result<RegistrationFlags> {
        Ok(RegistrationFlags {
            config_raw: self
                .fields
                .cached_51_raw
                .context("config registration gate is unknown")?,
            game_raw: self
                .fields
                .game_175_raw
                .context("game registration gate is unknown")?,
        })
    }
    /// Only the two registration-field writes in 800d3260. This does not
    /// execute its resource104 cleanup or claim completion of the full reset.
    pub fn reset_registration_fields(&mut self) {
        self.fields.cached_51_raw = Some(1);
        self.fields.requested_50_raw = Some(0);
        self.stage = ApplyStage::Unapplied;
    }
    /// Focused 800d3954..3a5c substage. Full application also owns earlier
    /// viewport/resource/scalar operations and later writes, outside this API.
    pub fn apply_registration_stage<P: ActorProvider>(
        &mut self,
        context: ApplyContext,
        provider: &mut P,
        budget: Budget,
    ) -> Result<Receipt> {
        if context == ApplyContext::Absent {
            self.stage = ApplyStage::ProviderAbsent;
            return Ok(Receipt {
                stage: self.stage,
                user_visits: 0,
                membership_updates: 0,
                cached_store: false,
            });
        }
        let master = self
            .fields
            .master_2a_raw
            .context("config master field is unknown")?;
        let requested = self
            .fields
            .requested_50_raw
            .context("config request field is unknown")?;
        let cached = self
            .fields
            .cached_51_raw
            .context("config cached field is unknown")?;
        let next = master & requested;
        if cached == next {
            self.stage = ApplyStage::Unchanged;
            return Ok(Receipt {
                stage: self.stage,
                user_visits: 0,
                membership_updates: 0,
                cached_store: false,
            });
        }
        let mut ordinal = 0usize;
        let mut updates = 0usize;
        let result = (|| -> Result<()> {
            loop {
                self.stage = ApplyStage::Updating { ordinal };
                let count = provider.actor_count()?;
                if i64::try_from(ordinal)? >= i64::from(count) {
                    break;
                }
                ensure!(
                    ordinal < budget.actor_visits,
                    "config actor visit budget exhausted"
                );
                if let Some(actor) = provider.actor_at(ordinal)? {
                    let old = provider.membership(actor)?;
                    let desired = if next != 0 { old | 8 } else { old & !8 };
                    provider.update_membership(actor, desired)?;
                    updates += 1;
                }
                ordinal += 1;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.stage = ApplyStage::Failed { ordinal };
            return Err(error);
        }
        // No cached gate mutation precedes any reached actor callback.
        self.fields.cached_51_raw = Some(next);
        self.stage = ApplyStage::Committed;
        Ok(Receipt {
            stage: self.stage,
            user_visits: ordinal,
            membership_updates: updates,
            cached_store: true,
        })
    }
    /// Source-safe canonical sparse words, not a native object image: origin,
    /// each field's known tag/value, then focused substage tag/optional ordinal.
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(match self.origin {
            Origin::ConstructorSnapshot => 0,
            Origin::SuppliedSnapshot => 1,
        });
        for field in [
            self.fields.master_2a_raw,
            self.fields.requested_50_raw,
            self.fields.cached_51_raw,
            self.fields.game_175_raw,
        ] {
            emit(u32::from(field.is_some()));
            if let Some(value) = field {
                emit(u32::from(value));
            }
        }
        let (tag, ordinal) = match self.stage {
            ApplyStage::Unapplied => (0, None),
            ApplyStage::ProviderAbsent => (1, None),
            ApplyStage::Unchanged => (2, None),
            ApplyStage::Updating { ordinal } => (3, Some(ordinal)),
            ApplyStage::Failed { ordinal } => (4, Some(ordinal)),
            ApplyStage::Committed => (5, None),
        };
        emit(tag);
        if let Some(ordinal) = ordinal {
            let ordinal = ordinal as u64;
            emit((ordinal >> 32) as u32);
            emit(ordinal as u32);
        }
    }
}

#[cfg(test)]
mod tests;
