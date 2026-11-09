//! Owned fresh FP objects and the explicit reset-prewarm adapter. Live player
//! creation, external scheduling and camera evaluation remain separate.
mod children;
mod evidence;
mod frame;
pub(super) mod host;
mod inventory;
mod prewarm;
mod scalars;

use super::{FirstPersonBinding, animation};
use anyhow::{Result, ensure};
pub(crate) use evidence::Evidence;
use ge_assets::resource::GameAssets;
use ge_core::animation::{clip, layers};

pub(super) struct InitializedLayers {
    inventory: inventory::InitialInventory,
    state: layers::State,
    nodes: host::OwnedNodes,
    plans: Vec<animation::PreparedAnimation>,
    raw_frames: frame::RawFrames,
    committed_precision: Option<Vec<ge_core::animation::frame::Precision>>,
    component_flags_raw: u8,
    evidence: Evidence,
}
impl InitializedLayers {
    pub fn initialize(assets: &mut GameAssets, fp: &FirstPersonBinding) -> Result<Self> {
        let inventory = inventory::InitialInventory::load(assets, &fp.component)?;
        let auxiliary_count = fp.source_skin.auxiliary_count(&fp.skin_resource)?;
        ensure!(
            auxiliary_count == 0,
            "unsupported nonzero control auxiliary capture"
        );
        let capture = host::ControlCapture {
            auxiliary_count,
            bones: fp
                .skeleton
                .bones()
                .iter()
                .map(|bone| host::ControlBone {
                    translation_bits: bone.local_translation_bits(),
                    quaternion_bits: bone.local_quaternion_bits(),
                })
                .collect(),
        };
        let mut factory = host::NodeHost::new(&fp.component, capture);
        let mut state = layers::State::from_empty_snapshot(
            layers::Snapshot::fresh_first_person(),
            layers::Budget {
                nodes: 4096,
                commands: 4096,
                groups: 4096,
                children: 16_384,
            },
        )?;
        let mut plans = Vec::new();
        for (channel, [requested, previous, fallback]) in inventory.requests.into_iter().enumerate()
        {
            let plan = animation::PreparedAnimation::prepare(
                &fp.component,
                requested,
                previous,
                fallback,
            )?;
            ensure!(
                plan.selection().return_word != 0,
                "required source fresh FP mode has no transition return"
            );
            for attempt in &plan.selection().attempts {
                state.execute(
                    &mut factory,
                    &attempt.commands,
                    layers::SlotOverride {
                        enabled_raw: 0,
                        slot_raw: 0,
                    },
                )?;
            }
            let slot = if channel == 0 { 1 } else { 0 };
            let value = if channel == 0 {
                0
            } else {
                scalars::SlotInputs {
                    player_9a0_bits: 0,
                    player_ec4_bits: 1f32.to_bits(),
                    upper_tuning_bits: 0.5f32.to_bits(),
                    lower_tuning_bits: 0,
                }
                .secondary_blend_bits()
            };
            ensure!(
                factory.set_head_blend(state.slots()[slot].nodes_head(), value)?,
                "required fresh FP post-selector script head is unavailable"
            );
            plans.push(plan);
        }
        ensure!(
            factory.head_blend_writes() == 2,
            "fresh FP post-selector writes were not executed"
        );
        let nodes = factory.finish();
        ensure!(!state.failed(), "FP layer initialization failed");
        let evidence = Evidence::new(&inventory.evidence, &state, &nodes, &plans);
        Ok(Self {
            inventory,
            state,
            nodes,
            plans,
            raw_frames: frame::RawFrames::new(),
            committed_precision: None,
            // Source bind8030589c initializes component+135 to zero. No later
            // operation in this explicit fresh callback writes that byte.
            component_flags_raw: 0,
            evidence,
        })
    }
    pub fn evidence(&self) -> &Evidence {
        debug_assert!(self.inventory.retained());
        debug_assert!(!self.state.failed());
        debug_assert_eq!(self.nodes.len(), self.evidence.live_nodes);
        debug_assert_eq!(self.plans.len(), self.evidence.mode_requests);
        if let Some(raw) = &self.evidence.raw_frame {
            debug_assert_eq!(self.raw_frames.len(), raw.cached_frames);
        }
        &self.evidence
    }

    pub fn committed_precision(&self) -> Option<&[ge_core::animation::frame::Precision]> {
        self.committed_precision.as_deref()
    }

    pub fn decode_initial_frame(
        &mut self,
        component: &ge_assets::edb::Edb,
        camera_bone: usize,
    ) -> Result<()> {
        let head = *self.state.slots()[0]
            .nodes_head()
            .first()
            .ok_or_else(|| anyhow::anyhow!("initial layer has no head"))?;
        let child = self
            .nodes
            .script(head)
            .ok_or_else(|| anyhow::anyhow!("initial layer head is not a script child"))?;
        let keys = child
            .clips
            .iter()
            .map(|c| c.source.blend_key_bits)
            .collect::<Vec<_>>();
        let selected = clip::select(
            &keys,
            clip::BlendInputs {
                scalar_bits: child.blend_scalar_bits,
                child_flags_raw: u32::from(child.flags_raw),
                component_flags_raw: self.component_flags_raw,
            },
            clip::BlendConstants::source_initial(),
            1024,
        )?;
        ensure!(
            selected.second_index.is_none(),
            "initial raw-frame path does not blend multiple clips"
        );
        let bound = &child.clips[selected.source_index];
        let header = bound.animation.header();
        let mut mask = [0u32; 4];
        for bone in 0..usize::from(header.rotation_channels) {
            mask[bone / 32] |= 1 << (bone % 32);
        }
        let request = clip::frame_request(
            clip::FrameInputs {
                start: bound.source.start_raw,
                end: bound.source.end_raw,
                animation_frames: header.frame_count,
                normalized_position_bits: child.normalized_position_bits,
                child_flags_raw: u32::from(child.flags_raw),
                component_flags_raw: self.component_flags_raw,
                animation_word10_raw: header.word_10_raw,
                child_format_raw: None,
                bone_mask: mask,
                skin_bone_mask: None,
            },
            clip::FrameConstants::source_initial(),
        )?;
        ensure!(
            request.cache_frame1.is_none(),
            "initial raw-frame path does not interpolate adjacent frames"
        );
        let (decoded, stats) = self.raw_frames.get(
            component,
            &bound.animation,
            request.cache_frame0,
            request.cache_mask,
        )?;
        let mut evidence = frame::Evidence::new(
            child.reference,
            selected.source_index,
            selected.fraction_bits,
            &bound.animation,
            decoded,
            stats,
            camera_bone,
        );
        evidence.requested_format_flags_raw = request.format_flags_raw;
        self.evidence.raw_frame = Some(evidence);
        Ok(())
    }
}
