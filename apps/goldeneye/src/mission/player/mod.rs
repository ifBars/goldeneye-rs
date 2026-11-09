//! Source character selection and owned first-person skeleton construction.
//! These dependencies do not construct PlayerNew or evaluate its camera pose.
mod animation;
mod camera;
mod evidence;
mod runtime;

use anyhow::{Context, Result, ensure};
use ge_assets::{
    character::{self, CharacterCatalog},
    edb::Edb,
    resource::GameAssets,
    skin::{self, Skin, Tag},
};
use ge_core::animation::skeleton::{self, BoneInput, Skeleton};
use sha2::{Digest, Sha256};

pub(crate) use evidence::Evidence;
const COMPONENT: u32 = 0x0102_0016;
const SKIN: u32 = 0x0d00_016f;
const CAMERA_TAG: u32 = 0x0e00_0028;
const CATALOG_LIMIT: u32 = 1024 * 1024;
const COMPONENT_LIMIT: u32 = 8 * 1024 * 1024;
const SKIN_LIMIT: u32 = 16 * 1024 * 1024;
const TOTAL_LIMIT: u32 = 32 * 1024 * 1024;
const BONE_LIMIT: usize = 128;

pub(super) struct FirstPersonBinding {
    component: Edb,
    skin_resource: Edb,
    source_skin: Skin,
    skeleton: Skeleton,
    camera_tag: Option<Tag>,
    camera_chain: Vec<u16>,
    animation_plan: Option<animation::PreparedAnimation>,
    initialized_layers: Option<runtime::InitializedLayers>,
    camera_bone_transform: Option<camera::PreparedBone>,
}

pub(super) struct PlayerComponents {
    registry_resource: Edb,
    catalog: CharacterCatalog,
    selected_character_key: u32,
    first_person: FirstPersonBinding,
    evidence: Evidence,
}

impl PlayerComponents {
    pub fn load(assets: &mut GameAssets, requested: u32) -> Result<Self> {
        Self::from_loader(requested, |hash, maximum| {
            super::resource::load(assets, hash, maximum)
        })
    }

