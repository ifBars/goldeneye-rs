//! Written-value proofs never read the allocation's uninitialized storage.
use super::*;
use std::ops::Range;

impl Allocation {
    /// An unwritten payload byte is unknown; an invalid offset is an error.
    pub fn payload_byte(&self, offset: usize) -> Result<Option<u8>> {
        self.payload_range(offset, 1)?;
        Ok(self
            .payload_written
            .as_ref()
            .and_then(|written| written[offset]))
    }

    /// Native big-endian word. Every byte must have an actual write proof.
    pub fn payload_word(&self, offset: usize) -> Result<Option<u32>> {
        ensure!(
            offset.is_multiple_of(4),
            "unaligned constructor payload word"
        );
        self.payload_range(offset, 4)?;
        let Some(written) = &self.payload_written else {
            return Ok(None);
        };
        Ok(match &written[offset..offset + 4] {
            [Some(a), Some(b), Some(c), Some(d)] => Some(u32::from_be_bytes([*a, *b, *c, *d])),
            _ => None,
        })
    }

    pub fn known_payload_bytes(&self) -> usize {
        self.payload_written
            .as_ref()
            .map_or(0, |written| written.iter().flatten().count())
    }

    pub(in crate::mission::player::constructor) fn put_payload_byte(
        &mut self,
        offset: usize,
        value: u8,
    ) -> Result<()> {
        self.put_payload(offset, &[value])
    }

    pub(in crate::mission::player::constructor) fn put_payload_word(
        &mut self,
        offset: usize,
        value: u32,
    ) -> Result<()> {
        ensure!(
            offset.is_multiple_of(4),
            "unaligned constructor payload word"
        );
        self.put_payload(offset, &value.to_be_bytes())
    }

    /// One aligned native half-word store (sth).
    pub(in crate::mission::player::constructor) fn put_payload_half(
        &mut self,
        offset: usize,
        value: u16,
    ) -> Result<()> {
        ensure!(
            offset.is_multiple_of(2),
            "unaligned constructor payload half-word"
        );
        self.put_payload(offset, &value.to_be_bytes())
    }

    /// One 8-byte paired-single store of two raw lanes, in lane order.
    pub(in crate::mission::player::constructor) fn put_payload_pair(
        &mut self,
        offset: usize,
        lanes: [u32; 2],
    ) -> Result<()> {
        // Entry translations start at +4: word alignment, as paired stores allow.
        ensure!(
            offset.is_multiple_of(4),
            "unaligned constructor payload pair"
        );
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&lanes[0].to_be_bytes());
        bytes[4..].copy_from_slice(&lanes[1].to_be_bytes());
        self.put_payload(offset, &bytes)
    }

    fn payload_range(&self, offset: usize, bytes: usize) -> Result<Range<usize>> {
        let end = offset
            .checked_add(bytes)
            .context("constructor payload range overflow")?;
        ensure!(
            end <= self.payload_bytes(),
            "constructor payload range outside allocation"
        );
        let start = self
            .payload_offset()
            .checked_add(offset)
            .context("constructor payload storage offset overflow")?;
        let storage_end = start
            .checked_add(bytes)
            .context("constructor payload storage range overflow")?;
        ensure!(
            storage_end <= self.storage.len(),
            "constructor payload storage range outside allocation"
        );
        Ok(start..storage_end)
    }

    fn put_payload(&mut self, offset: usize, values: &[u8]) -> Result<()> {
        // Check the entire source-width store before any mutation. Fallible
        // proof allocation also precedes the first native storage write.
        let storage_range = self.payload_range(offset, values.len())?;
        if self.payload_written.is_none() {
            let mut written = Vec::new();
            written
                .try_reserve_exact(self.payload_bytes())
                .context("allocating constructor payload write proofs")?;
            written.resize(self.payload_bytes(), None);
            self.payload_written = Some(written);
        }
        let written = self
            .payload_written
            .as_mut()
            .expect("allocated proof values");
        for ((storage, proof), &value) in self.storage[storage_range]
            .iter_mut()
            .zip(&mut written[offset..offset + values.len()])
            .zip(values)
        {
            storage.write(value);
            *proof = Some(value);
        }
        Ok(())
    }

    pub(super) fn visit_payload(&self, visit: &mut impl FnMut(u32)) {
        visit(1);
        visit(self.known_payload_bytes() as u32);
        if let Some(written) = &self.payload_written {
            for (offset, value) in written.iter().enumerate() {
                if let Some(value) = value {
                    visit(offset as u32);
                    visit(u32::from(*value));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/allocation_payload.rs"]
mod tests;
