use super::{Binding, Failure, Owner, Provider, Stage};
use anyhow::{Context, Result};

impl Owner {
    /// 80331c30 -> 80284880 -> 80281990, preserving the shared RNG draw point.
    pub fn initialize_generic(&mut self, provider: &mut impl Provider) -> Result<()> {
        self.require(Stage::Fresh)?;
        self.stage = Stage::GenericPrefix;
        self.bindings.insert(0, Binding::Generic);
        self.w(4, 0);
        for (header, vtable) in [(0x38, 0x48), (0x4c, 0x5c), (0x60, 0x70)] {
            self.w(header, 0);
            self.w(header + 4, 16);
            self.w(header + 8, 0);
            self.w(header + 12, 0);
            self.bindings.insert(vtable, Binding::Container);
        }
        self.zero_bytes(0x7d, 0x7f);
        self.b(0x7c, 1);
        self.w(0x74, 0);
        self.w(0x78, 0);
        // Complete nested 80284880 constructor, including its two explicit
        // memset spans. Unwritten 0x1ac and padding remain unknown.
        self.zero_words(0x98, 0x164);
        for offset in [0x168, 0x16c, 0x170, 0x178] {
            self.w(offset, 0x3f00_0000);
        }
        self.w(0x174, 0x3f80_0000);
        self.w(0x17c, 0x3da3_d70a);
        self.w(0x180, 0);
        self.w(0x184, 0);
        self.zero_words(0x188, 0x1a8);
        self.zero_bytes(0x1b0, 0x1b1);
        self.w(0x1b4, 0);
        self.w(0x1b8, 0);
        for offset in [0x220, 0x228, 0x22c, 0x2a0, 0x2f0, 0x340, 0x390] {
            self.w(offset, 0);
        }
        self.w(0x1bc, 0x1ff);
        self.w(0x1c0, 0);
        self.w(0x1c4, 0);
        let seed = match provider.draw_handler_seed() {
            Ok(seed) => seed,
            Err(error) => {
                self.failure = Some(Failure::RandomSeed);
                return Err(error).context("shared handler RNG draw failed");
            }
        };
        self.w(0x1c8, u32::from(seed));
        self.w(0x1cc, 0);
        self.zero_words(0x1d0, 0x210);
        self.w(0x220, 0x17);
        for (start, end) in [(0x98, 0xa4), (0xb8, 0x104)] {
            self.zero_words(start, end);
        }
        self.zero_words(0x1b4, 0x1b8);
        self.b(0x1b0, 0);
        for offset in [0x80, 0x84, 0x88, 0x8c, 0x90] {
            self.w(offset, 0x3f80_0000);
        }
        self.w(0x230, 0);
        self.b(0x3a4, 0);
        self.w(0x94, 0);
        self.w(0x214, 0x7f7f_ffff);
        self.w(0x218, 0xff7f_ffff);
        self.w(0x21c, 0xff7f_ffff);
        self.stage = Stage::GenericComplete;
        Ok(())
    }
}
