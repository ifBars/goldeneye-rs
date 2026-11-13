//! Wire-format fixtures, malformed input, sequence rules and lifecycle
//! rejection. Every byte here was authored for this library.

mod support;

use ge_net::protocol::compat::{DescriptorError, Mismatch};
use ge_net::protocol::envelope::{self, Fragment, Header};
use ge_net::protocol::message::{
    Accept, CommandGroup, Hello, InputBatch, KindClass, Message, MessageKind, RejectReason,
    SnapshotRequest, StateUpdate, SyncBegin, SyncChunk, WireCommand, classify, decode_payload,
};
use ge_net::protocol::{DecodeError, EncodeError, WIRE_VERSION};
use ge_net::replication::{DeltaError, ReplicationLimits, apply_delta, encode_delta};
use ge_net::session::host::HostEvent;
use ge_net::{
    AttemptId, ChannelId, ClientEvent, CloseReason, CommandAck, CommandSeq, ConfigError,
    DrainLimit, DropReason, EntityId, EntityRecord, IngestError, Ingested, Lane, Outbound, PeerId,
    Phase, PlayerSlot, SendOutcome, SessionBudget, SessionEpoch, Snapshot, SnapshotId, Tick,
    Violation,
};
use support::harness::descriptor;
use support::pipe::{CODEC, Pair, Wire};
use support::rng::SeededRng;

/// Parse authored hex: whitespace and `|` separators are ignored.
fn hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'|')
        .collect();
    assert!(digits.len().is_multiple_of(2), "odd hex length");
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn header(kind: MessageKind, epoch: u32, sequence: u32, tick: Option<u64>) -> Header {
    Header {
        kind: kind.wire(),
        channel: kind.channel(),
        epoch: SessionEpoch::new(epoch),
        sequence: ge_net::identity::Sequence::new(sequence),
        tick: tick.map(Tick::new),
        fragment: None,
    }
}

fn encode_envelope(header: &Header, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    envelope::encode(header, payload, 4_096, &mut out).expect("encodable");
    out
}

fn payload_of(message: &Message<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    message.encode_payload(&mut out).expect("encodable");
    out
}

/// The fixture must encode to exactly these bytes and decode back.
fn assert_payload_fixture(message: Message<'_>, fixture: &str) {
    let expected = hex(fixture);
    assert_eq!(payload_of(&message), expected, "encoding of {message:?}");
    let decoded = decode_payload(message.kind(), &expected, &CODEC).expect("fixture decodes");
    assert_eq!(decoded, message);
}

#[test]
fn wire_version_is_one() {
    assert_eq!(WIRE_VERSION, 1);
}

#[test]
fn envelope_fixtures_round_trip() {
    // Ping on the probe channel: no tick, no fragment.
    let ping = hex("01 20 04 00 | 01020304 | 00000005 | 0008 | 0000000000BC614E");
    let ping_header = header(MessageKind::Ping, 0x0102_0304, 5, None);
    assert_eq!(
        encode_envelope(&ping_header, &hex("0000000000BC614E")),
        ping
    );
    let decoded = envelope::decode(&ping, 4_096).unwrap();
    assert_eq!(decoded.header, ping_header);
    assert_eq!(decoded.payload, hex("0000000000BC614E"));

    // Activate on control: tick-bearing with an empty payload.
    let activate = hex("01 08 00 01 | 00000007 | 00000002 | 0000 | 0000000000000258");
    let activate_header = header(MessageKind::Activate, 7, 2, Some(600));
    assert_eq!(encode_envelope(&activate_header, &[]), activate);
    assert_eq!(
        envelope::decode(&activate, 4_096).unwrap().header,
        activate_header
    );

    // Second of two state-update fragments at the last sequence value.
    let fragment =
        hex("01 12 03 03 | 00000009 | FFFFFFFF | 0003 | 0000000000000001 | 01 02 | AABBCC");
    let mut fragment_header = header(MessageKind::StateUpdate, 9, u32::MAX, Some(1));
    fragment_header.fragment = Some(Fragment { index: 1, count: 2 });
    assert_eq!(encode_envelope(&fragment_header, &hex("AABBCC")), fragment);
    let decoded = envelope::decode(&fragment, 4_096).unwrap();
    assert_eq!(decoded.header, fragment_header);
    assert_eq!(decoded.payload, hex("AABBCC"));
}

#[test]
fn control_payload_fixtures_round_trip() {
    let mut hello_descriptor = descriptor();
    hello_descriptor.simulation_schema = 2;
    hello_descriptor.revision = 3;
    let hello = format!(
        "02 | 00000001 | 00000002 | {build} | 53594E5448000000 | 00000003 | {content} | {rules} \
         | 0000003C 00000001 | 0000000000000007 | 0000000000000001 | 02 00000007 00000028",
        build = "11".repeat(32),
        content = "22".repeat(32),
        rules = "33".repeat(32),
    );
    assert_payload_fixture(
        Message::Hello(Hello {
            requested_slots: 2,
            descriptor: hello_descriptor,
        }),
        &hello,
    );
    assert_payload_fixture(
        Message::Accept(Accept {
            peer: PeerId::new(3),
            slots: vec![PlayerSlot::new(0), PlayerSlot::new(1)],
            extensions: vec![7],
            command_origin: CommandSeq::new(0xffff_fff0),
        }),
        "0003 | 02 00 01 | 01 00000007 | FFFFFFF0",
    );
    assert_payload_fixture(
        Message::Reject(RejectReason::Incompatible(Mismatch::Content)),
        "01 06 0000000000000000",
    );
    assert_payload_fixture(
        Message::Reject(RejectReason::Incompatible(Mismatch::HostRequiresFeatures(
            8,
        ))),
        "01 09 0000000000000008",
    );
    assert_payload_fixture(
        Message::Reject(RejectReason::SessionFull),
        "02 00 0000000000000000",
    );
    assert_payload_fixture(
        Message::SnapshotRequest(SnapshotRequest { held: None }),
        "00 00000000",
    );
    assert_payload_fixture(
        Message::SnapshotRequest(SnapshotRequest {
            held: Some(SnapshotId::new(0x10)),
        }),
        "01 00000010",
    );
    assert_payload_fixture(
        Message::SyncBegin(SyncBegin {
            snapshot: SnapshotId::new(0x2a),
            total_bytes: 3_000,
            chunk_count: 3,
            acks: Vec::new(),
        }),
        "0000002A 00000BB8 0003 | 00",
    );
    assert_payload_fixture(
        Message::SyncBegin(SyncBegin {
            snapshot: SnapshotId::new(0x2a),
            total_bytes: 3_000,
            chunk_count: 3,
            acks: vec![CommandAck {
                slot: PlayerSlot::new(1),
                received_next: CommandSeq::new(9),
                processed_next: CommandSeq::new(8),
            }],
        }),
        "0000002A 00000BB8 0003 | 01 | 01 00000009 00000008",
    );
    assert_payload_fixture(
        Message::SyncChunk(SyncChunk {
            snapshot: SnapshotId::new(0x2a),
            index: 1,
            data: &[0xde, 0xad],
        }),
        "0000002A 0001 DEAD",
    );
    assert_payload_fixture(Message::SyncApplied(SnapshotId::new(0x2a)), "0000002A");
    assert_payload_fixture(Message::Activate, "");
    assert_payload_fixture(Message::Ready, "");
    assert_payload_fixture(Message::Close(1), "01");
    assert_payload_fixture(Message::Ping(12_345_678), "0000000000BC614E");
    assert_payload_fixture(Message::Pong(u64::MAX), "FFFFFFFFFFFFFFFF");
}

