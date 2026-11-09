//! Identity-bound local phase19 adaptation for the admitted fresh one-user path.
//! Actual phase7..18 owners and provider completion remain root's obligation;
//! an inspectable PhaseState alone is never proof those operations executed.
mod types;
pub use types::{
    Event26Receipt, FlagAction, FlagActionReceipt, LoadBinding, LoadReadinessSnapshot,
    LocalEvent26, Phase19Receipt,
};

use super::{FlagsDelta, Managers, PeerSnapshot, Session, Stage, receive_delta};
use crate::mission::loader::phase::{AttemptId, PhaseState};
use anyhow::{Context, Result, ensure};
use types::Identity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Owner {
    identity: Identity,
    peer: Option<PeerSnapshot>,
    local_flags: u32,
    invocations: usize,
    changed_events: usize,
    local_stores: usize,
    delivery_failed: bool,
}

impl Owner {
    fn apply(&mut self, managers: &mut Managers, action: FlagAction) -> FlagActionReceipt {
        let before = managers.readiness_flags_raw;
        let delta = match action {
            FlagAction::Clear(mask) => FlagsDelta {
                set: 0,
                clear: mask,
            },
            FlagAction::Set(mask) => FlagsDelta {
                set: mask,
                clear: 0,
            },
        };
        let after = delta.apply(before);
        managers.readiness_flags_raw = after;
        // Only the four fixed phase19 masks are supported. Generic source mask40
        // forced notification is deliberately not generalized by this helper.
        let event = (before != after).then(|| {
            let event = LocalEvent26 {
                session: self.identity.session,
                source: self.identity.user,
                delta,
            };
            let peer_updated = self
                .peer
                .as_ref()
                .is_some_and(|peer| peer.identity == event.source);
            let user_updated =
                receive_delta(event.source, self.peer.as_mut(), &mut managers.user, delta);
            Event26Receipt {
                event,
                peer_updated,
                user_updated,
            }
        });
        let local_before = self.local_flags;
        // Source compares mirror against the NEW owner, not against whether the
        // current OR/AND changes the mirror. Fresh bit1 causes all four stores.
        let local_store = local_before != after;
        if local_store {
            self.local_flags = delta.apply(local_before);
        }
        FlagActionReceipt {
            action,
            owner_before: before,
            owner_after: after,
            local_before,
            local_after: self.local_flags,
            local_store,
            event,
        }
    }
}

impl Session {
    pub(super) fn matches_load_binding(&self, binding: &LoadBinding) -> bool {
        self.readiness_load
            .as_ref()
            .is_some_and(|owner| owner.identity == binding.identity)
    }

    fn require_local_context(&self) -> Result<&Managers> {
        ensure!(
            self.stage == Stage::Active && self.active == Some(self.session_id),
            "loader readiness requires the exact active admitted session"
        );
        ensure!(
            self.mode == 3
                && self.backend == 2
                && self.lifecycle_byte_40 == 0
                && self.lifecycle_byte_41 == 0,
            "loader readiness requires the supported local mode and lifecycle"
        );
        let descriptor = self
            .descriptor
            .as_ref()
            .context("admitted descriptor absent")?;
        let managers = self
            .managers
            .as_ref()
            .context("admitted manager owner absent")?;
        ensure!(
            descriptor.word_2c_raw == 1
                && descriptor.local_user_count == 1
                && managers.local_user_count == 1
                && managers.user.identity == self.user_id,
            "loader readiness user membership or descriptor differs"
        );
        ensure!(
            self.request.level_hash() == 0x0181_016b
                && descriptor.level_hashes == [self.request.level_hash(); 3],
            "loader readiness level binding differs"
        );
        Ok(managers)
    }

    /// Bind exactly once after admission, before actual loading. Peer and mirror
    /// are real fresh host owners (0 and1); received deltas never fabricate them.
    /// No observer is owned in this bounded adaptation, so none is notified.
    pub fn bind_loader(&mut self, attempt: AttemptId) -> Result<LoadBinding> {
        let managers = self.require_local_context()?;
        ensure!(
            self.readiness_load.is_none(),
            "session already has a loader binding"
        );
        ensure!(
            managers.readiness_flags_raw == 0
                && managers.user.flags_raw == 0
                && managers.user.created_byte_raw == 0,
            "loader binding is not fresh admitted state"
        );
        let identity = Identity {
            session: self.session_id,
            user: self.user_id,
            attempt,
            level: self.request.level_hash(),
        };
        self.readiness_load = Some(Owner {
            identity,
            peer: Some(PeerSnapshot {
                identity: self.user_id,
                flags_raw: 0,
            }),
            local_flags: 1,
            invocations: 0,
            changed_events: 0,
            local_stores: 0,
            delivery_failed: false,
        });
        Ok(LoadBinding { identity })
    }

