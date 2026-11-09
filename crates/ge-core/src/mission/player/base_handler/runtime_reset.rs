//! Ordered 80100654..80100854 continuation, for the actually absent +610/+4f8
//! branch. This closes PlayerBasic reset only, before PlayerNew's C74 clear.
use super::{Binding, Failure, Owner, Stage};
use anyhow::{Context, Result};

pub trait RuntimeResetProvider {
    /// Actual shared float[r13-5508] write at 80100684.
    fn reset_runtime_global_float(&mut self, bits: u32) -> Result<()>;
    /// First 801ad470 lookup: no descriptor fields are read when absent.
    fn session_descriptor_present(&mut self) -> Result<bool>;
    /// Second 801ad470 lookup and reached descriptor+2c read. Failure or a
    /// disappeared descriptor must be an error, never a fabricated mode.
    fn session_descriptor_mode(&mut self) -> Result<u32>;
    /// Actual shared byte[r13-5503] write at 80100834.
    fn set_runtime_multiplayer_byte(&mut self, value: u8) -> Result<()>;
}

impl Owner {
    pub fn finish_runtime_reset(&mut self, provider: &mut impl RuntimeResetProvider) -> Result<()> {
        self.require(Stage::RuntimeResetPrefix)?;
        self.stage = Stage::RuntimeResetTail;
        if self.word(0x610) != Some(0) {
            self.failure = Some(Failure::RuntimeDependency);
            anyhow::bail!(
                "runtime reset requires actual absent +610 backing; release provider unavailable"
            );
        }
        // +608 growth and +614 semantic binding are deliberately preserved.
        self.w(0x610, 0);
        self.w(0x604, 0);
        self.w(0x60c, 0);
        self.w(0x708, 1);
        self.b(0x57c, 1);
        if let Err(error) = provider.reset_runtime_global_float(0) {
            self.failure = Some(Failure::RuntimeGlobalFloat);
            return Err(error).context("runtime shared float reset failed");
        }
        // Semantic self identity is already retained by Owner::player, rather
        // than being truncated into a native-looking u32 pointer.
        self.bindings.insert(0x86c, Binding::SelfPlayer);
        self.words.remove(&0x86c);
        self.w(0x6e8, 0);
        if self.word(0x4f8) != Some(0) {
            self.failure = Some(Failure::RuntimeDependency);
            anyhow::bail!(
                "runtime reset requires actual absent embedded interface; notification provider unavailable"
            );
        }
        // Each source store is followed by its own optional interface callback;
        // the supported owned-NULL interface skips all of those callbacks.
        for offset in [0x6ec, 0x6f0, 0x6f4, 0x6f8] {
            self.w(offset, 0);
        }
        self.h(0x6fe, 0);
        self.h(0x6fc, 0);
        self.b(0x700, 0);
        self.b(0x5ec, 0);
        // +5e8 may contain any float, including NaN. With a proven absent
        // interface its comparison only controls a skipped notification.
        self.w(0x5e8, 0);
        self.w(0x954, u32::MAX);
        self.w(0x958, u32::MAX);
        // Supplied DATA r2-6608 = 40800000; r2-6610 = positive zero.
        self.w(0x74c, 0x4080_0000);
        self.w(0x748, 0x4080_0000);
        let present = match provider.session_descriptor_present() {
            Ok(value) => value,
            Err(error) => {
                self.failure = Some(Failure::RuntimeDescriptor);
                return Err(error).context("runtime session descriptor lookup failed");
            }
        };
        let multiplayer = if present {
            match provider.session_descriptor_mode() {
                Ok(mode) => u8::from(mode != 1),
                Err(error) => {
                    self.failure = Some(Failure::RuntimeDescriptor);
                    return Err(error).context("runtime second session descriptor lookup failed");
                }
            }
        } else {
            0
        };
        if let Err(error) = provider.set_runtime_multiplayer_byte(multiplayer) {
            self.failure = Some(Failure::RuntimeGlobalByte);
            return Err(error).context("runtime multiplayer byte write failed");
        }
        self.w(0x960, 0);
        self.stage = Stage::RuntimeResetComplete;
        Ok(())
    }
}
