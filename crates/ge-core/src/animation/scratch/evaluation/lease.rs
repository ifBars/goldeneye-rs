//! Cloneable cancellation handle for one retained evaluation scope. It owns the
//! live reservation so a host can reap a stale attempt without its Pending
//! owner. Lock order: callers may hold their own state lock, then this lease;
//! lease operations never acquire any outer lock.
use super::super::{Arena, Context, Reservation};
use anyhow::{Context as _, Result, anyhow, ensure};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

#[derive(Debug)]
struct Scope {
    reservation: Option<Reservation>,
    aborted: bool,
}
#[derive(Debug, Clone)]
pub struct CancellationLease {
    owner: u64,
    arena_identity: Weak<()>,
    context_identity: Weak<()>,
    ordinal: u64,
    allocation: u64,
    scope: Arc<Mutex<Scope>>,
}
impl CancellationLease {
    pub(super) fn new(
        arena: &Arena,
        context: &Context,
        ordinal: u64,
        reservation: Reservation,
    ) -> Self {
        Self {
            owner: arena.owner,
            arena_identity: Arc::downgrade(&arena.identity),
            context_identity: Arc::downgrade(&context.identity),
            ordinal,
            allocation: reservation.ordinal,
            scope: Arc::new(Mutex::new(Scope {
                reservation: Some(reservation),
                aborted: false,
            })),
        }
    }
    fn lock(&self) -> Result<MutexGuard<'_, Scope>> {
        self.scope
            .lock()
            .map_err(|_| anyhow!("animation evaluation lease lock is poisoned"))
    }
    /// Arena identity that owns the reservation.
    pub fn owner(&self) -> u64 {
        self.owner
    }
    /// Context holder ordinal of the evaluator invocation.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub fn allocation_ordinal(&self) -> u64 {
        self.allocation
    }
    pub fn is_aborted(&self) -> Result<bool> {
        Ok(self.lock()?.aborted)
    }
    /// The reservation is still held: neither returned nor aborted.
    pub fn is_live(&self) -> Result<bool> {
        Ok(self.lock()?.reservation.is_some())
    }
    pub fn same_scope(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.scope, &other.scope)
    }
    /// Explicit host cancellation. Releases only this lease's own scope and
    /// clears only its own context holder. Ok(false) means nothing was held
    /// (already aborted or consumed by a normal return). On Err nothing changed.
    pub fn abort(&self, context: &mut Context, arena: &mut Arena) -> Result<bool> {
        let mut scope = self.lock()?;
        if scope.aborted || scope.reservation.is_none() {
            return Ok(false);
        }
        self.ensure_owner(context, arena)?;
        let reservation = scope.reservation.as_mut().expect("held reservation");
        arena.release(reservation)?;
        scope.reservation = None;
        scope.aborted = true;
        context.holder = None;
        Ok(true)
    }
    /// Normal evaluator return consumes the reservation through the same owner.
    pub(super) fn consume(&self, context: &mut Context, arena: &mut Arena) -> Result<()> {
        let mut scope = self.lock()?;
        ensure!(!scope.aborted, "animation evaluation scope was cancelled");
        self.ensure_owner(context, arena)?;
        let reservation = scope
            .reservation
            .as_mut()
            .context("animation evaluation scope already released")?;
        arena.release(reservation)?;
        scope.reservation = None;
        context.holder = None;
        Ok(())
    }
    fn ensure_owner(&self, context: &Context, arena: &Arena) -> Result<()> {
        ensure!(
            self.arena_identity
                .upgrade()
                .is_some_and(|identity| Arc::ptr_eq(&identity, &arena.identity))
                && self
                    .context_identity
                    .upgrade()
                    .is_some_and(|identity| Arc::ptr_eq(&identity, &context.identity))
                && arena.owner == self.owner
                && context.holder == Some(self.ordinal),
            "evaluation lease does not own this shared scratch scope"
        );
        Ok(())
    }
}