#[test]
fn input_and_state_payload_fixtures_round_trip() {
    assert_payload_fixture(
        Message::InputBatch(InputBatch {
            ack: Some(SnapshotId::new(0x2a)),
            groups: vec![
                CommandGroup {
                    slot: PlayerSlot::new(0),
                    first: CommandSeq::new(u32::MAX),
                    commands: vec![
                        WireCommand {
                            tick_offset: 0,
                            payload: &[0x00, 0x01, 0xff],
                        },
                        WireCommand {
                            tick_offset: 1,
                            payload: &[0x01],
                        },
                    ],
                },
                CommandGroup {
                    slot: PlayerSlot::new(2),
                    first: CommandSeq::new(5),
                    commands: vec![WireCommand {
                        tick_offset: 2,
                        payload: &[],
                    }],
                },
            ],
        }),
        "01 0000002A | 02 | 00 FFFFFFFF 02 | 0000 0003 0001FF | 0001 0001 01 \
         | 02 00000005 01 | 0002 0000",
    );
    assert_payload_fixture(Message::StateAck(SnapshotId::new(0x2a)), "0000002A");

    let delta =
        hex("0001 0004 00000002 | 0002 | 00 0004 00000003 0001 09 | 01 0006 00000000 0002 0A0B");
    assert_payload_fixture(
        Message::StateUpdate(StateUpdate {
            snapshot: SnapshotId::new(0x2b),
            baseline: Some(SnapshotId::new(0x2a)),
            acks: vec![CommandAck {
                slot: PlayerSlot::new(0),
                received_next: CommandSeq::new(7),
                processed_next: CommandSeq::new(5),
            }],
            delta: &delta,
        }),
        "0000002B | 01 0000002A | 01 | 00 00000007 00000005 \
         | 0001 0004 00000002 | 0002 | 00 0004 00000003 0001 09 | 01 0006 00000000 0002 0A0B",
    );
}

fn record(index: u16, generation: u32, state: &[u8]) -> EntityRecord {
    EntityRecord {
        id: EntityId::new(index, generation),
        state: state.to_vec(),
    }
}

const LIMITS: ReplicationLimits = ReplicationLimits {
    max_entities: 16,
    max_entity_bytes: 8,
    max_snapshot_bytes: 256,
};

#[test]
fn delta_fixture_matches_the_documented_layout() {
    let base = Snapshot::from_records(vec![record(4, 2, &[8]), record(6, 0, &[0x0a, 0])]).unwrap();
    let target =
        Snapshot::from_records(vec![record(6, 0, &[0x0a, 0x0b]), record(4, 3, &[9])]).unwrap();
    let expected =
        hex("0001 0004 00000002 | 0002 | 00 0004 00000003 0001 09 | 01 0006 00000000 0002 0A0B");
    let mut delta = Vec::new();
    encode_delta(Some(&base), &target, &mut delta);
    assert_eq!(delta, expected);
    assert_eq!(
        apply_delta(Some(&base), &expected, &LIMITS).unwrap(),
        target
    );

    // A full snapshot is the delta against nothing: creates only.
    let full = hex("0000 | 0002 | 00 0004 00000003 0001 09 | 00 0006 00000000 0002 0A0B");
    let mut encoded = Vec::new();
    encode_delta(None, &target, &mut encoded);
    assert_eq!(encoded, full);
    assert_eq!(encoded.len(), target.encoded_len());
    assert_eq!(apply_delta(None, &full, &LIMITS).unwrap(), target);
}

#[test]
fn deltas_that_contradict_their_baseline_are_rejected() {
    let base = Snapshot::from_records(vec![record(4, 2, &[8])]).unwrap();
    let cases: [(&str, DeltaError); 8] = [
        // Update of an entity the baseline never created.
        (
            "0000 | 0001 | 01 0005 00000000 0001 09",
            DeltaError::UpdateMissing(EntityId::new(5, 0)),
        ),
        // Update addressed to a generation that was replaced.
        (
            "0000 | 0001 | 01 0004 00000001 0001 09",
            DeltaError::UpdateMissing(EntityId::new(4, 1)),
        ),
        // Destroy of an entity that is not there.
        (
            "0001 0003 00000000 | 0000",
            DeltaError::DestroyMissing(EntityId::new(3, 0)),
        ),
        (
            "0001 0004 00000001 | 0000",
            DeltaError::DestroyMissing(EntityId::new(4, 1)),
        ),
        (
            "0001 0009 00000000 | 0000",
            DeltaError::DestroyMissing(EntityId::new(9, 0)),
        ),
        // Create over a live entity.
        (
            "0000 | 0001 | 00 0004 00000003 0001 09",
            DeltaError::CreateOccupied(EntityId::new(4, 3)),
        ),
        // Replace with a generation that is not newer.
        (
            "0001 0004 00000002 | 0001 | 00 0004 00000002 0001 09",
            DeltaError::CreateNotNewer(EntityId::new(4, 2)),
        ),
        // Unsorted upserts are not canonical.
        (
            "0000 | 0002 | 00 0007 00000000 0000 | 00 0006 00000000 0000",
            DeltaError::Decode(DecodeError::InvalidField("upsert order")),
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            apply_delta(Some(&base), &hex(bytes), &LIMITS),
            Err(expected),
            "{bytes}"
        );
    }
    // Limits are checked before allocation.
    assert_eq!(
        apply_delta(None, &hex("FFFF"), &LIMITS),
        Err(DeltaError::Decode(DecodeError::LimitExceeded(
            "destroyed entities"
        )))
    );
    assert_eq!(
        apply_delta(
            None,
            &hex("0000 | 0001 | 00 0001 00000000 0009 000000000000000000"),
            &LIMITS
        ),
        Err(DeltaError::Decode(DecodeError::LimitExceeded(
            "entity bytes"
        )))
    );
    assert_eq!(
        apply_delta(None, &hex("0000 | 0002 | 00 0001 00000000 0000"), &LIMITS),
        Err(DeltaError::Decode(DecodeError::Truncated))
    );
    assert_eq!(
        apply_delta(None, &hex("0000 | 0000 | 00"), &LIMITS),
        Err(DeltaError::Decode(DecodeError::TrailingBytes))
    );
}

