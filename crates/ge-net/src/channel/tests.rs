//! Queue budgets and sequence gates in isolation.

use std::borrow::Cow;

use super::{Admission, ChannelId, Delivery, InboundGate, Lane, OutboundQueues};
use crate::diagnostics::DropReason;
use crate::identity::{Sequence, SessionEpoch, Tick};
use crate::protocol::envelope::{Fragment, Header};
use crate::session::Violation;

fn header(channel: ChannelId, sequence: u32, fragment: Option<(u8, u8)>) -> Header {
    Header {
        kind: 0x12,
        channel,
        epoch: SessionEpoch::new(1),
        sequence: Sequence::new(sequence),
        tick: Some(Tick::new(7)),
        fragment: fragment.map(|(index, count)| Fragment { index, count }),
    }
}

fn delivered(admission: Admission<'_>) -> Vec<u8> {
    match admission {
        Admission::Deliver(bytes) => bytes.into_owned(),
        other => panic!("expected a delivery, got {other:?}"),
    }
}

#[test]
fn channels_map_to_lanes_and_contracts() {
    for channel in ChannelId::ALL {
        assert_eq!(ChannelId::from_wire(channel.wire()), Some(channel));
        assert_eq!(channel.delivery(), channel.lane().delivery());
    }
    assert_eq!(ChannelId::from_wire(5), None);
    assert_eq!(ChannelId::Control.lane(), Lane::Control);
    assert_eq!(ChannelId::Bulk.lane(), Lane::Bulk);
    for channel in [ChannelId::Input, ChannelId::State, ChannelId::Probe] {
        assert_eq!(channel.lane(), Lane::Datagram);
        assert_eq!(channel.delivery(), Delivery::UnreliableSequenced);
    }
    assert_eq!(Lane::Control.delivery(), Delivery::ReliableOrdered);
    assert_eq!(Lane::Bulk.delivery(), Delivery::ReliableOrdered);
}

#[test]
fn reliable_messages_are_never_evicted_and_overflow_is_an_error() {
    let mut queues = OutboundQueues::new(100);
    assert_eq!(queues.push(ChannelId::Control, vec![0; 40]), Ok(0));
    assert_eq!(queues.push(ChannelId::Bulk, vec![0; 40]), Ok(0));
    // No unreliable message exists to make room, so this must fail loudly.
    assert!(queues.push(ChannelId::Control, vec![0; 40]).is_err());
    assert!(queues.push(ChannelId::State, vec![0; 40]).is_err());
    assert_eq!(queues.total_bytes(), 80);
    assert_eq!(queues.len(ChannelId::Control), 1);
    assert_eq!(queues.high_water(), 80);
    // Draining frees the budget again.
    assert_eq!(queues.pop(ChannelId::Control).map(|m| m.len()), Some(40));
    assert_eq!(queues.push(ChannelId::Control, vec![0; 40]), Ok(0));
}

#[test]
fn unreliable_messages_yield_to_newer_and_to_reliable_ones() {
    let mut queues = OutboundQueues::new(100);
    for tag in 1..=4u8 {
        assert_eq!(queues.push(ChannelId::Probe, vec![tag; 25]), Ok(0));
    }
    // A newer probe evicts the oldest probe.
    assert_eq!(queues.push(ChannelId::Probe, vec![5; 25]), Ok(1));
    assert_eq!(queues.front(ChannelId::Probe), Some(&[2; 25][..]));
    // A state update evicts probes, oldest first, since it has none of its own.
    assert_eq!(queues.push(ChannelId::State, vec![9; 50]), Ok(2));
    assert_eq!(queues.front(ChannelId::Probe), Some(&[4; 25][..]));
    // A reliable message evicts whatever unreliable data is in the way.
    assert_eq!(queues.push(ChannelId::Control, vec![7; 90]), Ok(3));
    assert_eq!(queues.len(ChannelId::Probe), 0);
    assert_eq!(queues.len(ChannelId::State), 0);
    assert_eq!(queues.bytes(ChannelId::Control), 90);
    // Unreliable data never evicts reliable data.
    assert!(queues.push(ChannelId::State, vec![9; 50]).is_err());
    assert_eq!(queues.bytes(ChannelId::Control), 90);
    assert_eq!(queues.clear_channel(ChannelId::Control), 1);
    queues.clear();
    assert_eq!(queues.total_bytes(), 0);
}

#[test]
fn each_channel_numbers_its_own_messages_and_wraps() {
    let mut queues = OutboundQueues::new(10);
    queues.set_initial_sequence(Sequence::new(u32::MAX));
    assert_eq!(queues.allocate_sequence(ChannelId::State).get(), u32::MAX);
    assert_eq!(queues.allocate_sequence(ChannelId::State).get(), 0);
    assert_eq!(queues.allocate_sequence(ChannelId::Input).get(), u32::MAX);
}

#[test]
fn a_reliable_gate_accepts_only_the_next_sequence() {
    let mut gate = InboundGate::new(Delivery::ReliableOrdered, 4, 64);
    gate.set_initial_sequence(Sequence::new(u32::MAX));
    let mut next = header(ChannelId::Control, u32::MAX, None);
    assert_eq!(delivered(gate.admit(&next, b"a")), b"a");
    // The sequence wraps to zero.
    next.sequence = Sequence::new(0);
    assert_eq!(delivered(gate.admit(&next, b"b")), b"b");
    // A repeat, a skip and an old value are all contract violations.
    for wrong in [0, 2, u32::MAX] {
        next.sequence = Sequence::new(wrong);
        assert!(matches!(
            gate.admit(&next, b"x"),
            Admission::Violation(Violation::ReliableOrder { expected: 1, received }) if received == wrong
        ));
    }
    let fragment = header(ChannelId::Control, 1, Some((0, 2)));
    assert!(matches!(
        gate.admit(&fragment, b"x"),
        Admission::Violation(Violation::FragmentOnReliableChannel)
    ));
}

