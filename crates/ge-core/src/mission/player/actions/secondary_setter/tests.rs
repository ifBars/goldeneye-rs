use super::*;
use std::cell::RefCell;

fn inputs(tracks: bool) -> Inputs {
    Inputs {
        player: PlayerId::new(7).unwrap(),
        generic: 0x0900_00a1,
        mapped: 0x0900_00b2,
        tracks,
    }
}
fn trace(return_word: u32) -> SetterReturn {
    SetterReturn {
        return_word,
        history_updated: false,
        primary_changed: None,
    }
}

#[test]
fn tracked_entry_clears_before_reading_only_secondary_previous() {
    let calls = RefCell::new(Vec::new());
    let previous = enter_source(
        true,
        || {
            calls.borrow_mut().push(0x1c0);
            Ok(())
        },
        |offset| {
            calls.borrow_mut().push(offset);
            (offset == 0x718).then_some(0x0900_00c3)
        },
    )
    .unwrap();
    assert_eq!(previous, 0x0900_00c3);
    assert_eq!(*calls.borrow(), [0x1c0, 0x718]);
}

#[test]
fn untracked_entry_reads_primary_generic_without_calling_clear() {
    assert_eq!(
        enter_source(
            false,
            || panic!("untracked entry must not clear"),
            |offset| {
                assert_eq!(offset, 0x70c);
                Some(0x0900_00d4)
            },
        )
        .unwrap(),
        0x0900_00d4
    );
}

#[test]
fn entry_failure_keeps_clear_prefix_and_does_not_read_after_clear_failure() {
    let calls = RefCell::new(Vec::new());
    assert!(
        enter_source(
            true,
            || {
                calls.borrow_mut().push(0x1c0);
                Ok(())
            },
            |offset| {
                calls.borrow_mut().push(offset);
                None
            },
        )
        .is_err()
    );
    assert_eq!(*calls.borrow(), [0x1c0, 0x718]);
    assert!(
        enter_source(
            true,
            || anyhow::bail!("authored clear failure"),
            |_| panic!("history read after failed entry"),
        )
        .is_err()
    );
}

#[test]
fn untracked_and_zero_selection_skip_every_history_provider() {
    for (tracks, return_word) in [(false, 0), (false, 9), (true, 0)] {
        let mut result = trace(return_word);
        finish(&inputs(tracks), &mut result, |_, _| {
            panic!("skipped history provider")
        })
        .unwrap();
        assert_eq!(result, trace(return_word));
    }
}

#[test]
fn nonzero_selection_stores_generic_then_mapped_without_primary_writes() {
    let mut words =
        std::collections::BTreeMap::from([(0x70c, 17), (0x714, 18), (0x710, 19), (0x718, 20)]);
    let mut calls = Vec::new();
    let mut result = trace(0x0900_00e5);
    finish(&inputs(true), &mut result, |offset, word| {
        calls.push((offset, word));
        words.insert(offset, word);
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, [(0x710, 0x0900_00a1), (0x718, 0x0900_00b2)]);
    assert_eq!(words[&0x70c], 17);
    assert_eq!(words[&0x714], 18);
    assert!(result.history_updated);
    assert_eq!(result.primary_changed, None);
    assert_eq!(result.return_word, 0x0900_00e5);
}

#[test]
fn failed_history_store_retains_exact_prefix_before_history_updated() {
    for failed_offset in [0x710, 0x718] {
        let mut calls = Vec::new();
        let mut stored = Vec::new();
        let mut result = trace(9);
        assert!(
            finish(&inputs(true), &mut result, |offset, word| {
                calls.push((offset, word));
                ensure!(offset != failed_offset, "authored history write failure");
                stored.push((offset, word));
                Ok(())
            })
            .is_err()
        );
        assert_eq!(calls[0], (0x710, 0x0900_00a1));
        assert_eq!(calls.len(), if failed_offset == 0x710 { 1 } else { 2 });
        assert_eq!(
            stored,
            if failed_offset == 0x710 {
                vec![]
            } else {
                vec![(0x710, 0x0900_00a1)]
            }
        );
        assert!(!result.history_updated);
        assert_eq!(result.primary_changed, None);
        assert_eq!(result.return_word, 9);
    }
}
