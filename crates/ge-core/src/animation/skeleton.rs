//! Bone membership and constructor values from explicit checked skin inputs.
//!
//! Storage order determines root and sibling order. This supported constructor
//! requires parents to precede their children: the source initializes each
//! bone's child list immediately before inserting it into its parent's list.
//! A forward parent would address an uninitialized list, not a repaired tree.
//!
//! Four source range words are retained literally; they are not reconstructed
//! from the parent tree. Auxiliary skin words remain uninterpreted. Animation
//! channels, auxiliary runtime arrays and matrix rebuild are separate. A local
//! commit copies supplied prepared T/Q only; it establishes no camera readiness.
use anyhow::{Result, ensure};

pub const ROOT_PARENT: u16 = u16::MAX;
pub const SUPPORTED_BONES: usize = 128;
const ONE: u32 = 0x3f80_0000;
const CONSTRUCTOR_VECTOR: [u32; 4] = [0, 0, 0, ONE];
const IDENTITY: [u32; 16] = [ONE, 0, 0, 0, 0, ONE, 0, 0, 0, 0, ONE, 0, 0, 0, 0, ONE];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoneInput {
    pub parent: u16,
    pub range_end: u16,
    /// Hierarchy record +4 and +6, preserved without invented semantics.
    pub auxiliary_raw: [u16; 2],
}

/// Dense prepared bone locals. These are not raw sparse decoder channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalPose {
    pub translation_bits: [u32; 4],
    pub quaternion_bits: [u32; 4],
}

impl LocalPose {
    pub(crate) fn finite(self) -> bool {
        self.translation_bits
            .into_iter()
            .chain(self.quaternion_bits)
            .all(|bits| f32::from_bits(bits).is_finite())
    }
}

/// Raw primary-runtime cache bytes. Their matrix consumers remain separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFlags {
    pub byte_10_raw: u8,
    pub byte_0e_raw: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub bones: usize,
    /// Visits while validating the parent graph, separate from later queries.
    /// Each newly reached bone consumes one visit; shared ancestry is reused.
    pub parent_visits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bone {
    index: u16,
    input: BoneInput,
    children: Vec<u16>,
    range_bits: [u32; 4],
    local_translation_bits: [u32; 4],
    local_quaternion_bits: [u32; 4],
    matrix_bits: [u32; 16],
}

impl Bone {
    pub fn index(&self) -> u16 {
        self.index
    }

    pub fn parent_raw(&self) -> u16 {
        self.input.parent
    }

    pub fn range_end(&self) -> u16 {
        self.input.range_end
    }

    pub fn auxiliary_raw(&self) -> [u16; 2] {
        self.input.auxiliary_raw
    }

    pub fn children(&self) -> &[u16] {
        &self.children
    }

    /// Native bone +0x38..+0x44, with bit zero at the low bit of word zero.
    /// These raw words can include bits beyond the actual bone count.
    pub fn range_bits(&self) -> [u32; 4] {
        self.range_bits
    }

    /// Positive-zero XYZ and W=1, not a skin rest pose or sampled frame.
    pub fn local_translation_bits(&self) -> [u32; 4] {
        self.local_translation_bits
    }

    pub fn local_quaternion_bits(&self) -> [u32; 4] {
        self.local_quaternion_bits
    }

    pub fn matrix_bits(&self) -> [u32; 16] {
        self.matrix_bits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    bones: Vec<Bone>,
    roots: Vec<u16>,
    runtime_flags: RuntimeFlags,
}

impl Skeleton {
    pub fn initialize(inputs: &[BoneInput], budget: Budget) -> Result<Self> {
        ensure!(
            inputs.len() <= budget.bones,
            "skeleton bone budget exhausted"
        );
        ensure!(
            inputs.len() <= SUPPORTED_BONES,
            "skeleton exceeds supported four-word range representation (128 bones); this is a host bound"
        );
        for (index, input) in inputs.iter().enumerate() {
            ensure!(
                input.parent == ROOT_PARENT || usize::from(input.parent) < inputs.len(),
                "skeleton bone {index} parent is out of range"
            );
        }
        validate_parent_graph(inputs, budget.parent_visits)?;
        for (index, input) in inputs.iter().enumerate() {
            ensure!(
                input.parent == ROOT_PARENT || usize::from(input.parent) < index,
                "skeleton bone {index} has an unsupported forward parent; source parent lists must already be initialized"
            );
        }

        let mut bones: Vec<Bone> = Vec::with_capacity(inputs.len());
        let mut roots = Vec::new();
        for (index, &input) in inputs.iter().enumerate() {
            let index = u16::try_from(index)?;
            bones.push(Bone {
                index,
                input,
                children: Vec::new(),
                range_bits: source_range_bits(index, input.range_end),
                local_translation_bits: CONSTRUCTOR_VECTOR,
                local_quaternion_bits: CONSTRUCTOR_VECTOR,
                matrix_bits: IDENTITY,
            });
            if input.parent == ROOT_PARENT {
                roots.push(index);
            } else {
                bones[usize::from(input.parent)].children.push(index);
            }
        }
        Ok(Self {
            bones,
            roots,
            runtime_flags: RuntimeFlags {
                byte_10_raw: 0,
                byte_0e_raw: 0,
            },
        })
    }

    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    pub fn roots(&self) -> &[u16] {
        &self.roots
    }

