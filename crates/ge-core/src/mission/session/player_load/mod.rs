//! Same-worker player creation admission from retained Session and Registry.
//! This initiates actual host prefixes; it never publishes an incomplete player.
//! The admitted Managers.user is the narrow host-owned per-user association.
//! This does not replay the original heterogeneous registry's RTTI traversal.
mod operations;
mod types;
pub use operations::{
    Operation, OperationBudget, OperationId, OperationKind, OperationOwner, OperationState,
};
pub use types::{PlayerAttempt, PlayerAttemptOutcome, PlayerDependency, PlayerGate, PlayerRequest};

use super::{LoadBinding, Session, Stage};
use crate::mission::{spawn, triggers::Registry};
use anyhow::{Context, Result, ensure};

impl Session {
    fn require_player_binding(&self, binding: &LoadBinding) -> Result<()> {
        ensure!(
            self.matches_load_binding(binding)
                && binding.session() == self.session_id
                && binding.user() == self.user_id
                && binding.level_hash() == self.request.level_hash(),
            "player continuation has a stale or foreign loader binding"
        );
        ensure!(
            self.stage == Stage::Active && self.active == Some(self.session_id),
            "player continuation requires the admitted active session"
        );
        if let Some(state) = self.readiness_load_snapshot() {
            ensure!(
                !state.delivery_failed,
                "player continuation retains a failed readiness owner"
            );
        }
        Ok(())
    }

    /// Source 801b6030 gates are derived from owned state. Catalog selection uses
    /// actual live TriggerIds and raw pose, not the inspection/prepared snapshot.
    /// The host must keep this Session, binding, Registry and PendingPlayer on
    /// the same worker. This method does not execute a whole ordinary update.
    pub fn begin_player_attempt(
        &mut self,
        binding: &LoadBinding,
        operations: &OperationOwner,
        registry: &Registry,
        budget: spawn::Budget,
    ) -> Result<PlayerAttemptOutcome> {
        self.require_player_binding(binding)?;
        let gated = if self.lifecycle_byte_41 != 0 {
            Some(PlayerGate::Session41)
        } else if self.lifecycle_byte_40 != 0 {
            Some(PlayerGate::Session40)
        } else if operations.contains_kind(OperationKind::TYPE_0B) {
            Some(PlayerGate::ExternalOperation0b)
        } else if self.request.level_hash() == 0x0108_0004 {
            Some(PlayerGate::FrontendLevel)
        } else if self.managers.is_none() {
            Some(PlayerGate::ManagerAbsent)
        } else {
            None
        };
        if let Some(reason) = gated {
            return Ok(PlayerAttemptOutcome::Gated(reason));
        }
        let managers = self
            .managers
            .as_ref()
            .context("admitted manager owner absent")?;
        if managers.readiness_flags_raw & 0x100 == 0 {
            return Ok(PlayerAttemptOutcome::Gated(PlayerGate::ManagerNotReady));
        }
        ensure!(
            managers.user.identity == self.user_id,
            "player user membership differs"
        );
        if managers.user.created_byte_raw != 0 {
            return Ok(PlayerAttemptOutcome::Gated(PlayerGate::UserAlreadyCreated));
        }
        if managers.user.flags_raw & 0x100 == 0 {
            return Ok(PlayerAttemptOutcome::Gated(PlayerGate::UserNotEligible));
        }
        let descriptor = self
            .descriptor
            .as_ref()
            .context("admitted descriptor absent")?;
        ensure!(
            self.mode == 3
                && self.backend == 2
                && descriptor.word_30_raw == 0
                && descriptor.local_user_count == 1
                && managers.local_user_count == 1,
            "player continuation supports only the admitted single-local-user branch"
        );
        if let Some(attempt) = self.player_load {
            ensure!(
                attempt.attempt() == binding.attempt(),
                "player attempt generation differs"
            );
            return Ok(PlayerAttemptOutcome::InProgress(attempt));
        }
        // 801b65b8 reads the matching metadata before PlayerNew allocation and
        // reset. Descriptor word30=0 above preserves this exact raw argument.
        // A present but unknown field stays None until its actual consumer.
        let source_flag_raw = self
            .member_snapshot(self.session_id, self.user_id)?
            .source_flag_raw();
        if !registry.catalog_ready() {
            return Ok(PlayerAttemptOutcome::Gated(PlayerGate::CatalogNotReady));
        }
        let selected = registry.select_start_point(self.request.start_point_id(), budget)?;
        let (Some(trigger), Some(pose)) = (selected.trigger, selected.pose) else {
            // Original source uses a zero pose. A checked missing selection is
            // explicit and cannot masquerade as a valid playable origin.
            return Ok(PlayerAttemptOutcome::Gated(PlayerGate::StartPointMissing));
        };
        let source = registry.trigger(trigger)?;
        let map = source.map();
        let attempt = PlayerAttempt {
            attempt: binding.attempt(),
            session: self.session_id,
            user: self.user_id,
            trigger,
            instance: map.instance,
            map_selector: map.map_selector,
            ordinal: source.ordinal(),
            selection: selected.selection,
            pose,
            dependency: PlayerDependency::OwnerInitialization,
            source_flag_raw,
        };
        self.player_load = Some(attempt);
        Ok(PlayerAttemptOutcome::Started(PlayerRequest(attempt)))
    }

    pub fn player_attempt(&self) -> Option<PlayerAttempt> {
        self.player_load
    }

    /// Retain an actual host-reached incomplete dependency. This cannot set the
    /// created byte, manufacture completion links, or authorize phase21.
    pub fn record_player_dependency(
        &mut self,
        binding: &LoadBinding,
        dependency: PlayerDependency,
    ) -> Result<()> {
        self.require_player_binding(binding)?;
        let attempt = self
            .player_load
            .as_mut()
            .context("player attempt has not started")?;
        ensure!(
            attempt.attempt() == binding.attempt(),
            "player attempt generation differs"
        );
        attempt.dependency = dependency;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
