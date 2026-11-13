//! The reached kind-three attribute only. Both relative offsets remain anchored
//! to their own packed words; unrelated animation fields are never preflighted.
use super::*;

/// Runtime-owned mask data from the same logically resident animation body.
/// These asset words deliberately do not implement Serialize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoneMask {
    pub attribute_present: bool,
    pub words: [u32; 4],
}

impl LiveBody<'_> {
    pub fn bone_mask(self) -> Result<BoneMask> {
        let field = self
            .address
            .checked_add(0x38)
            .context("bone-mask field address overflow")?;
        let bytes = Bytes(self.data);
        let packed = bytes.u32(field)?;
        let presence = packed as u8;
        if presence & 8 == 0 {
            return Ok(BoneMask {
                attribute_present: false,
                words: [0; 4],
            });
        }
        let table_delta = (packed as i32) >> 8;
        ensure!(
            table_delta != 0,
            "required bone-mask attribute table is NULL"
        );
        let table = relative(field, table_delta)?;
        let entry = table
            .checked_add(((presence & 7).count_ones() as usize) * 4)
            .context("bone-mask attribute entry overflow")?;
        let entry_word = bytes.u32(entry)?;
        // Unlike the table's zero delta, a zero entry delta addresses the entry
        // itself. Its low byte is not another kind/presence field here.
        let payload = relative(entry, (entry_word as i32) >> 8)?;
        let mut words = [0; 4];
        for (index, word) in words.iter_mut().enumerate() {
            let at = payload
                .checked_add(index * 4)
                .context("bone-mask payload address overflow")?;
            *word = bytes.u32(at)?;
        }
        Ok(BoneMask {
            attribute_present: true,
            words,
        })
    }
}

fn relative(anchor: usize, delta: i32) -> Result<usize> {
    anchor
        .checked_add_signed(delta as isize)
        .context("bone-mask relative address overflow")
}

#[cfg(test)]
mod tests;
