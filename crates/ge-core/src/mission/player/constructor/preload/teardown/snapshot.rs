//! Bounded evidence captured before any destructor effect. It holds identities,
//! counts and a digest of the owner's state words, never payload or poses.
use crate::{
    animation::layers::NodeId,
    mission::player::components::{Identity, ResourceBinding, temporary},
};
use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    temporary: Identity,
    source_receiver: Identity,
    component: ResourceBinding,
    skin: ResourceBinding,
    bind_return: Option<u32>,
    head: Option<NodeId>,
    factory_node: Option<NodeId>,
    bones: usize,
    auxiliary_words: usize,
    block_bytes: usize,
    first_evaluation: Option<(u64, u64)>,
    state_words: usize,
    state_digest: u64,
}
impl Snapshot {
    pub(super) fn capture(temporary: &temporary::Pending) -> Result<Self> {
        let skeleton = temporary
            .animator()
            .skeleton()
            .context("temporary destructor snapshot requires its skeleton")?;
        // FNV-1a over the owner's own state words (host evidence, not native).
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        let mut state_words = 0;
        temporary.visit_state_words(|word| {
            state_words += 1;
            for byte in word.to_be_bytes() {
                digest = (digest ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        });
        Ok(Self {
            temporary: temporary.animator().identity(),
            source_receiver: temporary.source_receiver(),
            component: temporary.component(),
            skin: temporary.skin(),
            bind_return: temporary.bind_return(),
            head: temporary.head(),
            factory_node: temporary.factory_node(),
            bones: skeleton.skeleton().bones().len(),
            auxiliary_words: skeleton.auxiliary().len(),
            block_bytes: skeleton.native_storage_bytes(),
            first_evaluation: temporary
                .evaluation()
                .and_then(|e| e.ordinal().zip(e.allocation_ordinal())),
            state_words,
            state_digest: digest,
        })
    }
    pub fn temporary(&self) -> Identity {
        self.temporary
    }
    pub fn source_receiver(&self) -> Identity {
        self.source_receiver
    }
    pub fn component(&self) -> ResourceBinding {
        self.component
    }
    pub fn skin(&self) -> ResourceBinding {
        self.skin
    }
    pub fn bind_return(&self) -> Option<u32> {
        self.bind_return
    }
    pub fn head(&self) -> Option<NodeId> {
        self.head
    }
    pub fn factory_node(&self) -> Option<NodeId> {
        self.factory_node
    }
    pub fn bones(&self) -> usize {
        self.bones
    }
    pub fn auxiliary_words(&self) -> usize {
        self.auxiliary_words
    }
    pub fn block_bytes(&self) -> usize {
        self.block_bytes
    }
    /// The bind's own evaluation (ordinal, allocation), if it reserved one.
    pub fn first_evaluation(&self) -> Option<(u64, u64)> {
        self.first_evaluation
    }
    pub fn state_words(&self) -> usize {
        self.state_words
    }
    pub fn state_digest(&self) -> u64 {
        self.state_digest
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        for identity in [self.temporary, self.source_receiver] {
            identity.visit_state_words(&mut *emit);
        }
        for binding in [self.component, self.skin] {
            emit(binding.file);
            emit(binding.reference);
        }
        emit(self.bind_return.unwrap_or(0));
        for node in [self.head, self.factory_node] {
            let value = node.map_or(0, NodeId::get);
            emit((value >> 32) as u32);
            emit(value as u32);
        }
        for count in [
            self.bones,
            self.auxiliary_words,
            self.block_bytes,
            self.state_words,
        ] {
            emit(count as u32);
        }
        let (ordinal, allocation) = self.first_evaluation.unwrap_or((0, 0));
        for value in [ordinal, allocation, self.state_digest] {
            emit((value >> 32) as u32);
            emit(value as u32);
        }
    }
}