    fn from_loader(requested: u32, mut load: impl FnMut(u32, u32) -> Result<Edb>) -> Result<Self> {
        // This is the supported explicit level-row path. Invalid/missing keys
        // require source default/history inputs, not an invented character.
        ensure!(
            requested >> 16 == 0x4d00 && requested & 0xffff != 0,
            "unsupported character request; source default/history is unavailable"
        );
        let mut remaining = TOTAL_LIMIT;
        let mut bounded = |hash: u32, maximum: u32| -> Result<Edb> {
            let limit = maximum.min(remaining);
            let resource = load(hash, limit)?;
            ensure!(
                resource.header.hash == hash
                    && resource.header.version == 263
                    && resource.header.payload_bytes <= limit,
                "player component resource identity/version/byte limit differs"
            );
            remaining -= resource.header.payload_bytes;
            Ok(resource)
        };
        let registry_resource = bounded(character::CATALOG_RESOURCE, CATALOG_LIMIT)?;
        let catalog = CharacterCatalog::read(&registry_resource, character::Budget { rows: 4096 })?;
        let selected = catalog
            .find(requested)
            .context("character row is absent; native default-row fallback is unsupported")?;
        let skin_file = selected.fp_skin_file();
        ensure!(
            skin_file != character::CATALOG_RESOURCE && skin_file != COMPONENT,
            "character skin resource aliases an incompatible dependency"
        );
        let component = bounded(COMPONENT, COMPONENT_LIMIT)?;
        let skin_resource = bounded(skin_file, SKIN_LIMIT)?;
        let source_skin = skin_resource
            .skin(
                SKIN,
                skin::Budget {
                    skins: 4096,
                    bones: BONE_LIMIT,
                },
            )?
            .context("character first-person skin reference is absent")?;
        let inputs = source_skin
            .bones()
            .iter()
            .map(|bone| BoneInput {
                parent: bone.parent_raw,
                range_end: bone.range_raw,
                auxiliary_raw: [bone.word_4_raw, bone.word_6_raw],
            })
            .collect::<Vec<_>>();
        let skeleton = Skeleton::initialize(
            &inputs,
            skeleton::Budget {
                bones: BONE_LIMIT,
                parent_visits: BONE_LIMIT * BONE_LIMIT,
            },
        )?;
        let camera_tag = source_skin.find_tag(&skin_resource, CAMERA_TAG, 4096)?;
        let camera_chain = camera_tag
            .as_ref()
            .map(|tag| skeleton.parent_chain(usize::from(tag.bone_index), BONE_LIMIT))
            .transpose()?
            .unwrap_or_default();
        let mut topology_hash = Sha256::new();
        for bone in source_skin.bones() {
            for word in [
                bone.parent_raw,
                bone.range_raw,
                bone.word_4_raw,
                bone.word_6_raw,
            ] {
                topology_hash.update(word.to_be_bytes());
            }
        }
        let mut constructor_hash = Sha256::new();
        for bone in skeleton.bones() {
            for word in bone
                .local_translation_bits()
                .into_iter()
                .chain(bone.local_quaternion_bits())
                .chain(bone.matrix_bits())
            {
                constructor_hash.update(word.to_be_bytes());
            }
        }
        let camera_parent_chain_sha256 = camera_tag.as_ref().map(|_| {
            let mut digest = Sha256::new();
            for bone in &camera_chain {
                digest.update(bone.to_be_bytes());
            }
            format!("{:x}", digest.finalize())
        });
        let descriptor = source_skin.descriptor();
        let evidence = Evidence {
            state: "fp_skeleton_constructor_initialized",
            requested_character: requested,
            catalog_resource: registry_resource.header.hash,
            catalog_payload_bytes: registry_resource.header.payload_bytes,
            character_sheet: character::CHARACTER_SHEET,
            character_group: 1,
            character_row_stride: 24,
            character_rows: catalog.rows().len(),
            duplicate_character_rows: catalog.duplicate_count(),
            selected_character: selected.key(),
            selected_source_index: selected.source_index(),
            selected_source_address: selected.source_address(),
            native_default_fallback_used: false,
            component_resource: component.header.hash,
            component_payload_bytes: component.header.payload_bytes,
            component_reference: 0,
            skin_resource: skin_resource.header.hash,
            skin_payload_bytes: skin_resource.header.payload_bytes,
            skin_reference: SKIN,
            skin_directory_index: descriptor.directory_index,
            skin_hash: descriptor.hash,
            skin_body_address: descriptor.address,
            retained_resources: 3,
            retained_payload_bytes: u64::from(TOTAL_LIMIT - remaining),
            bone_count: skeleton.bones().len(),
            root_count: skeleton.roots().len(),
            topology_sha256: format!("{:x}", topology_hash.finalize()),
            constructor_state_sha256: format!("{:x}", constructor_hash.finalize()),
            camera_tag: CAMERA_TAG,
            camera_tag_found: camera_tag.is_some(),
            camera_bone: camera_tag.as_ref().map(|tag| tag.bone_index),
            camera_parent_chain_length: camera_chain.len(),
            camera_parent_chain_sha256,
            skeleton_constructor_initialized: true,
            animation_sampled: false,
            player_physics_initialized: false,
            camera_frame_ready: false,
            mode_plan: None,
            initial_layers: None,
            camera_bone_transform: None,
            scope: "Explicit selected level character row and retained registry, animation component and separate first-person skin. The owned CPU skeleton initializes source parent/child membership, range masks and constructor identity locals/matrices; the camera tag is resolved against that binding. Constructor identity is not a sampled animation frame. Native default/history fallback, third-person components, physics, live animator layers, player creation and camera matrices remain incomplete. Public diagnostics contain only IDs, provenance, counts and hashes; no source rows, rig arrays or transforms.",
        };
        Ok(Self {
            registry_resource,
            catalog,
            selected_character_key: requested,
            first_person: FirstPersonBinding {
                component,
                skin_resource,
                source_skin,
                skeleton,
                camera_tag,
                camera_chain,
                animation_plan: None,
                initialized_layers: None,
                camera_bone_transform: None,
            },
            evidence,
        })
    }

    pub fn actor_definition_source(&self) -> &Edb {
        &self.registry_resource
    }

    pub fn selected_character(&self) -> Result<&character::CharacterRow> {
        self.catalog
            .find(self.selected_character_key)
            .context("retained selected character row is absent")
    }

