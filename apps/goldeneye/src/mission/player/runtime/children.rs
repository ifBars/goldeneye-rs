//! Owned class-four constructor bindings. Compressed clips remain immutable
//! spans in the retained component EDB; this module performs no frame decoding.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    edb::{AnimationScriptReference, Edb},
    frontend_script::{AnimationClip, Command, Header, PlayAnimation},
    skeletal_animation::{self, Animation},
};
use std::{collections::BTreeMap, sync::Arc};

type AnimationKey = (usize, usize, usize, usize);
const CLIP_LIMIT: usize = 256;
const ANIMATION_LIMIT: usize = 4096;

#[derive(Default)]
pub(super) struct AnimationCache {
    entries: BTreeMap<AnimationKey, Arc<Animation>>,
    references: BTreeMap<u32, AnimationKey>,
}
impl AnimationCache {
    fn bind(&mut self, component: &Edb, reference: u32) -> Result<Arc<Animation>> {
        if let Some(key) = self.references.get(&reference) {
            return Ok(Arc::clone(&self.entries[key]));
        }
        ensure!(
            self.references.len() < ANIMATION_LIMIT,
            "class04 animation binding budget exhausted"
        );
        let animation = component
            .skeletal_animation(
                reference,
                skeletal_animation::Budget {
                    directory_entries: 32_767,
                    rotation_channels: 128,
                    translation_channels: 128,
                    auxiliary_channels: 128,
                    payload_bytes: 16 * 1024 * 1024,
                },
            )?
            .context("class04 skeletal animation reference is absent")?;
        // Verify the compressed span against the actual retained owner now,
        // without copying it or claiming its codec has been decoded.
        let _payload = animation.compressed_payload(component)?;
        let span = animation.payload_span();
        let key = (
            animation.descriptor().directory_index,
            animation.descriptor().address,
            span.address,
            span.bytes,
        );
        let bound = Arc::clone(
            self.entries
                .entry(key)
                .or_insert_with(|| Arc::new(animation)),
        );
        self.references.insert(reference, key);
        Ok(bound)
    }
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}

pub(super) struct BoundClip {
    pub source: AnimationClip,
    pub animation: Arc<Animation>,
    pub range_frames: u32,
}

pub(super) struct ScriptChild {
    pub resource_hash: u32,
    pub reference: u32,
    pub slot: u8,
    pub flags_raw: u16,
    pub descriptor: AnimationScriptReference,
    pub header: Header,
    pub selected_command: Command,
    pub play: PlayAnimation,
    pub clips: Vec<BoundClip>,
    pub normalized_position_bits: u32,
    pub script_phase_bits: u32,
    pub previous_phase_bits: u32,
    pub blend_scalar_bits: u32,
    pub transition_scalar_bits: u32,
    pub reciprocal_divisor_bits: u32,
    pub word8_raw: u32,
}

impl ScriptChild {
    /// Source virtual14: assign both script cursors without evaluating a pose.
    pub(super) fn assign_script_phase(&mut self, value_bits: u32) {
        self.script_phase_bits = value_bits;
        self.previous_phase_bits = value_bits;
    }

