use super::{BasicReceipt, Binding, Owner, Stage};
use anyhow::{Result, ensure};

impl Owner {
    /// 800b4c30, with its actual installed-base virtual+c0/c4 simple setters.
    pub fn initialize_basic(&mut self) -> Result<()> {
        self.require(Stage::GenericComplete)?;
        self.bindings.insert(0, Binding::Basic);
        self.container(0x474, 16);
        self.container(0x48c, 16);
        for offset in [0x4b4, 0x4bc, 0x4ac, 0x470] {
            self.w(offset, 0);
        }
        self.w(0x4b8, 0xc47a_0000);
        for offset in (0x4c0..0x4e4).step_by(12) {
            self.w(offset, 0);
            self.w(offset + 4, 0xc47a_0000);
            self.w(offset + 8, 0);
        }
        self.b(0x4e5, 1);
        self.b(0x4e6, 0);
        self.w(0x3ac, 0x42c8_0000);
        self.w(0x3a8, 0x42c8_0000);
        for offset in [0x3d8, 0x404, 0x430, 0x45c] {
            self.b(offset, 0);
        }
        self.zero_words(0x464, 0x46c);
        self.stage = Stage::BaseComplete;
        Ok(())
    }

    /// 801001f0 up to its call of 801002f0. The nested +770 constructor's
    /// pre-read +ec is +85c, explicitly zeroed by this immediate caller.
    pub fn initialize_player_basic_prefix(&mut self) -> Result<()> {
        self.require(Stage::BaseComplete)?;
        self.bindings.insert(0x4e8, Binding::EmbeddedInterface);
        self.w(0x4ec, 0);
        self.b(0x4f0, 0);
        self.h(0x4f2, u16::MAX);
        self.b(0x4f4, 0);
        self.w(0x4f8, 0);
        self.bindings.insert(0, Binding::PlayerBasic);
        self.bindings.insert(0x4fc, Binding::EmbeddedInterface);
        for (offset, table) in [(0x604, 0x614), (0x850, 0x860)] {
            for delta in [0, 4, 8, 12] {
                self.w(offset + delta, 0);
            }
            self.bindings.insert(table, Binding::NestedContainer);
        }
        self.w(0x764, 0);
        ensure!(
            self.word(0x85c) == Some(0),
            "nested constructor prior backing is not the owned absent value"
        );
        self.initialize_nested_770();
        self.w(0x990, 0);
        self.stage = Stage::PlayerBasicPrefix;
        Ok(())
    }

    fn initialize_nested_770(&mut self) {
        let base = 0x770;
        // 802789e0..80278cb0. Holes +b4,+e4,+f0 and byte padding are not
        // written. Prior +ec=0 skips only the actual conditional free call.
        self.zero_words(base, base + 0x5c);
        self.w(base + 0x60, 0x4974_2400);
        self.zero_words(base + 0x64, base + 0xa4);
        self.zero_bytes(base + 0xa8, base + 0xab);
        for offset in [
            0xac, 0xb0, 0xb8, 0xbc, 0xc0, 0xc4, 0xc8, 0xcc, 0xd0, 0xd4, 0xd8, 0xdc, 0xe0, 0xe8,
            0xec,
        ] {
            self.w(base + offset, 0);
        }
        self.zero_bytes(base + 0xf4, base + 0xfa);
        self.zero_bytes(base + 0x100, base + 0x131);
        self.zero_words(base + 0x134, base + 0x1e0);
        self.nested_initialized = true;
    }

    /// The initial 801001f0 return tail. This proof is issued once, after the
    /// first real reset; later 80100610 uses finish_reset_prefix instead.
    pub fn finish_basic(&mut self) -> Result<BasicReceipt> {
        self.require(Stage::ResetComplete)?;
        ensure!(
            self.completed_resets == 1,
            "initial basic constructor already finished"
        );
        self.zero_bytes(0x995, 0x996);
        self.w(0x4ec, 4);
        self.w(0x998, u32::MAX);
        // Embedded completion pointer remains genuinely absent from its ctor.
        ensure!(
            self.word(0x4f8) == Some(0),
            "completion notification requires an actual provider"
        );
        for offset in [0x570, 0x988, 0x98c] {
            self.w(offset, 0);
        }
        self.b(0x964, 0);
        self.stage = Stage::BasicComplete;
        Ok(BasicReceipt {
            player: self.player,
        })
    }

    /// Only 8010062c..54, after a later real 801002f0 invocation. Continue with
    /// finish_runtime_reset before the derived PlayerNew reset may begin.
    pub fn finish_reset_prefix(&mut self) -> Result<()> {
        self.require(Stage::ResetComplete)?;
        ensure!(
            self.completed_resets > 1,
            "runtime reset prefix requires the later invocation"
        );
        self.w(0x4ec, 4);
        self.w(0x998, u32::MAX);
        ensure!(
            self.word(0x4f8) == Some(0),
            "completion notification requires an actual provider"
        );
        self.stage = Stage::RuntimeResetPrefix;
        Ok(())
    }
}