#[test]
fn malformed_envelopes_are_rejected_with_the_field_at_fault() {
    let valid = hex("01 20 04 00 | 01020304 | 00000005 | 0008 | 0000000000BC614E");
    let mutate = |index: usize, value: u8| {
        let mut bytes = valid.clone();
        bytes[index] = value;
        bytes
    };
    let decode = |bytes: &[u8]| envelope::decode(bytes, 4_096).map(|envelope| envelope.header);

    assert_eq!(decode(&[]), Err(DecodeError::Truncated));
    assert_eq!(decode(&valid[..13]), Err(DecodeError::Truncated));
    assert_eq!(
        decode(&mutate(0, 2)),
        Err(DecodeError::UnsupportedVersion(2))
    );
    assert_eq!(
        decode(&mutate(0, 0)),
        Err(DecodeError::UnsupportedVersion(0))
    );
    assert_eq!(decode(&mutate(2, 5)), Err(DecodeError::UnknownChannel(5)));
    assert_eq!(
        decode(&mutate(3, 0x04)),
        Err(DecodeError::ReservedFlags(0x04))
    );
    assert_eq!(
        decode(&mutate(3, 0x80)),
        Err(DecodeError::ReservedFlags(0x80))
    );
    // Declared length longer and shorter than the message boundary.
    assert_eq!(
        decode(&mutate(13, 9)),
        Err(DecodeError::LengthMismatch {
            declared: 9,
            actual: 8
        })
    );
    assert_eq!(
        decode(&valid[..valid.len() - 1]),
        Err(DecodeError::LengthMismatch {
            declared: 8,
            actual: 7
        })
    );
    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        decode(&trailing),
        Err(DecodeError::LengthMismatch {
            declared: 8,
            actual: 9
        })
    );
    // The size limit is checked before anything is parsed.
    assert_eq!(
        envelope::decode(&valid, 21).map(|envelope| envelope.header),
        Err(DecodeError::MessageTooLarge { len: 22, max: 21 })
    );
    // A tick flag with too few bytes for the tick.
    assert_eq!(
        decode(&mutate(3, 0x01)),
        Err(DecodeError::LengthMismatch {
            declared: 8,
            actual: 0
        })
    );
    // Fragment fields: a count below two and an index outside the count.
    for (index, count) in [(0, 1), (2, 2), (0, 0)] {
        let bytes = hex(&format!(
            "01 12 03 03 | 00000009 | 00000001 | 0001 | 0000000000000001 | {index:02X} {count:02X} | AA"
        ));
        assert_eq!(
            decode(&bytes),
            Err(DecodeError::InvalidFragment { index, count })
        );
    }
}

#[test]
fn encoding_refuses_payloads_beyond_the_limit() {
    let header = header(MessageKind::SyncChunk, 1, 0, None);
    let mut out = Vec::new();
    assert_eq!(
        envelope::encode(&header, &[0; 100], 64, &mut out),
        Err(EncodeError::PayloadTooLarge { len: 100, max: 50 })
    );
    assert!(out.is_empty());
    // A header that alone exceeds the limit is refused, even with no
    // payload, and nothing is written.
    let tick_header = self::header(MessageKind::Activate, 1, 0, Some(600));
    assert_eq!(
        envelope::encode(&tick_header, &[], 21, &mut out),
        Err(EncodeError::HeaderTooLarge { len: 22, max: 21 })
    );
    assert!(out.is_empty());
    envelope::encode(&tick_header, &[], 22, &mut out).unwrap();
    assert_eq!(envelope::decode(&out, 22).unwrap().header, tick_header);
    out.clear();
    // The 16-bit length field bounds the payload even under a larger limit.
    assert_eq!(
        envelope::encode(&header, &vec![0; 70_000], 1 << 20, &mut out),
        Err(EncodeError::PayloadTooLarge {
            len: 70_000,
            max: usize::from(u16::MAX)
        })
    );
}

#[test]
fn kinds_are_bound_to_their_channel_tick_and_fragment_rules() {
    for kind in MessageKind::ALL {
        assert_eq!(MessageKind::from_wire(kind.wire()), Some(kind));
        let tick = kind.has_tick().then_some(9);
        let good = header(kind, 1, 0, tick);
        assert_eq!(classify(&good), Ok(KindClass::Known(kind)));

        let mut wrong_channel = good;
        wrong_channel.channel = if kind.channel() == ChannelId::Probe {
            ChannelId::Control
        } else {
            ChannelId::Probe
        };
        assert_eq!(
            classify(&wrong_channel),
            Err(DecodeError::WrongChannel {
                kind: kind.wire(),
                channel: wrong_channel.channel.wire()
            })
        );

        let mut wrong_tick = good;
        wrong_tick.tick = if kind.has_tick() {
            None
        } else {
            Some(Tick::new(1))
        };
        assert_eq!(
            classify(&wrong_tick),
            Err(DecodeError::TickPresence { kind: kind.wire() })
        );

        let mut fragmented = good;
        fragmented.fragment = Some(Fragment { index: 0, count: 2 });
        if kind.fragmentable() {
            assert_eq!(classify(&fragmented), Ok(KindClass::Known(kind)));
        } else {
            assert_eq!(
                classify(&fragmented),
                Err(DecodeError::FragmentNotAllowed { kind: kind.wire() })
            );
        }
    }
    // Unknown kinds: mandatory ones are errors, optional ones are skippable.
    let mut unknown = header(MessageKind::Ping, 1, 0, None);
    unknown.kind = 0x7f;
    assert_eq!(
        classify(&unknown),
        Err(DecodeError::UnknownMandatoryKind(0x7f))
    );
    unknown.kind = 0x80;
    assert_eq!(classify(&unknown), Ok(KindClass::UnknownOptional));
    // An unknown kind may not be fragmented: its partial assemblies would
    // never be counted against the skip budget.
    unknown.channel = ChannelId::State;
    unknown.fragment = Some(Fragment { index: 0, count: 2 });
    assert_eq!(
        classify(&unknown),
        Err(DecodeError::FragmentNotAllowed { kind: 0x80 })
    );
}

#[test]
fn malformed_payloads_are_rejected() {
    let decode = |kind, text: &str| decode_payload(kind, &hex(text), &CODEC).map(|_| ());
    // Trailing bytes are never ignored in version one.
    assert_eq!(
        decode(MessageKind::Close, "01 00"),
        Err(DecodeError::TrailingBytes)
    );
    assert_eq!(
        decode(MessageKind::Activate, "00"),
        Err(DecodeError::TrailingBytes)
    );
    assert_eq!(decode(MessageKind::Ping, "00"), Err(DecodeError::Truncated));
    // Optional snapshot: flag and value must agree.
    assert_eq!(
        decode(MessageKind::SnapshotRequest, "00 00000001"),
        Err(DecodeError::InvalidField("optional snapshot"))
    );
    assert_eq!(
        decode(MessageKind::SnapshotRequest, "02 00000001"),
        Err(DecodeError::InvalidField("optional snapshot"))
    );
    // Reject: unknown reason, unknown mismatch field, stray detail.
    assert_eq!(
        decode(MessageKind::Reject, "09 00 0000000000000000"),
        Err(DecodeError::InvalidField("reject reason"))
    );
    assert_eq!(
        decode(MessageKind::Reject, "01 63 0000000000000000"),
        Err(DecodeError::InvalidField("mismatch field"))
    );
    assert_eq!(
        decode(MessageKind::Reject, "02 00 0000000000000001"),
        Err(DecodeError::InvalidField("reject reason"))
    );
    // Accept: no slots, too many slots, unsorted slots, unsorted extensions.
    assert_eq!(
        decode(MessageKind::Accept, "0001 00 00 00000000"),
        Err(DecodeError::LimitExceeded("slots"))
    );
    assert_eq!(
        decode(
            MessageKind::Accept,
            "0001 09 000102030405060708 00 00000000"
        ),
        Err(DecodeError::LimitExceeded("slots"))
    );
    assert_eq!(
        decode(MessageKind::Accept, "0001 02 01 00 00 00000000"),
        Err(DecodeError::InvalidField("slots"))
    );
    assert_eq!(
        decode(
            MessageKind::Accept,
            "0001 01 00 02 00000007 00000007 00000000"
        ),
        Err(DecodeError::InvalidField("extensions"))
    );
    assert_eq!(
        decode(MessageKind::Accept, "0001 01 00 21"),
        Err(DecodeError::LimitExceeded("extensions"))
    );
    // Input batch: no groups, empty group, unsorted groups, oversized command.
    assert_eq!(
        decode(MessageKind::InputBatch, "00 00000000 00"),
        Err(DecodeError::LimitExceeded("command groups"))
    );
    assert_eq!(
        decode(MessageKind::InputBatch, "00 00000000 01 00 00000000 00"),
        Err(DecodeError::InvalidField("empty command group"))
    );
    assert_eq!(
        decode(
            MessageKind::InputBatch,
            "00 00000000 02 01 00000000 01 0000 0000 01 00000000 01 0000 0000"
        ),
        Err(DecodeError::InvalidField("command group order"))
    );
    assert_eq!(
        decode(
            MessageKind::InputBatch,
            "00 00000000 01 00 00000000 01 0000 0401"
        ),
        Err(DecodeError::LimitExceeded("command bytes"))
    );
    // A count larger than the bytes present is rejected before allocating.
    assert_eq!(
        decode(
            MessageKind::InputBatch,
            "00 00000000 01 00 00000000 FF 0000 0000"
        ),
        Err(DecodeError::Truncated)
    );
    assert_eq!(
        decode(MessageKind::StateUpdate, "00000001 00 00000000 FF"),
        Err(DecodeError::Truncated)
    );
    assert_eq!(
        decode(
            MessageKind::StateUpdate,
            "00000001 00 00000000 02 01 00000000 00000000 01 00000000 00000000"
        ),
        Err(DecodeError::InvalidField("command ack order"))
    );
}

