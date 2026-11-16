//! Header+80 phase storage that may hold a retained time-local expression
//! instead of known bits. While deferred, +80 has no known bytes and the
//! numeric getter fails; every concrete writer replaces the expression.
use super::*;
use std::sync::Weak;

/// One actual read of a persistent process scalar. Only its owner creates it;
/// the value stays bound to that read even if the owner changes later.
#[derive(Debug, Clone)]
pub struct ScalarCapture {
    owner: u64,
    identity: Weak<()>,
    read_sequence: u64,
    revision: u64,
    current: Option<u32>,
}
impl ScalarCapture {
    pub(crate) fn new(
        owner: u64,
        identity: Weak<()>,
        read_sequence: u64,
        revision: u64,
        current: Option<u32>,
    ) -> Self {
        Self {
            owner,
            identity,
            read_sequence,
            revision,
            current,
        }
    }
    pub fn owner(&self) -> u64 {
        self.owner
    }
    pub(crate) fn identity(&self) -> &Weak<()> {
        &self.identity
    }
    pub fn read_sequence(&self) -> u64 {
        self.read_sequence
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Known current bits at this read; None is UnknownCurrent, never 0 or 1.
    pub fn current_bits(&self) -> Option<u32> {
        self.current
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        for value in [self.owner, self.read_sequence, self.revision] {
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        emit(u32::from(self.current.is_some()));
        if let Some(bits) = self.current {
            emit(bits);
        }
    }
}
impl PartialEq for ScalarCapture {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
            && Weak::ptr_eq(&self.identity, &other.identity)
            && self.read_sequence == other.read_sequence
            && self.revision == other.revision
            && self.current == other.current
    }
}
impl Eq for ScalarCapture {}

/// Ordered `fcmpo(process, upper)` with LT|EQ merged: process when ordered
/// less-or-equal, otherwise (greater or unordered) the upper literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedUpperClamp {
    read: ScalarCapture,
    upper_bits: u32,
}
impl OrderedUpperClamp {
    pub fn new(read: ScalarCapture, upper_bits: u32) -> Self {
        Self { read, upper_bits }
    }
    pub fn read(&self) -> &ScalarCapture {
        &self.read
    }
    pub fn upper_bits(&self) -> u32 {
        self.upper_bits
    }
    /// Exact raw bits of the selected operand. Host comparison only: no
    /// FPSCR exception or trap behavior is modeled.
    pub fn selected_bits(&self) -> Option<u32> {
        let process = self.read.current?;
        Some(
            if f32::from_bits(process) <= f32::from_bits(self.upper_bits) {
                process
            } else {
                self.upper_bits
            },
        )
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(1); // ordered upper clamp, LT|EQ selects the process operand
        self.read.visit_state_words(emit);
        emit(self.upper_bits);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseAssignment {
    Known(u32),
    Deferred,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredPhase;
impl std::fmt::Display for DeferredPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("script phase holds an unresolved process-scalar expression")
    }
}
impl std::error::Error for DeferredPhase {}

impl Store {
    pub fn assign_phase_clamp(
        &mut self,
        id: NodeId,
        clamp: OrderedUpperClamp,
    ) -> Result<PhaseAssignment> {
        let header = self
            .headers
            .get_mut(&id)
            .context("phase assignment script absent")?;
        ensure!(
            header.initializer_returned(),
            "phase assignment precedes the actual factory return"
        );
        Ok(match clamp.selected_bits() {
            Some(bits) => {
                header.store_phase(bits);
                PhaseAssignment::Known(bits)
            }
            None => {
                header.bytes[0x80..0x84].fill(None);
                header.phase_expression = Some(clamp);
                PhaseAssignment::Deferred
            }
        })
    }
}
impl Header {
    /// Concrete +80 store; replaces any retained expression.
    pub(super) fn store_phase(&mut self, bits: u32) {
        self.w(0x80, bits);
        self.phase_expression = None;
    }
    pub fn phase_expression(&self) -> Option<&OrderedUpperClamp> {
        self.phase_expression.as_ref()
    }
    pub fn phase_bits(&self) -> Result<u32> {
        if self.phase_expression.is_some() {
            return Err(DeferredPhase.into());
        }
        self.word(0x80).context("script phase is unwritten")
    }
}

#[cfg(test)]
#[path = "phase/tests.rs"]
mod tests;
