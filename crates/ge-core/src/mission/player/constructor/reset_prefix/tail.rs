//! Source-ordered sparse continuation after the actual inventory selection.
//! Defaults are read from retained providers, including the initialized shared
//! quad; completing this reset does not complete the enclosing constructor.
use super::{PendingReset, Stage};
use crate::mission::player::constructor::PendingPrelude;
use anyhow::{Result, ensure};

pub trait TailProvider {
    fn default_quad_bits(&mut self) -> Result<[u32; 4]>;
    fn current_game_field_328_bits(&mut self) -> Result<u32>;
    fn default_vector_bits(&mut self) -> Result<[u32; 3]>;
}

impl PendingReset {
    pub fn finish_tail(
        &mut self,
        prelude: &mut PendingPrelude,
        provider: &mut impl TailProvider,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::InventorySelected,
            "derived reset tail is out of order"
        );
        ensure!(
            self.player == prelude.player(),
            "derived reset tail player mismatch"
        );
        self.stage = Stage::TailInProgress;
        match self.apply_tail(prelude, provider) {
            Ok(()) => {
                self.stage = Stage::Complete;
                Ok(())
            }
            Err(error) => {
                self.stage = Stage::TailFailed;
                Err(error)
            }
        }
    }

    fn tail_words(&mut self, offsets: &[u16], value: u32) {
        for &offset in offsets {
            self.word_store(offset, value);
        }
    }
    fn tail_bytes(&mut self, offsets: &[u16], value: u8) {
        for &offset in offsets {
            self.bytes.insert(offset, value);
        }
    }
    fn tail_vector(&mut self, offset: u16, bits: &[u32]) {
        for (index, &word) in bits.iter().enumerate() {
            self.word_store(offset + index as u16 * 4, word);
        }
    }

    fn apply_tail(
        &mut self,
        prelude: &mut PendingPrelude,
        provider: &mut impl TailProvider,
    ) -> Result<()> {
        let one = 1.0_f32.to_bits();
        // 8012f19c..244, before the first shared-quad read.
        self.tail_bytes(&[0xca0], 0);
        self.tail_words(&[0xafc, 0xb00, 0xaf8, 0xb84, 0xc9c], 0);
        self.tail_vector(0x9b0, &[0; 4]);
        self.tail_words(&[0xc5c, 0xc60, 0xc64], 0);
        self.tail_words(&[0x9a8, 0x9ac, 0xca8, 0xcac], one);
        self.tail_words(&[0xcb0, 0xcc8, 0xccc, 0xcd0, 0xcd4], 0);
        self.tail_bytes(&[0xcd8], 1);
        self.tail_words(&[0xcdc, 0xce0, 0xce4, 0xce8], 0);
        self.tail_bytes(&[0xcec], 0);
        self.tail_bytes(&[0xced], 1);
        self.tail_words(&[0xcf0], 0);

        let quad = provider.default_quad_bits()?;
        self.tail_vector(0xb88, &quad);
        let quad = provider.default_quad_bits()?;
        self.tail_vector(0xb98, &quad);
        for offset in [0x9c8, 0x9d8, 0x9e8, 0x9f8] {
            self.tail_vector(offset, &[0; 4]);
        }
        self.tail_vector(0xc08, &[one; 4]);
        self.tail_bytes(&[0xbd0, 0xbd2, 0xbd1], 0);
        self.tail_words(&[0xbd4], one);
        self.tail_words(&[0xd08], 0);
        self.tail_bytes(&[0xd00, 0xd01], 0);
        self.tail_words(&[0xd10, 0xd14, 0xd04, 0xd0c], 0);
        self.tail_bytes(&[0xd18, 0xd24], 0);
        self.tail_words(&[0xca4], 0x0e00_0070);
        self.tail_bytes(&[0xca1, 0xbad], 0);
        self.tail_words(&[0x1024], 0);
        // C54 was an input to inventory selection. Clear it only now.
        self.tail_bytes(&[0xc58, 0xc59, 0xc54, 0xc94, 0xc95], 0);
        self.tail_words(&[0xc80, 0xc84, 0xc88, 0xc8c], 0);
        self.tail_words(&[0xc90], one);
        self.tail_bytes(&[0xe59], 0);
        self.tail_words(&[0xe5c], 2.75_f32.to_bits());
        self.tail_words(&[0xe60], 0);
        self.tail_bytes(&[0xe64], 1);
        self.tail_vector(0xe68, &[0; 4]);
        self.tail_words(&[0xbb0, 0xbb4], 0);

        // 8012f380..390 reads the current owner; it does not write +720.
        let game = provider.current_game_field_328_bits()?;
        self.word_store(0x71c, game);
        self.tail_words(&[0xcf4], 0);
        self.tail_bytes(&[0xcf8, 0xba8, 0xd25], 0);
        self.tail_words(&[0xd28], u32::MAX);
        self.tail_vector(0xd68, &[0; 4]);
        self.tail_words(&[0xd40, 0xd3c], 0);
        self.tail_bytes(&[0xd51, 0xd52, 0xd53], 0);
        self.tail_words(&[0xd78], 0);
        self.tail_bytes(&[0xd54, 0xd80], 0);
        self.tail_words(&[0xd94], u32::MAX);
        self.tail_words(&[0xdc0, 0xdc4, 0xdc8, 0xdb8, 0xdbc], 0);
        self.tail_bytes(&[0xf58, 0xf59, 0xbaa], 0);

        let vector = provider.default_vector_bits()?;
        self.tail_vector(0xeb0, &vector);
        self.tail_words(&[0xebc], 0);
        self.tail_words(&[0xec0, 0xec4], one);
        self.tail_bytes(&[0xec8], 0);
        self.tail_words(&[0xecc], 0);
        self.tail_words(&[0xed0], one);
        self.tail_bytes(&[0xed4, 0xed5], 0);
        self.tail_words(&[0xed8], 0);
        let quad = provider.default_quad_bits()?;
        self.tail_vector(0xedc, &quad);
        self.tail_bytes(&[0xba9], 1);
        self.tail_bytes(&[0xbbb, 0xdcc, 0xdcd], 0);
        self.tail_vector(0xdd4, &[0; 4]);
        self.tail_vector(0xde4, &[0, 0, 0, one]);
        self.tail_vector(0xdf4, &[0; 4]);
        self.tail_vector(0xe04, &[0, 0, 0, one]);
        self.tail_words(&[0xe14], one);
        self.tail_bytes(&[0xbf0, 0xe18], 0);
        self.tail_words(&[0xea0, 0xea4, 0xea8, 0xe94, 0xe98], 0);
        self.tail_words(&[0x3ac, 0x3a8], 100.0_f32.to_bits());
        self.tail_words(&[0x750], 0);
        self.tail_bytes(&[0xbb8, 0xbd8, 0xbd9], 0);
        self.tail_bytes(&[0xbda], 1);
        self.tail_words(&[0xbdc, 0xbe0, 0xcfc, 0xef0, 0xef4], 0);
        self.tail_vector(0xef8, &[0; 4]);
        self.tail_vector(0xf08, &[0; 4]);
        self.tail_words(&[0xf18, 0xf1c, 0xf20, 0xf24, 0xf28], 0);
        self.tail_bytes(&[0xf2c], 0);
        self.tail_words(&[0xf30, 0xf34], one);
        self.tail_bytes(&[0xf38], 0);
        self.tail_words(&[0xf3c, 0xf40, 0xf44, 0xf48], 0);
        self.tail_bytes(&[0xf4c, 0xf4d], 0);
        self.tail_words(&[0xf50, 0xf54], 0);
        self.tail_vector(0xf5c, &[0, 0.825_f32.to_bits(), 0, 0]);
        self.tail_words(&[0xf70], 0.825_f32.to_bits());
        self.tail_words(&[0xf6c], 0.5_f32.to_bits());
        self.tail_bytes(&[0xbe4], 0);
        self.tail_words(&[0xbe8, 0xbec], 0);

        // 8012f598..5cc releases the actual C18 owner, retaining its growth.
        // Fresh constructor state has no backing; no allocator call is invented.
        let _released = prelude.clear_local_buffer();
        self.tail_words(&[0xc24, 0xc18, 0xc20], 0);
        self.tail_bytes(&[0xc68], 0);
        self.tail_words(&[0xc6c], u32::MAX);
        self.tail_bytes(&[0xc70], 0);
        for offset in 0xfec..0x1150 {
            self.bytes.insert(offset, 0);
        }

        let quad = provider.default_quad_bits()?;
        self.tail_vector(0xc30, &quad);
        for offset in (0x10a0..=0x10c4).step_by(4) {
            self.word_store(offset, 0);
        }
        self.word_store(0xc2c, 0);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/reset_tail.rs"]
mod tests;
