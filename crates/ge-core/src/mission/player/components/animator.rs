use super::super::identity::PlayerId;

pub const SOURCE_BYTES: usize = 0x1b8;

/// Actual pending TP allocation, scoped to its one-shot player initializer.
/// Item/resource/skeleton backlinks are NULL until their real bind operations.
#[derive(Debug)]
pub struct Animator {
    player: PlayerId,
    bytes: [Option<u8>; SOURCE_BYTES],
    // Base ctor clears bits1..7 but preserves allocation bit0 at +8f.
    flag_8f_known_mask: u8,
}
impl Animator {
    pub(super) fn from_constructor(player: PlayerId) -> Self {
        let mut owner = Self {
            player,
            bytes: [None; SOURCE_BYTES],
            flag_8f_known_mask: 0xfe,
        };
        for offset in [
            0, 4, 8, 0x10, 0x14, 0x1c, 0x20, 0x28, 0x5c, 0x60, 0x94, 0x114, 0x118, 0x11c, 0x120,
            0x124, 0x128, 0x12c,
        ] {
            owner.word_store(offset, 0);
        }
        owner.word_store(0x24, u32::MAX);
        for offset in (0x2c..0x4c)
            .step_by(4)
            .chain((0xd8..0xe8).step_by(4))
            .chain((0xf8..0x108).step_by(4))
        {
            owner.word_store(offset, 0);
        }
        for offset in (0x4c..0x5c).step_by(4).chain((0xe8..0xf8).step_by(4)) {
            owner.word_store(offset, 1f32.to_bits());
        }
        for index in 0..16 {
            owner.word_store(
                0x98 + index * 4,
                if index.is_multiple_of(5) {
                    1f32.to_bits()
                } else {
                    0
                },
            );
        }
        for offset in [0x64, 0x6c, 0x6e, 0x70, 0x72, 0x76, 0x90] {
            owner.half_store(offset, 0);
        }
        owner.half_store(0x68, 0x7fff);
        owner.half_store(0xc, 0xb);
        for offset in [
            0x66, 0x67, 0x74, 0x75, 0x8d, 0x8e, 0x110, 0x132, 0x133, 0x134, 0x135,
        ] {
            owner.bytes[offset] = Some(0);
        }
        owner.bytes[0x8c] = Some(0x10);
        owner.bytes[0x93] = Some(2);
        owner.word_store(0x108, 1f32.to_bits());
        owner.word_store(0x10c, 0);
        owner.bytes[0x130] = Some(1);
        owner.bytes[0x131] = Some(0xff);
        owner.bytes[0x136] = Some(0xff);
        for start in [0x138, 0x158, 0x178, 0x198] {
            owner.word_store(start, 0);
            owner.word_store(start + 4, 0);
            owner.half_store(start + 8, 0);
            owner.bytes[start + 10] = Some(2);
            owner.word_store(start + 12, 1f32.to_bits());
            for delta in [16, 20, 24, 28] {
                owner.word_store(start + delta, u32::MAX);
            }
        }
        owner
    }
    fn word_store(&mut self, at: usize, bits: u32) {
        for (index, byte) in bits.to_be_bytes().into_iter().enumerate() {
            self.bytes[at + index] = Some(byte);
        }
    }
    fn half_store(&mut self, at: usize, bits: u16) {
        for (index, byte) in bits.to_be_bytes().into_iter().enumerate() {
            self.bytes[at + index] = Some(byte);
        }
    }
    pub fn player(&self) -> PlayerId {
        self.player
    }
    pub fn byte(&self, at: usize) -> Option<u8> {
        self.bytes.get(at).copied().flatten()
    }
    pub fn word(&self, at: usize) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.byte(at)?,
            self.byte(at.checked_add(1)?)?,
            self.byte(at.checked_add(2)?)?,
            self.byte(at.checked_add(3)?)?,
        ]))
    }
    pub fn flag_8f_known_bits(&self) -> (u8, u8) {
        (self.flag_8f_known_mask, 0)
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.player.get() >> 32) as u32);
        emit(self.player.get() as u32);
        // Semantic Animator class instead of the original vtable pointer.
        emit(1);
        emit(u32::from(self.flag_8f_known_mask));
        emit(0);
        for (at, byte) in self.bytes.iter().enumerate() {
            emit(at as u32);
            emit(u32::from(byte.is_some()));
            if let Some(byte) = byte {
                emit(u32::from(*byte));
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/animator.rs"]
mod tests;
