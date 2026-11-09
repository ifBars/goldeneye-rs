//! An opaque digest of the owned ordered plan, never serialized source arrays.
use ge_core::animation::mode::{AttemptKind, CleanupIntent, CommandPlan, Selection};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    pub state: &'static str,
    pub input_source: &'static str,
    pub requested_mode: u32,
    pub previous_mode: u32,
    pub fallback_mode: u32,
    pub target_index: Option<usize>,
    pub return_word: u32,
    pub attempts: usize,
    pub matched_transitions: usize,
    pub planned_commands: usize,
    pub mask_operations: usize,
    pub start_sets: usize,
    pub layer_groups: usize,
    pub child_requests: usize,
    pub previous_matches: usize,
    pub fallback_matches: usize,
    pub plan_sha256: String,
    pub plan_encoding: &'static str,
    pub initial_request_verified: bool,
    pub runtime_children_created: bool,
    pub animation_sampled: bool,
    pub camera_frame_ready: bool,
    pub scope: &'static str,
}

impl Evidence {
    pub fn from_selection(plan: &Selection) -> Self {
        let mut result = Self {
            state: if plan.target_index.is_none() {
                "target_missing"
            } else if plan.return_word == 0 {
                "no_nonzero_return"
            } else {
                "planned"
            },
            input_source: "explicit_host_state",
            requested_mode: plan.inputs.requested,
            previous_mode: plan.inputs.previous,
            fallback_mode: plan.inputs.fallback,
            target_index: plan.target_index,
            return_word: plan.return_word,
            attempts: plan.attempts.len(),
            matched_transitions: 0,
            planned_commands: 0,
            mask_operations: 0,
            start_sets: 0,
            layer_groups: 0,
            child_requests: 0,
            previous_matches: 0,
            fallback_matches: 0,
            plan_sha256: String::new(),
            plan_encoding: "mode_plan_be_v1",
            initial_request_verified: false,
            runtime_children_created: false,
            animation_sampled: false,
            camera_frame_ready: false,
            scope: "Owned ordered source mode/transition and layer-request plan from explicit caller inputs. Previous reference zero is a coded index. Source return words, zero-hash effects and fallback attempts remain distinct. Masks and cleanup are intents; allocation, existing layer state, weapon remapping, initial animation history, pose sampling and camera output are not supplied. Hash encoding includes source order, raw flags/weights and derived child requests; original arrays/transforms are never exported.",
        };
        let mut digest = Sha256::new();
        digest.update([1]);
        digest.update(plan.inputs.requested.to_be_bytes());
        digest.update(plan.inputs.previous.to_be_bytes());
        digest.update(plan.inputs.fallback.to_be_bytes());
        index(&mut digest, plan.target_index);
        digest.update(plan.return_word.to_be_bytes());
        digest.update((plan.attempts.len() as u64).to_be_bytes());
        for attempt in &plan.attempts {
            digest.update([match attempt.kind {
                AttemptKind::Previous => 0,
                AttemptKind::Fallback => 1,
            }]);
            digest.update(attempt.reference.to_be_bytes());
            index(&mut digest, attempt.source_index);
            digest.update([u8::from(attempt.source_hash.is_some())]);
            if let Some(hash) = attempt.source_hash {
                digest.update(hash.to_be_bytes());
            }
            index(&mut digest, attempt.matched_transition);
            digest.update((attempt.commands.len() as u64).to_be_bytes());
            if attempt.matched_transition.is_some() {
                result.matched_transitions += 1;
                match attempt.kind {
                    AttemptKind::Previous => result.previous_matches += 1,
                    AttemptKind::Fallback => result.fallback_matches += 1,
                }
            }
            for command in &attempt.commands {
                result.planned_commands += 1;
                match command {
                    CommandPlan::MaskOperation { mask_raw, .. } => {
                        result.mask_operations += 1;
                        digest.update(0x0c00_0005u32.to_be_bytes());
                        digest.update(mask_raw.to_be_bytes());
                    }
                    CommandPlan::StartSet(set) => {
                        result.start_sets += 1;
                        let raw = &set.source;
                        digest.update(0x0c00_0002u32.to_be_bytes());
                        digest.update(raw.set_reference.to_be_bytes());
                        digest.update((raw.set_index as u64).to_be_bytes());
                        digest.update(raw.set_hash.to_be_bytes());
                        digest.update(raw.flags_override.to_be_bytes());
                        digest.update(raw.options_raw.to_be_bytes());
                        digest.update(raw.divisor_override_raw.to_be_bytes());
                        digest.update(set.effective_flags_or.to_be_bytes());
                        match &set.cleanup {
                            CleanupIntent::SuppressedByFlag20 => digest.update([0]),
                            CleanupIntent::Conditional { source_slots } => {
                                digest.update([1]);
                                digest.update((source_slots.len() as u64).to_be_bytes());
                                for slot in source_slots {
                                    digest.update(slot.to_be_bytes());
                                }
                            }
                        }
                        digest.update((set.groups.len() as u64).to_be_bytes());
                        result.layer_groups += set.groups.len();
                        for group in &set.groups {
                            digest.update(group.source_slot.to_be_bytes());
                            digest.update(group.weight_bits.to_be_bytes());
                            digest.update((group.children.len() as u64).to_be_bytes());
                            result.child_requests += group.children.len();
                            for child in &group.children {
                                digest.update(child.source.reference.to_be_bytes());
                                digest.update(child.source.flags_raw.to_be_bytes());
                                digest.update(child.source.divisor_raw.to_be_bytes());
                                digest.update(child.effective_flags.to_be_bytes());
                                digest.update(child.divisor_u32.to_be_bytes());
                                digest.update(child.reciprocal_divisor_bits.to_be_bytes());
                                digest.update(child.source.word8_raw.to_be_bytes());
                            }
                        }
                    }
                }
            }
        }
        result.plan_sha256 = format!("{:x}", digest.finalize());
        result
    }
}

fn index(digest: &mut Sha256, value: Option<usize>) {
    digest.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        digest.update((value as u64).to_be_bytes());
    }
}
