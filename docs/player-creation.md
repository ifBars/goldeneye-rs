# New Game player creation

The supported Tutorial loader reaches phase 21 after world initialization. That
phase waits for a real player completion object; it does not create the player.
The ordinary session update initiates creation separately.

The session now retains an identity-bound creation attempt. It checks its actual
lifecycle, user readiness and separate operation queue, then selects a StartPoint
from the live trigger catalog. Repeated dispatch returns the existing attempt.
The selected pose stays pending until the required player initializers finish.

The worker executes the generic, basic and player-basic constructor prefix,
including its first reset, and retains the derived player prelude. Its RNG draw
uses the same owner already consumed by Actor12 definition selection. The reset
writes the actual shared resource-state record, even when its value is zero;
this is a setter, separate from the earlier trigger lookups. The derived prelude
owns the actual inventory container, initially empty with growth 16.

These operations live under `ge-core/src/mission/session/player_load` and
`mission/player`, with host ownership in `apps/goldeneye/src/game/player`.
Persistent operation/configuration/reset inputs stay in
`game/world_hooks/player`. Cancellation invalidates the generation and releases
worker-owned state; already-reached persistent writes are retained.

The next constructor stage now acquires three actual allocations in a persistent
controller owner: 63,680 native-layout bytes, with 98 entry-constructor no-ops.
Unwritten payloads remain uninitialized and are never read for reporting. The
shared count increases before the larger reset starts. That reset repeats the
base resource/global writes, completes the runtime base-reset tail, and performs
the derived clear and flag writes up to the scratch-capacity operation. The RNG
is consumed once during construction, rather than again on each reset.

The shared scratch owner still needs its actual initial capacity and growth
provenance. It blocks character and default-inventory selection, even though
those subsystems have separate authored tests. Later reset fields remain unknown;
the constructor does not return a completed player.

Each player identity belongs to its creation generation, independently of the
reused user slot. Interrupted constructors retain their reached shared count and
backings across cancellation; later attempts reuse the allocations. Native cache
cleanup is not yet bound, so the host does not decrement that count or claim a
completed native destructor. A 128-lease host budget bounds retained partial
attempts; closing cleanup is required before repeated creation is fully supported.

World actor membership and sparse physics ownership are also implemented as
focused pure subsystems. Actor registration appends to the actor list while
membership changes insert at each bit-list head. Fresh items own five embedded
and five extra membership nodes. Physics preserves unwritten fields and performs
the source capsule calculation; an unavailable handler-kind input stops before
activation. Neither subsystem substitutes for the preceding constructor work.

Player completion, TP/FP component initialization, camera/view setup and gameplay
remain incomplete. The native checkpoint says **Preparing player**. Progress
evidence reports the reached constructor prefix and pending dependency, with
original history and timing explicitly unverified. It hashes private state and
does not export original pose arrays, source pointers or executable code.