#[test]
fn descriptors_validate_and_report_the_first_mismatching_field() {
    let host = descriptor();
    assert_eq!(host.validate(), Ok(()));
    assert_eq!(host.negotiate(&descriptor()), Ok(vec![7, 40]));

    type Change = fn(&mut ge_net::CompatibilityDescriptor);
    let cases: [(Change, Mismatch); 10] = [
        (|d| d.app_protocol = 2, Mismatch::AppProtocol),
        (|d| d.simulation_schema = 2, Mismatch::SimulationSchema),
        (|d| d.build[31] ^= 1, Mismatch::Build),
        (|d| d.edition[0] = b'X', Mismatch::Edition),
        (|d| d.revision = 1, Mismatch::Revision),
        (|d| d.content[0] ^= 1, Mismatch::Content),
        (|d| d.ruleset[5] ^= 1, Mismatch::Ruleset),
        (|d| d.tick_rate.numerator = 30, Mismatch::TickRate),
        (
            |d| {
                d.supported_features = 0b0110;
                d.required_features = 0;
            },
            Mismatch::HostRequiresFeatures(0b0001),
        ),
        (
            |d| {
                d.supported_features = 0b1111;
                d.required_features = 0b1000;
            },
            Mismatch::ClientRequiresFeatures(0b1000),
        ),
    ];
    for (change, expected) in cases {
        let mut client = descriptor();
        change(&mut client);
        assert_eq!(host.negotiate(&client), Err(expected));
        let (field, detail) = expected.to_wire();
        assert_eq!(Mismatch::from_wire(field, detail), Some(expected));
        assert!(!expected.to_string().is_empty());
    }
    assert_eq!(Mismatch::from_wire(0, 0), None);

    // Extensions are optional: the session uses the intersection.
    let mut client = descriptor();
    client.extensions = vec![3, 40, 99];
    assert_eq!(host.negotiate(&client), Ok(vec![40]));

    let mut invalid = descriptor();
    invalid.extensions = vec![5, 5];
    assert_eq!(
        invalid.validate(),
        Err(DescriptorError::ExtensionsNotAscending)
    );
    invalid.extensions = (0..33).collect();
    assert_eq!(
        invalid.validate(),
        Err(DescriptorError::TooManyExtensions(33))
    );
    let mut invalid = descriptor();
    invalid.tick_rate.denominator = 0;
    assert_eq!(invalid.validate(), Err(DescriptorError::ZeroTickRate));
    let mut invalid = descriptor();
    invalid.required_features = 0b1000;
    assert_eq!(
        invalid.validate(),
        Err(DescriptorError::RequiredFeatureUnsupported(0b1000))
    );
    assert_eq!(descriptor().tick_rate.tick_micros(), 16_666);
}

#[test]
fn transports_must_declare_both_delivery_contracts() {
    use ge_net::{ClientConfig, ClientSession, NetInstant, TransportCapabilities};
    let config = || ClientConfig {
        budget: SessionBudget::small_session(),
        descriptor: descriptor(),
        requested_slots: 1,
        sequence_origin: 0,
    };
    let caps = TransportCapabilities {
        reliable_lanes: 2,
        max_reliable_message_bytes: 65_536,
        max_datagram_bytes: Some(1_200),
    };
    let build =
        |config: ClientConfig, caps| ClientSession::new(config, caps, NetInstant::ZERO).map(|_| ());
    assert_eq!(build(config(), caps), Ok(()));
    // One reliable stream cannot stand in for two independent lanes.
    assert_eq!(
        build(
            config(),
            TransportCapabilities {
                reliable_lanes: 1,
                ..caps
            }
        ),
        Err(ConfigError::MissingReliableLanes { available: 1 })
    );
    // Nor can a reliable-only backend claim an unreliable channel.
    assert_eq!(
        build(
            config(),
            TransportCapabilities {
                max_datagram_bytes: None,
                ..caps
            }
        ),
        Err(ConfigError::MissingDatagrams)
    );
    assert_eq!(
        build(
            config(),
            TransportCapabilities {
                max_datagram_bytes: Some(40),
                ..caps
            }
        ),
        Err(ConfigError::DatagramTooSmall {
            usable: 40,
            required: 102
        })
    );
    let mut zero = config();
    zero.budget.max_entities = 0;
    assert_eq!(
        build(zero, caps),
        Err(ConfigError::InvalidBudget("max_entities"))
    );
    let mut chunk = config();
    chunk.budget.snapshot_chunk_bytes = 5_000;
    assert_eq!(
        build(chunk, caps),
        Err(ConfigError::InvalidBudget("snapshot_chunk_bytes"))
    );
    // The queue must hold a whole fragmented update beside a reliable
    // message; a smaller one could only ever deliver partial updates.
    // Four snapshot-chunk envelopes (4 x 1044) outweigh one 4096-byte
    // reliable message; eight 1200-byte fragments are the largest update.
    let floor = 4 * (14 + 6 + 1_024) + 8 * 1_200;
    for (bytes, valid) in [(5_000, false), (floor - 1, false), (floor, true)] {
        let mut queue = config();
        queue.budget.max_queued_bytes_per_peer = bytes;
        let expected = if valid {
            Ok(())
        } else {
            Err(ConfigError::InvalidBudget("max_queued_bytes_per_peer"))
        };
        assert_eq!(build(queue, caps), expected, "{bytes} queued bytes");
    }
    // SyncBegin cannot fragment, so its acknowledgements for every slot of
    // one peer must fit a reliable message: 54 slots need 519 bytes.
    let mut slots = config();
    slots.budget.max_message_bytes = 512;
    slots.budget.snapshot_chunk_bytes = 256;
    slots.budget.max_player_slots = 54;
    slots.budget.max_slots_per_peer = 54;
    assert_eq!(
        build(slots, caps),
        Err(ConfigError::InvalidBudget("max_message_bytes"))
    );
    let mut slots = config();
    slots.budget.max_message_bytes = 512;
    slots.budget.snapshot_chunk_bytes = 256;
    slots.budget.max_player_slots = 53;
    slots.budget.max_slots_per_peer = 53;
    assert_eq!(build(slots, caps), Ok(()));
    let mut history = config();
    history.budget.baseline_history = 1;
    assert_eq!(
        build(history, caps),
        Err(ConfigError::InvalidBudget("baseline_history"))
    );
    let mut slots = config();
    slots.requested_slots = 5;
    assert_eq!(
        build(slots, caps),
        Err(ConfigError::InvalidBudget("requested_slots"))
    );
    let mut bad = config();
    bad.descriptor.tick_rate.numerator = 0;
    assert_eq!(build(bad, caps), Err(ConfigError::InvalidDescriptor));
}

