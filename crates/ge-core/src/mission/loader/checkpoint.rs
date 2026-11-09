//! Retained checkpoint wrapper. Fresh payload allocation is deliberately
//! uninitialized; flags do not fabricate capture, restore admission or callbacks.
use anyhow::{Result, ensure};
use std::{fmt, mem::MaybeUninit};

pub const PAYLOAD_BYTES: usize = 0x37f8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub payload_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    FreshConstructor,
    SuppliedSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkipNotRestoring;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedRestore;

impl fmt::Display for UnsupportedRestore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("active checkpoint requires actual restore admission and callback providers")
    }
}
impl std::error::Error for UnsupportedRestore {}

enum Payload {
    Unwritten(Vec<MaybeUninit<u8>>),
    Supplied(Vec<u8>),
}

pub struct Owner {
    valid_raw: u8,
    restore_active_raw: u8,
    origin: Origin,
    payload: Payload,
}

impl Owner {
    pub fn from_constructor(budget: Budget) -> Result<Self> {
        ensure!(
            PAYLOAD_BYTES <= budget.payload_bytes,
            "checkpoint payload budget exhausted"
        );
        let mut payload = Vec::new();
        payload.try_reserve_exact(PAYLOAD_BYTES)?;
        payload.resize_with(PAYLOAD_BYTES, MaybeUninit::uninit);
        Ok(Self {
            valid_raw: 0,
            restore_active_raw: 0,
            origin: Origin::FreshConstructor,
            payload: Payload::Unwritten(payload),
        })
    }

    /// Explicit supplied retained state, not a public flag setter or proof that
    /// native capture/restore ran. Active restore remains unsupported here.
    pub fn from_snapshot(
        valid_raw: u8,
        restore_active_raw: u8,
        payload: Vec<u8>,
        budget: Budget,
    ) -> Result<Self> {
        ensure!(
            payload.len() == PAYLOAD_BYTES,
            "checkpoint payload size differs"
        );
        ensure!(
            payload.len() <= budget.payload_bytes,
            "checkpoint payload budget exhausted"
        );
        Ok(Self {
            valid_raw,
            restore_active_raw,
            origin: Origin::SuppliedSnapshot,
            payload: Payload::Supplied(payload),
        })
    }

    pub fn valid_raw(&self) -> u8 {
        self.valid_raw
    }
    pub fn restore_active_raw(&self) -> u8 {
        self.restore_active_raw
    }
    pub fn valid(&self) -> bool {
        self.valid_raw != 0
    }
    pub fn restore_active(&self) -> bool {
        self.restore_active_raw != 0
    }
    pub fn origin(&self) -> Origin {
        self.origin
    }
    pub fn payload_len(&self) -> usize {
        match &self.payload {
            Payload::Unwritten(bytes) => bytes.len(),
            Payload::Supplied(bytes) => bytes.len(),
        }
    }
    pub fn payload_initialized(&self) -> bool {
        matches!(self.payload, Payload::Supplied(_))
    }

    /// Only the restore-active byte is needed by the fresh case7/case10 gates.
    /// No payload, list, leaf or sound provider is consulted on this skip.
    pub fn require_not_restoring(&self) -> Result<SkipNotRestoring> {
        if self.restore_active() {
            return Err(UnsupportedRestore.into());
        }
        Ok(SkipNotRestoring)
    }
}
