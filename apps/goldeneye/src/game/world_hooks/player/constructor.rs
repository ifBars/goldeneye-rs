//! Controller-lifetime constructor storage. A partial reset never authorizes
//! native destruction; interrupted leases retain their reached count/backings.
use super::super::Attempt;
use anyhow::{Result, ensure};
use ge_core::mission::player::{constructor, identity::PlayerId};
use sha2::{Digest, Sha256};

pub(in crate::game) struct Snapshot {
    pub count: usize,
    pub live_leases: usize,
    pub allocations: usize,
    pub native_bytes: usize,
    pub host_capacity_bytes: usize,
    pub reset_calls: usize,
    pub entry_noops: usize,
    pub cache_stores: usize,
    pub reset_complete: bool,
    pub state_sha256: String,
}

impl Attempt {
    pub fn begin_player_constructor(&self, player: PlayerId) -> Result<constructor::Lease> {
        self.ensure_current()?;
        ensure!(
            player.get() == self.identity,
            "player constructor generation differs"
        );
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player constructor owner lock is poisoned"))?;
        self.ensure_current()?;
        let owner = &mut state.player.constructor;
        let lease = owner.begin(
            player,
            constructor::Budget {
                native_bytes: 63_680,
                leases: 128,
            },
        )?;
        owner.begin_reset(lease)?;
        self.ensure_current()?;
        Ok(lease)
    }

    pub fn player_constructor_snapshot(&self, lease: constructor::Lease) -> Result<Snapshot> {
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player constructor owner lock is poisoned"))?;
        self.ensure_current()?;
        ensure!(
            lease.player().get() == self.identity,
            "player constructor generation differs"
        );
        let owner = &state.player.constructor;
        let reset_stage = owner.reset_stage(lease)?;
        ensure!(
            matches!(
                reset_stage,
                constructor::ResetStage::InProgress | constructor::ResetStage::Complete
            ),
            "player constructor has no retained reset continuation"
        );
        let mut hash = Sha256::new();
        hash.update(b"ge_player_shared_constructor_v1\0");
        owner.visit_state_words(|word| hash.update(word.to_be_bytes()));
        Ok(Snapshot {
            count: owner.count() as usize,
            live_leases: owner.live_leases(),
            allocations: [
                constructor::Kind::Raw,
                constructor::Kind::Array34,
                constructor::Kind::Array64,
            ]
            .into_iter()
            .filter(|&kind| owner.allocation(kind).is_some())
            .count(),
            native_bytes: owner.retained_native_bytes(),
            host_capacity_bytes: owner.retained_host_capacity_bytes(),
            reset_calls: usize::try_from(owner.reset_calls())?,
            entry_noops: usize::try_from(owner.entry_noops())?,
            cache_stores: usize::try_from(owner.cache_stores())?,
            reset_complete: reset_stage == constructor::ResetStage::Complete,
            state_sha256: format!("{:x}", hash.finalize()),
        })
    }

    pub fn complete_player_reset(
        &self,
        lease: constructor::Lease,
        reset: &constructor::PendingReset,
    ) -> Result<()> {
        let completion = reset.completion()?;
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player constructor owner lock is poisoned"))?;
        self.ensure_current()?;
        ensure!(
            lease.player().get() == self.identity,
            "player constructor generation differs"
        );
        state
            .player
            .constructor
            .complete_pending_reset(lease, &completion)?;
        self.ensure_current()?;
        Ok(())
    }

    pub fn player_reset_quad_bits(&self) -> Result<[u32; 4]> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player reset defaults owner lock is poisoned"))?;
        self.ensure_current()?;
        let reads = state
            .player
            .reset_quad_reads
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("player reset defaults read count overflow"))?;
        state.player.reset_quad_reads = reads;
        Ok(state.player.reset_quad_bits)
    }

    pub fn reset_player_runtime_float(&self, bits: u32) -> Result<()> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player runtime owner lock is poisoned"))?;
        self.ensure_current()?;
        let writes = state
            .player
            .runtime_float_writes
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("player runtime float write count overflow"))?;
        state.player.runtime_float = Some(bits);
        state.player.runtime_float_writes = writes;
        Ok(())
    }

    pub fn set_player_multiplayer_byte(&self, value: u8) -> Result<()> {
        ensure!(value <= 1, "player multiplayer byte is not boolean");
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player runtime owner lock is poisoned"))?;
        self.ensure_current()?;
        let writes = state
            .player
            .multiplayer_writes
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("player multiplayer write count overflow"))?;
        state.player.multiplayer_byte = Some(value);
        state.player.multiplayer_writes = writes;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/constructor.rs"]
mod tests;
