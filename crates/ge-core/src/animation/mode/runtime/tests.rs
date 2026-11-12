use super::*;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    Resolve(u32),
    Mode(usize),
    Target(usize, usize),
    Count(usize, usize),
}
#[derive(Default)]
struct Source {
    references: BTreeMap<u32, usize>,
    modes: BTreeMap<usize, Mode>,
    targets: BTreeMap<(usize, usize), u32>,
    counts: BTreeMap<(usize, usize), usize>,
    reads: Vec<Read>,
}
impl Provider for Source {
    fn resolve_mode(&mut self, reference: u32) -> Result<Option<usize>> {
        self.reads.push(Read::Resolve(reference));
        Ok(self.references.get(&reference).copied())
    }
    fn mode(&mut self, index: usize) -> Result<Mode> {
        self.reads.push(Read::Mode(index));
        self.modes
            .get(&index)
            .copied()
            .context("unreached mode must stay unread")
    }
    fn transition_target(&mut self, index: usize, row: usize) -> Result<u32> {
        self.reads.push(Read::Target(index, row));
        self.targets
            .get(&(index, row))
            .copied()
            .context("transition provider absent")
    }
    fn command_count(&mut self, index: usize, row: usize) -> Result<usize> {
        self.reads.push(Read::Count(index, row));
        self.counts
            .get(&(index, row))
            .copied()
            .context("unmatched command list must stay unread")
    }
}
fn inputs() -> Inputs {
    Inputs {
        requested: 0x0900_00a1,
        previous: 0,
        fallback: 0x0900_00b1,
        resource_available: true,
    }
}
fn budget() -> Budget {
    Budget {
        lookups: 3,
        transition_rows: 8,
        commands: 8,
        groups: 8,
        children: 8,
    }
}
fn fixture() -> Source {
    Source {
        references: BTreeMap::from([(inputs().requested, 9), (0, 0), (inputs().fallback, 3)]),
        modes: BTreeMap::from([
            (
                0,
                Mode {
                    hash: 0,
                    transition_count: 3,
                },
            ),
            (
                3,
                Mode {
                    hash: 0x0900_00b1,
                    transition_count: 1,
                },
            ),
        ]),
        targets: BTreeMap::from([((0, 0), 8), ((0, 1), 9), ((3, 0), 9)]),
        counts: BTreeMap::from([((0, 1), 2), ((3, 0), 1)]),
        ..Source::default()
    }
}
#[test]
fn zero_hash_previous_commands_complete_before_fallback_and_no_later_duplicate_is_read() {
    let mut source = fixture();
    let mut cursor = Cursor::new(inputs(), budget());
    let first = cursor.next_command(&mut source).unwrap().unwrap();
    assert_eq!(
        (first.attempt, first.mode, first.transition, first.ordinal),
        (0, 0, 1, 0)
    );
    assert_eq!(cursor.return_word(), None);
    assert_eq!(cursor.completed_commands(), 0);
    let reads = source.reads.clone();
    assert!(cursor.next_command(&mut source).is_err());
    assert_eq!(source.reads, reads);
    assert!(
        cursor
            .executed(Invocation {
                ordinal: 1,
                ..first
            })
            .is_err()
    );
    cursor.executed(first).unwrap();
    assert!(cursor.executed(first).is_err());
    let second = cursor.next_command(&mut source).unwrap().unwrap();
    assert_eq!(second.ordinal, 1);
    assert!(!source.reads.contains(&Read::Resolve(inputs().fallback)));
    cursor.executed(second).unwrap();
    let fallback = cursor.next_command(&mut source).unwrap().unwrap();
    assert_eq!(
        (fallback.attempt, fallback.mode, fallback.ordinal),
        (1, 3, 0)
    );
    assert_eq!(cursor.return_word(), None);
    cursor.executed(fallback).unwrap();
    assert_eq!(cursor.next_command(&mut source).unwrap(), None);
    assert_eq!(cursor.stage(), Stage::Complete);
    assert_eq!(cursor.return_word(), Some(0x0900_00b1));
    assert_eq!(cursor.completed_commands(), 3);
    assert_eq!(cursor.work().commands, 3);
    assert!(!source.reads.contains(&Read::Mode(9)));
    assert!(!source.reads.contains(&Read::Target(0, 2)));
}
#[test]
fn command_failure_keeps_executed_prefix_and_never_reaches_fallback_or_replays() {
    let mut source = fixture();
    let mut cursor = Cursor::new(inputs(), budget());
    let first = cursor.next_command(&mut source).unwrap().unwrap();
    cursor.executed(first).unwrap();
    let second = cursor.next_command(&mut source).unwrap().unwrap();
    cursor.failed(second).unwrap();
    let reads = source.reads.clone();
    assert!(cursor.next_command(&mut source).is_err());
    assert!(cursor.executed(second).is_err());
    assert_eq!(cursor.completed_commands(), 1);
    assert_eq!(cursor.pending(), Some(second));
    assert_eq!(cursor.stage(), Stage::Failed);
    assert_eq!(source.reads, reads);
    assert!(!reads.contains(&Read::Resolve(inputs().fallback)));
    assert_eq!(cursor.return_word(), None);
}
#[test]
fn missing_target_or_resource_performs_no_unreached_descriptor_reads() {
    let mut source = Source::default();
    let mut cursor = Cursor::new(inputs(), budget());
    assert_eq!(cursor.next_command(&mut source).unwrap(), None);
    assert_eq!(cursor.return_word(), Some(0));
    assert_eq!(source.reads, [Read::Resolve(inputs().requested)]);
    source.reads.clear();
    let mut cursor = Cursor::new(
        Inputs {
            resource_available: false,
            ..inputs()
        },
        budget(),
    );
    assert_eq!(cursor.next_command(&mut source).unwrap(), None);
    assert!(source.reads.is_empty());
}
#[test]
fn empty_matched_list_returns_source_hash_and_bad_count_latches_reached_metadata() {
    let mut source = fixture();
    source.modes.get_mut(&0).unwrap().hash = 0x0900_00cc;
    source.counts.insert((0, 1), 0);
    let mut cursor = Cursor::new(inputs(), budget());
    assert_eq!(cursor.next_command(&mut source).unwrap(), None);
    assert_eq!(cursor.return_word(), Some(0x0900_00cc));
    assert_eq!(cursor.completed_commands(), 0);
    assert!(!source.reads.contains(&Read::Resolve(inputs().fallback)));
    let mut source = fixture();
    source.counts.insert((0, 1), 9);
    let mut cursor = Cursor::new(inputs(), budget());
    assert!(cursor.next_command(&mut source).is_err());
    assert_eq!(cursor.stage(), Stage::Failed);
    assert_eq!(cursor.attempts()[0].command_count, Some(9));
    assert_eq!(cursor.return_word(), None);
    let reads = source.reads.clone();
    assert!(cursor.next_command(&mut source).is_err());
    assert_eq!(source.reads, reads);
}

