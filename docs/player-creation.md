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
the source capsule calculation. The handler-scalar setter always overwrites its
field in the original, so unavailable input now invalidates the constructor value
and retains the captured item identity, half-height and setter invocation. Reading
that scalar errors explicitly. Independent activation and suffix operations can
continue without guessing a value. The wrapperless item factory updates the actual persistent
numeric-ID registry before it returns. Its counter increments before storage
growth; a later failure preserves that prefix. The initial zone query uses the
real constructor-origin item, before any selected StartPoint pose is applied.

After activation, the basic initializer clears its session field and counts the
active session's retained kind2 member records. Each record owns the actual
matching metadata and its backlink; the constructor's interface kind is distinct
from the metadata's platform field. The count does not come from the request or
descriptor. Supplied flag snapshots preserve record ownership, including when the
flag is unknown. The ordered semantic registry is a single-player host adaptation;
SDK storage layout, handles, transport and original history remain unverified.

The final byte store and typed basic return receipt follow a successful session
query. A failed provider retains earlier activation and writes without publishing
that return. Completing this basic initializer does not complete PlayerNew or
authorize player publication. The next source operation enters TP/FP setup; the
actual visual skin dependencies and component owners still need to be connected.

TP entry now performs its separate post-basic clear and checks the actual prior
target and item component list. Fresh empty lists need no fabricated destructor
calls. It applies the slot transition to the existing inventory, retaining raw
high-bit slot values, before constructing the TP animator. Four slot headers are
constructed while the active count remains one. Padding and the allocation bit
preserved by the base constructor remain unknown.

The visual row comes from direct primary-key lookup in the retained definition
registry; it does not consume a group cursor or another random draw. Its file and
skin feed a real target request through the existing resource wrapper. The
player retains shared payload and logical section owners across cancellation.
The skin's texture dependencies are then enumerated from that same retained
file owner. Skin body parts (+0x68, +0x70 and +0x78 counts, summed as wrapping
32-bit values) are read in order. Each part's low-24 reference indexes the entity
directory directly. A 0x601 mesh is one group, an expanding 0x603 split
contributes its children in order and 0x607/0x608 contribute nothing. Each group's
+0x54 list selects texture-directory records whose first word is the hash. Only
hashes not yet present are appended, so first-encounter order is preserved. A
missing entity or texture directory is a distinct outcome, not an empty list;
absent lists, null children, unknown kinds and out-of-range indices are errors.
A private source census of the supported Tutorial skin found five unique hashes
across seven mesh groups and nine indices. The reader has not been verified in a
fresh native game run. Texture discovery uses only reached geometry fields;
bone topology, tags and unrelated payloads remain unread. Each hash then feeds
an ordered same-wrapper target request. Successful targets retain their actual
payload and logical section owners across cancellation, and a failed request
preserves the completed prefix. The preparation stage reaches default skeleton
loading only after all these requests finish.

The next two file lookups retain the default skeleton wrapper and then the TP
component wrapper through the existing resource registry. This synchronous
supplied-data adapter does not establish the original registry's preload
history. File lookup creates no extra target request. The supported default
skin belongs to the base section; another section needs its real residency
provider. The skin and both wrappers stay owned across cancellation.

The fresh animator bind clears its empty active control list and resets its
component reference before primary initialization. The skeletal runtime owns
the actual bone hierarchy, source range words, constructor local vectors and
identity matrices. Its auxiliary words are allocated and zeroed separately.
A private census of the supported default skin found 37 bones, one root and 16
auxiliary words; this is static asset evidence, not a fresh runtime pass. The
runtime and animator retain matching semantic backlinks. Constructor NULL in
the sparse numeric view becomes unknown when replaced by a semantic owner link,
so later callers cannot mistake stale zero for the current pointer. The same
default pair becomes the secondary skin. The component file's raw reference zero
produces no initial mode and does not select the first animation entry.

The skeletal constructor captures the engine frame counter minus one, with
32-bit wrapping. Its actual source counter is not yet bound to the host clock;
the reached field retains a captured unknown and its getter fails. No later
counter value can repair that historical read. The reached bind prefix does
not read that field.

The source bind has not returned successfully. Its next zero-time refresh needs
a shared 0x2200-byte scratch allocation and changes shared animation-context
regions and flags even with empty controls. That provider is still pending.
No diagnostic prewarm, TP attachment or FP constructor substitutes for it.

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
