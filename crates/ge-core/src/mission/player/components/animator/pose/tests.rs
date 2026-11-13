use super::*;
use crate::mission::player::components::Registry;
impl Animator {
    pub(crate) fn pose_fixture(identity: Identity, item: ItemId, binding: ResourceBinding) -> Self {
        let mut owner = Self::from_constructor(identity);
        owner.prepare_first_item_append(item).unwrap();
        owner.component = Some(binding);
        owner.word_store(0x20, binding.reference);
        owner.bind_empty_mode_layers().unwrap();
        owner
    }
}
fn owner() -> Animator {
    Animator::from_constructor(
        Registry::from_constructor(2)
            .allocate(PlayerId::new(3).unwrap())
            .unwrap(),
    )
}
#[test]
fn semantic_enable_uses_live_reference_and_only_touches_component_flags() {
    let mut owner = owner();
    owner.word_store(0x20, 0x8400_0021);
    owner.bytes[0x8e] = Some(7);
    let before = owner.bytes;
    let missed = owner.apply_pose_enable(0, false).unwrap();
    assert!(!missed.reference_matched);
    assert_eq!(owner.bytes, before);
    let disabled = owner.apply_pose_enable(0x8400_0021, false).unwrap();
    assert_eq!(
        (disabled.flags_before, disabled.flags_after),
        (Some(0xb), Some(0xa))
    );
    assert!(!disabled.invalidated_8e);
    assert_eq!(owner.byte(0x8e), Some(7));
    for (at, value) in before.into_iter().enumerate() {
        if !(0xc..0xe).contains(&at) {
            assert_eq!(owner.bytes[at], value);
        }
    }
    let enabled = owner.apply_pose_enable(0x8400_0021, true).unwrap();
    assert_eq!(
        (enabled.flags_before, enabled.flags_after),
        (Some(0xa), Some(0xb))
    );
    assert!(enabled.invalidated_8e);
    assert_eq!(owner.byte(0x8e), Some(0));
}
#[test]
fn missing_flags_fail_before_enabled_invalidation() {
    let mut owner = owner();
    owner.bytes[0xd] = None;
    owner.bytes[0x8e] = Some(7);
    let before = owner.bytes;
    assert!(owner.apply_pose_enable(0, true).is_err());
    assert_eq!(owner.bytes, before);
}