    pub fn runtime_flags(&self) -> RuntimeFlags {
        self.runtime_flags
    }

    /// Accept an explicit current primary-runtime snapshot, never inferred flags.
    pub fn set_runtime_flags(&mut self, flags: RuntimeFlags) {
        self.runtime_flags = flags;
    }

    /// Supported zero-auxiliary, unlinked-runtime T/Q commit. The caller owns
    /// the successful sample gate. A supplied mask represents an active native
    /// LOD mask; None copies all bones. Disabled values stay unread. Preflight
    /// errors leave locals and cache flags unchanged (checked-host atomicity).
    /// Matrices are deliberately not rebuilt or declared current.
    pub fn commit_local_pose(
        &mut self,
        locals: &[LocalPose],
        mask: Option<[u32; 4]>,
        bone_budget: usize,
    ) -> Result<usize> {
        ensure!(
            locals.len() == self.bones.len(),
            "local pose count differs from skeleton"
        );
        let enabled =
            |index: usize| mask.is_none_or(|words| words[index / 32] & (1 << (index % 32)) != 0);
        let copies = (0..self.bones.len())
            .filter(|&index| enabled(index))
            .count();
        ensure!(
            copies <= bone_budget,
            "skeleton local commit budget exhausted"
        );
        for (index, local) in locals.iter().enumerate() {
            if enabled(index) {
                ensure!(
                    local.finite(),
                    "unsupported nonfinite committed bone locals"
                );
            }
        }
        for (index, (bone, local)) in self.bones.iter_mut().zip(locals).enumerate() {
            if enabled(index) {
                bone.local_translation_bits = local.translation_bits;
                bone.local_quaternion_bits = local.quaternion_bits;
            }
        }
        // Source clears +10 and then +0e on this primary runtime. Linked owners
        // and auxiliary arrays do not exist in this explicit supported subset.
        self.runtime_flags.byte_10_raw = 0;
        self.runtime_flags.byte_0e_raw = 0;
        Ok(copies)
    }

    /// Queried bone first, followed by its parents through the root. Each
    /// returned bone consumes one visit, including the initial bone and root.
    /// No partial chain is returned when the explicit host budget is exhausted.
    pub fn parent_chain(&self, index: usize, budget: usize) -> Result<Vec<u16>> {
        ensure!(
            index < self.bones.len(),
            "skeleton chain index is out of range"
        );
        let mut chain = Vec::new();
        let mut next = Some(index);
        while let Some(index) = next {
            ensure!(
                chain.len() < budget,
                "skeleton parent-chain budget exhausted"
            );
            let bone = &self.bones[index];
            chain.push(bone.index);
            next = (bone.input.parent != ROOT_PARENT).then_some(usize::from(bone.input.parent));
        }
        Ok(chain)
    }
}

fn validate_parent_graph(inputs: &[BoneInput], budget: usize) -> Result<()> {
    let mut colors = vec![0u8; inputs.len()];
    let mut visits = 0;
    for start in 0..inputs.len() {
        let mut next = Some(start);
        let mut path = Vec::new();
        while let Some(index) = next {
            if colors[index] == 2 {
                break;
            }
            ensure!(colors[index] != 1, "skeleton parent graph contains a cycle");
            ensure!(
                visits < budget,
                "skeleton parent-validation budget exhausted"
            );
            visits += 1;
            colors[index] = 1;
            path.push(index);
            next =
                (inputs[index].parent != ROOT_PARENT).then_some(usize::from(inputs[index].parent));
        }
        for index in path {
            colors[index] = 2;
        }
    }
    Ok(())
}

fn source_range_bits(index: u16, end: u16) -> [u32; 4] {
    let mut words = [0; 4];
    if index == end {
        words[usize::from(index / 32)] = 1 << (index % 32);
        return words;
    }
    for (ordinal, word) in words.iter_mut().enumerate() {
        let base = i32::try_from(ordinal).expect("four range words") * 32;
        *word = low_mask(i32::from(index) - base) ^ low_mask(i32::from(end) - base);
    }
    words
}

fn low_mask(bits: i32) -> u32 {
    if bits <= 0 {
        0
    } else if bits >= 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}

#[cfg(test)]
#[path = "tests/skeleton.rs"]
mod tests;
