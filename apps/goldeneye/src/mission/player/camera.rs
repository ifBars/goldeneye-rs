//! Retained tagged-bone transform, before owner/world and camera adjustments.
//! Original matrix lanes remain private; diagnostics export provenance/digests.
use anyhow::{Result, ensure};
use ge_assets::skin::Tag;
use ge_core::animation::{
    frame::Precision,
    skeleton::Skeleton,
    transform::{self, ActorBoneCache, RequestedBone},
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
pub(super) enum CacheMode {
    DisabledInspection,
    FreshFpCache,
}

pub(super) struct PreparedBone {
    transform: RequestedBone,
    cache: Option<ActorBoneCache>,
    evidence: Evidence,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    pub state: &'static str,
    pub boundary: &'static str,
    pub cache_selection: &'static str,
    pub tag: u32,
    pub bone: u16,
    pub chain_length: usize,
    pub local_transforms: usize,
    pub matrix_products: usize,
    pub cache_slots: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub valid_cached_bones: usize,
    pub unverified_input_rotations: usize,
    pub host_matrix_arithmetic_unverified: bool,
    pub encoding: &'static str,
    pub transform_sha256: String,
    pub committed_locals_sha256: String,
    pub tag_bound: bool,
    pub committed_pose_used: bool,
    pub owner_world_applied: bool,
    pub matrices_refreshed: bool,
    pub native_numeric_parity_verified: bool,
    pub first_displayed_pose_verified: bool,
    pub camera_frame_ready: bool,
    pub scope: &'static str,
}

impl PreparedBone {
    pub fn prepare(
        skeleton: &Skeleton,
        tag: &Tag,
        parent_chain: &[u16],
        precision: &[Precision],
        committed_locals_sha256: &str,
        mode: CacheMode,
    ) -> Result<Self> {
        let (transform, cache, encoding, cache_selection) = match mode {
            CacheMode::DisabledInspection => (
                transform::requested_bone(
                    skeleton,
                    usize::from(tag.bone_index),
                    transform::CacheSelection::Disabled,
                    precision,
                    transform::Budget { parent_visits: 128 },
                )?,
                None,
                "source_camera_bone_fk_be_v1",
                "explicit_disabled_inspection",
            ),
            CacheMode::FreshFpCache => {
                let mut cache = ActorBoneCache::fresh(skeleton.bones().len(), 128)?;
                let transform = cache.camera_bone(
                    skeleton,
                    usize::from(tag.bone_index),
                    precision,
                    transform::Budget { parent_visits: 128 },
                )?;
                (
                    transform,
                    Some(cache),
                    "source_cached_camera_bone_fk_be_v1",
                    "fresh_fp_actor_cache",
                )
            }
        };
        ensure!(
            transform.chain == parent_chain,
            "camera bone parent binding changed"
        );
        let mut digest = Sha256::new();
        digest.update(encoding.as_bytes());
        digest.update(transform.bone.to_be_bytes());
        digest.update((transform.chain.len() as u64).to_be_bytes());
        for index in &transform.chain {
            digest.update(index.to_be_bytes());
        }
        for bits in transform
            .full_bits
            .into_iter()
            .chain(transform.ancestors_bits)
        {
            digest.update(bits.to_be_bytes());
        }
        let evidence = Evidence {
            state: "local_camera_bone_fk_prepared",
            boundary: "source_reset_prewarm_committed_locals",
            cache_selection,
            tag: 0x0e00_0028,
            bone: transform.bone,
            chain_length: transform.chain.len(),
            local_transforms: transform.local_transforms,
            matrix_products: transform.matrix_products,
            cache_slots: cache.as_ref().map_or(0, |cache| cache.slots().len()),
            cache_hits: cache.as_ref().map_or(0, |cache| cache.stats().hits),
            cache_misses: cache.as_ref().map_or(0, |cache| cache.stats().misses),
            valid_cached_bones: cache.as_ref().map_or(0, ActorBoneCache::valid_count),
            unverified_input_rotations: transform.precision.unverified_input_rotations,
            host_matrix_arithmetic_unverified: transform
                .precision
                .host_matrix_arithmetic_unverified,
            encoding,
            transform_sha256: format!("{:x}", digest.finalize()),
            committed_locals_sha256: committed_locals_sha256.to_owned(),
            tag_bound: true,
            committed_pose_used: true,
            owner_world_applied: false,
            matrices_refreshed: false,
            native_numeric_parity_verified: false,
            first_displayed_pose_verified: false,
            camera_frame_ready: false,
            scope: "Actual retained camera tag and committed prewarm locals evaluated with an explicit cache boundary. Disabled inspection preserves the ancestor accumulator's identity products; the fresh FP actor cache instead publishes root local directly and composes parent-cache times local, then serves the parent hit for the ancestor output. Both matrices and the owned cache remain private. This cache is separate from cached skeleton hierarchy matrices. Host matrix arithmetic and provisional input quaternion estimates remain unverified. Full native FP/TP cache setup/history, owner/world transform, later camera adjustments, external update and displayed camera frame are not established.",
        };
        Ok(Self {
            transform,
            cache,
            evidence,
        })
    }

    pub fn evidence(&self) -> &Evidence {
        debug_assert_eq!(self.transform.chain.len(), self.evidence.chain_length);
        debug_assert_eq!(self.transform.bone, self.evidence.bone);
        debug_assert_eq!(
            self.cache.as_ref().map_or(0, |cache| cache.slots().len()),
            self.evidence.cache_slots
        );
        &self.evidence
    }
}