fn empty() -> Snapshot {
    Snapshot::empty()
}

fn established(origin: u32, seed: u64) -> Pair {
    let mut pair = Pair::new(SessionBudget::small_session(), origin, seed);
    pair.establish(1, empty());
    pair
}

#[test]
fn handshake_assigns_identity_and_requires_applied_acknowledgement() {
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 5);
    pair.auto_apply = false;
    assert_eq!(pair.client.phase(), Phase::Negotiating);
    assert_eq!(pair.host.phase(pair.host_attempt), Some(Phase::Negotiating));

    pair.settle();
    // A connection and a negotiation alone give no active player.
    assert_eq!(pair.client.phase(), Phase::Synchronizing);
    assert_eq!(
        pair.host.phase(pair.host_attempt),
        Some(Phase::Synchronizing)
    );
    assert!(matches!(
        pair.client_events[..],
        [ClientEvent::Accepted { ref slots, ref extensions, .. }]
            if slots == &[PlayerSlot::new(0)] && extensions == &[7, 40]
    ));
    assert!(pair.client.peer().is_some_and(|peer| peer != PeerId::HOST));
    assert_eq!(
        pair.client
            .submit_command(PlayerSlot::new(0), Tick::new(1), &[0]),
        Err(ge_net::replication::SubmitError::NotActive)
    );

    let id = pair.publish(1, empty());
    // The snapshot bytes arrived, but nothing was applied yet.
    assert!(matches!(
        pair.client_events.last(),
        Some(ClientEvent::SyncSnapshot { id: got, .. }) if *got == id
    ));
    for _ in 0..4 {
        pair.advance(16_666);
        pair.settle();
    }
    assert_eq!(pair.client.phase(), Phase::Synchronizing);
    assert_eq!(
        pair.host.phase(pair.host_attempt),
        Some(Phase::Synchronizing)
    );

    // Acknowledging something else is refused.
    assert_eq!(
        pair.client.acknowledge_applied(id.next()),
        Err(ge_net::session::StateError::UnknownSnapshot)
    );
    pair.client.acknowledge_applied(id).unwrap();
    pair.settle();
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(pair.host.phase(pair.host_attempt), Some(Phase::Active));
    assert!(matches!(
        pair.host_events[..],
        [HostEvent::PeerAccepted { .. }, HostEvent::PeerActive { tick, .. }] if tick == Tick::new(1)
    ));
    assert!(matches!(
        pair.client_events.last(),
        Some(ClientEvent::Activated { tick }) if *tick == Tick::new(1)
    ));
}

#[test]
fn sequences_wrap_around_without_losing_or_repeating_messages() {
    // Every channel starts three messages before the wrap point.
    let mut pair = established(u32::MAX - 2, 6);
    for tick in 2..40 {
        pair.publish(tick, empty());
        pair.client
            .submit_command(PlayerSlot::new(0), Tick::new(tick + 2), &[tick as u8])
            .unwrap();
    }
    // The last command reaches the host after the next update left, so its
    // acknowledgement rides on the one after that.
    pair.publish(40, empty());
    pair.publish(41, empty());
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(pair.applied.as_ref().unwrap().1, Tick::new(41));
    assert_eq!(pair.client.pending_commands(PlayerSlot::new(0)), 0);
    assert_eq!(pair.host.diagnostics().commands_accepted, 38);
    // Sequences on every channel passed zero.
    let wrapped = pair
        .host
        .diagnostics()
        .channel(ChannelId::State)
        .messages_sent;
    assert!(wrapped > 3, "state channel wrapped: {wrapped}");
}

#[test]
fn command_sequences_wrap_around() {
    // Find a seed whose command origin is close below the wrap point by
    // submitting enough commands to pass it instead: origins are arbitrary
    // 32-bit values, so drive one stream across the boundary explicitly.
    let mut inbox_pair = established(0, 7);
    let slot = PlayerSlot::new(0);
    let mut sequences = Vec::new();
    for tick in 2..300u64 {
        let sequence = inbox_pair
            .client
            .submit_command(slot, Tick::new(tick + 1), &[1])
            .unwrap();
        sequences.push(sequence);
        inbox_pair.host.take_commands(Tick::new(tick)).unwrap();
        inbox_pair.publish(tick, empty());
    }
    for pair in sequences.windows(2) {
        assert!(pair[1].is_newer_than(pair[0]));
        assert_eq!(pair[1], pair[0].next());
    }
    // Serial arithmetic at the boundary itself.
    let last = CommandSeq::new(u32::MAX);
    assert_eq!(last.next(), CommandSeq::new(0));
    assert!(last.next().is_newer_than(last));
    assert!(!last.is_newer_than(last.next()));
    assert!(!last.is_newer_than(last));
    // Exactly half the range away is neither newer nor older.
    let far = CommandSeq::new(1 << 31);
    let zero = CommandSeq::new(0);
    assert!(!far.is_newer_than(zero) && !zero.is_newer_than(far));
    assert!(CommandSeq::new((1 << 31) - 1).is_newer_than(zero));
    assert_eq!(ge_net::identity::SERIAL_WINDOW, (1 << 31) - 1);
}

#[test]
fn duplicated_and_stale_datagrams_are_discarded() {
    let mut pair = established(u32::MAX, 8);
    pair.publish(2, empty());
    pair.advance(16_666);
    pair.flush_host();
    pair.to_client.clear();

    let world = Snapshot::from_records(vec![record(1, 0, &[1])]).unwrap();
    pair.host.publish_snapshot(Tick::new(3), world).unwrap();
    pair.flush_host();
    let older: Vec<Wire> = pair.to_client.drain(..).collect();
    let world = Snapshot::from_records(vec![record(1, 0, &[2])]).unwrap();
    pair.host.publish_snapshot(Tick::new(4), world).unwrap();
    pair.flush_host();
    let newer: Vec<Wire> = pair.to_client.drain(..).collect();

    // Newer first, then the older one, then a duplicate of the newer.
    for wire in &newer {
        assert_eq!(pair.give_client(wire), Ok(Ingested::Accepted));
    }
    for wire in older.iter().filter(|wire| wire.lane == Lane::Datagram) {
        assert_eq!(
            pair.give_client(wire),
            Ok(Ingested::Dropped(DropReason::Stale))
        );
    }
    for wire in newer.iter().filter(|wire| wire.lane == Lane::Datagram) {
        assert_eq!(
            pair.give_client(wire),
            Ok(Ingested::Dropped(DropReason::Duplicate))
        );
    }
    assert_eq!(pair.applied.as_ref().unwrap().1, Tick::new(4));
    assert_eq!(pair.client.phase(), Phase::Active);
    assert!(pair.client.diagnostics().drops(DropReason::Stale) >= 1);
    assert!(pair.client.diagnostics().drops(DropReason::Duplicate) >= 1);
    assert!(pair.client.diagnostics().total_drops() >= 2);
}

