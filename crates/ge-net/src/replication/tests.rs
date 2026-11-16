//! Command queues, generation floors and history in isolation.

use std::sync::Arc;

use super::{
    CommandInbox, CommandOutbox, EntityRecord, GenerationFloors, Snapshot, SnapshotError,
    SnapshotHistory, StoredSnapshot, SubmitError,
};
use crate::identity::{CommandSeq, EntityId, PlayerSlot, SnapshotId, Tick};

fn record(index: u16, generation: u32) -> EntityRecord {
    EntityRecord {
        id: EntityId::new(index, generation),
        state: vec![index as u8],
    }
}

fn world(records: Vec<EntityRecord>) -> Snapshot {
    Snapshot::from_records(records).unwrap()
}

fn commands(ticks: &[u64]) -> impl Iterator<Item = (Tick, &'static [u8])> + '_ {
    ticks.iter().map(|tick| (Tick::new(*tick), &b"c"[..]))
}

#[test]
fn an_outbox_numbers_commands_and_releases_them_on_acknowledgement() {
    let origin = CommandSeq::new(u32::MAX - 1);
    let mut outbox = CommandOutbox::new(PlayerSlot::new(2), origin, 3);
    let first = outbox.submit(Tick::new(5), b"a").unwrap();
    let second = outbox.submit(Tick::new(5), b"b").unwrap();
    let third = outbox.submit(Tick::new(9), b"c").unwrap();
    assert_eq!(first, origin);
    assert_eq!((second.get(), third.get()), (u32::MAX, 0), "sequences wrap");
    assert_eq!(
        outbox.submit(Tick::new(9), b"d"),
        Err(SubmitError::OutboxFull)
    );
    // The host received the first two.
    assert_eq!(outbox.acknowledge(third), Ok(()));
    assert_eq!(outbox.pending().len(), 1);
    assert_eq!(outbox.pending()[0].sequence, third);
    // A reordered older acknowledgement changes nothing.
    assert_eq!(outbox.acknowledge(second), Ok(()));
    assert_eq!(outbox.acknowledge(origin), Ok(()));
    assert_eq!(outbox.pending().len(), 1);
    // An acknowledgement for something never sent is a protocol error.
    assert_eq!(outbox.acknowledge(third.offset(2)), Err(()));
    assert_eq!(outbox.acknowledge(third.next()), Ok(()));
    assert!(outbox.pending().is_empty());
    assert_eq!(
        outbox.submit(Tick::new(8), b"e"),
        Err(SubmitError::TickRegression)
    );
    assert!(outbox.submit(Tick::new(9), b"e").is_ok());
}

#[test]
fn an_inbox_accepts_each_sequence_once_in_order() {
    let origin = CommandSeq::new(u32::MAX);
    let slot = PlayerSlot::new(0);
    let mut inbox = CommandInbox::new(origin, 4);
    assert_eq!(inbox.ack(slot).received_next, origin);

    let report = inbox.offer(origin, commands(&[3, 3, 4]), None, 100);
    assert_eq!((report.accepted, report.duplicates), (3, 0));
    // A resend of the same run plus one new command.
    let report = inbox.offer(origin, commands(&[3, 3, 4, 6]), None, 100);
    assert_eq!((report.accepted, report.duplicates), (1, 3));
    assert_eq!(inbox.ack(slot).received_next, origin.offset(4));
    assert_eq!(inbox.ack(slot).processed_next, origin);
    // The buffer is full: the next command waits for a resend.
    let report = inbox.offer(origin.offset(4), commands(&[7]), None, 100);
    assert_eq!((report.accepted, report.full), (0, 1));
    // A run that starts beyond the next expected sequence is a gap.
    let report = inbox.offer(origin.offset(6), commands(&[7]), None, 100);
    assert_eq!((report.accepted, report.gap), (0, 1));

    let mut released = Vec::new();
    assert_eq!(
        inbox.take(Tick::new(3), |command| released.push(command.sequence)),
        0
    );
    assert_eq!(released, vec![origin, origin.offset(1)]);
    assert_eq!(inbox.ack(slot).processed_next, origin.offset(2));
    // Tick 4 was skipped by the application: its command is discarded.
    assert_eq!(
        inbox.take(Tick::new(6), |command| released.push(command.sequence)),
        1
    );
    assert_eq!(released.last(), Some(&origin.offset(3)));
    assert_eq!(inbox.ack(slot).processed_next, origin.offset(4));
}

#[test]
fn an_inbox_enforces_the_late_and_future_windows() {
    let origin = CommandSeq::new(10);
    let mut inbox = CommandInbox::new(origin, 8);
    let consumed = Some(Tick::new(20));
    // At or before the consumed tick: late, and acknowledged as processed.
    let report = inbox.offer(origin, commands(&[19, 20]), consumed, 5);
    assert_eq!((report.late, report.accepted), (2, 0));
    assert_eq!(
        inbox.ack(PlayerSlot::new(0)).processed_next,
        origin.offset(2)
    );
    // Inside the window, then beyond it.
    let report = inbox.offer(origin.offset(2), commands(&[25, 26]), consumed, 5);
    assert_eq!((report.accepted, report.future), (1, 1));
    assert_eq!(
        inbox.ack(PlayerSlot::new(0)).received_next,
        origin.offset(3)
    );
    // Before anything was consumed the window starts at tick zero.
    let mut fresh = CommandInbox::new(origin, 8);
    let report = fresh.offer(origin, commands(&[5, 6]), None, 5);
    assert_eq!((report.accepted, report.future), (1, 1));
    // Ticks of one slot never go backwards.
    let report = inbox.offer(origin.offset(3), commands(&[24]), consumed, 5);
    assert!(report.tick_regression);
    // A horizon that would overflow means no upper bound.
    let mut edge = CommandInbox::new(origin, 8);
    let report = edge.offer(
        origin,
        commands(&[u64::MAX]),
        Some(Tick::new(u64::MAX - 1)),
        5,
    );
    assert_eq!(report.accepted, 1);
}

#[test]
fn generation_floors_retire_destroyed_and_replaced_entities() {
    let mut floors = GenerationFloors::default();
    let first = world(vec![record(0, 0), record(2, 5)]);
    assert_eq!(floors.advance(None, &first), Ok(()));
    // Entity 2.5 is replaced by 2.6; entity 0.0 is destroyed.
    let second = world(vec![record(2, 6)]);
    assert_eq!(floors.advance(Some(&first), &second), Ok(()));
    for stale in [record(2, 5), record(0, 0)] {
        let id = stale.id;
        assert_eq!(
            floors.advance(Some(&second), &world(vec![stale])),
            Err(SnapshotError::GenerationRegression(id))
        );
    }
    // A failed check changes nothing: the valid successor still passes.
    let third = world(vec![record(0, 1), record(2, 6), record(7, 0)]);
    assert_eq!(floors.advance(Some(&second), &third), Ok(()));
    // The last generation can be retired without overflow.
    let last = world(vec![record(1, u32::MAX)]);
    assert_eq!(floors.advance(Some(&third), &last), Ok(()));
    assert_eq!(floors.advance(Some(&last), &Snapshot::empty()), Ok(()));
    assert_eq!(
        floors.advance(None, &world(vec![record(1, u32::MAX)])),
        Err(SnapshotError::GenerationRegression(EntityId::new(
            1,
            u32::MAX
        )))
    );
}

#[test]
fn history_keeps_only_the_newest_snapshots() {
    let mut history = SnapshotHistory::new(2);
    assert!(history.latest().is_none());
    for value in 0..3u32 {
        history.push(StoredSnapshot {
            id: SnapshotId::new(u32::MAX - 1).offset(value),
            tick: Tick::new(u64::from(value)),
            state: Arc::new(Snapshot::empty()),
        });
    }
    assert!(
        history.get(SnapshotId::new(u32::MAX - 1)).is_none(),
        "evicted"
    );
    assert_eq!(
        history.get(SnapshotId::new(u32::MAX)).unwrap().tick,
        Tick::new(1)
    );
    assert_eq!(history.latest().unwrap().id, SnapshotId::new(0));
}
