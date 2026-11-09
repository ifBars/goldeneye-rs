//! The real item backlink and literal base/motion prefix of 80100ad0.
//! Physics and later virtual initialization are separate reached operations.
use super::{Binding, Owner, Stage};
use crate::mission::player::item::{Item, Registration};
use anyhow::{Result, ensure};

impl Owner {
    /// 80100b04..20, after the actual wrapperless factory returned and the item
    /// retained its handler. The native pointer at +4 becomes a typed backlink.
    pub fn bind_item(&mut self, item: &Item) -> Result<()> {
        self.require(Stage::ConstructorFlagComplete)?;
        ensure!(self.item.is_none(), "player already has a linked item");
        ensure!(
            item.handler() == Some(self.player),
            "item handler differs from player owner"
        );
        ensure!(
            item.factory().is_some(),
            "item wrapperless factory is incomplete"
        );
        ensure!(
            item.id().world() == item.world(),
            "item identity differs from its actual world"
        );
        self.item = Some(item.id());
        self.words.remove(&4);
        self.bindings.insert(4, Binding::LinkedItem);
        self.stage = Stage::VirtualItemBound;
        Ok(())
    }

    /// 800b4d50's installed literal callback is 80008ba0 (return one), then
    /// 80100c14..84 writes these exact sparse motion defaults. Registration
    /// must already have completed through the actual actor-list operation.
    pub fn initialize_virtual_motion(&mut self, item: &Item) -> Result<()> {
        self.require(Stage::VirtualItemBound)?;
        ensure!(
            self.item == Some(item.id()),
            "virtual motion item is not the bound item"
        );
        ensure!(
            item.id().world() == item.world(),
            "virtual motion item world differs"
        );
        ensure!(
            item.handler() == Some(self.player),
            "virtual motion item handler differs"
        );
        ensure!(
            item.factory().is_some(),
            "virtual motion item factory is incomplete"
        );
        ensure!(
            item.registration() == Registration::Complete,
            "virtual motion requires complete actor registration"
        );

        // 800b4d80/88/8c; DATA806ee3e4 is positive zero. No provider callback
        // is needed for the installed base's source literal return-one leaf.
        self.b(0x4e4, 0);
        self.w(0x4a4, 0);
        self.b(0x4e7, 0);
        self.w(0x75c, 0);
        self.w(0x760, 0x4300_0000);
        self.w(0x758, 0);
        for offset in [
            0x628, 0x630, 0x62c, 0x634, 0x638, 0x644, 0x63c, 0x640, 0x658, 0x664, 0x65c, 0x660,
            0x648, 0x654, 0x64c, 0x650,
        ] {
            self.w(offset, 0);
        }
        self.w(0x678, 0x3f80_0000);
        self.w(0x67c, 0);
        self.w(0x680, 0);
        self.b(0x5e4, 0);
        self.stage = Stage::VirtualMotionComplete;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/virtual_init.rs"]
mod tests;
