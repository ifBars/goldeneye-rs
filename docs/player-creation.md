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

The bind's zero-time refresh now runs through a controller-lived animation
scratch arena and context, separate from the player reset scratch. Its explicit
startup-initializer snapshot starts a 0x4000-byte aligned arena at offset zero.
Requests round up to 16 bytes. The source uses a strict capacity comparison, so
an exactly full request fails; checked host overflow and exhaustion leave the
cursor unchanged. Reservations must release in reverse order through their
actual owner. The backing exists, but the empty path reads no payload bytes, so
safe Rust allocation contents establish no source payload or history parity.

Fresh slot-zero controls have NULL head/tail, clear maintenance flags and
weight one. Forward tick, maintenance and reverse callbacks therefore reach no
child. The refresh still reserves 0x2200 bytes, binds four context regions,
clears shared flags, clears bit 0x8000 for the empty active slot and releases the
reservation. Flag 0x4000 remains absent, so no pose commit runs. Shared context
locators remain recorded after release; reservation identity prevents a later
allocation at the same offset from reviving them. Failed or stale attempts
cannot publish a bind receipt or replay its callback.

Only that successful refresh permits the typed TP bind return. Its receipt
includes the actual reservation prefix, release cursor and shared operation
ordinals. It does not complete TP entry. No diagnostic prewarm substitutes for
the following attachment, visual postbind, FP construction or later player work.

The TP constructor now receives a distinct identity from the controller's
persistent component registry. This is a checked host handle for the retained
animator, not a native address or a reconstruction of SDK allocation history.
Multiple components can belong to one player without sharing a handle. The
skeletal runtime and typed bind return carry the exact component identity.
Canceled attempts retain existing registry rows and cannot reuse those handles.

After successful binding, attachment checks the actual registered item, player
and component identities. It writes the item backlink, clears component byte
0x8e and writes the first node's NULL previous/next links before appending to
the item's component list. The semantic item link replaces constructor NULL in
the sparse numeric view. A checked list failure retains earlier writes while
leaving membership incomplete; a successful append produces an identity-bound
attachment receipt. Dispatch cannot repeat. This membership operation performs
no activation callback and does not mark a pose or camera ready.

Visual postbind now clears the actual base owner's three visual slots and looks
up the same retained visual wrapper. A lazy body reader resolves the requested
skin, checks its body tag and preserves provenance without reading bone
topology or treating the on-disk runtime-cache word as readiness. The supported
body belongs to the logically resident base; another section still needs its
actual provider. The player callback finds the attached animator through its
item list. It accepts a full class-0x0d reference with nonzero low16, otherwise
choosing the default reference; this normalization does not change the later
raw dependency request. The callback changes the secondary skin pair without
resizing the primary skeleton, sampling a pose or reading the unknown stamp.

The source then repeats the primary skin and ordered texture requests. Their
actual target owners share the earlier wrapper and retain completed prefixes
across cancellation or failure. Only after those requests does the base owner's
primary visual slot link to the actual skeletal runtime. Extra skin and explicit
override branches are skipped because their retained row fields are MAX, not
because a provider was absent.

The same base constructor already initialized the override list at +0x474. Its
empty reset preserves growth16, clears capacity/count/backing and increments the
32-bit generation. Nonempty or unknown backing requires real cleanup. The list
owner is then linked to the item and flag0x80 is set, even though the override
list is empty. The source's four header setters write mode3/flags1 and component
byte0x8c gains bit0; the active slot count stays one.

The TP helper tail now stores the actual embedded player animation receiver in
the item, then binds the animator to the shared startup callback table. The
controller owns that table once; retained animators share the same Arc across
cancellation and reentry. The four slots route events, two source no-ops and
acknowledgement. Matching numeric IDs do not substitute for the same table owner.
The skeletal runtime then records the PlayerNew pose postprocess target. This is
only a routing assignment: its postprocess body remains unavailable and no pose
or event is executed by this constructor tail. Only successful ordered stores
produce the TP helper return receipt and advance to AwaitingFirstPerson.

