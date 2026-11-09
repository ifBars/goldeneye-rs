//! Semantic owned handles. They never contain original addresses.
use anyhow::{Result, ensure};
use std::num::NonZeroU64;
macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);
        impl $name {
            pub fn new(value: u64) -> Result<Self> {
                ensure!(value != 0, "player owner identity must be nonzero");
                Ok(Self(NonZeroU64::new(value).expect("checked nonzero")))
            }
            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }
    };
}
identity!(WorldId);
identity!(PlayerId);
identity!(PhysicsId);
identity!(ComponentId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId {
    world: WorldId,
    ordinal: NonZeroU64,
}
impl ItemId {
    pub(super) fn new(world: WorldId, value: u64) -> Result<Self> {
        ensure!(value != 0, "item ordinal must be nonzero");
        Ok(Self {
            world,
            ordinal: NonZeroU64::new(value).expect("checked nonzero"),
        })
    }
    pub const fn get(self) -> u64 {
        self.ordinal.get()
    }
    pub const fn world(self) -> WorldId {
        self.world
    }
}