#[test]
fn an_unreliable_gate_discards_everything_not_newer() {
    let mut gate = InboundGate::new(Delivery::UnreliableSequenced, 4, 64);
    let at = |sequence| header(ChannelId::State, sequence, None);
    let dropped = |gate: &mut InboundGate, sequence| match gate.admit(&at(sequence), b"x") {
        Admission::Drop(reason) => reason,
        other => panic!("expected a drop, got {other:?}"),
    };
    // The first message is accepted whatever its sequence.
    assert_eq!(delivered(gate.admit(&at(u32::MAX - 1), b"a")), b"a");
    assert_eq!(dropped(&mut gate, u32::MAX - 1), DropReason::Duplicate);
    assert_eq!(dropped(&mut gate, u32::MAX - 5), DropReason::Stale);
    // Loss of intermediate messages does not block a newer one, across the wrap.
    assert_eq!(delivered(gate.admit(&at(3), b"b")), b"b");
    assert_eq!(dropped(&mut gate, u32::MAX), DropReason::Stale);
    assert_eq!(dropped(&mut gate, 0), DropReason::Stale);
    // The window is just under half the range: the furthest value still
    // counts as newer, one beyond it does not.
    let edge = 3u32.wrapping_add((1 << 31) - 1);
    assert_eq!(dropped(&mut gate, edge.wrapping_add(1)), DropReason::Stale);
    assert_eq!(delivered(gate.admit(&at(edge), b"c")), b"c");
}

#[test]
fn fragments_reassemble_in_any_order_and_only_once() {
    let mut gate = InboundGate::new(Delivery::UnreliableSequenced, 4, 64);
    let part = |sequence, index| header(ChannelId::State, sequence, Some((index, 3)));
    assert!(matches!(
        gate.admit(&part(10, 2), b"cc"),
        Admission::Pending
    ));
    assert!(matches!(
        gate.admit(&part(10, 0), b"aa"),
        Admission::Pending
    ));
    assert!(matches!(
        gate.admit(&part(10, 0), b"aa"),
        Admission::Drop(DropReason::Duplicate)
    ));
    let whole = gate.admit(&part(10, 1), b"bb");
    assert!(matches!(whole, Admission::Deliver(Cow::Owned(_))));
    assert_eq!(delivered(whole), b"aabbcc");
    // A fragment of a message that already completed is a duplicate.
    assert!(matches!(
        gate.admit(&part(10, 1), b"bb"),
        Admission::Drop(DropReason::Duplicate)
    ));
    assert_eq!(gate.abandoned(), 0);

    // A newer message abandons an incomplete older one, whose late
    // fragments are then stale.
    assert!(matches!(
        gate.admit(&part(11, 0), b"aa"),
        Admission::Pending
    ));
    assert!(matches!(
        gate.admit(&part(12, 0), b"zz"),
        Admission::Pending
    ));
    assert_eq!(gate.abandoned(), 1);
    assert!(matches!(
        gate.admit(&part(11, 1), b"bb"),
        Admission::Drop(DropReason::Stale)
    ));
    // An unfragmented newer message abandons the partial one too.
    assert_eq!(
        delivered(gate.admit(&header(ChannelId::State, 13, None), b"n")),
        b"n"
    );
    assert_eq!(gate.abandoned(), 2);
    assert!(matches!(
        gate.admit(&part(12, 1), b"bb"),
        Admission::Drop(DropReason::Stale)
    ));
}

#[test]
fn fragments_are_bounded_and_must_agree() {
    let mut gate = InboundGate::new(Delivery::UnreliableSequenced, 3, 5);
    let too_many = header(ChannelId::State, 1, Some((0, 4)));
    assert!(matches!(
        gate.admit(&too_many, b"a"),
        Admission::Violation(Violation::TooManyFragments(4))
    ));
    // Fragments of one sequence must describe the same message.
    let first = header(ChannelId::State, 2, Some((0, 3)));
    assert!(matches!(gate.admit(&first, b"aa"), Admission::Pending));
    let mut other_count = header(ChannelId::State, 2, Some((1, 2)));
    assert!(matches!(
        gate.admit(&other_count, b"bb"),
        Admission::Violation(Violation::FragmentMismatch)
    ));
    other_count.fragment = Some(Fragment { index: 1, count: 3 });
    other_count.tick = Some(Tick::new(8));
    assert!(matches!(
        gate.admit(&other_count, b"bb"),
        Admission::Violation(Violation::FragmentMismatch)
    ));
    // The reassembled size is bounded before the bytes are stored.
    let second = header(ChannelId::State, 2, Some((1, 3)));
    assert!(matches!(gate.admit(&second, b"bb"), Admission::Pending));
    let third = header(ChannelId::State, 2, Some((2, 3)));
    assert!(matches!(
        gate.admit(&third, b"cc"),
        Admission::Violation(Violation::AssembledMessageTooLarge)
    ));
}
