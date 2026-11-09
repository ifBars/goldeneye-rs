//! Fixed reset-prewarm boundary, separate from the external actor scheduler.
//! This adapter accepts only the untouched owned fresh FP state. It preflights
//! the supported subset before publishing local bones and child cursors.
use super::{InitializedLayers, children::ScriptChild, host::Node};
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, skin::Skin};
use ge_core::animation::{
    clip, clock, composition, context,
    frame::Precision,
    layers::NodeId,
    skeleton::{LocalPose, Skeleton},
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const BASELINE: u32 = 60;
const ONE: u32 = 1.0f32.to_bits();
const SAMPLE_THRESHOLD: u32 = 0x3a83_126f;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChildEvidence {
    pub node: u64,
    pub reference: u32,
    pub normalized_position_bits: u32,
    pub previous_phase_bits: u32,
    pub advanced_phase_bits: u32,
    pub transition_bits: u32,
    pub reciprocal_bits: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    pub state: &'static str,
    pub seconds_bits: u32,
    pub baseline: u32,
    pub scaled_step_bits: u32,
    pub children: Vec<ChildEvidence>,
    pub updated_controls: usize,
    pub sampled_controls: usize,
    pub sampled_scripts: usize,
    pub skipped_layers: usize,
    pub selected_clip: usize,
    pub frame: u32,
    pub fade_bits: u32,
    pub animation_translations: usize,
    pub skin_translations: usize,
    pub committed_bones: usize,
    pub scratch_flags_raw: u32,
    pub locals_sha256: String,
    pub locals_encoding: &'static str,
    pub post_update_bindings_sha256: String,
    pub unverified_sqrt_rotations: usize,
    pub root_bias_applied: bool,
    pub skin_mapping_applied: bool,
    pub skeleton_committed: bool,
    pub matrices_refreshed: bool,
    pub native_numeric_parity_verified: bool,
    pub external_step_executed: bool,
    pub first_displayed_pose_verified: bool,
    pub camera_frame_ready: bool,
    pub scope: &'static str,
}

struct Update {
    id: NodeId,
    position: clock::PreparedPosition,
    phase: clock::PhaseAdvance,
    transition: clock::TransitionState,
}

impl InitializedLayers {
    pub fn evaluate_prewarm_pose(
        &mut self,
        component: &Edb,
        skin_owner: &Edb,
        skin: &Skin,
        skeleton: &mut Skeleton,
    ) -> Result<()> {
        ensure!(
            self.evidence.prewarm_pose.is_none() && self.evidence.raw_frame.is_none(),
            "prewarm requires an untouched constructor snapshot"
        );
        ensure!(
            self.component_flags_raw == 0
                && self.nodes.update_flags_raw() == 3
                && self.state.active_count() == 2,
            "unsupported prewarm component or layer history"
        );
        let scaled_step = clock::scaled_actor_step(clock::source_prewarm_seconds(), BASELINE)?;
        let delta = clock::transition_delta(ONE, scaled_step)?;
        let mut updates = Vec::new();
        let mut updated_controls = 0;
        // Ordinary dispatch updates every active list, including zero-weight
        // lists. Fresh maintenance cannot remove the incomplete script heads.
        for slot in &self.state.slots()[..usize::from(self.state.active_count())] {
            ensure!(
                slot.flags_raw() == 0,
                "unsupported prewarm list maintenance"
            );
            for id in slot.nodes_head().iter().rev() {
                match self
                    .nodes
                    .nodes
                    .get(id)
                    .context("prewarm node owner absent")?
                {
                    Node::Control(_) => updated_controls += 1, // source virtual28 no-op
                    Node::Script(child) => {
                        updates.push(update(*id, child, component, scaled_step, delta)?)
                    }
                }
            }
        }
        let mut scratch = composition::Scratch::new(skeleton.bones().len(), 0, 128)?;
        let mut sampled_controls = 0;
        let mut sampled_scripts = 0;
        let mut skipped_layers = 0;
        let mut selected_clip = None;
        let mut candidate = None;
        let mut fade_bits = None;
        for slot in &self.state.slots()[..usize::from(self.state.active_count())] {
            if f32::from_bits(slot.weight_bits()) < f32::from_bits(SAMPLE_THRESHOLD) {
                skipped_layers += 1;
                continue;
            }
            ensure!(
                sampled_scripts == 0 && slot.weight_bits() == ONE,
                "unsupported prewarm inter-layer blend"
            );
            for id in slot.nodes_head().iter().rev() {
                match self
                    .nodes
                    .nodes
                    .get(id)
                    .context("sample node owner absent")?
                {
                    Node::Control(control) => {
                        let locals = control
                            .bones
                            .iter()
                            .map(|bone| LocalPose {
                                translation_bits: bone.translation_bits,
                                quaternion_bits: bone.quaternion_bits,
                            })
                            .collect::<Vec<_>>();
                        scratch.sample_control(&locals)?;
                        sampled_controls += 1;
                    }
                    Node::Script(child) => {
                        ensure!(
                            candidate.is_none(),
                            "unsupported extra prewarm script consumer"
                        );
                        let update = updates
                            .iter()
                            .find(|u| u.id == *id)
                            .context("sampled script was not updated")?;
                        let selected = select(child, self.component_flags_raw)?;
                        ensure!(
                            selected.second_index.is_none(),
                            "unsupported prewarm clip pair"
                        );
                        let bound = &child.clips[selected.source_index];
                        let header = bound.animation.header();
                        let pair = clip::SkinPair {
                            owner: u64::from(skin_owner.header.hash),
                            reference: skin.descriptor().hash,
                        };
                        let source_skin = clip::skin_request(
                            u64::from(component.header.hash),
                            bound.animation.descriptor().words_raw[7],
                            Some(pair),
                        )?;
                        ensure!(
                            source_skin == clip::SkinRequest::BoundSecondary(pair),
                            "unsupported prewarm descriptor-selected skin"
                        );
                        let mask = full_mask(skeleton.bones().len());
                        let request = clip::frame_request(
                            clip::FrameInputs {
                                start: bound.source.start_raw,
                                end: bound.source.end_raw,
                                animation_frames: header.frame_count,
                                normalized_position_bits: update.position.normalized_position_bits,
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
                            "unsupported prewarm frame pair"
                        );
                        let (raw, _) = self.raw_frames.get(
                            component,
                            &bound.animation,
                            request.cache_frame0,
                            request.cache_mask,
                        )?;
                        let defaults = skin.fallback_translations(skin_owner, 128)?;
                        let key = context::SkinKey {
                            file: skin_owner.header.hash,
                            skin: skin.descriptor().hash,
                        };
                        let pose = context::prepare(
                            context::Inputs {
                                frame: raw,
                                source: context::SkinTranslations {
                                    key,
                                    vectors: &defaults.vectors,
                                    auxiliary_count: 0,
                                },
                                target: context::SkinTranslations {
                                    key,
                                    vectors: &defaults.vectors,
                                    auxiliary_count: 0,
                                },
                                translation_mask: header.translation_mask_raw,
                                root_bias_bits: bound.animation.root_bias_bits(component)?,
                                format_flags_raw: request.format_flags_raw,
                                frame_fraction_bits: 0,
                                consumer_flags_raw: 0,
                            },
                            context::Budget { bones: 128 },
                        )?;
                        let fade = scratch.script_fade(
                            update.transition.scalar_bits,
                            child.word8_raw,
                            composition::CurveConstants::source_initial(),
                        )?;
                        let locals = pose
                            .locals
                            .iter()
                            .map(|bone| LocalPose {
                                translation_bits: bone.translation_bits,
                                quaternion_bits: bone.quaternion_bits,
                            })
                            .collect::<Vec<_>>();
                        scratch.sample_script(&locals, mask, fade)?;
                        selected_clip = Some(selected.source_index);
                        fade_bits = Some(fade);
                        candidate = Some(pose);
                        sampled_scripts += 1;
                    }
                }
            }
        }
        let candidate = candidate.context("prewarm did not produce script locals")?;
        scratch.finish_layer();
        let committed = scratch.commit(skeleton, None, 128)?;
        ensure!(
            committed,
            "prewarm script did not establish the commit gate"
        );
        let mut children = Vec::new();
        for update in updates {
            let Node::Script(child) = self
                .nodes
                .nodes
                .get_mut(&update.id)
                .expect("preflight owner")
            else {
                unreachable!()
            };
            child.normalized_position_bits = update.position.normalized_position_bits;
            child.previous_phase_bits = update.position.previous_phase_bits;
            child.script_phase_bits = update.phase.phase_bits;
            child.transition_scalar_bits = update.transition.scalar_bits;
            child.reciprocal_divisor_bits = update.transition.reciprocal_bits;
            child.flags_raw = u16::try_from(update.phase.child_flags_raw)?;
            children.push(ChildEvidence {
                node: update.id.get(),
                reference: child.reference,
                normalized_position_bits: child.normalized_position_bits,
                previous_phase_bits: child.previous_phase_bits,
                advanced_phase_bits: child.script_phase_bits,
                transition_bits: child.transition_scalar_bits,
                reciprocal_bits: child.reciprocal_divisor_bits,
            });
        }
        let mut hash = Sha256::new();
        hash.update(b"source_context_locals_be_v1");
        hash.update((skeleton.bones().len() as u64).to_be_bytes());
        for bone in skeleton.bones() {
            hash.update(bone.index().to_be_bytes());
            for bits in bone
                .local_translation_bits()
                .into_iter()
                .chain(bone.local_quaternion_bits())
            {
                hash.update(bits.to_be_bytes());
            }
        }
        self.committed_precision = Some(
            candidate
                .locals
                .iter()
                .map(|bone| bone.rotation_precision)
                .collect(),
        );
        self.evidence.prewarm_pose = Some(Evidence {
            state: "source_reset_prewarm_locals_committed",
            seconds_bits: clock::source_prewarm_seconds(),
            baseline: BASELINE,
            scaled_step_bits: scaled_step,
            children,
            updated_controls,
            sampled_controls,
            sampled_scripts,
            skipped_layers,
            selected_clip: selected_clip.expect("script consumer"),
            frame: candidate.frame,
            fade_bits: fade_bits.expect("script consumer"),
            animation_translations: candidate.animation_translations,
            skin_translations: candidate.fallback_translations,
            committed_bones: skeleton.bones().len(),
            scratch_flags_raw: scratch.flags_raw(),
            locals_sha256: format!("{:x}", hash.finalize()),
            locals_encoding: "source_context_locals_be_v1",
            post_update_bindings_sha256: self.nodes.bindings_sha256(),
            unverified_sqrt_rotations: candidate
                .locals
                .iter()
                .filter(|b| b.rotation_precision == Precision::HostSqrtUnverified)
                .count(),
            root_bias_applied: true,
            skin_mapping_applied: true,
            skeleton_committed: true,
            matrices_refreshed: false,
            native_numeric_parity_verified: false,
            external_step_executed: false,
            first_displayed_pose_verified: false,
            camera_frame_ready: false,
            scope: "Source reset's fixed 0.2-second prewarm at the untouched owned FP callback boundary. Active children update tail-to-previous, including the zero-weight layer; current normalized position precedes phase accumulation. The supported source event/root-motion gates are checked. Sampling traverses captured control then script, skips the zero-weight layer, reads root bias and sparse skin translations, and commits local T/Q through the script scratch gate. Host preflight publishes atomically; native mutation is incremental. Positive-W host square roots remain unverified. This executes no external scheduler step, matrix rebuild, live PlayerNew/physics/world activation or displayed camera frame.",
        });
        self.evidence.animation_ticked = true;
        self.evidence.pose_sampled = true;
        Ok(())
    }
}

fn select(child: &ScriptChild, component_flags_raw: u8) -> Result<clip::Selection> {
    clip::select(
        &child
            .clips
            .iter()
            .map(|c| c.source.blend_key_bits)
            .collect::<Vec<_>>(),
        clip::BlendInputs {
            scalar_bits: child.blend_scalar_bits,
            child_flags_raw: u32::from(child.flags_raw),
            component_flags_raw,
        },
        clip::BlendConstants::source_initial(),
        1024,
    )
}

fn update(
    id: NodeId,
    child: &ScriptChild,
    component: &Edb,
    scaled_step: u32,
    delta: u32,
) -> Result<Update> {
    ensure!(
        child.script_phase_bits == 0
            && child.previous_phase_bits == (-1f32).to_bits()
            && child.transition_scalar_bits == 0,
        "unsupported prewarm child history"
    );
    let selected = select(child, 0)?;
    ensure!(
        selected.second_index.is_none(),
        "unsupported prewarm timing clip pair"
    );
    let first = child.clips.first().context("empty prewarm clip list")?;
    let bound = &child.clips[selected.source_index];
    let position = clock::prepare_position(clock::PositionInputs {
        phase_bits: child.script_phase_bits,
        script_duration_raw: child.header.duration_raw,
        script_fps_bits: child.header.frames_per_second.to_bits(),
        play_start_frame: child.selected_command.start,
        first_animation_frames: first.animation.header().frame_count,
        animation_fps: bound.animation.header().frames_per_second,
        child_flags_raw: u32::from(child.flags_raw),
        timing_flags_raw: 0,
        script_flags_raw: child.header.state_flags_raw,
        animation_word38_raw: bound.animation.consumer_flags_bits(component)?,
    })?;
    let rate = clock::clip_rate(bound.animation.header().frames_per_second, ONE, BASELINE)?;
    let phase = clock::advance_phase(clock::PhaseInputs {
        phase_bits: child.script_phase_bits,
        script_duration_raw: child.header.duration_raw,
        first_animation_frames: first.animation.header().frame_count,
        selected_span_bits: (bound.range_frames as f32).to_bits(),
        selected_rate_bits: rate,
        child_flags_raw: u32::from(child.flags_raw),
        component_speed_bits: ONE,
        scaled_step_bits: scaled_step,
    })?;
    ensure!(
        !phase.reprocess_script,
        "unsupported prewarm endpoint callback"
    );
    u16::try_from(phase.child_flags_raw).context("unsupported prewarm extended child flags")?;
    let transition = clock::advance_transition(
        clock::TransitionState {
            scalar_bits: child.transition_scalar_bits,
            reciprocal_bits: child.reciprocal_divisor_bits,
        },
        delta,
    )?;
    Ok(Update {
        id,
        position,
        phase,
        transition,
    })
}

fn full_mask(bones: usize) -> [u32; 4] {
    let mut result = [0; 4];
    for bone in 0..bones {
        result[bone / 32] |= 1 << (bone % 32);
    }
    result
}
