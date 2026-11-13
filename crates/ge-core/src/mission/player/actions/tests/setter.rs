use super::*;
#[derive(Default)]
struct Reader {
    calls: Vec<(u16, u32)>,
    reads: usize,
    head: Option<NodeId>,
    word: u32,
    fail: bool,
}
impl Source for Reader {
    fn history(&mut self, at: u16, word: u32) -> Result<()> {
        self.calls.push((at, word));
        Ok(())
    }
    fn primary(&mut self) -> Result<(Option<NodeId>, u32)> {
        self.reads += 1;
        ensure!(!self.fail, "authored head failure");
        Ok((self.head, self.word))
    }
}
fn trace(word: u32) -> SetterReturn {
    SetterReturn {
        return_word: word,
        history_updated: false,
        primary_changed: None,
    }
}
#[test]
fn untracked_and_zero_returns_skip_all_history_and_head_providers() {
    for (tracks, word) in [(false, 5), (false, 0), (true, 0)] {
        let mut r = Reader {
            fail: true,
            ..Reader::default()
        };
        let mut t = trace(word);
        finish(&mut r, &mut t, tracks, None, 7, 8).unwrap();
        assert!(r.calls.is_empty());
        assert_eq!(r.reads, 0);
        assert!(!t.history_updated);
        assert_eq!(t.primary_changed, None);
    }
}
#[test]
fn tracked_return_stores_requested_history_before_new_head_and_preserves_failure_prefix() {
    let mut r = Reader {
        fail: true,
        ..Reader::default()
    };
    let mut t = trace(5);
    assert!(finish(&mut r, &mut t, true, Some((None, 0)), 7, 8).is_err());
    assert_eq!(r.calls, [(0x70c, 7), (0x714, 8)]);
    assert!(t.history_updated);
    assert_eq!(t.primary_changed, None);
}
#[test]
fn primary_identity_or_reference_change_alone_clears_secondary_history() {
    let id = NodeId::new(1).unwrap();
    for (head, word, changed) in [(None, 0, false), (Some(id), 0, true), (None, 99, true)] {
        let mut r = Reader {
            head,
            word,
            ..Reader::default()
        };
        let mut t = trace(5);
        finish(&mut r, &mut t, true, Some((None, 0)), 7, 8).unwrap();
        assert_eq!(t.primary_changed, Some(changed));
        assert_eq!(r.reads, 1);
        assert_eq!(r.calls.len(), if changed { 3 } else { 2 });
        if changed {
            assert_eq!(r.calls[2], (0x710, 0));
        }
    }
}