#[test]
fn reliable_lanes_must_deliver_exactly_in_order() {
    // A repeated reliable message is a broken transport contract.
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 9);
    pair.flush_client();
    let hello = pair.to_host.pop_front().unwrap();
    assert_eq!(hello.lane, Lane::Control);
    assert_eq!(pair.give_host(&hello), Ok(Ingested::Accepted));
    assert_eq!(
        pair.give_host(&hello),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::ReliableOrder {
                expected: 1,
                received: 0
            }
        )))
    );
    pair.flush_host();
    assert!(matches!(
        pair.host_events.last(),
        Some(HostEvent::PeerClosed {
            reason: CloseReason::Violation(Violation::ReliableOrder { .. }),
            ..
        })
    ));
    assert_eq!(pair.host_disconnects, vec![pair.host_attempt]);

    // A skipped reliable message is the same failure on the client.
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 9);
    pair.settle();
    pair.host.publish_snapshot(Tick::new(1), empty()).unwrap();
    pair.flush_host();
    let bulk: Vec<Wire> = pair
        .to_client
        .drain(..)
        .filter(|wire| wire.lane == Lane::Bulk)
        .collect();
    assert!(bulk.len() >= 2, "begin and at least one chunk");
    assert!(matches!(
        pair.give_client(&bulk[1]),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::ReliableOrder { .. }
        )))
    ));
    assert_eq!(pair.client.phase(), Phase::Closing);
    pair.flush_client();
    assert_eq!(pair.client.phase(), Phase::Closed);
    assert_eq!(pair.client_disconnects, vec![pair.client_attempt]);
}

#[test]
fn messages_from_another_epoch_or_lane_are_refused() {
    let mut pair = established(0, 10);
    pair.host.publish_snapshot(Tick::new(2), empty()).unwrap();
    pair.advance(16_666);
    pair.flush_host();
    let update = pair
        .to_client
        .iter()
        .find(|wire| matches!(wire.message(), Some(Message::StateUpdate(_))))
        .cloned()
        .expect("a state update");
    pair.to_client.clear();

    // The same datagram stamped with a previous connection's epoch.
    let envelope = update.envelope();
    let mut old = envelope.header;
    old.epoch = SessionEpoch::new(old.epoch.get().wrapping_sub(1));
    let stale = Wire {
        lane: Lane::Datagram,
        bytes: encode_envelope(&old, envelope.payload),
    };
    assert_eq!(
        pair.give_client(&stale),
        Ok(Ingested::Dropped(DropReason::EpochMismatch))
    );
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(
        pair.client.diagnostics().drops(DropReason::EpochMismatch),
        1
    );

    // A datagram-channel message arriving on a reliable lane.
    let wrong_lane = Wire {
        lane: Lane::Control,
        bytes: update.bytes.clone(),
    };
    assert_eq!(
        pair.give_client(&wrong_lane),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::LaneMismatch
        )))
    );

    // A reliable message from another epoch is a violation, not a drop.
    let mut pair = established(0, 10);
    let epoch = {
        pair.advance(600_000);
        pair.flush_client();
        pair.to_host.back().unwrap().envelope().header.epoch
    };
    pair.to_host.clear();
    let mut forged = header(MessageKind::SnapshotRequest, epoch.get() ^ 0x55, 0, None);
    // Continue the client's control sequence so only the epoch is wrong.
    forged.sequence = ge_net::identity::Sequence::new(4);
    let payload = payload_of(&Message::SnapshotRequest(SnapshotRequest { held: None }));
    let wire = Wire {
        lane: Lane::Control,
        bytes: encode_envelope(&forged, &payload),
    };
    assert_eq!(
        pair.give_host(&wire),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::EpochMismatch
        )))
    );
}

#[test]
fn unknown_optional_kinds_are_skipped_within_a_bound() {
    let mut budget = SessionBudget::small_session();
    budget.max_skipped_optional_messages = 3;
    let mut pair = Pair::new(budget, 0, 11);
    pair.establish(1, empty());
    pair.advance(600_000);
    pair.flush_host();
    let epoch = pair.to_client.back().unwrap().envelope().header.epoch;
    pair.to_client.clear();

    let optional = |sequence: u32| {
        let header = Header {
            kind: 0x9c,
            channel: ChannelId::Probe,
            epoch,
            sequence: ge_net::identity::Sequence::new(sequence),
            tick: None,
            fragment: None,
        };
        Wire {
            lane: Lane::Datagram,
            bytes: encode_envelope(&header, &[1, 2, 3]),
        }
    };
    for sequence in 100..103 {
        assert_eq!(
            pair.give_client(&optional(sequence)),
            Ok(Ingested::Dropped(DropReason::UnknownOptional))
        );
    }
    // Its sequence was consumed: a repeat is a duplicate, not a second skip.
    assert_eq!(
        pair.give_client(&optional(102)),
        Ok(Ingested::Dropped(DropReason::Duplicate))
    );
    assert_eq!(pair.client.phase(), Phase::Active);
    assert_eq!(
        pair.give_client(&optional(103)),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::TooManySkippedOptional
        )))
    );

    // An unknown mandatory kind is refused outright.
    let mut pair = established(0, 11);
    let header = Header {
        kind: 0x7e,
        channel: ChannelId::Probe,
        epoch,
        sequence: ge_net::identity::Sequence::new(500),
        tick: None,
        fragment: None,
    };
    let wire = Wire {
        lane: Lane::Datagram,
        bytes: encode_envelope(&header, &[]),
    };
    assert_eq!(
        pair.give_client(&wire),
        Ok(Ingested::Closed(CloseReason::Violation(Violation::Decode(
            DecodeError::UnknownMandatoryKind(0x7e)
        ))))
    );
}

#[test]
fn messages_for_the_wrong_side_or_phase_are_violations() {
    // A client must never receive a client-to-host kind.
    let mut pair = established(0, 12);
    pair.advance(16_666);
    pair.flush_client();
    let ack = pair
        .to_host
        .iter()
        .find(|wire| matches!(wire.message(), Some(Message::StateAck(_))))
        .cloned()
        .expect("a baseline acknowledgement");
    assert_eq!(
        pair.give_client(&ack),
        Ok(Ingested::Closed(CloseReason::Violation(
            Violation::UnexpectedMessage(MessageKind::StateAck)
        )))
    );

    // A second Hello after negotiation is refused by the host.
    let mut pair = Pair::new(SessionBudget::small_session(), 0, 12);
    pair.flush_client();
    let hello = pair.to_host.pop_front().unwrap();
    pair.give_host(&hello).unwrap();
    let envelope = hello.envelope();
    let mut again = envelope.header;
    again.sequence = again.sequence.next();
    let wire = Wire {
        lane: Lane::Control,
        bytes: encode_envelope(&again, envelope.payload),
    };
    assert!(matches!(
        pair.give_host(&wire),
        Ok(Ingested::Closed(CloseReason::Violation(
            // The epoch is assigned by now, so the stale one is caught first.
            Violation::EpochMismatch
        )))
    ));
}

