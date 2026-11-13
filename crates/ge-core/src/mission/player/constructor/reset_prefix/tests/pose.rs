use super::*;
impl PendingReset {
    pub(crate) fn pose_fixture(player: PlayerId, character: Option<CharacterBinding>) -> Self {
        let mut reset = Self::constructor_tail_fixture(player);
        reset.character = character;
        reset.bytes.insert(0xba8, 0);
        reset.bytes.insert(0xbab, 7);
        reset.word_store(0x710, 0x0900_0765);
        reset.word_store(0xcfc, 0);
        reset
    }
}
#[test]
fn pose_reset_stores_preserve_adjacent_bytes() {
    let mut reset = PendingReset::pose_fixture(PlayerId::new(4).unwrap(), None);
    reset.bytes.insert(0xbaa, 13);
    reset.bytes.insert(0xbac, 17);
    reset.store_pose_input(0x80).unwrap();
    reset.store_pose_weapon_mode().unwrap();
    assert_eq!(
        (reset.byte(0xbaa), reset.byte(0xbab), reset.byte(0xbac)),
        (Some(13), Some(0x80), Some(17))
    );
    assert_eq!(reset.word(0xca4), Some(0x0e00_0070));
}
