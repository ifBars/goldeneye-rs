use super::*;

#[test]
fn actual_constructor_retains_null_links_four_headers_one_active_slot_and_unknown_padding() {
    let owner = Animator::from_constructor(PlayerId::new(37).unwrap());
    assert_eq!(owner.player().get(), 37);
    for at in [0, 4, 8, 0x114, 0x118, 0x124, 0x12c] {
        assert_eq!(owner.word(at), Some(0));
    }
    assert_eq!(owner.byte(0x130), Some(1));
    assert_eq!(owner.byte(0x131), Some(0xff));
    assert_eq!(owner.byte(0x136), Some(0xff));
    for start in [0x138, 0x158, 0x178, 0x198] {
        assert_eq!(owner.word(start), Some(0));
        assert_eq!(owner.word(start + 4), Some(0));
        assert_eq!(owner.byte(start + 10), Some(2));
        assert_eq!(owner.word(start + 12), Some(1f32.to_bits()));
        assert_eq!(owner.word(start + 16), Some(u32::MAX));
        assert_eq!(owner.word(start + 28), Some(u32::MAX));
        assert_eq!(owner.byte(start + 11), None);
    }
    for at in [0xe, 0x18, 0x69 + 1, 0x111, 0x137, SOURCE_BYTES, usize::MAX] {
        assert_eq!(owner.byte(at), None);
    }
}

#[test]
fn base_flag_keeps_unwritten_bit_and_constructor_transforms_are_not_sampled_poses() {
    let owner = Animator::from_constructor(PlayerId::new(1).unwrap());
    assert_eq!(owner.byte(0x8f), None);
    assert_eq!(owner.flag_8f_known_bits(), (0xfe, 0));
    assert_eq!(owner.word(0x108), Some(1f32.to_bits()));
    assert_eq!(owner.word(0x10c), Some(0));
    for index in 0_usize..16 {
        assert_eq!(
            owner.word(0x98 + index * 4),
            Some(if index.is_multiple_of(5) {
                1f32.to_bits()
            } else {
                0
            })
        );
    }
    assert_eq!(owner.word(usize::MAX), None);
    let mut first = Vec::new();
    owner.visit_state_words(|word| first.push(word));
    let other = Animator::from_constructor(PlayerId::new(2).unwrap());
    let mut second = Vec::new();
    other.visit_state_words(|word| second.push(word));
    assert_ne!(first, second);
}