For the supported PlayerNew receiver and TP/FP animator class, event kinds 3 and
4 return zero; kind 5 reads the existing shared base-reset byte and sign-extends
it. A sentinel file resolves through the actual component binding before kind
dispatch, including ignored kinds. Recognized kinds require the control channel.
Acknowledgement invalidates its token only after a nonzero shared-byte return;
the zero-byte branch returns false for this class, whose ancestry excludes the
separate Sound class. Missing owner/global providers remain errors, distinct from
known NULL receiver links. Dispatch takes the actual shared-byte provider when
invoked; it does not capture a new constant or execute during binding. Live frame
scheduling and Sound-class callback behavior remain outside this reached path.


After the TP return, the same retained character row now supplies FP's skin file.
A second persistent component identity is allocated before the repeated direct
visual lookup. The flag1 branch skips TP visual residency and visual postbind;
it looks up the character skin file and FP component wrapper, then binds fixed
skin 0x0d00016f and raw component reference zero. FP owns its own skeletal locals,
constructor matrices, auxiliary storage and unknown captured frame stamp. The
same controller scratch/context performs and releases its second zero-time
refresh. No diagnostic prewarm, new inventory, or sampled pose substitutes for it.

FP attachment follows that real bind return. Its item backlink and cache byte
are written before the intrusive append: FP.previous links to TP, FP.next is NULL,
then TP.next links to FP before publishing the item's new tail. Checked host
list failure retains the reached prefix and cannot return the helper. The
successful tail configures all four FP headers and binds the same embedded
receiver, shared callback table and pose target as TP. It does not reset the
base visual override list or replace TP's visual skeleton.

Only after FP helper return do the two original resource-header searches walk
the actual item list to find FP. They write masks +0x136 and +0x131 to 0xfb in
that order. The checkpoint now awaits the initial animation mode requests.
Actual mode controls, postprocessed poses, scheduling, completed player and
camera remain required. Constructor state and both helper returns establish no
reference parity or native frame behavior.


The fresh initial action caller now reads the retained reset and base-handler
fields. Positive-zero heading and the fresh default branch select the first
generic request and store direction -1 before component searches. The zero c54
branch does not read d00/d01. FP then TP are resolved from the actual item list;
the current signed inventory slot yields its real record or the source NULL
result. The supported normal variant reads the actual record flags, retaining
an explicit missing-provider boundary for alternate metadata.

Three remaps execute in source order through the loadout's existing supplied
weapon tables: FP normal, TP normal and TP secondary. TP-specific maps override
the normal map when present. Secondary lookup takes priority; after a miss with
a nonzero parent, the source skips that record's normal map and follows the TP
parent. Without a parent it tries the same record's normal map. Each nonzero
candidate queries only the matching retained component's mode directory. Parent
cycles, exhausted bounds and provider errors remain errors; generic fallback
comes from actual unsuccessful resolution. Reached results survive cancellation.

The remapping stage creates no controls, changes no mode-history words and
returns no initial action callback. Its following TP dispatch starts with the
capture helper described below. FP mode dispatch and the optional second generic
request follow TP dispatch and its side effects. Diagnostic FP plans or freshly loaded inventories cannot stand in
for that order. Pose evaluation, player state transition and camera remain later
work.


TP's first dispatch now reaches the physical-slot capture helper. The empty
slot2 query returns positive zero; the first request's positive-zero parameter
selects capture mask 0xf and preserve mask 3. The shared fallback receives the
default mode before capture and the branch fallback after it. Its prior value
remains unknown until an actual reached store; later attempts reuse the same
owner rather than a new per-player global.

A control header is retained before its owner refresh. The fresh LOD/linked-child
and actor-transform links are constructor NULL. Ordinary refresh clears the
reached LOD byte and update flag bit8, runs empty maintenance and uses the same
shared scratch/context. There are no sampled child results or 0x4000 commit
flag, so pose postprocessing is not invoked and the unresolved frame stamp is
not consumed. The primary runtime's actual translation/quaternion locals and
auxiliary words then populate the snapshot backing.

