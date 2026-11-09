use crate::mission::{
    world::InstanceId,
    world_hooks::buffer::{Buffer, ClearStats},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSlot {
    Unknown,
    Absent,
    Bound(InstanceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimatorFields {
    pub word: Option<u32>,
    pub byte: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreResetReceipt {
    pub context_writes: usize,
    pub animator_word_writes: usize,
    pub animator_byte_writes: usize,
    pub world_buffers: [ClearStats; 2],
}

/// Sparse resource/animator state and the two actual world-owned backings.
/// Constructor does not establish original global or prior level history.
pub struct Owner {
    contexts: [ContextSlot; 4],
    animator: AnimatorFields,
    world_buffers: [Buffer; 2],
}

impl Owner {
    pub fn from_constructor() -> Self {
        Self {
            // The resource constructor leaves these four leading words unwritten.
            contexts: [ContextSlot::Unknown; 4],
            animator: AnimatorFields {
                word: None,
                byte: None,
            },
            // Actual world constructor: D8 growth0, EC growth4.
            world_buffers: [Buffer::empty(0), Buffer::empty(4)],
        }
    }

    pub fn reset_contexts(&mut self) -> usize {
        self.contexts.fill(ContextSlot::Absent);
        4
    }

    pub fn reset_animator(&mut self) -> AnimatorFields {
        self.animator = AnimatorFields {
            word: Some(0),
            byte: Some(0),
        };
        self.animator
    }

    pub fn reset_world_buffers(&mut self) -> [ClearStats; 2] {
        let first = self.world_buffers[0].clear_backing();
        self.world_buffers[0].set_growth(16);
        let second = self.world_buffers[1].clear_backing();
        self.world_buffers[1].set_growth(16);
        [first, second]
    }

    pub fn run_pre_hooks(&mut self) -> PreResetReceipt {
        let context_writes = self.reset_contexts();
        self.reset_animator();
        let world_buffers = self.reset_world_buffers();
        PreResetReceipt {
            context_writes,
            animator_word_writes: 1,
            animator_byte_writes: 1,
            world_buffers,
        }
    }

    pub fn contexts(&self) -> &[ContextSlot; 4] {
        &self.contexts
    }
    pub fn context_mut(&mut self, index: usize) -> Option<&mut ContextSlot> {
        self.contexts.get_mut(index)
    }
    pub fn animator(&self) -> AnimatorFields {
        self.animator
    }
    pub fn world_buffers(&self) -> &[Buffer; 2] {
        &self.world_buffers
    }
    pub fn world_buffer_mut(&mut self, index: usize) -> Option<&mut Buffer> {
        self.world_buffers.get_mut(index)
    }
}
