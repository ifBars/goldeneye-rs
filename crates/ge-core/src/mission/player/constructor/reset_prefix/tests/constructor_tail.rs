use super::*;

impl PendingReset {
    pub(crate) fn constructor_tail_fixture(player: PlayerId) -> Self {
        let mut owner = Self {
            player,
            bytes: BTreeMap::new(),
            stage: Stage::Complete,
            character: None,
        };
        for at in [0xafc, 0xaf8, 0xb00, 0xbb0, 0xbb4] {
            owner.word_store(at, 0);
        }
        owner.word_store(0xb84, 0x1234_5678);
        for (at, byte) in [
            (0xc96, 0),
            (0xc98, 23),
            (0xc7d, 255),
            (0xbd0, 7),
            (0xbd1, 9),
            (0xbd2, 11),
        ] {
            owner.bytes.insert(at, byte);
        }
        owner
    }
    pub(crate) fn constructor_tail_test_word(&mut self, at: u16, value: Option<u32>) {
        for delta in 0..4 {
            self.bytes.remove(&(at + delta));
        }
        if let Some(value) = value {
            self.word_store(at, value);
        }
    }
    pub(crate) fn constructor_tail_test_byte(&mut self, at: u16, value: Option<u8>) {
        self.bytes.remove(&at);
        if let Some(value) = value {
            self.bytes.insert(at, value);
        }
    }
}

#[test]
fn cleanup_stores_only_b84_word_and_bd1_byte() {
    let mut reset = PendingReset::constructor_tail_fixture(PlayerId::new(3).unwrap());
    let before = reset.bytes.clone();
    reset.clear_constructor_effect_word().unwrap();
    reset.clear_constructor_marker().unwrap();
    assert_eq!(reset.word(0xb84), Some(0));
    assert_eq!(reset.byte(0xbd1), Some(0));
    for (at, value) in before {
        if !(0xb84..0xb88).contains(&at) && at != 0xbd1 {
            assert_eq!(reset.byte(at), Some(value));
        }
    }
}
