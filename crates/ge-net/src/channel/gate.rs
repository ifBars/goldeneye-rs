//! Inbound sequence acceptance for both delivery contracts.

use std::borrow::Cow;

use super::Delivery;
use crate::diagnostics::DropReason;
use crate::identity::{Sequence, Tick};
use crate::protocol::envelope::{Fragment, Header};
use crate::session::Violation;

/// What the gate decided about one inbound envelope.
#[derive(Debug)]
pub(crate) enum Admission<'a> {
    /// A complete logical message.
    Deliver(Cow<'a, [u8]>),
    /// A fragment was stored; the logical message is incomplete.
    Pending,
    /// Expected under the unreliable contract; not a failure.
    Drop(DropReason),
    /// The peer or the transport broke a contract; the connection must fail.
    Violation(Violation),
}

/// Partial reassembly of one fragmented unreliable message.
#[derive(Debug)]
pub(crate) struct FragmentAssembler {
    sequence: Sequence,
    kind: u8,
    tick: Option<Tick>,
    parts: Vec<Option<Vec<u8>>>,
    missing: usize,
    bytes: usize,
}

/// Per-channel inbound state.
///
/// Reliable ordered: the next sequence must be exactly the successor of the
/// previous one. Anything else means the lane lost, duplicated or reordered a
/// message, which is a terminal failure.
///
/// Unreliable sequenced: a message is accepted only if it is newer than the
/// last accepted one in serial arithmetic. Fragments of one logical message
/// share a sequence and may arrive in any order.
#[derive(Debug)]
pub(crate) struct InboundGate {
    delivery: Delivery,
    expected: Sequence,
    last: Option<Sequence>,
    assembler: Option<FragmentAssembler>,
    max_fragments: u8,
    max_assembled_bytes: usize,
    abandoned: u64,
}

impl InboundGate {
    pub(crate) fn new(delivery: Delivery, max_fragments: u8, max_assembled_bytes: usize) -> Self {
        Self {
            delivery,
            expected: Sequence::default(),
            last: None,
            assembler: None,
            max_fragments,
            max_assembled_bytes,
            abandoned: 0,
        }
    }

    /// Expect the remote's first sequence to be `start`.
    pub(crate) fn set_initial_sequence(&mut self, start: Sequence) {
        self.expected = start;
    }

    /// Partial messages abandoned because a newer one superseded them.
    pub(crate) fn abandoned(&self) -> u64 {
        self.abandoned
    }

    pub(crate) fn admit<'a>(&mut self, header: &Header, payload: &'a [u8]) -> Admission<'a> {
        match self.delivery {
            Delivery::ReliableOrdered => self.admit_reliable(header, payload),
            Delivery::UnreliableSequenced => self.admit_unreliable(header, payload),
        }
    }

    fn admit_reliable<'a>(&mut self, header: &Header, payload: &'a [u8]) -> Admission<'a> {
        if header.fragment.is_some() {
            return Admission::Violation(Violation::FragmentOnReliableChannel);
        }
        if header.sequence != self.expected {
            return Admission::Violation(Violation::ReliableOrder {
                expected: self.expected.get(),
                received: header.sequence.get(),
            });
        }
        self.expected = self.expected.next();
        Admission::Deliver(Cow::Borrowed(payload))
    }

    fn admit_unreliable<'a>(&mut self, header: &Header, payload: &'a [u8]) -> Admission<'a> {
        let sequence = header.sequence;
        let newer = self.last.is_none_or(|last| sequence.is_newer_than(last));
        match header.fragment {
            None if newer => {
                self.accept(sequence);
                Admission::Deliver(Cow::Borrowed(payload))
            }
            None => Admission::Drop(self.stale_or_duplicate(sequence)),
            Some(fragment) => {
                if fragment.count > self.max_fragments {
                    return Admission::Violation(Violation::TooManyFragments(fragment.count));
                }
                if newer {
                    self.accept(sequence);
                    self.assembler = Some(FragmentAssembler {
                        sequence,
                        kind: header.kind,
                        tick: header.tick,
                        parts: vec![None; usize::from(fragment.count)],
                        missing: usize::from(fragment.count),
                        bytes: 0,
                    });
                }
                self.add_fragment(header, fragment, payload)
            }
        }
    }

    fn accept(&mut self, sequence: Sequence) {
        self.last = Some(sequence);
        if self.assembler.take().is_some() {
            self.abandoned += 1;
        }
    }

    fn stale_or_duplicate(&self, sequence: Sequence) -> DropReason {
        if self.last == Some(sequence) {
            DropReason::Duplicate
        } else {
            DropReason::Stale
        }
    }

    fn add_fragment<'a>(
        &mut self,
        header: &Header,
        fragment: Fragment,
        payload: &'a [u8],
    ) -> Admission<'a> {
        let Some(assembler) = self
            .assembler
            .as_mut()
            .filter(|assembler| assembler.sequence == header.sequence)
        else {
            // Its logical message was already delivered or superseded.
            return Admission::Drop(self.stale_or_duplicate(header.sequence));
        };
        if assembler.kind != header.kind
            || assembler.tick != header.tick
            || assembler.parts.len() != usize::from(fragment.count)
        {
            return Admission::Violation(Violation::FragmentMismatch);
        }
        let slot = &mut assembler.parts[usize::from(fragment.index)];
        if slot.is_some() {
            return Admission::Drop(DropReason::Duplicate);
        }
        if assembler.bytes + payload.len() > self.max_assembled_bytes {
            return Admission::Violation(Violation::AssembledMessageTooLarge);
        }
        assembler.bytes += payload.len();
        assembler.missing -= 1;
        *slot = Some(payload.to_vec());
        if assembler.missing > 0 {
            return Admission::Pending;
        }
        let complete = self.assembler.take().expect("assembler present");
        let mut message = Vec::with_capacity(complete.bytes);
        for part in complete.parts.into_iter().flatten() {
            message.extend_from_slice(&part);
        }
        Admission::Deliver(Cow::Owned(message))
    }
}
