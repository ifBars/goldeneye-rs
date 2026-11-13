# Networking wire format, version 1

This is the normative description of the `ge-net` wire format. The codec lives
in `crates/ge-net/src/protocol` and `crates/ge-net/src/replication/delta.rs`.
Authored byte fixtures in `crates/ge-net/tests/protocol.rs` pin every layout
below.

The format carries opaque application payloads. It never carries game assets,
executable code, file paths, addresses or credentials. Rust struct memory,
pointers, `usize`, enum layouts and native object handles never appear on the
wire.

## Conventions

- Every integer is unsigned, big-endian and fixed width.
- One transport message holds exactly one envelope. The transport preserves
  message boundaries, so nothing is length-prefixed at this layer.
- Payloads have exact lengths. A trailing byte is an error. Version 1 has no
  ignorable fields: a new field needs a new wire version or a negotiated
  optional kind.
- A decoder checks every count and length against the bytes present and the
  configured limits before it allocates.

## Envelope

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | Wire version, `1` |
| 1 | 1 | Message kind |
| 2 | 1 | Channel |
| 3 | 1 | Flags: bit 0 `TICK`, bit 1 `FRAGMENT`; bits 2 to 7 must be zero |
| 4 | 4 | Session epoch |
| 8 | 4 | Sequence |
| 12 | 2 | Payload length |
| 14 | 8 | Simulation tick, only if `TICK` |
| next | 1 | Fragment index, only if `FRAGMENT` |
| next | 1 | Fragment count, only if `FRAGMENT` |
| next | n | Payload |

The envelope length must equal the header plus the declared payload length.
A fragment count below two, or an index at or beyond the count, is an error.
The largest header is 24 bytes.

An envelope is bound to the authenticated transport connection it arrived on.
Nothing inside a payload can authorize the sender to act as another peer.

### Channels and lanes

| Channel | Value | Lane | Contract |
| --- | --- | --- | --- |
| Control | 0 | Control | Reliable ordered |
| Bulk | 1 | Bulk | Reliable ordered, independent of Control |
| Input | 2 | Datagram | Unreliable sequenced |
| State | 3 | Datagram | Unreliable sequenced |
| Probe | 4 | Datagram | Unreliable sequenced |

A message that arrives on a lane other than its channel's lane closes the
connection.

### Session epoch

The host assigns a fresh nonzero epoch to every accepted attempt. Zero means
unassigned. `Hello` and `Reject` carry zero. `Accept` carries the new epoch and
the client adopts it from that envelope. Every later message carries it.

- A datagram with another epoch is discarded and counted. It is expected debris
  from an earlier connection.
- A reliable message with another epoch closes the connection. `Close` is
  exempt: it is accepted with any epoch.

A reconnect is a new attempt and gets a new epoch. Nothing is resumed.

### Sequences

Each channel has its own sequence in each direction. Both ends start every
channel at the same configured origin, normally zero. Sequences wrap at
2^32.

- Reliable ordered: each message must carry exactly the previous sequence plus
  one. Anything else means the lane lost, repeated or reordered a message and
  closes the connection.
- Unreliable sequenced: a message is accepted only if its sequence is newer
  than the last accepted one. `a` is newer than `b` when `(a - b) mod 2^32` is
  between 1 and 2^31 - 1. The acceptance window is therefore one less than
  half the range. An equal sequence is a duplicate; any other is stale. Both
  are discarded and counted.

Command sequences and snapshot identifiers use the same comparison.

### Fragments

Only `StateUpdate` may be fragmented. A fragmented envelope of any other
kind, including an unknown optional one, closes the connection. The fragments
of one update share one sequence and one tick, and each carries a slice of the
logical payload in index order. A receiver accepts fragments of the newest
sequence in any order, ignores repeats, and abandons an incomplete message
when a newer sequence arrives.
Fragment count and reassembled size are bounded by the session budget.

### Unknown kinds

A kind with bit 7 set is an optional extension. A receiver that does not know
it accounts for its sequence, discards it and counts it, up to a budgeted
number per connection; beyond that the connection is closed. Any other unknown
kind closes the connection. Version 1 defines no optional kind.

## Messages

`[tick]` marks kinds whose envelope carries a tick. A known kind on the wrong
channel, or with the wrong tick presence, is an error.

