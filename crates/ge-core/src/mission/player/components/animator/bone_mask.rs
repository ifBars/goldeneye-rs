//! Per-bone masks on physical headers, independently of active node count.
use super::*;
use anyhow::Context;

impl super::super::Preparation {
    pub fn apply_mode_bone_mask(
        &mut self,
        receiver: Identity,
        slot: u8,
        words: [u32; 4],
    ) -> Result<()> {
        self.mode_animator_mut(receiver)?
            .apply_bone_mask(slot, words)
    }
}

impl Animator {
    fn apply_bone_mask(&mut self, slot: u8, words: [u32; 4]) -> Result<()> {
        let index = usize::from(slot);
        ensure!(index < 4, "bone mask physical slot exceeds four");
        ensure!(self.layers.is_some(), "bone mask layer owner absent");
        let start = 0x138 + index * 0x20;
        // The destination flags are read first. All four mask stores then
        // precede the final flag store, including on inactive physical slots.
        let flags = u16::from_be_bytes([
            self.byte(start + 8).context("bone mask flags unknown")?,
            self.byte(start + 9).context("bone mask flags unknown")?,
        ]);
        ensure!(
            self.layers.as_ref().expect("checked layers").slots()[index].flags_raw() == flags,
            "bone mask raw/typed flags differ"
        );
        for (i, word) in words.into_iter().enumerate() {
            self.word_store(start + 0x10 + i * 4, word);
        }
        self.layers
            .as_mut()
            .expect("checked layers")
            .set_slot_mask_words(index, words)?;
        self.half_store(start + 8, flags | 2);
        self.layers
            .as_mut()
            .expect("checked layers")
            .or_slot_flags(index, 2)
    }
}

#[cfg(test)]
#[path = "bone_mask/tests.rs"]
mod tests;
