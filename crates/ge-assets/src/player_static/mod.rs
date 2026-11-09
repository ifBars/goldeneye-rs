//! Narrow reset inputs from the already verified, retained supplied DATA owner.
//! No default vector or arbitrary-address reader is embedded in the rewrite.
use crate::supplied_data::Data;
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct ResetDefaults {
    vector_bits: [u32; 3],
}
impl ResetDefaults {
    pub(crate) fn read(data: &Data) -> Result<Self> {
        let vector_bits = [
            data.u32(0x8057_4548)?,
            data.u32(0x8057_454c)?,
            data.u32(0x8057_4550)?,
        ];
        ensure!(
            vector_bits
                .iter()
                .all(|&word| f32::from_bits(word).is_finite()),
            "nonfinite supplied player reset vector"
        );
        Ok(Self { vector_bits })
    }
    pub fn vector_bits(self) -> [u32; 3] {
        self.vector_bits
    }
}

#[cfg(test)]
#[path = "tests/reader.rs"]
mod tests;