    /// Source virtual18: assign input * script FPS / duration, never accumulate.
    /// The caller owns the input's meaning and scheduling.
    #[allow(
        dead_code,
        reason = "Explicit position API; no implicit clock caller yet"
    )]
    pub(super) fn assign_rate_scaled_position(&mut self, input_bits: u32) {
        let numerator = f32::from_bits(input_bits) * self.header.frames_per_second;
        let value = numerator / self.header.duration_raw as f32;
        self.assign_script_phase(value.to_bits());
    }

    /// Source virtual1c: assign input / duration, without the FPS multiply.
    #[allow(
        dead_code,
        reason = "Explicit position API; no implicit clock caller yet"
    )]
    pub(super) fn assign_position(&mut self, input_bits: u32) {
        let value = f32::from_bits(input_bits) / self.header.duration_raw as f32;
        self.assign_script_phase(value.to_bits());
    }

    pub(super) fn bind(
        component: &Edb,
        reference: u32,
        slot: u8,
        flags_raw: u16,
        bones: usize,
        cache: &mut AnimationCache,
    ) -> Result<Self> {
        ensure!(
            slot < 4 && ((reference >> 24) & 0x7f) == 4,
            "unsupported animation child factory class/slot"
        );
        let descriptor = component
            .animation_script_reference(reference)?
            .context("class04 script reference is absent")?;
        ensure!(
            descriptor.section & 0x7fff == 0,
            "unsupported class04 script section ownership"
        );
        let script = component
            .frontend_script(reference)?
            .context("class04 script body is absent")?;
        let header = script.header()?;
        ensure!(
            header.duration_raw > 0 && header.frames_per_second > 0.0,
            "unsupported class04 zero duration/rate"
        );
        // Fresh class04 phase is zero. Source selects the last reached opcode02
        // whose signed start is <= phase, stopping at the next future play/END.
        let mut at = header
            .address
            .checked_add(0x44)
            .context("class04 command address overflow")?;
        let mut selected = None;
        let mut stopped = false;
        for _ in 0..4096 {
            let command = script.command_at(at)?;
            if command.opcode == 0x12 {
                stopped = true;
                break;
            }
            if command.opcode == 2 {
                if command.start > 0 {
                    stopped = true;
                    break;
                }
                selected = Some(command.clone());
            } else {
                anyhow::bail!(
                    "unsupported reached class04 phase-zero opcode {}",
                    command.opcode
                );
            }
            ensure!(
                command.next_bytes != 0,
                "class04 command chain lacks terminal"
            );
            at = at
                .checked_add(usize::from(command.next_bytes))
                .context("class04 command link overflow")?;
        }
        ensure!(stopped, "class04 command traversal budget exhausted");
        let selected_command =
            selected.context("unsupported class04 phase zero without PlayAnimation")?;
        ensure!(
            selected_command.start == 0,
            "unsupported class04 nonzero play start requires phase conversion"
        );
        let play = selected_command
            .play_animation(CLIP_LIMIT)?
            .context("class04 selected command is not PlayAnimation")?;
        ensure!(
            play.secondary_clips.is_empty(),
            "unsupported class04 secondary clips"
        );
        ensure!(
            !play.normal_clips.is_empty()
                && usize::from(play.initial_index_raw) < play.normal_clips.len(),
            "unsupported class04 initial clip index/count"
        );
        ensure!(
            play.flags_raw == 0,
            "unsupported class04 PlayAnimation flags"
        );
        let tracks = script.track_bundle(play.track_selector)?;
        ensure!(
            tracks.channel_mask == 0 && tracks.option_flags_raw == 0,
            "unsupported class04 phase/timing tracks"
        );
        let mut clips = Vec::new();
        clips
            .try_reserve_exact(play.normal_clips.len())
            .context("retaining bounded class04 clips")?;
        for source in &play.normal_clips {
            ensure!(
                source.file_reference_raw == u32::MAX,
                "unsupported class04 external clip file token"
            );
            ensure!(
                source.start_raw == 0,
                "unsupported class04 nonzero clip start requires phase conversion"
            );
            let animation = cache.bind(component, source.animation_reference)?;
            ensure!(
                usize::from(animation.header().rotation_channels) == bones
                    && animation.header().auxiliary_channels == 0,
                "unsupported class04 skin channel mapping or auxiliary channels"
            );
            let range = i32::from(source.end_raw) - i32::from(source.start_raw);
            ensure!(
                range > 0,
                "unsupported class04 nonpositive clip range requires native frame-count fallback"
            );
            clips.push(BoundClip {
                source: *source,
                animation,
                range_frames: range as u32,
            });
        }
        Ok(Self {
            resource_hash: component.header.hash,
            reference,
            slot,
            flags_raw,
            descriptor,
            header,
            selected_command,
            play,
            clips,
            normalized_position_bits: 0,
            script_phase_bits: 0,
            previous_phase_bits: (-1.0f32).to_bits(),
            blend_scalar_bits: 0,
            transition_scalar_bits: 1.0f32.to_bits(),
            reciprocal_divisor_bits: 0,
            word8_raw: 0,
        })
    }
}
