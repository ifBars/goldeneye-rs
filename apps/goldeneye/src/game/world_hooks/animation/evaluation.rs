//! State-lock-owned cancellation bookkeeping. No I/O or window-thread cleanup
//! runs here; the next scratch consumer reaps a retired attempt's exact lease.
use super::super::{Shared, State};
use anyhow::{Result, ensure};
use ge_core::animation::scratch::evaluation::CancellationLease;

pub(in crate::game::world_hooks) struct PendingScope {
    attempt: u64,
    lease: CancellationLease,
}
impl State {
    pub(in crate::game::world_hooks) fn reap_animation_scope(
        &mut self,
        current: Option<u64>,
    ) -> Result<()> {
        let Some(scope) = &self.animation_pending_evaluation else {
            return Ok(());
        };
        if !scope.lease.is_live()? {
            self.animation_pending_evaluation = None;
        } else if current != Some(scope.attempt) {
            scope
                .lease
                .abort(&mut self.animation_context, &mut self.animation_scratch)?;
            self.animation_pending_evaluation = None;
        }
        Ok(())
    }
    pub(in crate::game::world_hooks) fn retain_animation_scope(
        &mut self,
        attempt: u64,
        lease: Option<CancellationLease>,
    ) -> Result<()> {
        if let Some(lease) = lease {
            if lease.is_live()? {
                if let Some(previous) = &self.animation_pending_evaluation {
                    ensure!(
                        previous.attempt == attempt && previous.lease.same_scope(&lease),
                        "another evaluation owns shared scratch"
                    );
                }
                self.animation_pending_evaluation = Some(PendingScope { attempt, lease });
            } else {
                self.reap_animation_scope(Some(attempt))?;
            }
        }
        Ok(())
    }
}
impl Shared {
    /// Called after all workers join at explicit shutdown, never by the native
    /// cancellation callback. Cleanup is not a source evaluator/bind return.
    pub(in crate::game) fn abort_pending_evaluation(&self) -> Result<()> {
        self.0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("evaluation cleanup state lock poisoned"))?
            .reap_animation_scope(None)
    }
}

#[cfg(test)]
mod tests;
