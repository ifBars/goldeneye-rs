//! Cache-disabled requested-bone transforms from current skeleton locals.
//!
//! This follows the camera helper's ancestor association and forced translation
//! W, not the separate cached full-hierarchy rebuild. Host ordered f32/FMA is
//! provisional arithmetic; neither these matrices nor an exact-branch input
//! label establishes hardware parity, owner-world state or a camera frame.
mod cache;
mod matrix;
pub use cache::{ActorBoneCache, CacheStats, CacheValue, CachedBone};
pub use matrix::{multiply, quaternion_matrix};

use super::{frame::Precision, skeleton::Skeleton};
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheSelection {
    /// The caller explicitly selects the source helper's uncached branch.
    Disabled,
    /// Alternate actor-cache state requires its own provider/history contract.
    Enabled,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Counts the requested bone and every ancestor, including its root.
    pub parent_visits: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provenance {
    /// Input W provenance for visited bones only, not unrelated skeleton bones.
    pub unverified_input_rotations: usize,
    /// Ordered host operations have not been compared with hardware vectors.
    pub host_matrix_arithmetic_unverified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedBone {
    pub bone: u16,
    /// Requested bone first, then immediate parent through root.
    pub chain: Vec<u16>,
    /// Four contiguous columns; these remain privately owned semantic data.
    pub full_bits: [u32; 16],
    pub ancestors_bits: [u32; 16],
    pub local_transforms: usize,
    /// One product per ancestor, including the first identity product, then
    /// the final ancestors * requested-local product, including a root request.
    pub matrix_products: usize,
    pub precision: Provenance,
}

/// Evaluate the explicit uncached 800f6b80 branch against actual current locals.
/// Per-bone input precision must describe this same skeleton snapshot. This
/// does not read/update cached matrices, runtime dirty flags or owner state.
pub fn requested_bone(
    skeleton: &Skeleton,
    bone: usize,
    cache: CacheSelection,
    precision: &[Precision],
    budget: Budget,
) -> Result<RequestedBone> {
    ensure!(
        cache == CacheSelection::Disabled,
        "unsupported requested-bone actor cache branch"
    );
    ensure!(
        precision.len() == skeleton.bones().len(),
        "requested-bone precision count differs from skeleton"
    );
    // Skeleton construction has already checked range/cycles. The query also
    // checks its reached path and budget before evaluating any local matrix;
    // a failed walk never becomes an identity/root fallback or partial result.
    let mut chain = Vec::new();
    let mut visited = [false; super::skeleton::SUPPORTED_BONES];
    let mut index = bone;
    loop {
        let current = skeleton
            .bones()
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("requested-bone parent/index is out of range"))?;
        ensure!(
            chain.len() < budget.parent_visits,
            "requested-bone parent budget exhausted"
        );
        ensure!(
            index < visited.len() && !visited[index],
            "requested-bone parent cycle or unsupported bone count"
        );
        visited[index] = true;
        chain.push(current.index());
        if current.parent_raw() == super::skeleton::ROOT_PARENT {
            break;
        }
        index = usize::from(current.parent_raw());
    }

    let requested = local_matrix(&skeleton.bones()[usize::from(chain[0])]);
    let mut ancestors = matrix::identity();
    for &index in &chain[1..] {
        ancestors = multiply(
            &local_matrix(&skeleton.bones()[usize::from(index)]),
            &ancestors,
        );
    }
    let full = multiply(&ancestors, &requested);
    let unverified_input_rotations = chain
        .iter()
        .filter(|&&index| precision[usize::from(index)] == Precision::HostSqrtUnverified)
        .count();
    Ok(RequestedBone {
        bone: chain[0],
        local_transforms: chain.len(),
        matrix_products: chain.len(),
        chain,
        full_bits: full,
        ancestors_bits: ancestors,
        precision: Provenance {
            unverified_input_rotations,
            host_matrix_arithmetic_unverified: true,
        },
    })
}

fn local_matrix(current: &super::skeleton::Bone) -> [u32; 16] {
    let mut result = quaternion_matrix(current.local_quaternion_bits());
    result[12..15].copy_from_slice(&current.local_translation_bits()[..3]);
    // Both actor-cache misses and the uncached camera helper force W=1;
    // the distinct full-hierarchy rebuild preserves the local fourth lane.
    result[15] = 1.0f32.to_bits();
    result
}

#[cfg(test)]
mod tests;