    /// Root calls this only after its actual owned catalog/phase18 operation and
    /// Wait28 provider promotion reached19 in this same worker/generation. A
    /// caller-created phase snapshot or evidence JSON cannot supply that proof.
    /// This method queues21; the app must still perform the actual Wait28 poll.
    /// It never creates a player, changes the created byte or produces a camera.
    pub fn finish_tutorial_phase19(
        &mut self,
        binding: &LoadBinding,
        phase: &mut PhaseState,
    ) -> Result<Phase19Receipt> {
        self.require_local_context()?;
        let expected = Identity {
            session: self.session_id,
            user: self.user_id,
            attempt: phase.attempt(),
            level: self.request.level_hash(),
        };
        ensure!(
            binding.identity == expected
                && phase.current() == 19
                && phase.pending().is_none()
                && !phase.is_cancelled(),
            "phase19 loader/session identity or phase differs"
        );
        let owner = self
            .readiness_load
            .as_mut()
            .context("loader peer/mirror owner is unbound")?;
        ensure!(
            owner.identity == binding.identity,
            "loader binding belongs to another peer owner"
        );
        ensure!(
            !owner.delivery_failed,
            "loader owner retains a failed event delivery"
        );
        // Reserve all bounded counters before the first actual source write.
        let invocations = owner
            .invocations
            .checked_add(1)
            .context("phase19 invocation count overflow")?;
        owner
            .changed_events
            .checked_add(4)
            .context("phase19 event count overflow")?;
        owner
            .local_stores
            .checked_add(4)
            .context("phase19 mirror count overflow")?;
        let managers = self
            .managers
            .as_mut()
            .context("admitted manager owner absent")?;
        let actions = [
            FlagAction::Clear(2),
            FlagAction::Set(0x100),
            FlagAction::Set(8),
            FlagAction::Clear(0x400),
        ]
        .map(|action| owner.apply(managers, action));
        let changed_events = actions
            .iter()
            .filter(|action| action.event.is_some())
            .count();
        let peer_updates = actions
            .iter()
            .filter_map(|action| action.event)
            .filter(|event| event.peer_updated)
            .count();
        let user_updates = actions
            .iter()
            .filter_map(|action| action.event)
            .filter(|event| event.user_updated)
            .count();
        let local_mirror_stores = actions.iter().filter(|action| action.local_store).count();
        owner.invocations = invocations;
        owner.changed_events += changed_events;
        owner.local_stores += local_mirror_stores;
        // An unknown received peer does not become a new matching peer. Keep all
        // reached source writes and latch incompletion; never empty-ready retry.
        owner.delivery_failed = peer_updates != changed_events || user_updates != changed_events;
        ensure!(
            !owner.delivery_failed,
            "phase19 event did not reach its matching admitted peer/user"
        );
        let receipt = Phase19Receipt {
            actions,
            changed_events,
            peer_updates,
            user_updates,
            local_mirror_stores,
            local_observer_callbacks: 0,
            user_observer_callbacks: 0,
            owner_flags_raw: managers.readiness_flags_raw,
            peer_flags_raw: owner
                .peer
                .as_ref()
                .context("phase19 peer absent")?
                .flags_raw,
            user_flags_raw: managers.user.flags_raw,
            local_mirror_flags_raw: owner.local_flags,
            created_byte_raw: managers.user.created_byte_raw,
            native_timing_verified: false,
        };
        phase.queue(binding.identity.attempt, 21)?;
        Ok(receipt)
    }

    pub fn readiness_load_snapshot(&self) -> Option<LoadReadinessSnapshot> {
        let owner = self.readiness_load.as_ref()?;
        let managers = self.managers.as_ref()?;
        Some(LoadReadinessSnapshot {
            owner_flags_raw: managers.readiness_flags_raw,
            peer_flags_raw: owner.peer.as_ref().map(|peer| peer.flags_raw),
            user_flags_raw: managers.user.flags_raw,
            local_mirror_flags_raw: owner.local_flags,
            created_byte_raw: managers.user.created_byte_raw,
            invocations: owner.invocations,
            changed_events: owner.changed_events,
            local_mirror_stores: owner.local_stores,
            delivery_failed: owner.delivery_failed,
        })
    }
}

#[cfg(test)]
#[path = "tests/load.rs"]
mod tests;
