use super::*;

impl PendingReset {
    pub(crate) fn callback_tail_fixture(player: PlayerId, e59: Option<u8>) -> Self {
        let mut reset = Self {
            player,
            bytes: BTreeMap::new(),
            stage: Stage::Complete,
            character: None,
        };
        if let Some(value) = e59 {
            reset.bytes.insert(0xe59, value);
        }
        reset.word_store(0xe60, 0x7f7f_ffff);
        reset
    }
}
