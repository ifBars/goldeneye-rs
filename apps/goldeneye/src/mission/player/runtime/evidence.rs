use super::{animation, host::OwnedNodes, inventory};
use ge_core::animation::layers::State;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    pub state: &'static str,
    pub boundary: &'static str,
    pub inventory: inventory::Evidence,
    pub mode_requests: usize,
    pub active_layers: u8,
    pub live_nodes: usize,
    pub controls: usize,
    pub script_children: usize,
    pub bound_clips: usize,
    pub unique_animations: usize,
    pub captured_bones: usize,
    pub selection_mask: u8,
    pub clear_mask: u8,
    pub slot_zero_nodes: usize,
    pub slot_one_nodes: usize,
    pub membership_sha256: String,
    pub bindings_sha256: String,
    pub bindings_encoding: &'static str,
    pub post_selector_scalars_applied: bool,
    pub post_selector_head_writes: usize,
    pub raw_frame: Option<super::frame::Evidence>,
    pub prewarm_pose: Option<super::prewarm::Evidence>,
    pub animation_ticked: bool,
    pub pose_sampled: bool,
    pub player_initialized: bool,
    pub camera_frame_ready: bool,
    pub scope: &'static str,
}
impl Evidence {
    pub(super) fn new(
        inventory: &inventory::Evidence,
        state: &State,
        nodes: &OwnedNodes,
        plans: &[animation::PreparedAnimation],
    ) -> Self {
        let mut hash = Sha256::new();
        hash.update([
            state.active_count(),
            state.selection_mask(),
            state.clear_mask(),
        ]);
        for slot in state.slots() {
            hash.update(slot.flags_raw().to_be_bytes());
            hash.update(slot.weight_bits().to_be_bytes());
            hash.update((slot.nodes_head().len() as u64).to_be_bytes());
            for id in slot.nodes_head() {
                hash.update(id.get().to_be_bytes());
                hash.update(nodes.reference(*id).unwrap_or(0).to_be_bytes());
            }
        }
        Self {
            state: "fresh_fp_layers_initialized",
            boundary: "fresh_fp_constructor_callback_snapshot",
            inventory: inventory.clone(),
            mode_requests: plans.len(),
            active_layers: state.active_count(),
            live_nodes: nodes.len(),
            controls: nodes.control_count(),
            script_children: nodes.script_count(),
            bound_clips: nodes.clip_count(),
            unique_animations: nodes.animation_count(),
            captured_bones: nodes.capture_bone_count(),
            selection_mask: state.selection_mask(),
            clear_mask: state.clear_mask(),
            slot_zero_nodes: state.slots()[0].nodes_head().len(),
            slot_one_nodes: state.slots()[1].nodes_head().len(),
            membership_sha256: format!("{:x}", hash.finalize()),
            bindings_sha256: nodes.bindings_sha256(),
            bindings_encoding: "owned_animation_nodes_be_v1",
            post_selector_scalars_applied: nodes.head_blend_writes() == 2,
            post_selector_head_writes: nodes.head_blend_writes(),
            raw_frame: None,
            prewarm_pose: None,
            animation_ticked: false,
            pose_sampled: false,
            player_initialized: false,
            camera_frame_ready: false,
            scope: "Actual owned default inventory, supplied weapon DATA, mode plans, control capture and class04 script/clip objects executed at an explicit fresh FP callback boundary. Layer membership, constructor scalars and both source post-selector head blend writes exist. Original data remains local and public receipts export only counts, identifiers and digests. Optional raw-frame and fixed reset-prewarm evidence are separate from this constructor snapshot. The prewarm can advance supported child clocks, prepare skin/root locals and commit the skeleton; physics, live player/world activation, matrices and the displayed game frame remain incomplete.",
        }
    }
}
