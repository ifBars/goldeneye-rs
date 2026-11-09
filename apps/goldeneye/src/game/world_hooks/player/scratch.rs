use super::super::Attempt;
use anyhow::{Result, ensure};
use ge_core::mission::player::constructor::{
    DerivedResetStage, Lease, PendingReset, ResetStage, ScratchBudget,
};
use sha2::{Digest, Sha256};

pub(in crate::game) struct Snapshot {
    pub capacity: usize,
    pub growth: usize,
    pub count: usize,
    pub native_bytes: usize,
    pub host_capacity_bytes: usize,
    pub reallocations: usize,
    pub state_sha256: String,
}
impl Attempt {
    pub fn prepare_player_scratch(&self, lease: Lease, reset: &mut PendingReset) -> Result<()> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player scratch owner lock is poisoned"))?;
        self.ensure_current()?;
        ensure!(
            lease.player().get() == self.identity && reset.player() == lease.player(),
            "player scratch generation differs"
        );
        ensure!(
            reset.stage() == DerivedResetStage::AwaitingScratch
                && state.player.constructor.reset_stage(lease)? == ResetStage::InProgress,
            "player scratch continuation is out of source order"
        );
        let receipt = state.player.scratch.prepare_reset(ScratchBudget {
            native_bytes: 32 * 0x94,
            growth_steps: 32,
        })?;
        reset.after_scratch(&receipt)?;
        self.ensure_current()?;
        Ok(())
    }
    pub fn player_scratch_snapshot(&self) -> Result<Snapshot> {
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player scratch owner lock is poisoned"))?;
        self.ensure_current()?;
        let scratch = &state.player.scratch;
        let mut hash = Sha256::new();
        hash.update(b"ge_player_shared_scratch_v1\0");
        scratch.visit_state_words(|word| hash.update(word.to_be_bytes()));
        Ok(Snapshot {
            capacity: scratch.capacity() as usize,
            growth: scratch.growth() as usize,
            count: scratch.count() as usize,
            native_bytes: scratch.backing_len(),
            host_capacity_bytes: scratch.host_capacity_bytes(),
            reallocations: usize::try_from(scratch.realloc_calls())?,
            state_sha256: format!("{:x}", hash.finalize()),
        })
    }
    pub fn player_multiplayer_byte(&self) -> Result<u8> {
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player reset owner lock is poisoned"))?;
        self.ensure_current()?;
        state
            .player
            .multiplayer_byte
            .ok_or_else(|| anyhow::anyhow!("player runtime multiplayer write has not executed"))
    }
}

#[cfg(test)]
#[path = "tests/scratch.rs"]
mod tests;
