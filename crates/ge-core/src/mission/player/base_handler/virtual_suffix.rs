//! Independent 80100e04..3c suffix after real physics activation. The reached
//! deferred physics scalar is not read by these source operations.
use super::{Failure, Owner, Stage};
use crate::mission::player::{
    identity::{ItemId, PlayerId},
    item::{Item, Registration},
    physics,
};
use anyhow::{Context, Result, ensure};

pub trait VirtualSessionProvider {
    /// Actual 801ad470 lookup. False is a known NULL result, not an unavailable
    /// provider, unknown descriptor or an invented zero-user session.
    fn session_present(&mut self) -> Result<bool>;
    /// Actual 801e7830(previously resolved session, selector0). For the active
    /// session this iterates retained manager users and counts records with
    /// state+44 == 2. This is NOT descriptor.local_user_count or mode+2c.
    /// The nonactive fallback reads descriptor+34. Unknown inputs must error.
    fn session_user_count(&mut self) -> Result<u32>;
}

/// Proof only that the source PlayerBasic initializer returned literal one.
/// It is not the later PlayerNew/TP-FP/component/completion/camera initializer.
#[derive(Debug)]
pub struct VirtualBasicReceipt {
    player: PlayerId,
    item: ItemId,
}
impl VirtualBasicReceipt {
    pub const fn player(&self) -> PlayerId {
        self.player
    }
    pub const fn item(&self) -> ItemId {
        self.item
    }
}

impl Owner {
    pub fn finish_virtual_basic(
        &mut self,
        item: &mut Item,
        physics: &physics::Owner,
        provider: &mut impl VirtualSessionProvider,
    ) -> Result<VirtualBasicReceipt> {
        self.require(Stage::VirtualMotionComplete)?;
        ensure!(
            self.item == Some(item.id()) && item.id().world() == item.world(),
            "virtual basic suffix item differs from actual player backlink"
        );
        ensure!(
            item.handler() == Some(self.player)
                && item.factory().is_some()
                && item.registration() == Registration::Complete,
            "virtual basic suffix requires the registered actual handler item"
        );
        ensure!(
            item.physics().is_some()
                && physics.item_identity() == Some(item.id())
                && physics.item() == Some(item.id().get()),
            "virtual basic suffix physics backlink is not the same typed item"
        );
        ensure!(
            item.membership_mask() & 0x10 != 0
                && physics.byte(0x1ff) == Some(1)
                && physics.word(0x180) == Some(0x4000_0000)
                && physics.half(8).is_some_and(|flags| flags & 0x4000 != 0),
            "virtual basic suffix requires actual physics activation and suffix stores"
        );

        self.stage = Stage::VirtualBasicSuffix;
        self.w(0x574, 0);
        let present = match provider.session_present() {
            Ok(present) => present,
            Err(error) => {
                self.failure = Some(Failure::VirtualSession);
                return Err(error).context("virtual basic session lookup failed");
            }
        };
        if present {
            let count = match provider.session_user_count() {
                Ok(count) => count,
                Err(error) => {
                    self.failure = Some(Failure::VirtualSession);
                    return Err(error).context("virtual basic source user count unavailable");
                }
            };
            // 80100e1c is a signed cmpwi; preserve high-bit raw count behavior.
            if (count as i32) > 1 {
                item.mark_multiple_local_users();
            }
        }
        // Byte store at 80100e38, not a whole word write to 618..61b.
        self.b(0x618, 1);
        self.stage = Stage::VirtualBasicComplete;
        Ok(VirtualBasicReceipt {
            player: self.player,
            item: item.id(),
        })
    }
}

#[cfg(test)]
#[path = "tests/virtual_suffix.rs"]
mod tests;