    pub fn evidence(&self) -> &Evidence {
        let fp = &self.first_person;
        debug_assert_eq!(
            self.registry_resource.header.hash,
            self.evidence.catalog_resource
        );
        debug_assert!(
            self.catalog
                .find(self.evidence.selected_character)
                .is_some()
        );
        debug_assert_eq!(fp.component.header.hash, self.evidence.component_resource);
        debug_assert_eq!(fp.skin_resource.header.hash, self.evidence.skin_resource);
        debug_assert_eq!(fp.source_skin.bone_count(), fp.skeleton.bones().len());
        debug_assert_eq!(fp.camera_tag.is_some(), self.evidence.camera_tag_found);
        debug_assert_eq!(
            fp.camera_chain.len(),
            self.evidence.camera_parent_chain_length
        );
        debug_assert_eq!(
            fp.animation_plan.is_some(),
            self.evidence.mode_plan.is_some()
        );
        debug_assert_eq!(
            fp.initialized_layers.is_some(),
            self.evidence.initial_layers.is_some()
        );
        debug_assert_eq!(
            fp.camera_bone_transform.is_some(),
            self.evidence.camera_bone_transform.is_some()
        );
        if let Some(prepared) = &fp.camera_bone_transform {
            debug_assert_eq!(Some(prepared.evidence().bone), self.evidence.camera_bone);
        }
        if let Some(initialized) = &fp.initialized_layers {
            debug_assert_eq!(
                initialized.evidence().live_nodes,
                self.evidence
                    .initial_layers
                    .as_ref()
                    .expect("owned layer evidence")
                    .live_nodes
            );
        }
        &self.evidence
    }

    pub fn initialize_layers(&mut self, assets: &mut GameAssets) -> Result<()> {
        ensure!(
            self.first_person.initialized_layers.is_none() && !self.evidence.animation_sampled,
            "fresh layers cannot replace previously initialized animation state"
        );
        let initialized = runtime::InitializedLayers::initialize(assets, &self.first_person)?;
        self.evidence.initial_layers = Some(initialized.evidence().clone());
        self.first_person.initialized_layers = Some(initialized);
        Ok(())
    }

    pub fn decode_initial_frame(&mut self) -> Result<()> {
        let fp = &mut self.first_person;
        let camera = fp
            .camera_tag
            .as_ref()
            .context("camera tag unavailable for raw frame evidence")?
            .bone_index;
        let runtime = fp
            .initialized_layers
            .as_mut()
            .context("animation layers must be initialized before raw frame decoding")?;
        runtime.decode_initial_frame(&fp.component, usize::from(camera))?;
        self.evidence.initial_layers = Some(runtime.evidence().clone());
        Ok(())
    }

    pub fn evaluate_prewarm_pose(&mut self) -> Result<()> {
        let fp = &mut self.first_person;
        let runtime = fp
            .initialized_layers
            .as_mut()
            .context("animation layers must be initialized before prewarm")?;
        runtime.evaluate_prewarm_pose(
            &fp.component,
            &fp.skin_resource,
            &fp.source_skin,
            &mut fp.skeleton,
        )?;
        self.evidence.initial_layers = Some(runtime.evidence().clone());
        self.evidence.animation_sampled = true;
        Ok(())
    }

    pub fn prepare_animation(
        &mut self,
        requested: u32,
        previous: u32,
        fallback: u32,
    ) -> Result<()> {
        let plan = animation::PreparedAnimation::prepare(
            &self.first_person.component,
            requested,
            previous,
            fallback,
        )?;
        self.evidence.mode_plan = Some(plan.evidence().clone());
        self.first_person.animation_plan = Some(plan);
        Ok(())
    }

    pub fn evaluate_uncached_camera_bone(&mut self) -> Result<()> {
        self.prepare_camera_bone(camera::CacheMode::DisabledInspection)
    }

    pub fn evaluate_cached_camera_bone(&mut self) -> Result<()> {
        self.prepare_camera_bone(camera::CacheMode::FreshFpCache)
    }

    fn prepare_camera_bone(&mut self, mode: camera::CacheMode) -> Result<()> {
        let fp = &mut self.first_person;
        ensure!(
            fp.camera_bone_transform.is_none(),
            "initial camera-bone query cannot replace existing cache history"
        );
        let runtime = fp
            .initialized_layers
            .as_ref()
            .context("camera bone requires initialized layers")?;
        let prewarm = runtime
            .evidence()
            .prewarm_pose
            .as_ref()
            .context("camera bone requires committed prewarm locals")?;
        ensure!(
            prewarm.skeleton_committed,
            "camera bone requires a committed skeleton"
        );
        let precision = runtime
            .committed_precision()
            .context("committed rotation precision is unavailable")?;
        let tag = fp
            .camera_tag
            .as_ref()
            .context("source camera tag unavailable")?;
        let prepared = camera::PreparedBone::prepare(
            &fp.skeleton,
            tag,
            &fp.camera_chain,
            precision,
            &prewarm.locals_sha256,
            mode,
        )?;
        self.evidence.camera_bone_transform = Some(prepared.evidence().clone());
        fp.camera_bone_transform = Some(prepared);
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/fixtures.rs"]
pub(super) mod fixtures;
#[cfg(test)]
mod tests;