#[test]
fn stale_attempts_and_cancelled_attempts_reject_later_events() {
    let mut pair = established(0, 13);
    pair.host.publish_snapshot(Tick::new(2), empty()).unwrap();
    pair.flush_host();
    let late: Vec<Wire> = pair.to_client.drain(..).collect();

    // Cancel invalidates the attempt before anything else is accepted.
    pair.client.cancel().unwrap();
    assert_eq!(pair.client.phase(), Phase::Closed);
    assert_eq!(pair.client.attempt(), None);
    for wire in &late {
        assert_eq!(pair.give_client(wire), Err(IngestError::StaleAttempt));
    }
    pair.flush_client();
    assert_eq!(pair.client_disconnects, vec![pair.client_attempt]);
    assert!(pair.to_host.is_empty(), "a cancelled attempt says nothing");
    assert_eq!(
        pair.client.diagnostics().stale_attempt_events,
        late.len() as u64
    );
    assert_eq!(pair.client.diagnostics().disconnects.cancelled, 1);
    // A delayed transport callback for it is ignored as well.
    pair.client
        .transport_closed(pair.client_attempt, ge_net::TransportFailure::RemoteClosed);
    assert_eq!(
        pair.client_events
            .iter()
            .filter(|event| matches!(event, ClientEvent::Closed { .. }))
            .count(),
        1
    );

    // The host cancels its side; the attempt can never be admitted again.
    pair.host.cancel(pair.host_attempt).unwrap();
    assert_eq!(pair.host.phase(pair.host_attempt), None);
    let wire = Wire {
        lane: Lane::Datagram,
        bytes: vec![1, 2, 3],
    };
    assert_eq!(pair.give_host(&wire), Err(IngestError::StaleAttempt));
    assert_eq!(
        pair.host.admit(pair.host_attempt),
        Err(ge_net::session::StateError::AttemptNotIncreasing)
    );
    assert_eq!(
        pair.host.cancel(pair.host_attempt),
        Err(ge_net::session::StateError::StaleAttempt)
    );
    pair.flush_host();
    assert_eq!(pair.host_disconnects, vec![pair.host_attempt]);

    // A reconnect is a new attempt with a new epoch and peer.
    assert_eq!(
        pair.client.begin(pair.client_attempt),
        Err(ge_net::session::StateError::AttemptNotIncreasing)
    );
    let first_peer = match &pair.client_events[0] {
        ClientEvent::Accepted { peer, .. } => *peer,
        other => panic!("unexpected {other:?}"),
    };
    pair.client_attempt = AttemptId::new(78);
    pair.host_attempt = AttemptId::new(12);
    pair.client.begin(pair.client_attempt).unwrap();
    pair.client
        .transport_connected(pair.client_attempt)
        .unwrap();
    pair.host.admit(pair.host_attempt).unwrap();
    pair.client_events.clear();
    pair.settle();
    pair.publish(3, empty());
    pair.advance(16_666);
    pair.settle();
    assert_eq!(pair.client.phase(), Phase::Active);
    assert!(matches!(
        pair.client_events[0],
        ClientEvent::Accepted { peer, .. } if peer != first_peer
    ));
    // The old connection's datagrams carry the old epoch and attempt.
    for wire in late.iter().filter(|wire| wire.lane == Lane::Datagram) {
        assert_eq!(
            pair.give_client(wire),
            Ok(Ingested::Dropped(DropReason::EpochMismatch))
        );
    }
    assert_eq!(pair.client.phase(), Phase::Active);
}

#[test]
fn every_failure_explains_itself_without_payloads_or_addresses() {
    use ge_net::protocol::message::RejectReason;
    use ge_net::replication::{SnapshotError, SubmitError};
    use ge_net::session::host::PublishError;
    use ge_net::session::{StateError, TimeRegression};

    let entity = EntityId::new(3, 1);
    let decode = [
        DecodeError::Truncated,
        DecodeError::TrailingBytes,
        DecodeError::UnsupportedVersion(9),
        DecodeError::UnknownChannel(9),
        DecodeError::ReservedFlags(4),
        DecodeError::MessageTooLarge { len: 9, max: 8 },
        DecodeError::LengthMismatch {
            declared: 9,
            actual: 8,
        },
        DecodeError::InvalidFragment { index: 2, count: 2 },
        DecodeError::UnknownMandatoryKind(0x7f),
        DecodeError::WrongChannel {
            kind: 1,
            channel: 4,
        },
        DecodeError::TickPresence { kind: 1 },
        DecodeError::FragmentNotAllowed { kind: 1 },
        DecodeError::InvalidField("field"),
        DecodeError::LimitExceeded("limit"),
    ];
    let violations = [
        Violation::Decode(DecodeError::Truncated),
        Violation::LaneMismatch,
        Violation::EpochMismatch,
        Violation::ReliableOrder {
            expected: 1,
            received: 3,
        },
        Violation::FragmentOnReliableChannel,
        Violation::TooManyFragments(9),
        Violation::FragmentMismatch,
        Violation::AssembledMessageTooLarge,
        Violation::UnexpectedMessage(MessageKind::Hello),
        Violation::TooManySkippedOptional,
        Violation::UnownedSlot(PlayerSlot::new(2)),
        Violation::CommandTickRegression,
        Violation::CommandAckAhead,
        Violation::Transfer("chunk order"),
        Violation::Delta(DeltaError::UpdateMissing(entity)),
        Violation::Delta(DeltaError::DestroyMissing(entity)),
        Violation::Delta(DeltaError::CreateOccupied(entity)),
        Violation::Delta(DeltaError::CreateNotNewer(entity)),
        Violation::Delta(DeltaError::Decode(DecodeError::Truncated)),
        Violation::Delta(DeltaError::Snapshot(SnapshotError::DuplicateIndex(3))),
        Violation::StaleGeneration(entity),
        Violation::InvalidAccept,
    ];
    let closes = [
        CloseReason::Requested,
        CloseReason::RemoteClosed(7),
        CloseReason::Rejected(RejectReason::SessionFull),
        CloseReason::HandshakeTimeout,
        CloseReason::SyncTimeout,
        CloseReason::IdleTimeout,
        CloseReason::QueueOverflow,
        CloseReason::Transport(ge_net::TransportFailure::TimedOut),
        CloseReason::Cancelled,
        CloseReason::Shutdown,
    ];
    let mut messages: Vec<String> = Vec::new();
    messages.extend(decode.iter().map(ToString::to_string));
    messages.extend(violations.iter().map(ToString::to_string));
    messages.extend(
        violations
            .iter()
            .map(|violation| CloseReason::Violation(*violation).to_string()),
    );
    messages.extend(closes.iter().map(ToString::to_string));
    messages.extend(
        [
            EncodeError::PayloadTooLarge { len: 9, max: 8 },
            EncodeError::CountTooLarge("slots"),
            EncodeError::HeaderTooLarge { len: 22, max: 21 },
        ]
        .iter()
        .map(ToString::to_string),
    );
    messages.extend(
        [
            IngestError::StaleAttempt,
            IngestError::InboundBudgetExhausted,
            IngestError::EventQueueFull,
        ]
        .iter()
        .map(ToString::to_string),
    );
    messages.extend(
        [
            StateError::WrongPhase(Phase::Closed),
            StateError::StaleAttempt,
            StateError::AttemptNotIncreasing,
            StateError::TooManyAttempts,
            StateError::UnknownSnapshot,
            StateError::QueueOverflow,
        ]
        .iter()
        .map(ToString::to_string),
    );
    messages.extend(
        [
            ConfigError::MissingReliableLanes { available: 1 },
            ConfigError::MissingDatagrams,
            ConfigError::DatagramTooSmall {
                usable: 40,
                required: 102,
            },
            ConfigError::InvalidBudget("max_peers"),
            ConfigError::InvalidDescriptor,
        ]
        .iter()
        .map(ToString::to_string),
    );
    messages.extend(
        [
            SubmitError::NotActive,
            SubmitError::UnknownSlot(PlayerSlot::new(1)),
            SubmitError::OutboxFull,
            SubmitError::CommandTooLarge { len: 9, max: 8 },
            SubmitError::TickRegression,
        ]
        .iter()
        .map(ToString::to_string),
    );
    messages.push(TimeRegression.to_string());
    messages.push(PublishError::TickNotIncreasing { last: Tick::new(4) }.to_string());
    messages.push(PublishError::Snapshot(SnapshotError::IndexOutOfRange(9)).to_string());
    messages.push(ge_net::identity::EntityAllocError::Exhausted.to_string());
    messages.push(ge_net::identity::EntityAllocError::NotLive(entity).to_string());
    messages.push(DescriptorError::ZeroTickRate.to_string());
    messages.push(DescriptorError::ExtensionsNotAscending.to_string());
    messages.push(DescriptorError::TooManyExtensions(40).to_string());
    messages.push(DescriptorError::RequiredFeatureUnsupported(8).to_string());
    messages.push(PeerId::new(3).to_string());
    messages.push(SnapshotId::new(3).to_string());
    messages.push(CommandSeq::new(3).to_string());

    for message in &messages {
        assert!(!message.is_empty());
        assert!(message.is_ascii(), "{message}");
        assert!(!message.contains('\n'), "{message}");
    }
    // Within one error type each failure reads differently, so a report can
    // tell them apart.
    let distinct = |texts: Vec<String>| {
        let unique: std::collections::BTreeSet<&String> = texts.iter().collect();
        assert_eq!(unique.len(), texts.len(), "{texts:?}");
    };
    distinct(decode.iter().map(ToString::to_string).collect());
    distinct(violations.iter().map(ToString::to_string).collect());
    distinct(closes.iter().map(ToString::to_string).collect());
    // Wire close codes name a category; local-only reasons send none.
    let codes: Vec<u8> = closes.iter().map(CloseReason::wire_code).collect();
    assert_eq!(codes, [1, 0, 8, 4, 5, 6, 7, 0, 0, 2]);
    assert_eq!(
        CloseReason::Violation(Violation::LaneMismatch).wire_code(),
        3
    );
}

