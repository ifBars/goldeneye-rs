# Native extensions after parity

This note records features the native Windows build could add once the
reconstruction is close to parity. These features are not implemented. Planning
does not count as feature progress or resume paused game development.

The reusable networking foundation can be developed in parallel using synthetic
simulations. See the [networking crate implementation plan](networking-crate-plan.md)
for that workstream. Live game integration still depends on the simulation and
parity foundations below.

Extensions are kept separate from the parity scope in `progress/roadmap.json`.
They never count toward a parity percentage, and a working extension is not
evidence that the matching original behavior is correct.

## Ground rules

- **Parity stays the default.** Every extension is opt-in. Turning all of them off
  must produce the reconstructed original behavior, so reference comparisons keep
  working.
- **Extensions sit around the simulation, not inside it.** Netcode, bots, cameras
  and replay tools feed or observe `ge-core`; they do not change its rules. When an
  extension needs a rule change, such as a custom match option, it is a named,
  default-off setting with its own tests.
- **No game data leaves the player's machine.** Each player supplies their own
  game image. Networking, replays, spectating and mods exchange inputs, rule
  settings, state and identities only, never extracted assets. See
  [legal.md](legal.md).
- **Separate claims.** As with the rest of the project, "it builds", "the tests
  pass", "it works over a real network" and "it matches the original" are
  different claims and are reported separately.

## Foundations to protect now

These cost little today and keep the extensions possible later. They are design
constraints, not new work items.

1. **Per-player input commands.** The simulation should read abstract actions
   (move, look, fire, reload, use and so on) for each player slot on each fixed
   tick, never raw device events. Keyboard, controller, network peers, bots and
   replays then all feed the same interface. The `platform.input` roadmap item is
   where this boundary is likely to be set up.
2. **Players are not viewports.** Local split screen (`multi.local`) should mean
   "N local players on one client". Online play then becomes "several clients,
   each with its own local players", and the split-screen work carries over.
3. **The whole simulation state can be snapshotted.** Rollback, joining a match in
   progress, replay seeking and desync detection (hashing state each tick) all
   depend on a complete, serializable snapshot of the simulation.
4. **Rendering is decoupled from ticks.** Rendering draws between fixed ticks
   (interpolating if needed). High refresh rates must not change gameplay timing.
   `docs/architecture.md` already requires fixed ticks and recorded input.
5. **Same inputs, same results on every machine.** Keep `ge-core` free of
   unordered-collection iteration, wall-clock reads and unseeded randomness, as it
   is today. Be explicit about fused and unfused float arithmetic, and avoid maths
   functions whose results depend on the compiler or C library. Treat a single
   release build as the unit of determinism.

## Online multiplayer

The Wii original had online multiplayer through Nintendo's online service, which
has since shut down. The work therefore splits in two:

| Part | Scope | Tracked by |
| --- | --- | --- |
| Original online rules: player counts, modes, lobby flow, scoring, disconnect handling | Parity, verified against the original | `multi.network`, `multi.rules`, `multi.content` |
| Transport, finding friends, NAT traversal, host migration, anything the original never had | Native extension, no reference to compare against | This note |

The original wire protocol and service endpoints are not a goal. Recreating them
would add nothing, because the service no longer exists.

### Topology

The likely starting point is a host-authoritative model: one player hosts (a
listen server) and decides outcomes. Each client predicts its own movement and
reconciles with the host. The host compensates for latency when resolving shots.
This is the usual model for first-person shooters played over the internet.

Two alternatives are worth testing once the simulation runs:

- **Deterministic lockstep:** the simplest model given a deterministic core, but
  it adds input delay that tends to feel poor in a shooter.
- **Rollback:** good for responsiveness. Its cost depends on how cheap it is to
  snapshot and re-simulate a full match with up to the original player count.

A host-authoritative design can still use determinism for desync detection,
replays and automated tests.

### Headless host

Because `ge-core` has no rendering or wall-clock dependencies, a headless host
that runs matches without a window should be feasible. It still needs a user
supplied game image for map and rule data. It would be a separate app or mode,
added only when the networking code exists.

### Sessions