The helper visits all four physical headers, independently of active count.
Each receives a distinct control with independent backing. Clone constructors
keep their own initial scalars and copy snapshot data. The active count remains
one; this operation differs from the mode command's active-slot mask loop.
Header/list ownership and each successful prefix survive allocation failure,
which cannot publish a returned capture or replay its failed state. The live
checkpoint now enters TP selection as described below, before FP dispatch and
the remaining initial action callback. Constructor snapshots remain separate
from evaluated poses, camera output and reference parity.

TP mode qualification now queries the actual retained FP wrapper for its mapped
request. An available FP mode selects generic previous history; otherwise the
tracked TP branch clears its reached handler words and checks the actual dirty
interface. The fresh NULL interface needs no notification. That branch retains
the real slot-zero head before selection. Its current snapshot-control type is
outside the compared mode-child subtype, so the saved comparison word is zero.
Later node types require their own provider; unknown head types cannot be skipped.

A resumable generic animation cursor resolves the requested target first. Previous
reference zero is a valid coded index. Transition keys are visited in order and
only the first matching row opens its command list. The host retains the first reached command and then reads its body incrementally.
Set resolution precedes options and flags. Disabled overrides leave serialized
words unread. The additive command uses only a positive signed command override
and still reads the serialized child divisor. The first-action slot override
comes from the same controller-lived, source-zeroed globals; no reached first
branch changes it.

For each reached set group, slot zero writes one without reading serialized
weight. Other slots write positive zero only when source weight compares equal
to zero; otherwise they preserve current weight. The actual layer owner and
animator bytes retain each write even if the next provider read fails. A negative
child count remains retained before its unsupported boundary. Child divisor and
flags honor their distinct override gates, then the actual reference reaches a
pending factory request. The factory has not returned: child post-word reads,
allocation, copy/wrapper effects, insertion and initialization remain later work.
Sets without a reached child stop before actual cleanup. Mask commands retain
their required control-capture or clear dependency. Unhandled opcodes stay pending.
No command is acknowledged yet. The cursor's protocol requires command effects
to return before reading the next command or trying fallback after source hash
zero. Provider/command errors preserve the reached prefix and prohibit replay.
If no command is reached, the selector return stays separate from its pending
setter tail, including native zero returns. No history words are updated yet.

Live directory, count and opcode reads use the same component wrapper's resident
base, independently of its full cached payload. Reports carry aggregate
cursor/body state and typed pending factory metadata, never source addresses or
private payload arrays.
The dashboard distinguishes a pending command from a pending setter tail. Both
leave factories and remaining command effects, mode history, callback completion,
pose and camera work incomplete. Hosted tests use authored malformed pointers, missing targets, zero
source hashes, ordered command returns and failures; they establish no original
runtime or numerical parity.

The first class-four factory now retains its actual header before initialization.
Controls and script children share one component-local host ID sequence; each
payload store remains separate. Constructors preserve unknown intrusive links
and use typed roles for self-relative pointers. Factory context writes flags and
links the actual slot before updating the animator's current resource reference
and resetting its cache. The snapshot controls and active count stay unchanged.

The same file wrapper owns its publication generation. Its source constructor
starts at minus one, and the first completed base load wraps it to zero. This is
a reconstructed fresh owner, not original preload history. Repeated lookups and
cancelled attempts retain that owner. Replacing/unloading it is not implemented.
The child compares its cache generation before resolving script metadata. A
successful metadata lookup updates the cache before the body is validated against
the logically resident base. A later failure retains those writes and the real
allocation. A cached payload from a later section grants no runtime residence.
A resident script now awaits phase-zero interpretation, while a genuine absent
body takes the native early return. No phase initialization, parent copy/wrapper
step, layer insertion, child scalar initialization or command return is inferred.

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
