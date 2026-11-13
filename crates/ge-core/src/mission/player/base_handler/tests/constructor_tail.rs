use super::*;

impl Owner {
    pub(crate) fn constructor_tail_fixture(
        player: PlayerId,
        state: Option<u32>,
        alternate: Option<u8>,
        actor: Option<u32>,
        count: Option<u32>,
    ) -> Self {
        let mut owner = Self::new(player);
        owner.stage = Stage::VirtualBasicComplete;
        for (at, word) in [(0x704, state), (0x570, actor), (0x480, count)] {
            if let Some(word) = word {
                owner.w(at, word);
            }
        }
        if let Some(value) = alternate {
            owner.b(0x5ed, value);
        }
        owner
    }
    pub(crate) fn constructor_tail_test_word(&mut self, at: u16, value: u32) {
        self.w(at, value);
    }
}

#[test]
fn count_read_does_not_require_generation_capacity_or_backing() {
    let owner =
        Owner::constructor_tail_fixture(PlayerId::new(2).unwrap(), None, None, None, Some(0));
    assert_eq!(owner.constructor_override_count().unwrap(), 0);
    for at in [0x474, 0x478, 0x47c, 0x484] {
        assert_eq!(owner.word(at), None);
    }
}

#[test]
fn conditional_restore_is_a_literal_write_without_old_state_read() {
    let mut owner =
        Owner::constructor_tail_fixture(PlayerId::new(2).unwrap(), None, None, None, None);
    owner.restore_constructor_state_five().unwrap();
    assert_eq!(owner.word(0x704), Some(5));
}
