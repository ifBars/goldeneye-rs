//! Process-lived owner of the scalar read by the bind caller (r13-7be8).
//! KnownInitial is the audited startup data word; it is provenance only and
//! never reported as the current value. No current-value producer is admitted,
//! so reads capture UnknownCurrent until a real writer/proof exists.
use crate::animation::script_child::phase::ScalarCapture;
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Initialized data word preserved by the startup copy/zero tables (v98).
pub const STARTUP_BITS: u32 = 0x3f80_0000;

#[derive(Debug)]
pub struct ProcessScalar {
    owner: u64,
    identity: Arc<()>,
    current: Option<u32>,
    revision: u64,
    reads: u64,
}
impl ProcessScalar {
    /// Created once per process/session by the host's shared state. Retry and
    /// cancellation reuse this owner; it is never reset to KnownInitial.
    pub fn from_startup(owner: u64) -> Result<Self> {
        ensure!(owner != 0, "process scalar owner identity is zero");
        Ok(Self {
            owner,
            identity: Arc::new(()),
            current: None,
            revision: 0,
            reads: 0,
        })
    }
    /// The actual unconditional source read. The capture is bound to this
    /// owner instance, read sequence and value revision.
    pub fn read(&mut self) -> Result<ScalarCapture> {
        let sequence = self
            .reads
            .checked_add(1)
            .context("process scalar read sequence exhausted")?;
        self.reads = sequence;
        Ok(ScalarCapture::new(
            self.owner,
            Arc::downgrade(&self.identity),
            sequence,
            self.revision,
            self.current,
        ))
    }
    /// The capture came from this owner instance.
    pub fn issued(&self, capture: &ScalarCapture) -> bool {
        capture.owner() == self.owner
            && capture
                .identity()
                .upgrade()
                .is_some_and(|identity| Arc::ptr_eq(&identity, &self.identity))
            && capture.read_sequence() <= self.reads
    }
    pub fn owner(&self) -> u64 {
        self.owner
    }
    pub fn initial_bits(&self) -> u32 {
        STARTUP_BITS
    }
    /// KnownCurrent bits; None while the current value is unproven.
    pub fn current_bits(&self) -> Option<u32> {
        self.current
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn reads(&self) -> u64 {
        self.reads
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        for value in [self.owner, self.revision, self.reads] {
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        emit(STARTUP_BITS);
        emit(u32::from(self.current.is_some()));
        if let Some(bits) = self.current {
            emit(bits);
        }
    }
}
