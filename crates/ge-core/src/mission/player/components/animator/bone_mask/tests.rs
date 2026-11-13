use super::*;
use crate::mission::player::components::Registry;

fn owner() -> Animator {
    let identity = Registry::from_constructor(4)
        .allocate(PlayerId::new(9).unwrap())
        .unwrap();
    let mut owner = Animator::from_constructor(identity);
    owner.configure_headers();
    owner.bind_empty_mode_layers().unwrap();
    owner
}

#[test]
fn inactive_physical_header_receives_mask_and_numeric_two_without_activation() {
    let mut owner = owner();
    assert_eq!(owner.layers().unwrap().active_count(), 1);
    assert_eq!(owner.layers().unwrap().slots()[2].bone_mask_words(), None);
    let mask = [0x8000_0001, 0x0123_4567, 0, u32::MAX];
    owner.apply_bone_mask(2, mask).unwrap();
    assert_eq!(owner.layers().unwrap().active_count(), 1);
    for (i, word) in mask.into_iter().enumerate() {
        assert_eq!(owner.word(0x188 + i * 4), Some(word));
    }
    let slot = &owner.layers().unwrap().slots()[2];
    assert_eq!(slot.bone_mask_words(), Some(mask));
    assert_eq!(slot.flags_raw(), 3);
    assert_eq!(owner.byte(0x181), Some(3));
    assert_eq!(owner.byte(0x182), Some(3));
    assert!(slot.nodes_head().is_empty());
    assert_eq!(slot.weight_bits(), 1f32.to_bits());
    assert_eq!(owner.configured_header_flags(), Some([1; 4]));
}

#[test]
fn successful_zero_mask_still_sets_the_flag_and_can_replace_a_previous_mask() {
    let mut owner = owner();
    owner.apply_bone_mask(0, [u32::MAX; 4]).unwrap();
    owner.apply_bone_mask(0, [0; 4]).unwrap();
    assert_eq!(
        owner.layers().unwrap().slots()[0].bone_mask_words(),
        Some([0; 4])
    );
    assert_eq!(owner.layers().unwrap().slots()[0].flags_raw(), 3);
    assert_eq!(owner.layers().unwrap().slots()[1].bone_mask_words(), None);
}

#[test]
fn unknown_destination_flags_fail_before_any_mask_copy() {
    let mut owner = owner();
    owner.bytes[0x141] = None;
    let before = owner.bytes;
    assert!(owner.apply_bone_mask(0, [7; 4]).is_err());
    assert_eq!(owner.bytes, before);
    assert_eq!(owner.layers().unwrap().slots()[0].bone_mask_words(), None);
    assert_eq!(owner.layers().unwrap().slots()[0].flags_raw(), 1);
    assert_eq!(owner.byte(0x141), None);
}

#[test]
fn invalid_slot_or_missing_layers_does_not_write_another_header() {
    let mut owner = owner();
    let before = owner.bytes;
    assert!(owner.apply_bone_mask(4, [7; 4]).is_err());
    assert_eq!(owner.bytes, before);
    owner.layers = None;
    assert!(owner.apply_bone_mask(0, [7; 4]).is_err());
    assert_eq!(owner.bytes, before);
}
