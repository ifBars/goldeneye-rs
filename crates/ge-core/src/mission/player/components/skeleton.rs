//! Fresh skeletal runtime ownership reached by the TP bind. Constructor locals
//! and matrices are retained separately from sampling and hierarchy refresh.
use super::Identity;
use crate::animation::skeleton::{self, BoneInput, Skeleton};
use anyhow::{Context, Result, ensure};

pub const DEFAULT_SKIN: ResourceBinding = ResourceBinding {
    file: 0x010a_0056,
    reference: 0x0d00_0001,
};
pub const TP_COMPONENT: ResourceBinding = ResourceBinding {
    file: 0x0102_0017,
    reference: 0,
};

/// Semantic reference to the retained host wrapper. Directory ordinals are not
/// native file-cache generations and never stand in for that mutable cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBinding {
    pub file: u32,
    pub reference: u32,
}
pub struct SkeletonInput<'a> {
    pub primary: ResourceBinding,
    pub bones: &'a [BoneInput],
    pub auxiliary_words: usize,
    /// Captured at construction, before subtracting one. None keeps the reached
    /// engine counter read unresolved; a later counter cannot repair it.
    pub captured_frame_counter: Option<u32>,
}
#[derive(Debug, Clone, Copy)]
pub struct SkeletonBudget {
    pub bones: usize,
    pub parent_visits: usize,
    pub auxiliary_words: usize,
}
impl Default for SkeletonBudget {
    fn default() -> Self {
        Self {
            bones: skeleton::SUPPORTED_BONES,
            parent_visits: skeleton::SUPPORTED_BONES,
            auxiliary_words: 4096,
        }
    }
}
#[derive(Debug)]
pub struct SkeletonRuntime {
    primary: ResourceBinding,
    secondary: Option<ResourceBinding>,
    animator: Option<Identity>,
    skeleton: Skeleton,
    auxiliary: Vec<u32>,
    frame_stamp: Option<u32>,
    flags_0f: u8,
    aliases: [u8; 4],
    native_storage_bytes: usize,
}
impl SkeletonRuntime {
    pub(super) fn initialize(input: SkeletonInput<'_>, budget: SkeletonBudget) -> Result<Self> {
        ensure!(
            input.primary == DEFAULT_SKIN,
            "TP primary skeleton binding differs"
        );
        ensure!(
            input.auxiliary_words <= budget.auxiliary_words,
            "skeleton auxiliary budget exhausted"
        );
        let native_storage_bytes = input
            .bones
            .len()
            .checked_mul(0x88)
            .and_then(|bytes| {
                input
                    .auxiliary_words
                    .checked_mul(4)
                    .and_then(|aux| bytes.checked_add(aux))
            })
            .context("skeletal storage size overflow")?;
        let skeleton = Skeleton::initialize(
            input.bones,
            skeleton::Budget {
                bones: budget.bones,
                parent_visits: budget.parent_visits,
            },
        )?;
        let mut auxiliary = Vec::new();
        auxiliary
            .try_reserve_exact(input.auxiliary_words)
            .context("allocating bounded skeleton auxiliary words")?;
        auxiliary.resize(input.auxiliary_words, 0);
        Ok(Self {
            primary: input.primary,
            secondary: None,
            animator: None,
            skeleton,
            auxiliary,
            frame_stamp: input
                .captured_frame_counter
                .map(|value| value.wrapping_sub(1)),
            flags_0f: 0x0f,
            aliases: [3; 4],
            native_storage_bytes,
        })
    }
    pub(super) fn bind_animator(&mut self, identity: Identity) {
        self.animator = Some(identity);
    }
    pub(super) fn bind_secondary(&mut self, binding: ResourceBinding) {
        // Class0d bypasses animation-to-skin remapping. Slots58/68/78 are
        // constructor NULL, so the source alias walk retains fallback index3.
        self.secondary = Some(binding);
        self.flags_0f |= 8;
    }
    pub fn primary(&self) -> ResourceBinding {
        self.primary
    }
    pub fn secondary(&self) -> Option<ResourceBinding> {
        self.secondary
    }
    pub fn animator(&self) -> Option<Identity> {
        self.animator
    }
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }
    pub fn auxiliary(&self) -> &[u32] {
        &self.auxiliary
    }
    pub fn frame_stamp_bits(&self) -> Result<u32> {
        self.frame_stamp
            .context("skeletal constructor frame counter was unresolved")
    }
    pub fn native_storage_bytes(&self) -> usize {
        self.native_storage_bytes
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(2);
        for binding in [Some(self.primary), self.secondary] {
            emit(u32::from(binding.is_some()));
            if let Some(binding) = binding {
                emit(binding.file);
                emit(binding.reference);
            }
        }
        emit(u32::from(self.animator.is_some()));
        if let Some(identity) = self.animator {
            identity.visit_state_words(&mut emit);
        }
        emit(u32::from(self.frame_stamp.is_some()));
        if let Some(stamp) = self.frame_stamp {
            emit(stamp);
        }
        emit(u32::from(self.flags_0f));
        for alias in self.aliases {
            emit(u32::from(alias));
        }
        emit(self.native_storage_bytes as u32);
        emit(self.skeleton.bones().len() as u32);
        for bone in self.skeleton.bones() {
            emit(u32::from(bone.index()));
            emit(u32::from(bone.parent_raw()));
            emit(u32::from(bone.range_end()));
            for raw in bone.auxiliary_raw() {
                emit(u32::from(raw));
            }
            emit(bone.children().len() as u32);
            for child in bone.children() {
                emit(u32::from(*child));
            }
            for bits in bone
                .range_bits()
                .into_iter()
                .chain(bone.local_translation_bits())
                .chain(bone.local_quaternion_bits())
                .chain(bone.matrix_bits())
            {
                emit(bits);
            }
        }
        emit(self.skeleton.roots().len() as u32);
        for root in self.skeleton.roots() {
            emit(u32::from(*root));
        }
        let flags = self.skeleton.runtime_flags();
        emit(u32::from(flags.byte_10_raw));
        emit(u32::from(flags.byte_0e_raw));
        emit(self.auxiliary.len() as u32);
        for word in &self.auxiliary {
            emit(*word);
        }
    }
}
