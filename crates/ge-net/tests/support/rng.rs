//! A small seeded generator for tests. Not for gameplay or security.

use ge_net::EntropySource;

#[derive(Clone, Debug)]
pub struct SeededRng(u64);

impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..bound`; zero when `bound` is zero.
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 { 0 } else { self.next() % bound }
    }

    /// True with probability `per_mille / 1000`.
    pub fn chance(&mut self, per_mille: u32) -> bool {
        self.below(1_000) < u64::from(per_mille)
    }

    pub fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte = self.next() as u8;
        }
    }
}

impl EntropySource for SeededRng {
    fn next_u64(&mut self) -> u64 {
        self.next()
    }
}