| Kind | Value | Channel | Direction | Payload |
| --- | --- | --- | --- | --- |
| Hello | `0x01` | Control | client to host | `u8` requested slots, descriptor |
| Accept | `0x02` | Control | host to client | `u16` peer, `u8` slot count, `u8` slots ascending, extensions, `u32` command origin |
| Reject | `0x03` | Control | host to client | `u8` reason, `u8` field, `u64` detail |
| SnapshotRequest | `0x04` | Control | client to host | optional snapshot held |
| SyncBegin `[tick]` | `0x05` | Bulk | host to client | `u32` snapshot, `u32` total bytes, `u16` chunk count, command acknowledgements (as in `StateUpdate`) |
| SyncChunk | `0x06` | Bulk | host to client | `u32` snapshot, `u16` chunk index, data |
| SyncApplied | `0x07` | Control | client to host | `u32` snapshot |
| Activate `[tick]` | `0x08` | Control | host to client | empty; the tick is the accepted tick |
| Ready `[tick]` | `0x09` | Control | client to host | empty; the tick is the applied tick |
| Close | `0x0a` | Control | either | `u8` code |
| InputBatch `[tick]` | `0x10` | Input | client to host | see below; the tick is the base tick |
| StateAck | `0x11` | Input | client to host | `u32` snapshot held |
| StateUpdate `[tick]` | `0x12` | State | host to client | see below; the tick is the snapshot tick |
| Ping | `0x20` | Probe | either | `u64` sender timestamp in microseconds |
| Pong | `0x21` | Probe | either | `u64` echoed timestamp |

An optional snapshot is a `u8` flag (0 or 1) followed by a `u32` that must be
zero when the flag is zero.

Extensions are a `u8` count of at most 32 followed by that many strictly
ascending `u32` identifiers.

### Descriptor

| Size | Field |
| --- | --- |
| 4 | Application protocol version |
| 4 | Simulation schema version |
| 32 | Build identity |
| 8 | Content edition |
| 4 | Content revision |
| 32 | Content fingerprint |
| 32 | Map and rules identity |
| 4 + 4 | Tick rate numerator and denominator, in ticks per second |
| 8 | Supported feature bits |
| 8 | Required feature bits |
| 1 + 4n | Extensions |

The host compares fields in this order and reports the first mismatch. The
initial policy is exact equality for every identity field. Each side must
support every feature bit the other requires. The session uses the
intersection of the two extension lists.

### Reject

| Reason | Meaning | Field and detail |
| --- | --- | --- |
| 1 | Incompatible | Field 1 to 8 names the descriptor field in the order above; 9 and 10 mean the host or the client requires feature bits, given in the detail |
| 2 | Session full | Zero |
| 3 | Slots unavailable | Zero |
| 4 | Invalid request | Zero |

### Close codes

The code names a category only: 1 requested, 2 shutdown, 3 protocol
violation, 4 handshake timeout, 5 synchronization timeout, 6 idle timeout,
7 queue overflow, 8 rejected. Unknown codes are preserved as raw values.

### InputBatch

```text
optional snapshot                     newest snapshot the client holds
u8  group count                       at least one; strictly ascending by slot
    u8  slot
    u32 first command sequence        the rest follow consecutively
    u8  command count                 at least one
        u16 tick offset               command tick = envelope tick + offset
        u16 payload length
        payload                       opaque to the library
```

A client resends every unacknowledged command of each slot, oldest first,
until the host reports it received. The host accepts each sequence once and in
order. A command for a slot the connection does not own closes the connection.

### StateUpdate

```text
u32 snapshot
optional snapshot                     baseline; absent means empty
u8  acknowledgement count             strictly ascending by slot
    u8  slot
    u32 received next                 every earlier command was received
    u32 processed next                every earlier command was released or discarded
delta
```

The host only names a baseline that the client acknowledged holding.

`SyncBegin` carries the same acknowledgement list, as of the moment the
transfer starts, so a client that only ever receives full transfers still
learns which commands to stop resending. The client applies it when the
transfer completes.

### Delta and full snapshot

```text
u16 destroyed count                   strictly ascending by index
    u16 index
    u32 generation
u16 upsert count                      strictly ascending by index
    u8  operation                     0 create, 1 update
    u16 index
    u32 generation
    u16 state length
    state                             opaque to the library
```

A delta is applied to its baseline atomically and must be consistent with it:
a destroy or update names an entity the baseline holds with that exact
generation, and a create targets a free index, or one freed by a destroy in
the same delta with an older generation. An inconsistent delta closes the
connection. A delta whose baseline the client does not hold is never applied;
the client requests recovery.

A full snapshot is the same encoding against an empty baseline. `SyncChunk`
messages carry it in order, split at the chunk size.

An entity index never reappears with a generation at or below one that was
destroyed or replaced. Both ends enforce this.

## Session flow

```text
client                                   host
Hello (epoch 0)          ->
                         <-   Accept (assigns the epoch) or Reject
SnapshotRequest          ->
                         <-   SyncBegin, SyncChunk ... on the bulk lane
(application applies the snapshot)
SyncApplied              ->
                         <-   Activate [accepted tick]
                         <-   StateUpdate ... until the accepted tick is reached
(application applies up to the accepted tick)
Ready                    ->
InputBatch / StateAck    ->   <-   StateUpdate            while active
```

The client asks for its first snapshot after it processed `Accept`, so the
transfer never races the acceptance across the two reliable lanes. Received
snapshot bytes do not advance the session: the application must report that it
applied the snapshot, and later that it reached the accepted tick.

Recovery reuses `SnapshotRequest`. The host answers with a delta against the
snapshot the client says it holds when it still retains that snapshot, with a
full state in datagrams when that fits the update budget, and otherwise with a
rate-limited `SyncBegin` and `SyncChunk` transfer.
