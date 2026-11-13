//! Constructor-lived math defaults and a separate unresolved process input.
use super::super::Attempt;
use anyhow::{Context, Result};

pub(super) struct MathDefaults {
    pub reset_quad_bits: [u32; 4],
    pub reset_quad_reads: usize,
    pub ones_bits: [u32; 3],
    pub ones_reads: usize,
}
impl MathDefaults {
    pub fn from_constructor() -> Self {
        // The same pre-main constructor explicitly writes these distinct
        // objects. Neither comes from an implied BSS value or supplied quad.
        Self {
            reset_quad_bits: [0; 4],
            reset_quad_reads: 0,
            ones_bits: [1.0f32.to_bits(); 3],
            ones_reads: 0,
        }
    }
}

pub(super) struct Globals {
    pub weapon_byte: Option<u8>,
    pub weapon_reads: usize,
}
impl Globals {
    pub fn unavailable() -> Self {
        Self {
            weapon_byte: None,
            weapon_reads: 0,
        }
    }
}

impl Attempt {
    pub fn player_pose_ones_bits(&self) -> Result<[u32; 3]> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player math defaults lock is poisoned"))?;
        self.ensure_current()?;
        let owner = &mut state.player.math_defaults;
        owner.ones_reads = owner
            .ones_reads
            .checked_add(1)
            .context("pose ones read count overflow")?;
        Ok(owner.ones_bits)
    }

    pub fn player_pose_weapon_byte(&self) -> Result<Option<u8>> {
        let mut state = self
            .shared
            .0
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("player pose input lock is poisoned"))?;
        self.ensure_current()?;
        let owner = &mut state.player.pose_globals;
        owner.weapon_reads = owner
            .weapon_reads
            .checked_add(1)
            .context("pose global read count overflow")?;
        Ok(owner.weapon_byte)
    }
}

#[cfg(test)]
#[path = "tests/pose_inputs.rs"]
mod tests;
