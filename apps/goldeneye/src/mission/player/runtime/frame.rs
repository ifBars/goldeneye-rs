//! Owned raw-frame cache. No root correction, skin mapping or skeleton commit.
#[cfg(test)]
mod tests;
use anyhow::{Result, ensure};
use ge_assets::{edb::Edb, skeletal_animation::Animation};
use ge_core::animation::frame::{self, DecodedFrame, Precision};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    resource: u32,
    bytes: u32,
    directory: usize,
    body: usize,
    payload: usize,
    frame: u32,
}

pub(super) struct RawFrames {
    frames: BTreeMap<Key, DecodedFrame>,
    pub decode_calls: usize,
    pub hits: usize,
}
impl RawFrames {
    pub fn new() -> Self {
        Self {
            frames: BTreeMap::new(),
            decode_calls: 0,
            hits: 0,
        }
    }
    pub fn get(
        &mut self,
        owner: &Edb,
        animation: &Animation,
        index: u32,
        mask: [u32; 4],
    ) -> Result<(&DecodedFrame, (usize, usize, usize))> {
        let d = animation.descriptor();
        let header = animation.header();
        let key = Key {
            resource: owner.header.hash,
            bytes: owner.header.payload_bytes,
            directory: d.directory_index,
            body: d.address,
            payload: animation.payload_span().address,
            frame: index,
        };
        let valid = self
            .frames
            .get(&key)
            .map(|f| f.valid_mask)
            .unwrap_or([0; 4]);
        let mut requested = [0; 4];
        for bone in 0..usize::from(header.rotation_channels) {
            requested[bone / 32] |= mask[bone / 32] & (1 << (bone % 32));
        }
        if self.frames.contains_key(&key) && requested.iter().zip(valid).all(|(r, v)| r & !v == 0) {
            self.hits += 1;
            return Ok((
                &self.frames[&key],
                (self.frames.len(), self.decode_calls, self.hits),
            ));
        }
        ensure!(
            self.frames.contains_key(&key) || self.frames.len() < 64,
            "raw-frame cache budget exhausted"
        );
        let needed = std::array::from_fn(|i| requested[i] & !valid[i]);
        let decoded = frame::decode(
            animation.compressed_payload(owner)?,
            frame::Layout {
                frame_count: header.frame_count,
                rotation_count: header.rotation_channels,
                translation_mask: header.translation_mask_raw,
                auxiliary_count: header.auxiliary_channels,
            },
            index,
            needed,
            frame::Budget {
                scalar_records: 4096,
                keys: 65_536,
                coefficients: 65_536,
            },
        )?;
        self.decode_calls += 1;
        if let Some(previous) = self.frames.get_mut(&key) {
            for i in 0..4 {
                previous.valid_mask[i] |= decoded.valid_mask[i];
            }
            previous.rotations.extend(decoded.rotations);
            previous.rotations.sort_by_key(|r| r.bone_index);
            previous.translations.extend(decoded.translations);
            previous.translations.sort_by_key(|t| t.bone_index);
            previous.work.scalar_records += decoded.work.scalar_records;
            previous.work.keys += decoded.work.keys;
            previous.work.coefficients += decoded.work.coefficients;
        } else {
            self.frames.insert(key, decoded);
        }
        Ok((
            &self.frames[&key],
            (self.frames.len(), self.decode_calls, self.hits),
        ))
    }
    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    pub state: &'static str,
    pub source_child: u32,
    pub selected_clip: usize,
    pub blend_fraction_bits: u32,
    pub requested_format_flags_raw: u32,
    pub animation_reference: u32,
    pub animation_directory_index: usize,
    pub frame: u32,
    pub rotations: usize,
    pub translations: usize,
    pub exact_branch_rotations: usize,
    pub unverified_sqrt_rotations: usize,
    pub raw_frame_sha256: String,
    pub camera_raw_sha256: Option<String>,
    pub camera_raw_exact_branch: bool,
    pub raw_encoding: &'static str,
    pub cached_frames: usize,
    pub decode_calls: usize,
    pub cache_hits: usize,
    pub root_bias_applied: bool,
    pub skin_mapping_applied: bool,
    pub layer_blending_applied: bool,
    pub skeleton_committed: bool,
    pub native_numeric_parity_verified: bool,
    pub first_displayed_pose_verified: bool,
    pub camera_frame_ready: bool,
    pub scope: &'static str,
}
impl Evidence {
    pub fn new(
        child: u32,
        clip: usize,
        fraction: u32,
        animation: &Animation,
        decoded: &DecodedFrame,
        cache_stats: (usize, usize, usize),
        camera_bone: usize,
    ) -> Self {
        let mut hash = Sha256::new();
        hash.update(decoded.frame.to_be_bytes());
        for word in decoded.valid_mask {
            hash.update(word.to_be_bytes());
        }
        hash.update((decoded.rotations.len() as u64).to_be_bytes());
        for r in &decoded.rotations {
            hash.update(r.bone_index.to_be_bytes());
            for bits in r.quaternion_bits {
                hash.update(bits.to_be_bytes());
            }
            hash.update([u8::from(r.precision == Precision::HostSqrtUnverified)]);
        }
        hash.update((decoded.translations.len() as u64).to_be_bytes());
        for t in &decoded.translations {
            hash.update(t.bone_index.to_be_bytes());
            for bits in t.translation_bits {
                hash.update(bits.to_be_bytes());
            }
        }
        let camera = decoded
            .rotations
            .iter()
            .find(|r| usize::from(r.bone_index) == camera_bone)
            .zip(
                decoded
                    .translations
                    .iter()
                    .find(|t| usize::from(t.bone_index) == camera_bone),
            );
        let camera_raw_sha256 = camera.map(|(r, t)| {
            let mut h = Sha256::new();
            for bits in r.quaternion_bits.into_iter().chain(t.translation_bits) {
                h.update(bits.to_be_bytes());
            }
            format!("{:x}", h.finalize())
        });
        let unverified = decoded
            .rotations
            .iter()
            .filter(|r| r.precision == Precision::HostSqrtUnverified)
            .count();
        Self {
            state: "raw_frame_decoded",
            source_child: child,
            selected_clip: clip,
            blend_fraction_bits: fraction,
            animation_reference: animation.descriptor().reference,
            animation_directory_index: animation.descriptor().directory_index,
            frame: decoded.frame,
            rotations: decoded.rotations.len(),
            translations: decoded.translations.len(),
            requested_format_flags_raw: 0,
            exact_branch_rotations: decoded.rotations.len() - unverified,
            unverified_sqrt_rotations: unverified,
            raw_frame_sha256: format!("{:x}", hash.finalize()),
            camera_raw_sha256,
            camera_raw_exact_branch: camera
                .is_some_and(|(r, _)| r.precision == Precision::ExactBranch),
            raw_encoding: "raw_polynomial_frame_be_v1",
            cached_frames: cache_stats.0,
            decode_calls: cache_stats.1,
            cache_hits: cache_stats.2,
            root_bias_applied: false,
            skin_mapping_applied: false,
            layer_blending_applied: false,
            skeleton_committed: false,
            native_numeric_parity_verified: false,
            first_displayed_pose_verified: false,
            camera_frame_ready: false,
            scope: "Raw polynomial frame vectors decoded from the actual selected owned clip. Positive quaternion square roots use explicitly unverified host estimates; exact-branch labels are relative to evaluated XYZ, not original-runtime parity. Raw root/bone vectors are privately cached; only counts and fingerprints are public. Root adjustment, skin mapping, control/layer blending, skeleton commit, clock and first displayed pose are not supplied; this is not a camera frame.",
        }
    }
}
