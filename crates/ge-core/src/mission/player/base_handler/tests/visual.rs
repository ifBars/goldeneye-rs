use super::*;
use crate::mission::player::{components::Registry, identity::PlayerId};

fn owner() -> Owner {
    // Explicit boundary snapshot using the same real container constructor.
    let mut owner = Owner::new(PlayerId::new(7).unwrap());
    owner.container(0x474, 16);
    owner.zero_words(0x464, 0x46c);
    owner.stage = Stage::VirtualBasicComplete;
    owner
}
#[test]
fn empty_override_reset_preserves_growth_and_wraps_source_generation() {
    let mut owner = owner();
    owner.w(0x474, u32::MAX);
    let reset = owner.reset_empty_visual_overrides().unwrap();
    assert_eq!(
        reset,
        VisualOverrides {
            generation: 0,
            capacity: 0,
            growth: 16,
            count: 0
        }
    );
    assert_eq!(owner.word(0x484), Some(0));
}
#[test]
fn nonempty_unknown_or_backed_collection_requires_actual_cleanup() {
    for at in [0x480, 0x484] {
        let mut owner = owner();
        owner.w(at, 1);
        assert!(owner.reset_empty_visual_overrides().is_err());
        assert_eq!(owner.word(0x474), Some(0));
    }
    let mut owner = owner();
    owner.words.remove(&0x474);
    assert!(owner.reset_empty_visual_overrides().is_err());
}
#[test]
fn skeleton_link_replaces_actual_constructor_null_without_fabricating_an_address() {
    let mut owner = owner();
    owner.clear_visual_slots().unwrap();
    let mut registry = Registry::from_constructor(2);
    let wrong = registry.allocate(PlayerId::new(8).unwrap()).unwrap();
    assert!(owner.bind_visual_skeleton(wrong).is_err());
    assert_eq!(owner.word(0x464), Some(0));
    let identity = registry.allocate(owner.player()).unwrap();
    owner.bind_visual_skeleton(identity).unwrap();
    assert_eq!(owner.visual_skeleton(), Some(identity));
    assert_eq!(owner.word(0x464), None);
    assert!(owner.clear_visual_slots().is_err());
    assert_eq!(owner.word(0x468), Some(0));
    assert_eq!(owner.word(0x46c), Some(0));
}
