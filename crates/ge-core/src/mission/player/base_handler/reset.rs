use super::{Binding, Failure, Owner, Provider, ResetReceipt, Stage};
use anyhow::{Context, Result, ensure};

impl Owner {
    /// Exactly the supported sparse 801002f0 reset. The explicit +328 input is
    /// a bound game-owner field snapshot, not clock time or a default zero.
    /// Repeated source invocations are retained; generic RNG is never redrawn.
    pub fn apply_reset(
        &mut self,
        game_field_328_bits: u32,
        provider: &mut impl Provider,
    ) -> Result<ResetReceipt> {
        ensure!(
            self.failure.is_none(),
            "base-handler retains a failed provider operation"
        );
        ensure!(
            matches!(
                self.stage,
                Stage::PlayerBasicPrefix | Stage::BasicComplete | Stage::RuntimeResetPrefix
            ),
            "base reset is out of source order"
        );
        let calls = self
            .reset_calls
            .checked_add(1)
            .context("base reset invocation count overflow")?;
        let completed = self
            .completed_resets
            .checked_add(1)
            .context("base reset completion count overflow")?;
        self.reset_calls = calls;
        self.stage = Stage::ResetPrefix;
        self.zero_words(0x500, 0x51c);
        for offset in [0x528, 0x534, 0x538, 0x52c, 0x53c] {
            self.w(offset, 0);
        }
        self.zero_bytes(0x530, 0x532);
        self.zero_words(0x540, 0x578);
        self.zero_bytes(0x57c, 0x57d);
        self.zero_words(0x580, 0x5ac);
        self.b(0x5b0, 0);
        self.zero_words(0x5b4, 0x5e0);
        self.zero_bytes(0x5ec, 0x5f2);
        for offset in [0x5f4, 0x960, 0x5fc, 0x600, 0x61c, 0x620] {
            self.w(offset, 0);
        }
        self.b(0x95c, 0);
        self.zero_bytes(0x624, 0x625);
        self.zero_words(0x628, 0x674);
        self.w(0x678, 0x3f80_0000);
        self.zero_words(0x67c, 0x6a8);
        self.w(0x6ac, u32::MAX);
        self.zero_words(0x6b0, 0x6d8);
        for offset in [
            0x6e0, 0x6e4, 0x6e8, 0x6ec, 0x6f0, 0x6f4, 0x6f8, 0x704, 0x708, 0x70c, 0x710, 0x714,
            0x718,
        ] {
            self.w(offset, 0);
        }
        self.h(0x6fc, 0);
        self.h(0x6fe, 0);
        self.b(0x700, 0);
        self.w(0x71c, game_field_328_bits);
        self.w(0x720, game_field_328_bits);
        self.zero_words(0x728, 0x750);
        self.w(0x760, u32::MAX);
        self.w(0x768, 0);
        self.zero_words(0x968, 0x984);
        self.b(0x724, 0);
        for offset in [0x754, 0x758, 0x75c, 0x954, 0x958, 0x988, 0x98c] {
            self.w(offset, 0);
        }
        self.b(0x964, 0);
        if let Err(error) = provider.reset_base_global_byte() {
            self.failure = Some(Failure::GlobalReset);
            return Err(error).context("shared base reset global write failed");
        }
        // Actual source empty-string constructor/reset; no asset text/pointer.
        self.reset_string = Some(String::new());
        self.words.remove(&0x990);
        self.bindings.insert(0x990, Binding::EmptyString);
        self.zero_bytes(0x994, 0x996);
        self.w(0x998, 0);
        self.w(0x5e8, 0);
        self.b(0x5e4, 0);
        self.w(0x5f8, 0);
        self.zero_bytes(0x701, 0x703);
        if let Err(error) = provider.reset_resource_state(0x4200_050b, 0) {
            self.failure = Some(Failure::ResourceReset);
            return Err(error).context("persistent base reset resource setter failed");
        }
        self.completed_resets = completed;
        self.stage = Stage::ResetComplete;
        Ok(ResetReceipt {
            player: self.player,
            invocation: completed,
        })
    }
}