#[test]
fn missing_previous_and_unmatched_fallback_return_zero_without_command_reads() {
    let mut source = fixture();
    source.references.remove(&0);
    source.targets.insert((3, 0), 8);
    let mut cursor = Cursor::new(inputs(), budget());
    assert_eq!(cursor.next_command(&mut source).unwrap(), None);
    assert_eq!(cursor.return_word(), Some(0));
    assert_eq!(cursor.attempts().len(), 2);
    assert_eq!(cursor.work().commands, 0);
    assert!(
        !source
            .reads
            .iter()
            .any(|read| matches!(read, Read::Count(..)))
    );
    let reads = source.reads.clone();
    assert_eq!(cursor.next_command(&mut source).unwrap(), None);
    assert_eq!(source.reads, reads);
}

#[test]
fn lookup_and_scan_budget_failures_keep_the_reached_attempt_and_no_return() {
    for limit in [
        Budget {
            lookups: 1,
            ..budget()
        },
        Budget {
            transition_rows: 1,
            ..budget()
        },
    ] {
        let mut source = fixture();
        let mut cursor = Cursor::new(inputs(), limit);
        assert!(cursor.next_command(&mut source).is_err());
        assert_eq!(cursor.stage(), Stage::Failed);
        assert_eq!(cursor.return_word(), None);
        assert_eq!(cursor.attempts().len(), 1);
        assert_eq!(cursor.work().commands, 0);
        let reads = source.reads.clone();
        assert!(cursor.next_command(&mut source).is_err());
        assert_eq!(source.reads, reads);
    }
}
