use super::*;
use crate::mission::player::identity::WorldId;

#[test]
fn motion_clear_uses_halfword_widths_and_preserves_unreached_storage() {
    let item = ItemId::new(WorldId::new(9).unwrap(), 3).unwrap();
    let mut owner = Owner::from_constructor();
    owner.bind_item_identity(item).unwrap();
    owner.store_half(8, 0xffff);
    owner.store_half(0xa, 0x1234);
    owner.store_half(0xc, 0x5678);
    owner.store_word(0x15c, u32::MAX);
    let before = owner.known;
    assert_eq!(owner.clear_callback_motion(item).unwrap(), (0xffff, 0xfcfc));
    assert_eq!(owner.half(8), Some(0xfcfc));
    assert_eq!(owner.half(0xa), Some(0));
    assert_eq!(owner.half(0xc), Some(0x5678));
    assert_eq!(owner.word(0x15c), Some(0));
    for (at, byte) in before.into_iter().enumerate() {
        if !(8..0xc).contains(&at) && !(0x15c..0x160).contains(&at) {
            assert_eq!(owner.known[at], byte, "changed unrelated byte {at:x}");
        }
    }
}

#[test]
fn unknown_physics_flags_fail_before_any_clear_store() {
    let item = ItemId::new(WorldId::new(9).unwrap(), 3).unwrap();
    let mut owner = Owner::from_constructor();
    owner.bind_item_identity(item).unwrap();
    owner.known[9] = None;
    let before = owner.known;
    assert!(owner.clear_callback_motion(item).is_err());
    assert_eq!(owner.known, before);
}
