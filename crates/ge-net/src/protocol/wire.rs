//! Checked big-endian primitives.

use super::DecodeError;

/// A bounds-checked cursor over received bytes.
#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub const fn remaining(&self) -> usize {
        self.bytes.len()
    }

    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let (head, tail) = self
            .bytes
            .split_at_checked(len)
            .ok_or(DecodeError::Truncated)?;
        self.bytes = tail;
        Ok(head)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    /// Everything not yet read.
    pub fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.bytes)
    }

    /// Succeeds only if every byte was consumed.
    pub fn finish(self) -> Result<(), DecodeError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::TrailingBytes)
        }
    }
}

pub(crate) fn put_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

pub(crate) fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub(crate) fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub(crate) fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}
