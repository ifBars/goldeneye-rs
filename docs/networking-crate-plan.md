# Networking crate implementation plan

Build `ge-net` as the reusable backbone for future native multiplayer. Another
model can implement this library while the main rewrite continues through player
creation and gameplay. Start with synthetic simulations and direct connections.

This workstream builds the networking foundation. The current game cannot create
a complete player, run a complete match or
restore a complete simulation snapshot. Networking must not report those
capabilities before the game supplies them.

Status: `crates/ge-net` and `crates/ge-net-transport` exist and milestones M0
to M5 have their evidence. See [implementation status](#implementation-status).
Nothing is wired into the game. This is a networking foundation, not
multiplayer.

## Target and first deliverable

Use a host-authoritative session model. A listen host and a future dedicated host
should use the same authority API. Clients submit commands for assigned player
slots. The host validates commands and publishes authoritative state. Prediction,
interpolation and shot compensation belong in a later game adapter.

The first complete deliverable is an asset-free Rust library and test harness:

- A host and two clients negotiate compatibility and exchange bounded messages.
- A synthetic simulation accepts commands once and publishes recoverable state.
- Tests cover packet loss, reordering, duplication, congestion and disconnects.
- A real transport runs the same scenario across separate loopback processes on
  Windows and Linux.
- Documentation shows how another Rust application uses the library without
  depending on GoldenEye, a game image, a renderer or a particular async runtime.

This proves the networking foundation. It does not prove playable GoldenEye,
original online rules, internet connectivity through NAT, or reference parity.

## Read before implementing

Read the root [AGENTS.md](../AGENTS.md), [architecture](architecture.md),
[development guide](development.md), [native extensions](native-extensions.md)
and [asset boundaries](legal.md). Inspect the current workspace before editing.

The existing ownership model matters:

- `ge-core` contains pure runtime rules. It does not own sockets or wall clocks.
- The app owns lifecycle, workers, cancellation and user-supplied asset I/O.
- Mission resources and the non-Send disc reader stay on their existing worker.
- Current component and animation node IDs are local owner handles. They are not
  network identities or stable save identifiers.
- Fixed simulation ticks, complete snapshots and cross-machine determinism are
  requirements still to be implemented and verified in the game.

Keep these boundaries. Do not make the current player initializer depend on a
network session, or expose private asset buffers to a networking worker.

## Workspace and ownership

Start with `crates/ge-net`. Keep its protocol/session core free of game crates,
socket I/O and runtime-specific types. Add `crates/ge-net-transport` when selecting
the first real backend. That companion crate owns socket/runtime integration.
There should be one production backend initially.

Suggested layout; create modules when their behavior exists:

```text
crates/ge-net/
  src/
    lib.rs
    identity.rs
    protocol/       # envelope, codec, versions, compatibility
    session/        # handshake, membership, state transitions, shutdown
    channel/        # delivery contracts, priorities, bounded queues
    replication/    # snapshot IDs, baselines, acknowledgements, recovery
    transport/      # adapter contract and capabilities
    diagnostics/    # bounded counters and structured failures
  tests/
    support/        # synthetic simulation and seeded link simulator
    protocol.rs
    sessions.rs
    replication.rs
  examples/
    synthetic_session.rs
  README.md
```

The library model owns these new crate paths and their tests. The primary rewrite
workstream owns `ge-core`, `ge-assets`, rendering and the current game runtime.
Keep game-specific integration in a future `apps/goldeneye/src/net/` adapter.
Do not move networking rules into the root of `ge-core`.

Workspace manifests, the lockfile and CI configuration are shared files. Coordinate
their edits and use one Git writer at a time in a shared checkout. Stage only the
workstream's files. Preserve unrelated edits. The repository currently uses only
`main`; do not create leftover feature branches or force-push another model's work.
Run Cargo commands serially with two jobs, including across parallel workstreams.

## Library contracts

### Drive the core explicitly

The application feeds monotonic time, transport events and commands into the
library. The library produces outbound messages and typed session events.
It must not start hidden threads, sleep, read wall-clock time or load files.

Keep advancing network time separate from advancing simulation ticks. Rendering
frequency must not determine command ordering or simulation progress. The
synthetic harness can use a configurable 60 Hz tick rate; that is a test profile,
not evidence of the original game's simulation rate.

Make these operations explicit in the API:

1. Admit a transport connection and begin compatibility negotiation.
2. Submit a local command or authoritative snapshot with its tick and sequence.
3. Ingest a bounded message from an identified transport connection.
4. Advance network timers using caller-supplied monotonic time.
5. Drain outbound work and events within caller-supplied limits.
6. Close or cancel an attempt and reject its later events.

Return typed errors for malformed input, unsupported capabilities, queue pressure
and invalid state transitions. Expected queue pressure must not panic or silently
drop required control messages.

### Keep transport and protocol responsibilities distinct

The adapter provides authenticated connection identity, message boundaries,
delivery capabilities, congestion handling and transport failures. `ge-net` owns
the application handshake, session membership, scheduling budgets and replication
state. Neither layer should implement a second reliability mechanism over an
already reliable backend channel.

Expose at least two delivery contracts:

| Contract | Intended traffic | Required behavior |
| --- | --- | --- |
| Reliable ordered | Negotiation, membership, spawn/despawn, baseline transfer | Preserve order within its channel or report a terminal failure |
| Unreliable sequenced | Replaceable input batches and state updates | Discard stale sequences; loss of one message cannot block a newer one |

Use independent channels for control and bulk state. Bound bulk work so joining
one client cannot starve active clients. A backend must declare unsupported
contracts. It must not label a single reliable stream as an unreliable channel.

The fake adapter must honor these contracts too. Simulate lost datagrams and
delayed reliable delivery separately. A reliable delivery failure produces an
explicit connection error. Logical-link tests and packet-level tests of the real
backend are separate evidence.

Select a maintained transport with an existing encryption, authentication and
congestion implementation. Do not invent cryptography or ship an unreviewed
reliable-UDP stack. Before adding dependencies, consult current upstream docs,
record the selected backend and license, and test its actual capabilities. The
core API must remain usable without that dependency or its runtime.

For direct connect, define how the client verifies the host's identity. Use a
pinned credential/fingerprint or an equivalent verified backend mechanism.
Accepting every certificate is not an authentication policy. Keep credentials
and invite secrets out of logs, fixtures and public reports.

### Define a small, versioned wire format

Write the version-one format before implementing the codec. Specify byte order,
field widths, discriminants, length limits and unknown-field behavior. Never send
Rust struct memory, pointers, `usize`, enum layouts or native object handles.
Game payloads remain opaque to the library and use a separately versioned adapter.

An envelope needs protocol version, message kind, channel, session epoch,
sequence and payload length. Tick-bearing messages carry a simulation tick.
Bind every envelope to its authenticated transport connection. A peer ID received
inside a payload does not authorize the sender to act as that peer.

Reject incompatible required features and unknown mandatory messages. Optional
extensions need explicit negotiation and bounded skipping rules. Check lengths
and allocation budgets before allocating or decompressing. If compression is
added, bound the decoded size independently of the encoded size.

Use checked session tick arithmetic and document sequence wraparound. Keep the
sequence acceptance window smaller than half its numeric range. Test wraparound,
duplicates, old-session messages and delayed messages from a previous connection.

### Separate the identities

Define distinct types for a connection attempt, peer, session epoch, player slot,
network entity, command sequence and snapshot ID. A peer can eventually own
multiple local player slots. Keep slot assignment separate from connection count.

Network entity identities need an authority-owned namespace and generation so a
delayed update cannot affect a replacement entity. Map these identities to game
owners in the adapter. Do not serialize the existing component-local IDs and
assume another process can resolve them.

Session/security randomness must not consume the gameplay RNG. Tests supply a
deterministic source; the real adapter uses an appropriate existing secure source.

## Admission, compatibility and lifecycle

Use explicit states such as connecting, negotiating, synchronizing, active,
closing and closed/failed. Define permitted inputs and retained state for each.
Transport connection alone must not produce an active player or a ready world.

The application supplies a compatibility descriptor containing:

- Application protocol and simulation schema versions.
- Build identity, with exact-build compatibility as the initial policy.
- Supported edition/revision and a canonical content fingerprint.
- Map/rules identity, enabled extensions, tick configuration and required features.

`ge-net` compares these values without opening a game image. The game adapter
computes them locally. Hash canonical logical content required for the session;
an RVZ container and an ISO can contain the same game despite different file
hashes. Report which field mismatched. A fingerprint is a compatibility check,
not proof of ownership or a trusted client's behavior.

Never transfer ROMs, extracted assets, executable code, native tables, game
captures or local paths. Protocol fixtures and example payloads must be authored
specifically for this library. Each participant supplies its own game data.

For synchronization, send an authoritative full snapshot at a declared tick and
epoch. Bound the live updates buffered while it transfers. Activation requires
the application to acknowledge that it applied the snapshot, then catch up to
the accepted tick. Received bytes alone cannot acknowledge application work.

On disconnect or cancellation, invalidate the attempt before accepting more
events. Clear its bounded queues and reject delayed callbacks. A reconnect starts
a new attempt/epoch under the initial policy. Host migration and seamless session
resumption are later features with their own recovery protocols.

The eventual app adapter should use a network worker with bounded typed queues.
The simulation worker retains authority over game state. Network callbacks never
mutate it directly. Shutdown joins owned workers without leaving a socket task
or stale callback attached to a new game session.

## Commands and replication

Clients submit ticked commands for their assigned slots. Deduplicate retransmitted
commands and enforce bounded future/late windows. The host adapter validates
game-specific permissions and values before execution. Authentication does not
make a command valid.

Define a stable per-tick ordering independent of packet arrival order. Acknowledge
the last processed command sequence so a future prediction layer can reconcile.
The library must not pretend that movement prediction, rollback or lag-compensated
shots exist merely because it transports commands.

Start replication with full snapshots. Add deltas only after acknowledgement and
recovery work. Track baselines per peer; never reference a baseline that peer has
not acknowledged. Lost or evicted baselines require a bounded full-state recovery
request. A dropped delta must not leave the client permanently waiting for it.

Treat create, update and destroy ordering explicitly. Test updates arriving
before creation and after destruction. Retain enough bounded generation/tombstone
state to reject stale messages. A late joiner must be able to synchronize without
restarting the host or blocking existing players.

Keep state encoding in the adapter. Any later quantization needs a stated error
tolerance and game evidence. Deterministic state hashes require a canonical
simulation representation; hashing current diagnostic receipts is not a
replacement for a complete simulation snapshot.

Use an explicit allowlist of simulation fields for the game snapshot. Resource
caches and asset buffers stay local. Peers reconstruct resource bindings from
agreed identities using their own image.

## Limits and diagnostics

Require a session budget configuration. Bound peers, channels, message bytes,
queued bytes per peer, inbound work per update, handshakes, snapshot chunks,
baseline history and catch-up time. Document who closes a connection when a
limit is exceeded and which state survives that failure.

Get usable datagram size from backend capabilities and account for framing and
encryption overhead. Keep snapshots chunked and bulk transfer rate-limited.
Do not rely on IP fragmentation or unlimited reliable queues.

Expose counters for bytes/messages by channel, queue occupancy, drops by reason,
RTT/jitter, disconnect reasons, baseline recovery and synchronization progress.
Keep samples bounded. Public diagnostics contain aggregate state and sanitized
identities, not packet payloads, IP addresses, credentials or game content.

## Implementation milestones

Complete each milestone with evidence before marking it done. Keep the checklist
in this document current. These milestones are networking work, not additions to
the game's parity numerator.

| Milestone | Work | Acceptance evidence |
| --- | --- | --- |
| M0: contracts | Crate skeleton, ownership, identities, budgets, transport contract, wire specification | `ge-net` builds independently; no game/runtime dependency; documented transitions and limits |
| M1: protocol | Bounded codec, handshake, compatibility, channels, close/cancel handling | Authored byte fixtures, malformed-input tests, stale-attempt and sequence tests |
| M2: simulated sessions | Seeded link simulator, host/two-client synthetic simulation, authoritative commands and full snapshots | Repeatable state/command assertions under loss, duplication, reordering, congestion and timeout |
| M3: recovery | Acknowledged per-peer baselines, deltas, late join, bounded catch-up and recovery | Dropped-baseline, delayed-destroy, partition/rejoin and queue-exhaustion tests converge or close explicitly |
| M4: real transport | Selected backend adapter, identity verification and process lifecycle | Separate host/two-client loopback processes on Windows and Linux; clean shutdown; dependency notices |
| M5: handoff | Examples, API documentation, measured costs and integration contract | Another model can run the example without a game image; runtime integration prerequisites are listed honestly |

Implementation checklist:

- [x] M0 contracts and independent crate build.
- [x] M1 protocol and lifecycle tests.
- [x] M2 simulated host/two-client session.
- [x] M3 replication recovery and late join.
- [x] M4 authenticated real transport and loopback processes.
- [x] M5 documentation, measurements and game-adapter handoff.

## Test and measurement plan

Use a small authored simulation, such as numbered actors with integer positions
and commands. It tests networking contracts without creating another shooter or
depending on unfinished GoldenEye systems.

The simulated link must control time, loss, duplication, delay, reordering,
bandwidth and temporary partitions from a recorded seed. Test combinations,
including a reliable control message delayed behind bulk state and a client that
stops consuming its queue. Assert bounded memory/work as well as eventual state
or an explicit terminal failure. Do not assert convergence during permanent loss.

Include scenario profiles for 0/1/5/10 percent loss and 0/50/150/300 ms RTT.
Add jitter, reordering, burst loss, bandwidth limits and multi-second partitions.
These are test conditions, not promised real-network performance.

Fuzz or property-test the decoder and lifecycle transitions using authored input.
Keep long fuzz/stress runs separate from the ordinary fast test suite. Process
tests use dynamically assigned loopback ports, bounded deadlines and cleanup on
failure. Ordinary CI must not access public hosts, matchmaking services or game
images.

Report codec time, allocations, queued bytes and bandwidth against the exact
scenario, seed, build, tick rate and peer count. Coverage measures tested source
lines. A separate milestone checklist measures this plan. Neither is a percentage
of the full game or a claim of original multiplayer parity.

After registration, use the repository checks:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo test --locked -p ge-net -j 2
cargo clippy --locked -p ge-net --all-targets -j 2 -- -D warnings
cargo xtask check
cargo xtask report
```

Run them serially. Include the transport crate's checks once it exists. Follow the
repository's dependency notice workflow before distributing binaries. Keep the
workspace's `publish = false` policy; publishing a crate is a separate action.

## Implementation status

This section records what exists and what its evidence is. Counts are of
authored tests in these two crates. They are not a percentage of the game, of
this plan or of original multiplayer behaviour.

Changed paths: `crates/ge-net/`, `crates/ge-net-transport/`,
`docs/networking-wire-format.md`, this document, the two workspace manifests
and lockfile, one license supplement under `third-party/`, and short pointers
in `AGENTS.md`, `docs/development.md` and `docs/third-party.md`.

Commands, run serially with two build jobs:

```text
cargo test --locked -p ge-net -p ge-net-transport
cargo test --locked -p ge-net --release -- --ignored
cargo clippy --locked -p ge-net -p ge-net-transport --all-targets -- -D warnings
cargo run --release -p ge-net --example synthetic_session
cargo xtask check
cargo xtask notices
```

| Milestone | Evidence | Where |
| --- | --- | --- |
| M0 | `ge-net` has no dependencies and builds alone. Identities, the required budget with its who-closes table, the transport contract and the lifecycle table are documented in the source. The wire format is specified in [networking-wire-format.md](networking-wire-format.md) | `crates/ge-net/src`, `budget.rs`, `session/mod.rs`, `transport/mod.rs` |
| M1 | Authored byte fixtures for the envelope, every message and the delta; malformed-input, limit, wrong-lane, wrong-epoch, reliable-order, optional-kind, stale-attempt and sequence-wraparound tests; a seeded decoder and lifecycle fuzz (4,000 mutations in the fast suite, 1.92 million in the ignored long run, seeds 0 to 31) | `tests/protocol.rs`, `src/channel/tests.rs` |
| M2 | Host and two clients over the seeded link simulator: the 0/1/5/10% loss by 0/50/150/300 ms RTT matrix (seeds 100 to 115), duplication with reordering, burst loss, fragmented updates, a bandwidth bottleneck, a short and a permanent partition, a stalled consumer, handshake and synchronization timeouts, close, cancel, reconnect, shutdown, rejection and command windows | `tests/sessions.rs`, `tests/support/` |
| M3 | Acknowledged per-peer baselines, an evicted and a dropped baseline, recovery by delta, by datagram and by bulk transfer, a recovery that cannot complete, a delayed update after destroy and reuse, a retired generation, a late joiner beside active clients, a join that cannot catch up, oversized deltas, a blocked datagram lane and partition then rejoin | `tests/replication.rs`, `src/replication/tests.rs` |
| M4 | QUIC adapter with a pinned host fingerprint. In-process loopback tests of both delivery contracts, backpressure, lane independence, authentication failure, flush on close and joined shutdown. One host and two clients as separate processes, plus a wrong-fingerprint client process. Passed locally on Linux and in hosted CI on `ubuntu-latest` and `windows-latest`. Windows was observed in CI only | `crates/ge-net-transport` |
| M5 | Crate READMEs with the driving loop, guarantees, non-goals, measured costs of one named scenario and the integration prerequisites; a runnable example that needs no game image | `crates/ge-net/README.md`, `examples/synthetic_session.rs` |

Each scenario asserts, as applicable: every snapshot a client applied equals
what the host published for that tick; applied ticks only increase; no command
executes twice or without having been submitted; commands are released in slot
order; queues stay within budget; and a connection that cannot continue ends
with a typed reason. Convergence is asserted only after a loss or partition
ends. A permanent partition asserts the explicit failure, not convergence.

### Decisions that differ from the suggested design

- **Replication is snapshot based throughout.** The host publishes a complete
  entity set per tick and the library computes per-peer deltas against
  acknowledged baselines. Create, update and destroy are explicit operations
  inside a delta, validated against the baseline, so they apply atomically
  with their state. They are not separate reliable spawn and despawn messages.
- **The envelope sequence numbers a logical message.** Fragments of one state
  update share a sequence, so reordering among them does not lose the update.
- **The client requests its first snapshot** after it processed the
  acceptance. The transfer therefore never races the acceptance across the
  two reliable lanes, and no live update needs buffering during the transfer:
  the host sends none until the applied acknowledgement arrives.
- **Recovery reuses that request**, and is bounded by the same timer as the
  initial synchronization.
- **The link simulator and synthetic simulation live in `tests/support`**, as
  suggested. The example and the loopback binary include them by path.

### Limitations that remain

- Windows results for the transport crate come from hosted CI. They were not
  observed on a local Windows machine.
- Real-transport tests use the loopback interface only. Loss, latency,
  reordering and reachability on a real network are unverified, and the link
  simulator is a model, not evidence about one.
- Clients are not authenticated individually. The host pins its own identity;
  it knows a client only as a connection.
- Both ends must configure the same budget profile. It is not negotiated; a
  mismatch surfaces as a typed close.
- The baseline history must exceed the snapshots received during one
  acknowledgement round trip. Below that the client repeats full-state
  recovery, rate limited but wasteful.
- An oversized delta falls back to a whole bulk transfer. There is no partial
  or prioritized delta.
- Input datagrams repeat every unacknowledged command, so input bandwidth
  grows with round-trip time.
- No optional extension message is defined. Extension identifiers are
  negotiated and unknown optional kinds are skipped, but nothing sends one.
- Measured costs come from one scenario on one machine.
- Prediction, interpolation, lag compensation, NAT traversal, relays,
  matchmaking, host migration and session resumption do not exist.

## Deferred game integration

Do not wire this into New Game while the primary model is reconstructing that
path. Integration requires an implemented per-player command boundary, an
authoritative simulation tick, complete snapshot/apply operations, stable entity
mapping and tested lifetime cleanup.

When those exist, add the app adapter and verify an actual two-peer game session.
Keep networking opt-in and preserve the offline reference path. Original lobby
rules, match sizes, scoring, spawning and disconnect behavior need their own
source evidence and parity checks.

NAT traversal, relays, platform accounts, matchmaking services, host migration,
voice chat, replay integration, anti-cheat services and internet deployment are
separate follow-up work. The foundation should leave room for them without
claiming they are implemented or adding service dependencies now.

## Handoff to the implementing model

Implement M0 through M5 in order. Begin with `crates/ge-net`, its wire contract
and deterministic synthetic tests. Keep the core independent of game crates.
Choose and document one real transport when reaching M4. Preserve the primary
model's runtime work and coordinate shared manifests, Git operations and builds.

For each completed milestone, report changed paths, commands run, scenario seeds,
observed results and remaining limitations. Update the checklist only when its
acceptance evidence exists. Return a usable networking foundation and an honest
integration contract; do not mark multiplayer or the game complete.
