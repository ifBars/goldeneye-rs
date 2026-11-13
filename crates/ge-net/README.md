# ge-net

A host-authoritative networking core: sessions, channels, ticked commands and
snapshot replication. It has no dependencies. It opens no sockets, starts no
threads, reads no clock and loads no files. Your application feeds it time,
transport events and commands, and it hands back bounded outbound messages and
typed events.

It was written for the GoldenEye Rust rewrite, but it does not depend on that
project, a game image, a renderer or an async runtime. Game payloads are opaque
bytes.

## What it does and does not do

It provides:

- Compatibility negotiation that names the field that differs.
- Peer, epoch and player-slot assignment, with slots separate from peers.
- Two delivery contracts over three transport lanes: reliable ordered for
  control and bulk transfer, unreliable sequenced for input, state and probes.
- Ticked commands that the host releases once, in a stable order.
- Full snapshots for joiners and per-peer deltas against acknowledged
  baselines, with bounded full-state recovery.
- A required budget for every queue, history and unit of work.
- Sanitized diagnostics: counters, queue occupancy, round-trip time.

It does not provide movement prediction, rollback, interpolation, lag
compensation, NAT traversal, relays, matchmaking, host migration or session
resumption. It does not validate game rules: an authenticated, well-formed
command is still only a request. It has been tested against a small authored
simulation, not against a game.

## The loop

Every session is driven the same way. This is the host side; the client side
has the same shape.

```rust,no_run
use ge_net::transport::pump_host;
use ge_net::session::host::HostEvent;
use ge_net::{DrainLimit, HostConfig, HostSession, NetInstant, SessionBudget, Tick};
# fn example(mut transport: impl ge_net::Transport, descriptor: ge_net::CompatibilityDescriptor,
#            mut world: World, mut clock: impl FnMut() -> u64) -> Result<(), Box<dyn std::error::Error>> {
// Session randomness comes from the application, never a gameplay RNG.
let mut entropy = || 0x5eed_u64;
let mut host = HostSession::new(
    HostConfig {
        budget: SessionBudget::small_session(),
        descriptor,
        sequence_origin: 0,
    },
    transport.capabilities(),
    &mut entropy,
    NetInstant::ZERO,
)?;

let mut tick = 0;
loop {
    // 1. Advance network timers with your own monotonic clock.
    host.advance(NetInstant::from_micros(clock()))?;
    // 2. Ingest ready transport events and drain outbound work.
    pump_host(&mut host, &mut transport, DrainLimit::UNLIMITED);
    // 3. React to typed events.
    while let Some(event) = host.poll_event() {
        match event {
            HostEvent::PeerActive { attempt, .. } => world.spawn_players_of(attempt),
            HostEvent::PeerClosed { attempt, .. } => world.remove_players_of(attempt),
            HostEvent::PeerAccepted { .. } => {}
        }
    }
    // 4. Simulation ticks run on the simulation's schedule, not the pump's.
    if world.tick_is_due() {
        tick += 1;
        for command in host.take_commands(Tick::new(tick))? {
            world.validate_and_apply(&command);
        }
        world.step();
        host.publish_snapshot(Tick::new(tick), world.snapshot())?;
    }
}
# }
# struct World;
# impl World {
#     fn spawn_players_of(&mut self, _: ge_net::AttemptId) {}
#     fn remove_players_of(&mut self, _: ge_net::AttemptId) {}
#     fn tick_is_due(&self) -> bool { true }
#     fn validate_and_apply(&mut self, _: &ge_net::ReceivedCommand) {}
#     fn step(&mut self) {}
#     fn snapshot(&self) -> ge_net::Snapshot { ge_net::Snapshot::empty() }
# }
```

`pump_host` and `pump_client` are conveniences over the explicit operations:

| Operation | Host | Client |
| --- | --- | --- |
| Admit a connection | `admit` | `begin`, then `transport_connected` |
| Submit work | `publish_snapshot` | `submit_command` |
| Ingest one message | `ingest` | `ingest` |
| Advance timers | `advance` | `advance` |
| Drain within limits | `drain_outbound`, `poll_event` | `drain_outbound`, `poll_event` |
| Close or cancel | `close_peer`, `cancel`, `shutdown` | `close`, `cancel` |

