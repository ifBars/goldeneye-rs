//! Actual node owners for the pure layer executor. No source arrays serialize.
use super::children::{AnimationCache, ScriptChild};
use anyhow::{Context, Result, ensure};
use ge_assets::edb::Edb;
use ge_core::animation::{
    layers::{Host, NodeId},
    mode::ChildPlan,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct ControlBone {
    pub translation_bits: [u32; 4],
    pub quaternion_bits: [u32; 4],
}
#[derive(Clone)]
pub(super) struct ControlCapture {
    pub bones: Vec<ControlBone>,
    pub auxiliary_count: u32,
}

pub(super) enum Node {
    Control(ControlCapture),
    Script(Box<ScriptChild>),
}

pub(super) struct NodeHost<'a> {
    component: &'a Edb,
    capture: ControlCapture,
    next: u64,
    nodes: BTreeMap<NodeId, Node>,
    animations: AnimationCache,
    head_blend_writes: usize,
    update_flags_raw: u16,
}
pub(super) struct OwnedNodes {
    pub nodes: BTreeMap<NodeId, Node>,
    animations: AnimationCache,
    capture_bones: usize,
    head_blend_writes: usize,
    update_flags_raw: u16,
}

impl<'a> NodeHost<'a> {
    pub fn new(component: &'a Edb, capture: ControlCapture) -> Self {
        Self {
            component,
            capture,
            next: 0,
            nodes: BTreeMap::new(),
            animations: AnimationCache::default(),
            head_blend_writes: 0,
            // Base component constructor stores 0x000b at +0xc.
            update_flags_raw: 0x000b,
        }
    }
    pub fn finish(self) -> OwnedNodes {
        OwnedNodes {
            nodes: self.nodes,
            animations: self.animations,
            capture_bones: self.capture.bones.len(),
            head_blend_writes: self.head_blend_writes,
            update_flags_raw: self.update_flags_raw,
        }
    }
    /// The source player setter addresses only the current expected script head.
    /// A live control head is a type mismatch, not permission to search behind it.
    pub fn set_head_blend(&mut self, slot_nodes: &[NodeId], value_bits: u32) -> Result<bool> {
        let Some(head) = slot_nodes.first() else {
            return Ok(false);
        };
        let node = self
            .nodes
            .get_mut(head)
            .context("animation blend head owner is absent")?;
        let Node::Script(child) = node else {
            return Ok(false);
        };
        let writes = self
            .head_blend_writes
            .checked_add(1)
            .context("animation blend write count overflow")?;
        child.blend_scalar_bits = value_bits;
        self.head_blend_writes = writes;
        Ok(true)
    }
    pub fn head_blend_writes(&self) -> usize {
        self.head_blend_writes
    }
    fn allocate(&mut self, node: Node) -> Result<NodeId> {
        ensure!(
            self.nodes.len() < 4096,
            "owned animation node budget exhausted"
        );
        self.next = self
            .next
            .checked_add(1)
            .context("animation node identity overflow")?;
        let id = NodeId::new(self.next)?;
        ensure!(
            self.nodes.insert(id, node).is_none(),
            "animation node identity collision"
        );
        Ok(id)
    }
    fn validate_capture(&self) -> Result<()> {
        ensure!(
            !self.capture.bones.is_empty() && self.capture.bones.len() <= 128,
            "unsupported control capture bone count"
        );
        ensure!(
            self.capture.auxiliary_count == 0,
            "unsupported control capture auxiliary channels"
        );
        Ok(())
    }
}
impl Host for NodeHost<'_> {
    fn create_control(&mut self) -> Result<Option<NodeId>> {
        self.validate_capture()?;
        // The reached capture refreshes its owner before copying locals. This
        // supported fresh factory has no children yet: LOD binding is absent,
        // no sample can set4000, and ordinary dispatch clears component+c bit8.
        ensure!(
            self.nodes.is_empty(),
            "unsupported control capture of active animation children"
        );
        self.update_flags_raw &= !8;
        self.allocate(Node::Control(self.capture.clone())).map(Some)
    }
    fn clone_control(&mut self, source: NodeId) -> Result<Option<NodeId>> {
        let Node::Control(capture) = self
            .nodes
            .get(&source)
            .context("control clone owner is absent")?
        else {
            anyhow::bail!("control clone source has another type");
        };
        self.allocate(Node::Control(capture.clone())).map(Some)
    }
    fn create_child(&mut self, slot: u8, child: &ChildPlan) -> Result<Option<NodeId>> {
        if child.source.reference == 0 {
            return Ok(None);
        }
        self.validate_capture()?;
        ensure!(
            child.factory_class == 4,
            "unsupported owned animation child factory class"
        );
        let bound = ScriptChild::bind(
            self.component,
            child.source.reference,
            slot,
            child.effective_flags,
            self.capture.bones.len(),
            &mut self.animations,
        )?;
        self.allocate(Node::Script(Box::new(bound))).map(Some)
    }
    fn copy_head(&mut self, new: NodeId, old: NodeId) -> Result<()> {
        let Node::Script(source) = self
            .nodes
            .get(&old)
            .context("copied animation head owner absent")?
        else {
            anyhow::bail!("unsupported copied animation head value provider");
        };
        let value = source.normalized_position_bits;
        ensure!(value == 0, "unsupported copied nonzero class04 phase");
        let Node::Script(target) = self
            .nodes
            .get_mut(&new)
            .context("new animation child owner absent")?
        else {
            anyhow::bail!("copied animation target is not a script child");
        };
        target.assign_script_phase(value);
        Ok(())
    }
    fn initialize_child(&mut self, node: NodeId, child: &ChildPlan) -> Result<()> {
        let Node::Script(target) = self
            .nodes
            .get_mut(&node)
            .context("initialized animation child owner absent")?
        else {
            anyhow::bail!("initialized animation node is not a script child");
        };
        ensure!(
            target.reference == child.source.reference,
            "initialized animation reference differs"
        );
        target.transition_scalar_bits = child.scalar_zero_bits;
        target.reciprocal_divisor_bits = child.reciprocal_divisor_bits;
        target.word8_raw = child.source.word8_raw;
        Ok(())
    }
    fn destroy_node(&mut self, node: NodeId) -> Result<()> {
        self.nodes
            .remove(&node)
            .context("destroyed animation node owner absent")?;
        Ok(())
    }
}
impl OwnedNodes {
    pub fn update_flags_raw(&self) -> u16 {
        self.update_flags_raw
    }
    pub fn script(&self, id: NodeId) -> Option<&ScriptChild> {
        match self.nodes.get(&id)? {
            Node::Script(child) => Some(child),
            Node::Control(_) => None,
        }
    }
    pub fn head_blend_writes(&self) -> usize {
        self.head_blend_writes
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn control_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| matches!(node, Node::Control(_)))
            .count()
    }
    pub fn script_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|node| matches!(node, Node::Script(_)))
            .count()
    }
    pub fn clip_count(&self) -> usize {
        self.nodes
            .values()
            .map(|node| match node {
                Node::Script(child) => child.clips.len(),
                _ => 0,
            })
            .sum()
    }
    /// Actual unique metadata/payload-span owners retained by the binding cache.
    pub fn animation_count(&self) -> usize {
        self.animations.len()
    }
    pub fn reference(&self, id: NodeId) -> Option<u32> {
        match self.nodes.get(&id)? {
            Node::Script(child) => Some(child.reference),
            Node::Control(_) => None,
        }
    }
    pub fn capture_bone_count(&self) -> usize {
        self.capture_bones
    }

    /// Stable BE v1 encoding of node IDs/types, constructor/capture state,
    /// source metadata and ordered clips. No compressed frames are sampled.
    pub fn bindings_sha256(&self) -> String {
        let mut h = Sha256::new();
        h.update(b"owned_animation_nodes_be_v1");
        count(&mut h, self.nodes.len());
        for (id, node) in &self.nodes {
            h.update(id.get().to_be_bytes());
            match node {
                Node::Control(capture) => {
                    h.update([0]);
                    h.update(capture.auxiliary_count.to_be_bytes());
                    count(&mut h, capture.bones.len());
                    for bone in &capture.bones {
                        for bits in bone
                            .translation_bits
                            .into_iter()
                            .chain(bone.quaternion_bits)
                        {
                            h.update(bits.to_be_bytes());
                        }
                    }
                }
                Node::Script(child) => {
                    h.update([1, child.slot]);
                    for raw in [
                        child.resource_hash,
                        child.reference,
                        child.normalized_position_bits,
                        child.script_phase_bits,
                        child.previous_phase_bits,
                        child.blend_scalar_bits,
                        child.transition_scalar_bits,
                        child.reciprocal_divisor_bits,
                        child.word8_raw,
                    ] {
                        h.update(raw.to_be_bytes());
                    }
                    h.update(child.flags_raw.to_be_bytes());
                    let descriptor = &child.descriptor;
                    count(&mut h, descriptor.directory_index);
                    for raw in [
                        descriptor.hash,
                        descriptor.address,
                        descriptor.runtime_pointer_raw,
                    ] {
                        h.update(raw.to_be_bytes());
                    }
                    h.update(descriptor.section.to_be_bytes());
                    h.update(descriptor.debug.to_be_bytes());
                    let header = &child.header;
                    count(&mut h, header.address);
                    for raw in [
                        header.duration_raw,
                        header.frames_per_second.to_bits(),
                        header.state_flags_raw,
                    ] {
                        h.update(raw.to_be_bytes());
                    }
                    h.update([header.slots, header.variables]);
                    h.update(header.flags_raw.to_be_bytes());
                    for raw in header.bounds_words_raw {
                        h.update(raw.to_be_bytes());
                    }
                    for raw in [
                        header.table_30_relative_raw,
                        header.tracks_relative_raw,
                        header.slot_resources_relative_raw,
                    ] {
                        h.update(raw.to_be_bytes());
                    }
                    h.update(header.command_count_raw.to_be_bytes());
                    h.update(header.channel_cache_count_raw.to_be_bytes());
                    let command = &child.selected_command;
                    count(&mut h, command.address);
                    h.update([
                        command.opcode,
                        command.next_bytes,
                        command.previous_bytes,
                        command.flags_raw,
                    ]);
                    h.update(command.start.to_be_bytes());
                    h.update(command.duration.to_be_bytes());
                    count(&mut h, command.bytes_raw.len());
                    h.update(&command.bytes_raw);
                    let play = &child.play;
                    count(&mut h, play.address);
                    h.update(play.track_selector.to_be_bytes());
                    for raw in [play.word_0c_raw, play.word_4_raw, play.word_8_raw] {
                        h.update(raw.to_be_bytes());
                    }
                    h.update([
                        play.normal_count_raw,
                        play.flags_raw,
                        play.initial_index_raw,
                        play.secondary_count_raw,
                    ]);
                    count(&mut h, child.clips.len());
                    for clip in &child.clips {
                        let raw = clip.source;
                        count(&mut h, raw.source_address);
                        for word in [
                            raw.file_reference_raw,
                            raw.animation_reference,
                            raw.blend_key_bits,
                            clip.range_frames,
                        ] {
                            h.update(word.to_be_bytes());
                        }
                        h.update(raw.start_raw.to_be_bytes());
                        h.update(raw.end_raw.to_be_bytes());
                        let animation = &clip.animation;
                        let d = animation.descriptor();
                        h.update(d.reference.to_be_bytes());
                        count(&mut h, d.directory_index);
                        count(&mut h, d.source_address);
                        count(&mut h, d.address);
                        h.update(d.hash.to_be_bytes());
                        h.update(d.section_raw.to_be_bytes());
                        h.update(d.debug_raw.to_be_bytes());
                        for word in d.words_raw {
                            h.update(word.to_be_bytes());
                        }
                        let a = animation.header();
                        count(&mut h, a.address);
                        for word in [
                            a.stored_key_raw,
                            a.word_08_raw,
                            a.word_10_raw,
                            a.word_18_raw,
                            a.runtime_wrapper_raw,
                        ] {
                            h.update(word.to_be_bytes());
                        }
                        h.update([
                            a.frames_per_second,
                            a.cache_chunk_frames,
                            a.rotation_channels,
                            a.translation_channels,
                        ]);
                        h.update(a.frame_count.to_be_bytes());
                        h.update(a.auxiliary_channels.to_be_bytes());
                        for word in a
                            .model_pair_raw
                            .into_iter()
                            .chain(a.words_28_raw)
                            .chain(a.translation_mask_raw)
                        {
                            h.update(word.to_be_bytes());
                        }
                        count(&mut h, animation.translation_bones().len());
                        for index in animation.translation_bones() {
                            h.update(index.to_be_bytes());
                        }
                        let span = animation.payload_span();
                        count(&mut h, span.address);
                        count(&mut h, span.bytes);
                    }
                }
            }
        }
        format!("{:x}", h.finalize())
    }
}

fn count(hash: &mut Sha256, value: usize) {
    hash.update((value as u64).to_be_bytes());
}

#[cfg(test)]
#[path = "tests/host.rs"]
mod tests;
