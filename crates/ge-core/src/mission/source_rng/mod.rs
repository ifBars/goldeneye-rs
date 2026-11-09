//! Source integer RNG with explicit snapshot origin. A fresh constructor seed
//! does not establish the original stream's history at a later gameplay stage.
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Maximum successful draws since this owner snapshot was established.
    pub draws: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    FreshConstructor,
    SuppliedSnapshot,
}

#[derive(Debug, Clone)]
pub struct SourceRngOwner {
    state: u32,
    draws: u64,
    origin: Origin,
}

impl SourceRngOwner {
    pub fn from_constructor() -> Self {
        Self {
            state: 0x955,
            draws: 0,
            origin: Origin::FreshConstructor,
        }
    }

    pub fn from_snapshot(state: u32) -> Self {
        Self {
            state,
            draws: 0,
            origin: Origin::SuppliedSnapshot,
        }
    }

    pub fn draw_u32(&mut self, budget: Budget) -> Result<u32> {
        ensure!(
            self.draws < budget.draws,
            "source RNG draw budget exhausted"
        );
        let first = step(self.state);
        let second = step(first);
        self.state = second;
        // The preceding strict bound also prevents counter overflow at MAX.
        self.draws += 1;
        Ok((first & 0xffff_0000) | (second >> 16))
    }

    /// Internal owner/probe state; not a public progress serialization format.
    pub fn state_raw(&self) -> u32 {
        self.state
    }
    pub fn draws_since_snapshot(&self) -> u64 {
        self.draws
    }
    pub fn origin(&self) -> Origin {
        self.origin
    }
}

fn step(state: u32) -> u32 {
    state.wrapping_mul(0x343fd).wrapping_add(0x269ec3)
}

#[cfg(test)]
#[path = "tests/owner.rs"]
mod tests;
