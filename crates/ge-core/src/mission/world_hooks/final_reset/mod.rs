//! The final bounded world hook bodies. These scoped owners do not publish
//! loader/user readiness or implement the installed callback's later behavior.
mod binding;
mod buffers;
mod leaf;
mod two_buffers;

pub use binding::{Binding, BindingId, BindingRelease, BindingSlot, BindingWrite};
pub use buffers::{BufferClear, Buffers, BuffersReset, TailFields};
pub use leaf::{Leaf, LeafReset};
pub use two_buffers::{TwoBuffers, TwoBuffersReset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Noop {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoopReached {
    pub hook: Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledCallback {
    WorldTraversal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackSlot {
    Unknown,
    Installed(InstalledCallback),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackInstalled {
    pub callback: InstalledCallback,
}

/// Constructor-compatible independently owned state retained across attempts.
/// Original global consumer/buffer history is not established by construction.
#[derive(Debug)]
pub struct Owners {
    leaf: Leaf,
    buffers: Buffers,
    two: TwoBuffers,
    callback: CallbackSlot,
    noops: Vec<NoopReached>,
}

impl Owners {
    pub fn fresh() -> Self {
        Self {
            leaf: Leaf::fresh(),
            buffers: Buffers::fresh(),
            two: TwoBuffers::fresh(),
            callback: CallbackSlot::Unknown,
            noops: Vec::new(),
        }
    }
    pub fn leaf(&self) -> &Leaf {
        &self.leaf
    }
    pub fn leaf_mut(&mut self) -> &mut Leaf {
        &mut self.leaf
    }
    pub fn buffers(&self) -> &Buffers {
        &self.buffers
    }
    pub fn buffers_mut(&mut self) -> &mut Buffers {
        &mut self.buffers
    }
    pub fn two_buffers(&self) -> &TwoBuffers {
        &self.two
    }
    pub fn two_buffers_mut(&mut self) -> &mut TwoBuffers {
        &mut self.two
    }
    pub fn callback(&self) -> CallbackSlot {
        self.callback
    }
    pub fn noop_reaches(&self) -> &[NoopReached] {
        &self.noops
    }

    /// Clear per-operation diagnostics only. Source buffers, fields, binding
    /// pool/link and installed callback survive attempt boundaries unchanged.
    pub fn begin_operation(&mut self) {
        self.noops.clear();
        self.buffers.clear_receipt();
    }

    pub fn reach_noop(&mut self, hook: Noop) -> NoopReached {
        let receipt = NoopReached { hook };
        self.noops.push(receipt);
        receipt
    }
    pub fn reset_leaf(&mut self) -> LeafReset {
        self.leaf.reset()
    }
    pub fn reset_buffers(&mut self) -> anyhow::Result<BuffersReset> {
        self.buffers.reset()
    }
    pub fn reset_two_buffers(&mut self) -> TwoBuffersReset {
        self.two.reset()
    }
    /// Overwrite the typed callback slot only. No invocation API is provided.
    pub fn install_callback(&mut self) -> CallbackInstalled {
        self.callback = CallbackSlot::Installed(InstalledCallback::WorldTraversal);
        CallbackInstalled {
            callback: InstalledCallback::WorldTraversal,
        }
    }
}

#[cfg(test)]
mod tests;
