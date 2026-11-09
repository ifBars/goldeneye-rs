//! Source 800ef810 writes +168 unconditionally and returns void. A missing
//! captured handler input therefore owns an unresolved result, not stale1.
use super::{MissingHandlerKind, Owner};
use crate::mission::player::identity::ItemId;
use anyhow::{Context, Result, ensure};

/// Time-local operands of the reached setter. The sequence is an owner-local
/// host operation identity, not original elapsed time or history verification.
/// No resolution API can substitute a later handler/height into this capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredHandlerScalar {
    item: Option<u64>,
    item_identity: Option<ItemId>,
    setter_sequence: u64,
    half_height_bits: u32,
}
impl DeferredHandlerScalar {
    pub fn item(&self) -> Option<u64> {
        self.item
    }
    pub fn item_identity(&self) -> Option<ItemId> {
        self.item_identity
    }
    pub fn setter_sequence(&self) -> u64 {
        self.setter_sequence
    }
    pub fn half_height_bits(&self) -> u32 {
        self.half_height_bits
    }
}

impl Owner {
    /// Apply the captured input at this setter invocation. Missing kind owns a
    /// deferred result; independent initialization must continue past this void
    /// source operation. Unsupported checked arithmetic still returns an error.
    pub fn apply_handler_kind(&mut self, kind: Option<u32>) -> Result<()> {
        let sequence = self
            .handler_scalar_sequence
            .checked_add(1)
            .context("physics handler scalar operation identity exhausted")?;
        let Some(kind) = kind else {
            let half_height_bits = self
                .word(0x1d0)
                .context("physics captured height is unwritten")?;
            self.deferred_handler_scalar = Some(DeferredHandlerScalar {
                item: self.item(),
                item_identity: self.item_identity(),
                setter_sequence: sequence,
                half_height_bits,
            });
            self.handler_scalar_sequence = sequence;
            for byte in &mut self.known[0x168..0x16c] {
                *byte = None;
            }
            return Ok(());
        };
        // Only this source arm reads halfheight. Other known arms stay lazy.
        let value = if kind & 0xff00_0000 == 0x0100_0000 {
            4000. * f32::from_bits(self.word(0x1d0).context("physics height is unwritten")?)
        } else {
            match kind & 0xffff_0000 {
                0x4003_0000 | 0x4004_0000 => 1000.,
                0x4000_0000 | 0x4001_0000 | 0x4002_0000 => 500.,
                _ => 0.,
            }
        };
        ensure!(
            value.is_finite(),
            "physics handler scalar exceeds the finite host subset"
        );
        self.store_word(0x168, value.to_bits());
        self.handler_scalar_sequence = sequence;
        self.deferred_handler_scalar = None;
        Ok(())
    }

    /// The actual scalar consumer. Never substitutes zero or constructor1 for
    /// the overwritten field while its time-local expression is unresolved.
    pub fn handler_scalar_bits(&self) -> Result<u32> {
        if self.deferred_handler_scalar.is_some() {
            return Err(MissingHandlerKind.into());
        }
        self.word(0x168)
            .context("physics handler scalar is unwritten")
    }
    pub fn scalar_pending(&self) -> bool {
        self.deferred_handler_scalar.is_some()
    }
    pub fn deferred_handler_scalar(&self) -> Option<DeferredHandlerScalar> {
        self.deferred_handler_scalar
    }
}
