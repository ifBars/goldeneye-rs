//! Owned constructor state for a fresh source StartPoint base animator.
//!
//! The fresh trigger owns no player. Source event 0x100 is handled but denied
//! before any state/program read; event 0xff must remain unmapped here. Later
//! allowed create events require synchronous execution, which is unsupported.
use anyhow::{Context, Result, ensure};
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub statement_count_raw: u16,
    pub variables: u8,
    pub shared_variables: u8,
    pub states: u8,
    pub format: u8,
    pub event_ff: u8,
    pub event_create: u8,
    pub event_105: u8,
    pub event_10e: u8,
}

/// Known absence is a lifecycle input; it is never inferred from missing assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraContext {
    KnownMissing,
    Bound(NonZeroU64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshContext {
    /// A host-owned TriggerId, not an original pointer or an animator index.
    pub trigger_handle: u64,
    pub map_selector: i8,
    pub position_bits: [u32; 4],
    pub manager_present: bool,
    pub camera: CameraContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Bound on each source variable-count byte. The native extra scratch
    /// allocation includes one additional word when variables > shared.
    pub variables: usize,
    pub states: usize,
}

/// Supply this from an actual owned handler binding, not a requested-ready flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerBinding {
    Unbound,
    PlayerBasic(NonZeroU64),
    Other(NonZeroU64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    Denied,
    Unmapped,
    UnsupportedImmediateExecution { state: u8 },
}

/// Only fields assigned by the supported constructor path. Unassigned native
/// padding/neighbor bytes are deliberately not represented as known zeros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceState {
    pub unique_states_raw: u8,
    pub execution_budget_raw: u32,
    pub callback_flags_raw: u8,
    pub selected_marker_raw: u8,
    pub secondary_marker_raw: u8,
    pub active_executions: usize,
    pub owner: OwnerBinding,
    pub constructor_create: EventOutcome,
    pub registration_event: EventOutcome,
}

#[derive(Debug, Clone)]
pub struct Runtime {
    header: Header,
    context: FreshContext,
    shared: Vec<u32>,
    // Source allocates but does not initialize these words. None records an
    // unwritten word, not a missing or null original pointer.
    extra: Option<Vec<Option<u32>>>,
    state_flags: Vec<u8>,
    executions: [Option<NonZeroU64>; 16],
    source: SourceState,
}

impl Runtime {
    pub fn fresh_startpoint(header: Header, context: FreshContext, budget: Budget) -> Result<Self> {
        ensure!(
            context.trigger_handle != 0,
            "animator trigger identity must be nonzero"
        );
        ensure!(
            context.manager_present,
            "source base animator manager absent"
        );
        ensure!(header.format == 0x38, "unsupported base animator format");
        let variables = usize::from(header.variables);
        let shared = usize::from(header.shared_variables);
        let states = usize::from(header.states);
        ensure!(
            variables <= budget.variables && shared <= budget.variables,
            "base animator variable budget exhausted"
        );
        ensure!(
            states <= budget.states,
            "base animator state budget exhausted"
        );
        // Owner absence comes from this fresh trigger constructor, so the
        // create callback denies before looking up its state/program. The
        // following registration event must not queue unsupported work.
        ensure!(
            header.event_ff >= header.states,
            "registration event requires unsupported queued execution"
        );
        let mut shared_words = Vec::new();
        shared_words
            .try_reserve_exact(shared)
            .context("owning animator shared variables")?;
        shared_words.resize(shared, 0);
        let extra = if variables > shared {
            let count = variables - shared + 1;
            let mut words = Vec::new();
            words
                .try_reserve_exact(count)
                .context("owning animator unwritten scratch")?;
            words.resize(count, None);
            Some(words)
        } else {
            None
        };
        let mut state_flags = Vec::new();
        state_flags
            .try_reserve_exact(states)
            .context("owning animator state flags")?;
        state_flags.resize(states, 0);
        let callback_flags_raw = u8::from(header.event_105 < header.states)
            | (u8::from(header.event_10e < header.states) << 1);
        Ok(Self {
            header,
            context,
            shared: shared_words,
            extra,
            state_flags,
            executions: [None; 16],
            source: SourceState {
                unique_states_raw: 1,
                execution_budget_raw: 5,
                callback_flags_raw,
                selected_marker_raw: 0xff,
                secondary_marker_raw: 0xff,
                active_executions: 0,
                owner: OwnerBinding::Unbound,
                constructor_create: EventOutcome::Denied,
                registration_event: EventOutcome::Unmapped,
            },
        })
    }

    pub fn header(&self) -> Header {
        self.header
    }

    pub fn context(&self) -> FreshContext {
        self.context
    }

    pub fn source_state(&self) -> SourceState {
        self.source
    }

    pub fn shared_variables(&self) -> &[u32] {
        &self.shared
    }

    pub fn extra_scratch(&self) -> Option<&[Option<u32>]> {
        self.extra.as_deref()
    }

    pub fn state_flags(&self) -> &[u8] {
        &self.state_flags
    }

    pub fn execution_slots(&self) -> &[Option<NonZeroU64>; 16] {
        &self.executions
    }

    /// Evaluate a later source create callback without attaching an owner or
    /// claiming the immediate program ran. Unsupported results never mutate
    /// this initialized object or allocate fake queued execution nodes.
    pub fn evaluate_create(
        &self,
        owner: OwnerBinding,
        trigger_id: u32,
        requested_id: u32,
    ) -> EventOutcome {
        if !matches!(owner, OwnerBinding::PlayerBasic(_)) || trigger_id != requested_id {
            return EventOutcome::Denied;
        }
        if self.header.event_create >= self.header.states {
            EventOutcome::Unmapped
        } else {
            EventOutcome::UnsupportedImmediateExecution {
                state: self.header.event_create,
            }
        }
    }
}

#[cfg(test)]
mod tests;