A client must also tell the session when its application has applied a
snapshot: `acknowledge_applied`. Until it applies the first full snapshot and
then one at or after the host's accepted tick, it is not active and cannot
submit commands. Received bytes never stand in for that work.

## What your application supplies

- **A transport.** Implement `ge_net::Transport`, or use `ge-net-transport`
  (QUIC). A backend declares its capabilities; one that lacks two independent
  reliable lanes or an unreliable lane is refused at construction.
- **A compatibility descriptor.** Protocol and schema versions, a build
  identity, edition and revision, a fingerprint of the logical content, a map
  and rules identity, the tick rate and feature bits. The library compares
  them; it never opens your data.
- **A budget.** `SessionBudget` has no implicit default. Start from
  `SessionBudget::small_session()` and size it for your simulation. Both ends
  should use the same profile.
- **Snapshots.** A `Snapshot` is a set of entities, each an `EntityId` and
  opaque state bytes that only your adapter understands. The library computes
  and applies deltas per entity. Allocate identities with `EntityAllocator`:
  an index is never reissued with an old generation.
- **Monotonic time and session randomness.** Both are parameters.

## Guarantees

- A command is released to the host application at most once, for its tick,
  ordered by slot and then sequence, whatever order packets arrived in.
- A client only ever reports a snapshot that is byte-for-byte what the host
  published for that tick, and never one older than the last.
- The host only sends a delta against a baseline the client acknowledged. A
  delta whose baseline is missing is never applied.
- A destroyed or replaced entity cannot be resurrected by a delayed message.
- A closed or cancelled attempt is invalid immediately; its later events are
  rejected as stale. A reconnect is a new attempt, epoch and peer.
- Every limit in the budget ends in a typed error, a counted drop or an
  explicit close with a reason. The table in `src/budget.rs` says which.

## Running it

No game image is needed for any of these.

```text
cargo test --locked -p ge-net
cargo test --locked -p ge-net --release -- --ignored      # long decoder and lifecycle fuzz
cargo run --release -p ge-net --example synthetic_session -- --seed 7 --loss-per-mille 50 --rtt-ms 150
```

The tests drive a host and clients over a seeded link simulator
(`tests/support/link.rs`) that models loss, duplication, delay, reordering,
bandwidth and partitions, with reliable and unreliable delivery simulated
separately. `tests/support/pipe.rs` scripts exact message orders instead.
The synthetic simulation (`tests/support/simulation.rs`) is numbered actors
with integer positions.

## Measured cost of one scenario

One run of the example above, release profile, on the development container.
These figures describe that scenario, seed, build and machine. They are not a
performance promise and say nothing about a real network or a real game.

| Measure | Value |
| --- | --- |
| Scenario | seed 7, host and 2 clients, 1800 ticks at 60 Hz, 150 ms RTT, 5% datagram loss, 16 actors changing per tick |
| Host state sent | about 57 kB/s for both clients, in 3621 datagrams |
| Host input received | about 12 kB/s for both clients, in 3428 datagrams |
| Control, bulk and probe traffic | under 250 B/s combined |
| Largest outbound queue on one connection | 557 bytes |
| Processing, host plus both clients plus simulator | about 20 µs and 179 allocations per tick |
| One 388-byte state update | encode 0.2 µs with 3 allocations; decode and apply 0.8 µs with 18 allocations |

Input traffic is dominated by redundancy: every input datagram repeats all
commands the host has not yet acknowledged, so its size grows with round-trip
time. State deltas are computed against the acknowledged baseline, so their
size grows with how much changed since then.

## Integrating a game

The library cannot make a simulation networkable. Before a game adapter can
exist, the game must supply:

1. A per-player command boundary: abstract actions per slot per fixed tick.
2. An authoritative fixed simulation tick, independent of rendering.
3. A complete snapshot and apply operation over an explicit allowlist of
   simulation fields, encoded by the adapter.
4. Stable entity mapping between `EntityId` values and game owners.
   Component-local handles are not network identities.
5. Lifetime cleanup that survives disconnects and cancelled attempts.
6. A canonical content fingerprint computed locally from the user's own data.

The wire format is specified in
[docs/networking-wire-format.md](../../docs/networking-wire-format.md) and the
workstream in [docs/networking-crate-plan.md](../../docs/networking-crate-plan.md).
