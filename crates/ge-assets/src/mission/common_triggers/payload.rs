use super::super::triggers::Record;
use anyhow::{Result, ensure};

/// Raw source common fields. No Serialize: payload arrays/coordinates stay
/// owned privately and are not progress-report or package content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Common {
    pub header_flags_raw: u32,
    pub property_mask_raw: u32,
    pub position_bits: [u32; 4],
    pub rotation_bits: [u32; 4],
    pub numeric: [u32; 16],
    pub reference_ordinals: [u32; 8],
    pub word_74_raw: u32,
    pub word_78_raw: u32,
    pub animator_present: bool,
    pub animator_index_raw: i32,
    /// Unprocessed selector only. A present shape requires a separate shape
    /// dependency before the runtime may complete common initialization.
    pub shape_index_raw: Option<u32>,
    pub property_words_read: usize,
}

pub(super) fn read(record: &Record<'_>, maximum_words: usize) -> Result<Common> {
    let header_flags_raw = record.word(4)?;
    let mask = record.word(8)?;
    let position_bits = [record.word(0xc)?, record.word(0x10)?, record.word(0x14)?, 0];
    let rotation_bits = [
        record.word(0x18)?,
        record.word(0x1c)?,
        record.word(0x20)?,
        0,
    ];
    let mut cursor = Cursor {
        record,
        words: 0,
        maximum_words,
    };
    let mut numeric = [0; 16];
    for (bit, value) in numeric.iter_mut().enumerate() {
        if mask & (1 << bit) != 0 {
            *value = cursor.next()?;
        }
    }
    let mut reference_ordinals = [u32::MAX; 8];
    for (bit, value) in reference_ordinals.iter_mut().enumerate() {
        if mask & (1 << (bit + 16)) != 0 {
            *value = cursor.next()?;
        }
    }
    let (mut word_78_raw, mut word_74_raw) = (0, 0);
    // Bit25 is consumed only inside the bit24 branch, not independently.
    if mask & (1 << 24) != 0 {
        word_78_raw = cursor.next()?;
        if mask & (1 << 25) != 0 {
            word_74_raw = cursor.next()?;
        }
    }
    let animator_present = mask & (1 << 26) != 0;
    let animator_index_raw = if animator_present {
        cursor.next()? as i32
    } else {
        0
    };
    let shape_index_raw = if mask & (1 << 27) != 0 {
        Some(cursor.next()?)
    } else {
        None
    };
    Ok(Common {
        header_flags_raw,
        property_mask_raw: mask,
        position_bits,
        rotation_bits,
        numeric,
        reference_ordinals,
        word_74_raw,
        word_78_raw,
        animator_present,
        animator_index_raw,
        shape_index_raw,
        property_words_read: cursor.words,
    })
}

struct Cursor<'record, 'data> {
    record: &'record Record<'data>,
    words: usize,
    maximum_words: usize,
}

impl Cursor<'_, '_> {
    fn next(&mut self) -> Result<u32> {
        ensure!(
            self.words < self.maximum_words,
            "common trigger property budget exhausted"
        );
        // At most28 source properties exist; no allocation, unchecked cursor or
        // alleged per-record span is inferred from neighboring records.
        let value = self.record.word(0x30 + self.words * 4)?;
        self.words += 1;
        Ok(value)
    }
}
