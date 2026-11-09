//! Explicit fresh FP actor-cache ownership, source 800ff790. This is neither
//! the skeleton's cached hierarchy nor a claim about the selected global owner.
use super::{Budget, Provenance, RequestedBone, local_matrix, matrix, multiply};
use crate::animation::{frame::Precision, skeleton::Skeleton};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub matrix_products: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheValue {
    pub matrix_bits: [u32; 16],
    /// Stored build provenance; a hit does not reread the parent chain.
    pub chain: Vec<u16>,
    pub precision: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedBone {
    valid: bool,
    /// Fresh source allocation does not supply initialized matrix values.
    /// Invalidation retains an actual previously built value instead of
    /// inventing an identity/zero matrix for an uninitialized slot.
    value: Option<CacheValue>,
}

impl CachedBone {
    pub fn valid(&self) -> bool {
        self.valid
    }

    pub fn value(&self) -> Option<&CacheValue> {
        self.value.as_ref()
    }

    pub fn matrix_bits(&self) -> Option<&[u32; 16]> {
        self.value.as_ref().map(|value| &value.matrix_bits)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorBoneCache {
    slots: Vec<CachedBone>,
    stats: CacheStats,
}

impl ActorBoneCache {
    pub fn fresh(bone_count: usize, maximum_slots: usize) -> Result<Self> {
        ensure!(
            bone_count > 0 && bone_count <= crate::animation::skeleton::SUPPORTED_BONES,
            "unsupported actor bone cache slot count"
        );
        ensure!(
            bone_count <= maximum_slots,
            "actor bone cache slot budget exhausted"
        );
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(bone_count)
            .context("allocating bounded actor bone cache")?;
        slots.resize(
            bone_count,
            CachedBone {
                valid: false,
                value: None,
            },
        );
        Ok(Self {
            slots,
            stats: CacheStats::default(),
        })
    }

    pub fn slots(&self) -> &[CachedBone] {
        &self.slots
    }

    pub fn valid_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.valid).count()
    }

    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Source 800ff660 clears only validity bytes. The retained matrix values
    /// and provenance remain untouched. Local commit does not implicitly call
    /// this helper: the lifecycle owner must request actual invalidation.
    pub fn invalidate_all(&mut self) {
        for slot in &mut self.slots {
            slot.valid = false;
        }
    }

    /// One source cache query. A valid hit returns its retained value without
    /// reading local T/Q, parent links or supplied rotation precision. A miss
    /// walks only as far as a valid ancestor; roots store local directly and
    /// descendants build cachedParent * local. Host preflight failures are
    /// atomic, rather than publishing a partial failed traversal.
    pub fn request(
        &mut self,
        skeleton: &Skeleton,
        bone: usize,
        precision: &[Precision],
        budget: Budget,
    ) -> Result<CacheValue> {
        ensure!(
            self.slots.len() == skeleton.bones().len(),
            "actor bone cache/skeleton count differs"
        );
        ensure!(
            bone < self.slots.len(),
            "actor bone cache index is out of range"
        );
        let mut path = Vec::new();
        let mut visited = [false; crate::animation::skeleton::SUPPORTED_BONES];
        let mut next = bone;
        let mut hit = false;
        loop {
            let slot = self
                .slots
                .get(next)
                .context("actor bone cache parent is out of range")?;
            if slot.valid {
                ensure!(
                    slot.value.is_some(),
                    "actor bone cache valid slot has no retained value"
                );
                hit = true;
                break;
            }
            ensure!(
                path.len() < budget.parent_visits,
                "actor bone cache parent budget exhausted"
            );
            ensure!(
                next < visited.len() && !visited[next],
                "actor bone cache parent cycle"
            );
            visited[next] = true;
            let current = skeleton
                .bones()
                .get(next)
                .context("actor bone cache parent is out of range")?;
            path.push(next);
            if current.parent_raw() == crate::animation::skeleton::ROOT_PARENT {
                break;
            }
            next = usize::from(current.parent_raw());
        }
        if !path.is_empty() {
            ensure!(
                precision.len() == skeleton.bones().len(),
                "actor bone cache precision count differs from skeleton"
            );
        }
        let products = path
            .iter()
            .filter(|&&index| {
                skeleton.bones()[index].parent_raw() != crate::animation::skeleton::ROOT_PARENT
            })
            .count();
        let stats = CacheStats {
            hits: self
                .stats
                .hits
                .checked_add(usize::from(hit))
                .context("actor cache hit counter overflow")?,
            misses: self
                .stats
                .misses
                .checked_add(path.len())
                .context("actor cache miss counter overflow")?,
            matrix_products: self
                .stats
                .matrix_products
                .checked_add(products)
                .context("actor cache product counter overflow")?,
        };
        for &index in path.iter().rev() {
            let current = &skeleton.bones()[index];
            let local = local_matrix(current);
            let mut chain = vec![current.index()];
            let mut unverified_input_rotations =
                usize::from(precision[index] == Precision::HostSqrtUnverified);
            let matrix_bits = if current.parent_raw() == crate::animation::skeleton::ROOT_PARENT {
                local
            } else {
                let parent = self.slots[usize::from(current.parent_raw())]
                    .value
                    .as_ref()
                    .expect("preflight and reverse build retain the parent cache");
                chain.extend_from_slice(&parent.chain);
                unverified_input_rotations += parent.precision.unverified_input_rotations;
                multiply(&parent.matrix_bits, &local)
            };
            self.slots[index] = CachedBone {
                valid: true,
                value: Some(CacheValue {
                    matrix_bits,
                    chain,
                    precision: Provenance {
                        unverified_input_rotations,
                        host_matrix_arithmetic_unverified: true,
                    },
                }),
            };
        }
        self.stats = stats;
        Ok(self.slots[bone]
            .value
            .as_ref()
            .expect("successful query retains the requested matrix")
            .clone())
    }

    /// Camera helper cache branch: request target first, then its immediate
    /// parent for the separate ancestor output (normally a hit). A requested
    /// root gets an identity ancestor output; its full output stays the direct
    /// cache result, with no extra identity product.
    pub fn camera_bone(
        &mut self,
        skeleton: &Skeleton,
        bone: usize,
        precision: &[Precision],
        budget: Budget,
    ) -> Result<RequestedBone> {
        let before = self.stats;
        let full = self.request(skeleton, bone, precision, budget)?;
        let parent = skeleton.bones()[bone].parent_raw();
        let ancestors_bits = if parent == crate::animation::skeleton::ROOT_PARENT {
            matrix::identity()
        } else {
            self.request(skeleton, usize::from(parent), precision, budget)?
                .matrix_bits
        };
        Ok(RequestedBone {
            bone: u16::try_from(bone)?,
            chain: full.chain,
            full_bits: full.matrix_bits,
            ancestors_bits,
            local_transforms: self.stats.misses - before.misses,
            matrix_products: self.stats.matrix_products - before.matrix_products,
            precision: full.precision,
        })
    }
}