#[test]
fn a_new_attempt_waits_until_the_old_one_is_drained() {
    let mut pair = established(0, 14);
    // Cancel without polling events or draining outbound work.
    pair.client.cancel().unwrap();
    let next = AttemptId::new(78);
    // Its `Closed` event is still queued: events carry no attempt, so a
    // new attempt's events must not follow it into the same queue.
    assert_eq!(
        pair.client.begin(next),
        Err(ge_net::session::StateError::EventQueueFull)
    );
    assert!(matches!(
        pair.client.poll_event(),
        Some(ClientEvent::Closed {
            reason: CloseReason::Cancelled
        })
    ));
    assert_eq!(pair.client.poll_event(), None);
    // The transport has not yet been told to drop the old connection.
    assert_eq!(
        pair.client.begin(next),
        Err(ge_net::session::StateError::EventQueueFull)
    );
    let mut disconnected = Vec::new();
    pair.client
        .drain_outbound(DrainLimit::UNLIMITED, |outbound| {
            if let Outbound::Disconnect { attempt } = outbound {
                disconnected.push(attempt);
            }
            SendOutcome::Sent
        });
    assert_eq!(disconnected, vec![pair.client_attempt]);
    assert_eq!(pair.client.begin(next), Ok(()));
    assert_eq!(pair.client.phase(), Phase::Connecting);
}

#[test]
fn every_malformed_prefix_and_bit_flip_is_handled_without_panic() {
    run_decoder_fuzz(0xf00d, 4_000);
}

/// Long decoder and lifecycle fuzzing. Run with `cargo test -- --ignored`.
#[test]
#[ignore = "long-running stress; not part of the fast suite"]
fn decoder_and_lifecycle_fuzz_long() {
    for seed in 0..32 {
        run_decoder_fuzz(seed, 60_000);
    }
}

/// Mutate valid traffic and feed it to the decoders and both sessions.
fn run_decoder_fuzz(seed: u64, iterations: usize) {
    let mut rng = SeededRng::new(seed);
    // Collect authored traffic from a real exchange as mutation seeds.
    let mut corpus: Vec<Wire> = Vec::new();
    let mut source = Pair::new(SessionBudget::small_session(), 0, seed);
    let world = |value: u8| {
        Snapshot::from_records((0..40).map(|i| record(i, 0, &[value; 8])).collect()).unwrap()
    };
    source.flush_client();
    corpus.extend(source.to_host.iter().cloned());
    source.settle();
    for tick in 1..40u64 {
        source
            .host
            .publish_snapshot(Tick::new(tick), world(tick as u8))
            .unwrap();
        if source.client.phase() == Phase::Active {
            let _ = source.client.submit_command(
                PlayerSlot::new(0),
                Tick::new(tick + 2),
                &[tick as u8; 5],
            );
        }
        source.advance(16_666);
        for _ in 0..4 {
            source.flush_host();
            source.flush_client();
            corpus.extend(source.to_client.iter().cloned());
            corpus.extend(source.to_host.iter().cloned());
            source.deliver_to_client();
            source.deliver_to_host();
        }
    }
    assert!(corpus.len() > 30, "corpus holds real traffic");

    let mut target = established(0, seed ^ 1);
    let lanes = [Lane::Control, Lane::Bulk, Lane::Datagram];
    for iteration in 0..iterations {
        let mut wire = corpus[rng.below(corpus.len() as u64) as usize].clone();
        match rng.below(6) {
            0 => {
                let cut = rng.below(wire.bytes.len() as u64 + 1) as usize;
                wire.bytes.truncate(cut);
            }
            1 => {
                let at = rng.below(wire.bytes.len() as u64) as usize;
                wire.bytes[at] ^= 1 << rng.below(8);
            }
            2 => {
                let extra = rng.below(40) as usize;
                let mut tail = vec![0; extra];
                rng.fill(&mut tail);
                wire.bytes.extend(tail);
            }
            3 => {
                let len = rng.below(64) as usize;
                wire.bytes = vec![0; len];
                rng.fill(&mut wire.bytes);
            }
            4 => wire.lane = lanes[rng.below(3) as usize],
            _ => {
                let at = rng.below(wire.bytes.len() as u64) as usize;
                wire.bytes[at] = rng.below(256) as u8;
            }
        }

        // The pure decoders never panic and never over-read.
        if let Ok(envelope) = envelope::decode(&wire.bytes, 4_096)
            && let Ok(KindClass::Known(kind)) = classify(&envelope.header)
        {
            let _ = decode_payload(kind, envelope.payload, &CODEC);
            let _ = apply_delta(None, envelope.payload, &LIMITS);
        }

        // Neither do the sessions; a closed target is replaced.
        let to_client = iteration % 2 == 0;
        let result = if to_client {
            target.give_client(&wire)
        } else {
            target.give_host(&wire)
        };
        target.advance(1_000);
        target.flush_host();
        target.flush_client();
        target.to_client.clear();
        target.to_host.clear();
        let closed = matches!(
            result,
            Ok(Ingested::Closed(_)) | Err(IngestError::StaleAttempt)
        ) || target.client.phase() != Phase::Active
            || target.host.phase(target.host_attempt) != Some(Phase::Active);
        if closed {
            target = established(0, seed ^ iteration as u64);
        }
    }
}