- **Handshake:** peers exchange the build identity and a hash identifying their
  game image (SJBE52 revision 0). A mismatch is refused with a clear message.
  Peers never send the image or extracted data.
- **Finding friends:** start with direct connect by address and an invite code.
  Platform services (such as Steam networking) or a relay can come later; they
  affect distribution and dependency licensing and need a separate decision.
- **Disconnects:** follow the original's rules where they are known. Host
  migration is an extension and default-off unless it changes no parity rule.
- **Trust:** the host validates every client command, such as rate, range and
  whether the action is allowed. Anti-cheat beyond host validation is out of
  scope for a friends-first feature.

### Transport

A UDP-based library is the likely choice; `quinn` (QUIC) and `renet` are
candidates. Pick one when the work starts, and record its license in
[third-party.md](third-party.md).

### Testing

- Simulated-link tests with authored latency, jitter, loss and reordering.
- Desync tests comparing per-tick state hashes across peers.
- Two-process loopback matches in CI, using synthetic maps only.
- Real-network play sessions are a separate, manually reported claim.

## Bots in multiplayer

If the original lacks multiplayer bots, they are an extension. If it has them,
the original behavior is parity scope first.

- Bots act through the same per-player input commands as people. They get no
  special access to the simulation, which keeps them fair and lets them work
  unchanged in online matches, replays and tests.
- Movement can build on the existing navigation work in `ge-core`, as long as the
  multiplayer maps have usable navigation data. Maps without it need a separate
  plan; bots must not run on invented data.
- Difficulty comes from reaction time, aim error and awareness limits, all driven
  by the match's seeded random source so bot matches replay identically.
- Bots can fill empty slots in local, online and practice matches. Whether bots
  run on the host only or on every peer depends on the chosen topology.

## Spectating

- A spectator receives the same state as a player but has no input slot.
- Camera modes: follow a player's view, follow from behind, and free camera.
- Online spectating supports an optional delay so spectators cannot pass live
  information to players.
- The replay viewer reuses the same cameras.

## Replays

With a deterministic simulation, a replay can be small: the match setup plus every
player's input commands per tick.

- **File contents:** a format version, the build identity, the game image hash,
  match settings, the random seed and per-tick commands. It holds no assets, so a
  replay plays back only against the same game image.
- **Build changes:** deterministic playback is only guaranteed for the build that
  recorded it. Periodic state snapshots (keyframes) make seeking fast and allow
  partial playback when determinism breaks. The viewer reports such a break
  instead of hiding it.
- **Uses:** reviewing matches, kill cams, sharing with friends, and as regression
  and desync test inputs for the project itself. Recorded-input tests also fit
  the project's planned reference comparisons.

## Other candidates

- **Display:** higher resolutions, widescreen, adjustable field of view,
  interpolated rendering above the tick rate, and optional upscaled textures
  generated locally from the user's own image.
- **Input:** mouse and keyboard aiming, full remapping, sensitivity curves,
  controller gyro aiming, and settings for aim assist (keeping the original as
  the default).
- **Accessibility:** subtitle size and background, colorblind-friendly HUD and
  team colors, hold or toggle for actions, and reduced screen shake.
- **Match options:** custom rule sets, map rotations and private-match settings
  beyond the original menus, all as named, default-off options.
- **Mods:** only after the core is stable. Mods could change rules or reference
  the user's own content; they must never be a route for distributing extracted
  game data. They need a defined, sandboxed interface rather than patches to the
  simulation.

## Rough order

1. Parity foundations: fixed ticks, per-player input commands, snapshots and
   local split screen (`multi.local`).
2. Replays, which exercise the same foundations with no networking.
3. Spectator and replay cameras.
4. Original online rules (`multi.network`, `multi.rules`) with a native
   transport and direct connect.
5. Bots.
6. Wider connection options, a headless host and the other candidates above.

## Open questions

- Which topology to use, decided by measuring snapshot and re-simulation cost on
  real matches.
- Which original online behaviors (player counts, lobby flow, disconnects) can be
  verified, and how.
- Whether a platform service is worth its distribution and licensing cost compared
  with direct connect and a relay.
- How to version replays and network protocols across builds.
