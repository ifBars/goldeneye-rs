//! Persistent player-constructor inputs. The operation queue is distinct from
//! admission and loader work; reached reset writes survive cancellation.
mod actors;
mod constructor;
mod pose_inputs;
mod scratch;
mod transform_caches;
use super::Attempt;
pub(in crate::game) use actors::Actors;
use anyhow::{Result, ensure};
use ge_core::mission::{
    player::{config, constructor as player_constructor},
    session::{
        LoadBinding, Session,
        player_load::{OperationBudget, OperationOwner, PlayerAttemptOutcome},
    },
    triggers::Registry,
};

pub(super) struct Persistent {
    operations: OperationOwner,
    config: config::Owner,
    game_field_328_bits: u32,
    reset_byte: Option<u8>,
    reset_writes: usize,
    constructor: player_constructor::Shared,
    runtime_float: Option<u32>,
    runtime_float_writes: usize,
    multiplayer_byte: Option<u8>,
    multiplayer_writes: usize,
    scratch: player_constructor::Scratch,
    math_defaults: pose_inputs::MathDefaults,
    pose_globals: pose_inputs::Globals,
    transform_caches: ge_core::mission::player::transform_cache::Owner,
    game_item: Option<ge_core::mission::player::identity::ItemId>,
}
impl Persistent {
    pub fn from_constructor() -> Self {
        Self {
            operations: OperationOwner::fresh(OperationBudget { entries: 128 })
                .expect("bounded operation constructor is infallible"),
            config: config::Owner::from_constructor(),
            game_field_328_bits: 0x4244_0000,
            reset_byte: None,
            reset_writes: 0,
            constructor: player_constructor::Shared::from_boot(),
            runtime_float: None,
            runtime_float_writes: 0,
            multiplayer_byte: None,
            multiplayer_writes: 0,
            scratch: player_constructor::Scratch::from_constructor(),
            math_defaults: pose_inputs::MathDefaults::from_constructor(),
            pose_globals: pose_inputs::Globals::unavailable(),
            transform_caches: ge_core::mission::player::transform_cache::Owner::from_boot(),
            game_item: None,
        }
    }
}
impl Attempt {
    pub fn begin_player_attempt(
        &self,
        session: &mut Session,
        binding: &LoadBinding,
        registry: &Registry,
    ) -> Result<(PlayerAttemptOutcome, usize)> {
        self.ensure_current()?;
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player input owner lock is poisoned"))?;
        self.ensure_current()?;
        let result = session.begin_player_attempt(
            binding,
            &state.player.operations,
            registry,
            ge_core::mission::spawn::Budget {
                entries: 65_536,
                visits: 131_072,
            },
        )?;
        Ok((result, state.player.operations.entries().len()))
    }
    pub fn player_reset_input(&self) -> Result<u32> {
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player input owner lock is poisoned"))?;
        self.ensure_current()?;
        Ok(state.player.game_field_328_bits)
    }
    pub fn player_base_reset_writes(&self) -> Result<usize> {
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player input owner lock is poisoned"))?;
        self.ensure_current()?;
        Ok(state.player.reset_writes)
    }
    pub fn reset_player_base_byte(&self) -> Result<()> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player input owner lock is poisoned"))?;
        self.ensure_current()?;
        let writes = state
            .player
            .reset_writes
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("player reset write count overflow"))?;
        state.player.reset_byte = Some(0);
        state.player.reset_writes = writes;
        Ok(())
    }
    pub fn player_inputs_fingerprint(&self) -> Result<String> {
        use sha2::{Digest, Sha256};
        let state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player input owner lock is poisoned"))?;
        self.ensure_current()?;
        let mut hash = Sha256::new();
        hash.update(b"ge_player_persistent_inputs_v5\0");
        hash.update(state.player.game_field_328_bits.to_be_bytes());
        hash.update([u8::from(state.player.reset_byte.is_some())]);
        if let Some(byte) = state.player.reset_byte {
            hash.update([byte]);
        }
        hash.update((state.player.reset_writes as u64).to_be_bytes());
        for word in state.player.math_defaults.reset_quad_bits {
            hash.update(word.to_be_bytes());
        }
        hash.update((state.player.math_defaults.reset_quad_reads as u64).to_be_bytes());
        for word in state.player.math_defaults.ones_bits {
            hash.update(word.to_be_bytes());
        }
        hash.update((state.player.math_defaults.ones_reads as u64).to_be_bytes());
        hash.update([u8::from(state.player.pose_globals.weapon_byte.is_some())]);
        if let Some(byte) = state.player.pose_globals.weapon_byte {
            hash.update([byte]);
        }
        hash.update((state.player.pose_globals.weapon_reads as u64).to_be_bytes());
        hash.update([u8::from(state.player.game_item.is_some())]);
        if let Some(item) = state.player.game_item {
            hash.update(item.world().get().to_be_bytes());
            hash.update(item.get().to_be_bytes());
        }
        for value in [
            state.player.runtime_float,
            state.player.multiplayer_byte.map(u32::from),
        ] {
            hash.update([u8::from(value.is_some())]);
            if let Some(value) = value {
                hash.update(value.to_be_bytes());
            }
        }
        for writes in [
            state.player.runtime_float_writes,
            state.player.multiplayer_writes,
        ] {
            hash.update((writes as u64).to_be_bytes());
        }
        state
            .player
            .config
            .visit_state_words(|word| hash.update(word.to_be_bytes()));
        for entry in state.player.operations.entries() {
            for word in [entry.id().get(), entry.kind().raw(), entry.state().raw()] {
                hash.update(word.to_be_bytes());
            }
        }
        ensure!(
            state.player.reset_byte == Some(0),
            "player base reset store has not executed"
        );
        Ok(format!("{:x}", hash.finalize()))
    }
}
