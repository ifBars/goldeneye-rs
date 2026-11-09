use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Only constructor-proven or explicitly unknown scoped fields are modeled.
/// Offsets label independent semantic fields; this is not a native byte image.
#[derive(Debug)]
pub struct Leaf {
    bytes: BTreeMap<u16, Option<u8>>,
    words: BTreeMap<u16, Option<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeafReset {
    pub byte_writes: usize,
    pub word_writes: usize,
}

impl Leaf {
    pub fn fresh() -> Self {
        let mut bytes = BTreeMap::new();
        for field in [0x00, 0x02, 0x04, 0x05, 0x06, 0x10, 0x50] {
            bytes.insert(field, Some(0));
        }
        bytes.insert(0x01, Some(0xff));
        bytes.insert(0x03, None);
        bytes.insert(0x07, Some(1));
        bytes.insert(0x08, Some(1));
        let mut words = BTreeMap::new();
        for field in [
            0x0c, 0x28, 0x40, 0x44, 0x48, 0x4c, 0x54, 0x5c, 0x20, 0x58, 0x60,
        ] {
            words.insert(field, Some(0));
        }
        words.insert(0x14, Some(0x3f4c_cccd));
        words.insert(0x18, Some(0x3f80_0000));
        words.insert(0x1c, Some(0x3f80_0000));
        Self { bytes, words }
    }

    pub fn bytes(&self) -> &BTreeMap<u16, Option<u8>> {
        &self.bytes
    }
    pub fn words(&self) -> &BTreeMap<u16, Option<u32>> {
        &self.words
    }
    pub fn byte(&self, field: u16) -> Option<u8> {
        self.bytes.get(&field).copied().flatten()
    }
    pub fn word(&self, field: u16) -> Option<u32> {
        self.words.get(&field).copied().flatten()
    }

    pub fn set_byte(&mut self, field: u16, value: Option<u8>) -> Result<()> {
        *self
            .bytes
            .get_mut(&field)
            .context("leaf byte outside scoped ownership")? = value;
        Ok(())
    }
    pub fn set_word(&mut self, field: u16, value: Option<u32>) -> Result<()> {
        *self
            .words
            .get_mut(&field)
            .context("leaf word outside scoped ownership")? = value;
        Ok(())
    }

    pub fn reset(&mut self) -> LeafReset {
        for field in [0x00, 0x02, 0x04, 0x05, 0x06, 0x10] {
            self.bytes.insert(field, Some(0));
        }
        for field in [0x0c, 0x28, 0x54, 0x5c, 0x20, 0x58, 0x60] {
            self.words.insert(field, Some(0));
        }
        LeafReset {
            byte_writes: 6,
            word_writes: 7,
        }
    }
}
