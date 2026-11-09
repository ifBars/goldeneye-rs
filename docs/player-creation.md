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
the derived clear and flag writes before the scratch-capacity operation. The RNG
is consumed once during construction, rather than again on each reset.

The shared scratch owner now starts from its proven startup-constructor header:
capacity/count zero, growth 16 and no backing. The first reset allocates 4,736
bytes for 32 records without initializing their payloads. Later attempts reuse
the same backing. Its growth rule preserves the source's deficit comparison;
it is not a general-purpose minimum-capacity helper.

Character selection uses the actual catalog row already retained by
MissionContext and checks the admitted single-user session. The existing empty
inventory is reset, admits its default record, and selects slots using the
user-supplied metadata and both weapon orders. Weapon values use the reached
base/reset fields. The early C54 flag stays unknown through the supported
convergent calculation; its later reset store does not rewrite that history.

The remaining virtual reset now copies defaults through owned providers, clears
the actual C18 buffer and performs the final sparse writes. The shared quad has
an explicit pre-main constructor owner; the separate vector is read from local
supplied DATA rather than embedded in source. A typed, matching-player completion
proof finishes the shared reset lease.

The session now owns the matching per-user metadata flag independently of
readiness bits and the player's own flag byte. Fresh metadata constructs it as
zero; supplied unknown fields remain unknown. Request creation captures that
value once, before construction. After reset, the real compare/store/conditional
notification operation consumes the captured argument. Equality performs no
write or interface lookup; an unsupported bound notification remains an error.

Each player identity belongs to its creation generation, independently of the
reused user slot. Interrupted constructors retain their reached shared count and
backings across cancellation; later attempts reuse the allocations. Native cache
cleanup is not yet bound, so the host does not decrement that count or claim a
completed native destructor. A 128-lease host budget bounds retained partial
attempts; closing cleanup is required before repeated creation is fully supported.

The reset's completion does not authorize player publication. World actor
membership and sparse physics ownership are now connected to the virtual
initializer. Actor registration appends to the actor list while
membership changes insert at each bit-list head. Fresh items own five embedded
and five extra membership nodes. Physics preserves unwritten fields and performs
the source capsule calculation; an unavailable handler-kind input stops before
activation. The wrapperless item factory also updates the actual persistent
numeric-ID registry before it returns. Its counter increments before storage
growth; a later failure preserves that prefix. The initial zone query uses the
real constructor-origin item, before any selected StartPoint pose is applied.

The controller arena retains actual player, item and physics objects, their
registry rows and actor memberships across cancellation. Worker teardown leaves
no item reference pointing at a dropped player. The global world is constructed
once with the controller, rather than recreated on each load. Native removal and
destructor/cache cleanup remain required; retained cancelled objects do not imply
completed native lifetime behavior or active simulation.

Player completion, TP/FP component initialization, camera/view setup and gameplay
remain incomplete. The native checkpoint says **Preparing player**. Progress
evidence reports the reached constructor prefix and pending dependency, with
original history and timing explicitly unverified. It hashes private state and
does not export original pose arrays, source pointers or executable code.
