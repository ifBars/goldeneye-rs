use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

use super::super::buffer::{Buffer, ClearStats};
use super::{Binding, BindingId, BindingRelease, BindingSlot};

const TOP: [u16; 5] = [0x7c, 0x90, 0xa4, 0xb8, 0xcc];
const RECORD: [u16; 11] = [
    0xe8, 0x120, 0x158, 0x190, 0x1c8, 0x200, 0x238, 0x270, 0x2a8, 0x2e0, 0x318,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferClear {
    /// Header's capacity field; record headers begin at record+8.
    pub source_offset: u16,
    pub stats: ClearStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffersReset {
    pub cleared: Vec<BufferClear>,
    pub release: Option<BindingRelease>,
    pub binding_slot_cleared: bool,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TailFields {
    pub flag_1c: Option<u8>,
    pub flag_3b0: Option<u8>,
    pub word_28: Option<u32>,
    pub word_e0: Option<u32>,
    pub word_e4: Option<u32>,
}

/// Scoped backing owners only: five top buffers and eleven record buffers.
/// Untouched earlier vectors and unwritten record prefixes are not zero-filled.
#[derive(Debug)]
pub struct Buffers {
    top: [Buffer; 5],
    records: [Buffer; 11],
    binding_slot: BindingSlot,
    bindings: BTreeMap<BindingId, Binding>,
    tail: TailFields,
    last_reset: Option<BuffersReset>,
}

impl Buffers {
    pub fn fresh() -> Self {
        Self {
            top: std::array::from_fn(|_| Buffer::empty(4)),
            records: std::array::from_fn(|_| Buffer::empty(4)),
            binding_slot: BindingSlot::Absent,
            bindings: BTreeMap::new(),
            // Constructor plus the proven boot binding reset. Other tail words
            // are unknown until this hook actually writes them.
            tail: TailFields {
                flag_1c: Some(0),
                flag_3b0: None,
                word_28: Some(0),
                word_e0: None,
                word_e4: None,
            },
            last_reset: None,
        }
    }
    pub fn top_buffers(&self) -> &[Buffer; 5] {
        &self.top
    }
    pub fn record_buffers(&self) -> &[Buffer; 11] {
        &self.records
    }
    pub fn top_buffer_mut(&mut self, index: usize) -> Result<&mut Buffer> {
        self.top
            .get_mut(index)
            .context("final top buffer index out of bounds")
    }
    pub fn record_buffer_mut(&mut self, index: usize) -> Result<&mut Buffer> {
        self.records
            .get_mut(index)
            .context("final record buffer index out of bounds")
    }
    pub fn tail(&self) -> &TailFields {
        &self.tail
    }
    pub fn tail_mut(&mut self) -> &mut TailFields {
        &mut self.tail
    }
    pub fn binding_slot(&self) -> BindingSlot {
        self.binding_slot
    }
    pub fn bindings(&self) -> &BTreeMap<BindingId, Binding> {
        &self.bindings
    }
    pub fn binding(&self, id: BindingId) -> Option<&Binding> {
        self.bindings.get(&id)
    }
    pub fn last_reset(&self) -> Option<&BuffersReset> {
        self.last_reset.as_ref()
    }

    pub fn bind(&mut self, binding: Binding) -> Result<()> {
        let id = binding.id();
        ensure!(
            !self.bindings.contains_key(&id),
            "binding identity already owned"
        );
        self.bindings.insert(id, binding);
        self.binding_slot = BindingSlot::Bound(id);
        Ok(())
    }
    pub fn bind_existing(&mut self, id: BindingId) -> Result<()> {
        ensure!(self.bindings.contains_key(&id), "binding item is not owned");
        self.binding_slot = BindingSlot::Bound(id);
        Ok(())
    }
    /// Explicit unbound history is not a proven NULL source binding.
    pub fn mark_binding_unknown(&mut self) {
        self.binding_slot = BindingSlot::Unknown;
    }

    pub(super) fn clear_receipt(&mut self) {
        self.last_reset = None;
    }

    pub fn reset(&mut self) -> Result<BuffersReset> {
        let mut receipt = BuffersReset {
            cleared: Vec::with_capacity(16),
            release: None,
            binding_slot_cleared: false,
            complete: false,
        };
        for (index, buffer) in self.top.iter_mut().enumerate() {
            receipt.cleared.push(BufferClear {
                source_offset: TOP[index],
                stats: buffer.clear_backing(),
            });
        }
        for (index, buffer) in self.records.iter_mut().enumerate() {
            receipt.cleared.push(BufferClear {
                source_offset: RECORD[index] + 8,
                stats: buffer.clear_backing(),
            });
        }
        let binding_slot = self.binding_slot;
        self.tail.flag_3b0 = Some(1);
        self.tail.flag_1c = Some(0);
        match binding_slot {
            BindingSlot::Unknown => {
                self.last_reset = Some(receipt);
                anyhow::bail!("final reset binding history is unknown");
            }
            BindingSlot::Absent => {}
            BindingSlot::Bound(id) => {
                let Some(binding) = self.bindings.get_mut(&id) else {
                    self.last_reset = Some(receipt);
                    anyhow::bail!("final reset binding item is not owned");
                };
                receipt.release = Some(binding.release_flags());
                // Keep the actual item owned for later flag consumers. This
                // clears only its manager link, never destroys the item.
                self.binding_slot = BindingSlot::Absent;
                receipt.binding_slot_cleared = true;
            }
        }
        self.tail.word_e0 = Some(0);
        self.tail.word_e4 = Some(0);
        self.tail.word_28 = Some(0);
        receipt.complete = true;
        self.last_reset = Some(receipt.clone());
        Ok(receipt)
    }
}
