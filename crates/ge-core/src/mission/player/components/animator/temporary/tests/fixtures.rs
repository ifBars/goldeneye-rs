//! Test-only mutable access to a temporary animator's retained skeleton.
use super::*;

pub(crate) fn skeleton_mut(animator: &mut Animator) -> Option<&mut SkeletonRuntime> {
    animator.skeleton.as_mut()
}

/// Authored sparse word store; None makes the word unknown.
pub(crate) fn store_word(animator: &mut Animator, at: usize, bits: Option<u32>) {
    match bits {
        Some(bits) => animator.word_store(at, bits),
        None => animator.bytes[at..at + 4].fill(None),
    }
}

/// Authored sparse word store on one retained script header.
pub(crate) fn store_header_word(
    animator: &mut Animator,
    id: crate::animation::layers::NodeId,
    at: usize,
    bits: Option<u32>,
) {
    crate::animation::script_child::destroy::fixtures::store_header_word(
        &mut animator.script_children,
        id,
        at,
        bits,
    );
}
