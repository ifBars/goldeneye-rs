# Architecture

The native runtime will consume user-supplied game data through `ge-assets` and
execute independently authored Rust systems. Asset discovery precedes runtime
systems: a placeholder game would provide no evidence of parity.

The existing native window now owns `game::Controller`. Pure frontend state
opens the source difficulty page and drains one typed `NewGameRequest`; keyboard,
pointer and bounded probes share the app's command handler. Audio effects remain
separate from commands. Pure `mission::session` retains the actual admitted
descriptor and user-manager state with exact semantic identity selection. This
single-player host provider preserves mode3/backend2 and source count/counter
rules; native provider/event timing and profile history are unverified.

The controller's CPU worker creates and retains its own validated `GameAssets`,
`MissionContext` and admitted session. The non-Send disc partition never crosses
threads. Only typed progress/count snapshots reach the UI; they are not a runtime
data transport. Cancellation disconnects the attempt without waiting on disc I/O,
and stopped workers are bounded/reaped. Every post-window exit path joins workers.
The visible snapshot is captured before that cleanup. Resource loading alone
stops at `AwaitingWorld`. The fresh Tutorial path now completes its owned map,
trigger and loader operations, delivers matching-user readiness and reaches
`AwaitingPlayer`. It creates no player or camera.
The [player continuation](player-creation.md) now dispatches an identity-bound
attempt, selects from the actual live catalog and retains the first constructor
and reset prefix on this same worker. Shared RNG/resource mutations use their
existing owners. Controller-lifetime shared constructor storage now precedes the
second reset. The persistent scratch owner, actual retained character row and
default inventory now feed the remaining derived reset. Its default vector
comes from the same verified supplied DATA owner as the weapon tables, while
the shared quad has explicit startup-constructor ownership. Typed completion
finishes the reset. An identity-bound per-user metadata snapshot supplies the
constructor flag captured before construction. Its consumer precedes virtual
initialization, which now creates the wrapperless EXItem and numeric-ID record,
queries the actual constructor-origin zone, registers actor memberships and
binds and activates the sparse physics capsule. Its unconditional scalar setter
retains an unresolved expression when handler input is unknown; the actual getter
errors instead of using stale constructor data. Independent activation and the
basic session suffix execute in source order. That suffix counts actual retained
kind2 member records, not the descriptor's local-user count. A typed basic return
receipt remains separate from PlayerNew completion, TP/FP components and camera.
The next TP entry repeats the required sparse clear and checks actual fresh
cleanup gates before updating the same inventory's slot bytes. It constructs an
owned animator, performs a direct visual lookup without consuming an actor cursor
or RNG draw, and requests the skin through the existing resource wrapper. Shared
payload and logical section records outlive cancelled workers when retained by a
player. The post-trigger report remains scoped to its earlier operation. Texture
discovery reads only reached geometry fields and retains directory absence as a
distinct outcome. First-encounter hashes feed actual target requests through the
same wrapper, with each successful target owner retained before the next request.
The next file lookups retain the default skeleton and TP component wrappers in
that same registry, before binding. The fresh skeletal runtime owns checked bone
hierarchy, constructor locals/matrices and zeroed auxiliary words. Semantic
animator backlinks replace constructor NULL; primary/secondary skin and component
references remain tied to retained files. Raw reference zero creates no mode.
The captured source frame-counter read remains unresolved and its getter fails.
The bind's zero-time callback now uses a controller-lived shared animation
arena/context. It reserves aligned backing, binds four context regions, clears
the reached shared flags and releases its scope before returning a typed bind
receipt. Empty controls trigger no sample or pose commit. Context locators stay
recorded after release, while checked access rejects their expired reservation.
The actual TP attachment now follows that bind return. A controller-lived host
identity registry assigns the animator a distinct handle before construction;
the skeleton backlink, bind return and item membership retain that same handle.
Attachment writes the real item backlink, clears the source component flag and
appends the first list node. Membership invokes no activation callback. Host
identity ordinals establish no native allocator or SDK registry history. Visual
postbind reuses that wrapper and a lazy checked skin-body lookup to bind the
actual visual secondary skin without reading bone topology. It repeats the
source skin/texture requests and retains their target prefix. The base owner's
existing override list is reset and linked to the item, whose visual flag is
set. The retained row's MAX fields skip extra skins and explicit overrides.
Four constructed headers receive mode3/flags1, while one slot stays active.
The actual embedded player animation receiver now links to the item. The TP
animator retains the same constructor-owned four-slot callback table across
attempts, and its skeletal runtime records the PlayerNew postprocess target.
These assignments invoke no event or pose work. Supported PlayerNew event routes
preserve file fallback, signed reset-byte returns and acknowledgement token
updates; required owner/global inputs remain fallible. A typed TP helper return
now precedes a separate FP constructor. FP repeats the direct character visual
lookup, then uses the retained character's FP skin file and fixed skin reference;
it skips TP visual residency/postbind. Its own skeletal runtime binds before a
second zero-time callback through the same retained scratch/context. Attachment
writes FP's previous link and TP's next link before publishing the item tail.
Both animators retain the same callback table. After FP helper return, two actual
resource-header searches configure the FP masks. The initial action caller now
reads the actual fresh reset fields, stores direction -1, finds both components
by their resource headers and resolves the current signed inventory slot. The
same retained supplied tables map FP, TP and TP-secondary inputs in that order;
mode availability stays a lazy query on the matching component wrapper. No mode
body or control node is created by these queries. TP dispatch now performs its
initial control capture before mode selection. A real header owner precedes the
ordinary empty refresh through the same scratch/context; reached flags clear
before primary locals and auxiliary words are copied. The physical-slot helper
creates four separately owned controls while preserving active count one. Its
shared fallback stores persist across cancellation. TP now qualifies through the
actual FP component wrapper. Its tracked branch clears the reached handler words
and retains the current slot-zero snapshot head/comparison before selection.
The live mode cursor resolves the target before previous/fallback lookup, scans
keys without touching unmatched command lists, then retains one reached opcode.
Each command needs an exact successful acknowledgement before the next command
or fallback can run. Failed execution keeps the completed prefix and cannot replay.
Live asset reads use only the logically resident base; full-file caching grants
no later-section access. A selector return still awaits the actual setter tail.
Live command bodies now read set references before optional operands, preserving
override gates and signed additive divisors. The retained asset stream visits one
group at a time. Reached weight writes update both the layer owner and the actual
animator bytes, including prefixes before a later provider failure. Slot zero
skips its serialized weight; other slots preserve existing weight unless the
source compares equal to zero. The source-zeroed slot-override globals belong to
the controller and survive cancellation. The fresh first-action branch stores no
override. Execution stops at the first child factory or set cleanup dependency;
mask commands stop before required control capture/clear. No later child word,
factory return or command acknowledgement is synthesized. Actual factories,
remaining effects, setter side effects, FP dispatch and the optional second
request remain required. Pose
postprocessing and frame dispatch remain unavailable; these constructor and
mapping operations do not establish player or gameplay readiness.
The first class-four factory now allocates its own retained header before any
script-provider call. A component-local ID allocator covers both control snapshots
and script headers, preserving detached allocations without reusing IDs. Factory
context binds the actual slot and flags before updating the animator's current
resource reference and resetting its cache. Script lookup uses the same retained
file wrapper and its fresh base-publication generation. Metadata caching precedes
body access; a later body failure keeps the header, cache and owner mutations.
Intrusive links remain unwritten until the parent insertion really occurs.
A resident script body now performs fresh phase-zero selection and clip-reference
setup through lazy scalar reads. Sign/history writes precede the duration read;
unknown command payloads remain unread. Inline storage aliases the real header,
while heap records preserve unwritten cache-pointer bytes. The selected index
retains the source count clamp and subsequent raw-index overwrite. Shared host
frame time supplies the scaled step; original timing and numerical parity remain
unverified. The native first TP script binds seven local clip references and
returns clip setup before its animation-body and position dependency. A real
missing script body returns through the source early-out instead. Neither state
completes a layer or command. The separate inspection binder cannot replace this
live initializer. See [script initialization](animation-script-initialization.md).
The controller arena retains the actual player/item/physics owners and registry
rows across worker cancellation. Its world/arena mutex is separate from manager
state, so reached reset/configuration writes can use their original owners.
Native cleanup, later PlayerNew initialization and completion remain required.
Frontend primary music/video advancement is suspended while accepted effects can
finish; focus pause is separate, and Back restores the menu. The future world and
camera remain in this window/device lifecycle, not a nested inspection viewer.

The worker also owns `game/world` partial initialization. Checked asset views
feed the pure `mission/triggers` registry and `mission/animator` constructor.
Original map ordinals, common scratch, list membership and activation flags stay
separate. Source status1 constructors retain inactive objects and stop an absent
animator; status2 StartPoints activate and initialize required animators. Kind-specific
postprocessing keeps mutable derived fields separate from serialized provenance.
Base factory status and effective callback status are separate: a conditional
activation write can promote an initially inactive factory to active registration.
Checked type-0 shape selectors bind before common scratch insertion, with
unsupported scope and malformed data kept distinct; input binding constructs no
geometry. Actor definitions in `game/actors` reuse the character resource already
retained by MissionContext. `ge-assets/actor_definitions` reads the two source
groups; `ge-core/mission/actor_definitions` owns sorted records, logical groups
and fresh mutable cursors. Default/special providers remain separate. Actor12 uses a selected-level
snapshot, one shared source-compatible integer RNG and persistent registry
cursors within the current worker reconstruction. Constructor completion and
the four ordered reference scans have distinct state; missing references block
map initialization. Original prior RNG/cursor history remains unverified.
Additional trigger kinds retain their own typed state under `mission/triggers`.
Range27 shares the proven range calculations while keeping subtype-specific
flags and provider staging distinct. A3, Common09 and AD have separate initial
write rules; their later callbacks are outside the supported initialization API.
Shape71 has a focused subtree for owned geometry and its audited pure math.
The lazy asset view supplies source fields from the owning map, while the host
binds its selector before geometry construction. Materialization and placement
failures preserve reached stages without publishing completed membership. The
actual inline rotation order differs from static asset placement. See
[trigger shapes](trigger-shapes.md).
A reached unsupported provider leaves a staged object without final membership.
The app exports scalar evidence and a private-state digest; unsupported objects
and references continue to prevent catalog completion and readiness. See
[world initialization](world-initialization.md).

The Tutorial trigger pass now distinguishes a source-proven null factory result
from an unsupported host constructor. That slot preserves its original ordinal
without common state, membership or callbacks. Complete constructor/fixup passes
permit an actual owned StartPoint catalog rebuild; `catalog_ready()` separately
tracks that operation. This is component preparation, while the broader source
world-manager sequence and readiness event remain pending. A focused
[navigation adapter](navigation-owners.md) now retains the actual bound wrappers,
shared immutable graphs, fifteen runtime face-category lists and the existing
integer boundary cache. The first registration map supplies initial candidates;
all loaded maps participate in key lookup, while entity resolution preserves the
original level-resource owner. This bounded component does not publish readiness.
The following [resource hook](post-trigger-resources.md) consumes one selection
from the same definition registry, retains its ordered request plan and owns
checked full-file/section residency. Its synchronous supplied-data adapter is
separate from the original streaming scheduler and the value-based resource-state
owner. Source NULL returns, actual payload availability and the sparse manager
reset remain distinct facts.
The subsequent [world-hook segment](world-hooks.md) uses persistent controller
owners and an actual cached frame clock. A generation lease guards worker access
across cancellation; source fields survive between operations while diagnostics
reset separately. Scoped buffer cleanup releases backing storage without
destroying separately retained binding items. Dynamic clock receipts are
validated per probe rather than equated across windows.

`game/resource_states` retains a pure ordered record owner on `game::Controller`
and shares it with workers through a synchronized handle. It survives attempt
cancellation and re-entry, while lookup diagnostics remain attempt-local.
Radius03 binds real queries to this owner; missing ownership remains a different
outcome from a lookup miss. The current explicit fresh constructor snapshot does
not establish original saved/runtime history. Capture/import record semantics
live in `ge-core/mission/resource_state`; engine snapshot admission and disk
persistence remain separate. See [resource state](resource-state.md).

Mission loading is split across focused `mission/` modules. `ge-assets` reads
checked catalog, linked-map, map descriptor and StartPoint records; `ge-core`
owns selection, discovery, registration, map registry and spawn-plan rules. The
app owns bounded I/O and the retained `MissionContext`. It keeps each discovered
EDB once, prepares registration from an explicit initializer snapshot, then
binds actual source spatial/BSP data and placement matrices with resolved entity
directory indices/types. Entity prototype construction and execution remain
separate work.

The fresh map registry seeds null preregistration slot 0. Real map instances
therefore occupy slots starting at 1, while the separate loaded list starts at
ordinal 0; discovery source ordinals also retain their own meaning. The context
privately owns these data instances and the selected source StartPoint pose.
Public receipts contain counts, source locators and hashes rather than original
bounds, matrices or coordinates. `--start-point-id` defaults to explicit input 1
(source Tutorial New Game uses 1), and selection can fall back to ID 0. Prepared
membership is not a live trigger catalog. Base animators, trigger factories,
navigation registration, player and camera creation remain incomplete, so
`world_registered` and `player_initialized` remain false. See
[mission loading](mission-loading.md) for ownership, limits and pose hashing.

`mission::player` owns the character registry resource, selected first-person
animation component and separate skin resource. `ge-assets::character` reads
the character group without interpreting visual-variant rows; missing/default
character fallback requires unavailable native history and remains unsupported.
`ge-assets::skin` supplies checked topology and lazy tags to the pure
`ge-core::animation::skeleton` constructor. The context retains actual ordered
bone/root/child membership, source range masks, constructor identity locals and
matrices, and the resolved camera-tag parent chain. Public evidence contains
only provenance, counts and hashes. Constructor identity is not a sampled pose:
third-person components, physics, complete animator state
and camera frame creation remain incomplete. A raw component reference of zero
does not select the first stored animation mode.

Optional explicit mode inspection lives under `mission::player::animation`.
The retained-owner `ge-assets::animation_modes` view provides lazy mode/set
records; `ge-core::animation::mode` owns target-index selection, ordered
previous/fallback attempts and derived layer-request plans. The app retains the
actual plan and exposes counts plus a digest. CLI animation request, previous
and fallback references must be supplied together; the default context selects
no mode. Reference zero uses native coded-index lookup. Reached StartSet/MaskOperation
commands produce effective flags/divisor and cleanup intents, with no animator
child allocation or sampled pose. Unsupported reached sections/opcodes and
bounded parse failures stay errors, separate from missing targets. This plan
does not establish the initial weapon-remapped request or camera readiness.

`--initialize-animation-layers` selects a separate optional host path under
`mission::player::runtime`; it conflicts with manual mode references. The default
context leaves `initial_layers` null. This path owns the supported default
inventory and selected slot, reads supplied weapon DATA, prepares the two source
FP requests, and executes `ge-core::animation::layers` with actual control capture
and class04 child owners. Clip references share retained immutable animation
bindings. It applies both actual post-selector head-blend setters and counts two
writes even though both fresh values are zero. These setters affect blend
selection only. Explicit source phase setters assign supplied positions;
virtual18 multiplies by script FPS and divides by duration, without accumulating
an elapsed clock. Tick/sample/blend/commit, player initialization and camera
output remain separate.

`ge-assets::frontend_script` supplies typed opcode-02 PlayAnimation records;
`skeletal_animation` supplies checked table-three/body-102 headers and compressed
payload spans. These readers do not decode the skeletal codec or evaluate poses.
The lazy skin auxiliary count gates supported control capture. Animation
descriptor +0x1c is an optional same-wrapper skin reference; body +0x1c is the
owning-wrapper field. The fresh `0xffffffff` reference uses the skeleton's secondary skin.
Raw source model metadata does not bypass conditional runtime skin mapping.

`--decode-initial-frame` requires the owned-layer flag. The host selects the
actual slot-zero child's clip and frame, then uses the independent core
polynomial decoder and an owned bounded raw-frame cache. `initial_layers.raw_frame`
is otherwise null. Its safe receipt separates exact-branch rotations from
unverified host-square-root rotations; exact branch is relative to evaluated XYZ,
not native numeric parity. Raw root/bone vectors stay private. This path does not
apply root bias, skin mapping, control/layer blending or skeleton commit, and it
does not advance animation, initialize the player or create a camera frame.

`--evaluate-prewarm-pose` is a separate fresh-state path. Pure numeric helpers in
`animation::clock` preserve old-position preparation before phase accumulation;
`animation::context` reads root bias and target skin translation defaults; and
`animation::composition` handles captured-control/script scratch and local commit.
The actual fixed reset prewarm saturates the child fade, so the supported path
requires no nontrivial quaternion interpolation. Other fades fail explicitly.
The app retains component update flags separately from sampler flags: the
control capture's initial empty refresh clears update suppression. Fresh null
LOD and zero list-maintenance flags bound this adapter. The committed fingerprint
matches independent source/host reconstruction, while native numeric parity,
matrix rebuild, external scheduler integration and gameplay camera remain open.

`animation::transform` keeps source-ordered quaternion/matrix operations and
requested-bone queries together in a focused subtree. Its owned `ActorBoneCache`
publishes roots directly, builds children from parent-cache times local, and
returns valid hits without rereading local state. Invalidation clears validity,
preserving already computed matrix bits. The uncached helper preserves a
different ancestor association and its identity products. Neither operation is
the skeleton's separately cached hierarchy refresh.

`mission::player::camera` retains tagged-bone matrices and optional FP cache
privately after explicit prewarm commit. The app retains per-bone quaternion
precision alongside the committed pose. Optional `camera_bone_transform` evidence
contains only source IDs, counts, provenance and fingerprints. Full native FP/TP
cache setup/history, session-dependent owner scale/placement, camera forward/up
adjustments and the displayed gameplay frame remain unbound.

```text
User image (RVZ / supported disc format)
  -> nod (open-source disc reader, not a game runtime)
  -> ge-assets::disc (edition check, game data partition, bounded reads)
  -> ge-assets::filelist (EngineX v7 metadata, locations, names/hashes)
  -> ge-assets::resource (bounded EDB payloads; hash selection and texture links)
  -> ge-assets::edb / tpl (metadata, texture frames/mips, GX -> RGBA)
  -> ge-assets::scene (zones, shared placement prototypes, GX strips, texture bindings)
  -> ge-assets::graph (0x607 navmesh data; original traversal behavior unverified)
  -> ge-assets::collision (static root 0x601 inputs; raw strip order and flags)
  -> ge-assets::spatial (reachable map BSP/zone inputs; no render dependency)
  -> ge-assets::portals (ordered groups and addressed inline quads / linked polygons)
  -> ge-assets::frontend_script / frontend_morph (lazy script tracks and addressed color frames)
  -> ge-assets::frontend_text -> goldeneye::menu::credits (bounded English lookup and source credit lines)
  -> ge-assets::spreadsheet -> ge-core::action_tables (typed ResourceAction binding)
  -> goldeneye::startup_resources (asynchronous retained legal dependency subset)
  -> ge-core::frontend_sequence / frontend_fade (startup gates and global RGBA overlay)
  -> ge-assets::frontend_stream / sound_info -> goldeneye::frontend_cue (checked resident intro PCM)
  -> goldeneye::frontend_audio (primary PCM plus independent bounded host-clock voices)
  -> ge-core::frontend (pure menu model, screen tree, source slider settings)
  -> goldeneye::menu (host session, preferences, cached effects and layout)
  -> ge-core::frontend_animation / frontend_segments (bounded title poses, commands and segment timing)
  -> goldeneye::frontend_title -> ge-render::frontend (source mesh/UV overlays over moving THP media)
  -> ge-render::GpuScene (persistent textures/buffers, camera uniform, depth, unlit draws)
  -> goldeneye::viewer (native winit window/surface and inspection controls) / PNG readback
  -> [next] unknown entities, original visibility/material states, skeletal animation and full audio behavior
  -> goldeneye::navigation (checked asset-to-runtime triangle/group adapter)
  -> ge-core::navigation (first/nearest rays, selected/connected-face segments, ordered policy traversal)
  -> goldeneye::path (checked center/weight preparation) -> ge-core triangle-chain selection
  -> ge-core alternate predicate search (shared queue/pool, explicit absent-destination heuristic)
  -> ge-core optional actor target edges (destination flags/stored Y, full 3D direction)
  -> ge-core actor goal callbacks (proximity + ordered XZ face walk, explicit raw context)
  -> ge-core actor target caller binding (distinct owner/instance frames, manager surface policy)
  -> ge-core target action gate / selected-record state and motion update
  -> ge-core grenade launch packet / angular damping (unintegrated; no physics body)
  -> ge-core grenade dispatcher / analytic arcs / two-stage clearance (actor binding pending)
  -> ge-core oriented ascent box -> initialized static world adapter (first ascent only)
  -> ge-core box/triangle candidate + face/edge contact (live world integration pending)
  -> ge-core collision world selection / BSP binding / ordered portal distance + expansion (explicit contexts)
  -> ge-core collision rigid/general frames + consecutive record cache (live population pending)
  -> ge-assets common spatial headers -> ge-core affine bounds / ordered component union / margin gate
  -> ge-assets compact components / slot-six bounds -> initial bound assembly diagnostics
  -> ge-core lazy candidate hierarchy / ordered geometry filter / bounded record append (live dispatch pending)
  -> initialized hierarchy refresh + asset geometry adapter -> ordered static snapshot collection
  -> resident static batch cache / coarse tree -> ordered query-frame triangle/box traversal
  -> ge-core type-three contact-result binding -> collision-box authored snapshot diagnostics
  -> explicit world-manager snapshot -> BSP/lazy portal selection -> combined zone record/cache/contact sequence
  -> explicit world-line snapshot -> segment BSP zones / packed candidate gates / transpose placement frames
  -> line geometry recursion / resident mode-two bounds + planes -> nearest or early-stop result
  -> typed cached-apex grenade line constructor / pre-lift broad bounds -> world-only blocked diagnostic
  -> grenade-ascent-box typed source request (64 records / 512 contacts; blocked count only)
  -> ge-core::NavigationFrame (explicit world translation + raised nearest endpoint binding)
  -> ge-core alternate world binding (start only, selected raw center and retained route state)
  -> ge-core waypoint builder (centroid seeds, first-failure greedy portals, flags and vertex fans)
  -> ge-core WaypointRoute (next target, final-pair arrival, 3D length and corridor proximity)
  -> ge-core acquisition / actor dispatch (stored bounds, ordered candidate scoring, deferred query callbacks)
  -> ge-core NavigationRouteState (status, retained lists on miss, complete replacement on success)
  -> ge-assets static map registrations -> shared PathData + individual candidate frames
  -> ge-core initial ordered boundary caches / raw XZ first-blocker diagnostic
  -> ge-core actor band gates (explicit snapshots, lazy second position resolution)
  -> [future] fixed ticks, actors, steering/world movement -> renderer / audio / input
```

The workspace presently contains `ge-assets`, `ge-core`, `ge-render`, `goldeneye`, and `xtask`. Add future
crates when their actual systems exist. The simulation must eventually accept
recorded input and fixed ticks, enabling reference comparisons without tying game
logic to display refresh. Platform/rendering code belongs outside that core.

The startup sequence binds warning acceptance to an auto-clearing global fade.
Warning time accumulates in f32 before the 1.5-second input and 21-second timeout
tests. Acceptance retains the warning while a separate 0.5-second black fade
runs; its artwork clock continues independently. `frontend_fade` preserves the
first-update skip, separate f32 interpolation and byte truncation: midpoint alpha
is 127. The source UI update precedes boot logic, so a new fade first advances
on later UI updates, and completion clears the overlay before boot changes the
screen in that same update. There is no invented fixed-frame delay or audio gain.

Legal entry starts a global black-to-transparent fade over 0.5 seconds. Its
visual script has no alpha track. After that fade, `startup_resources` starts a
worker that reads the checked twenty-byte spreadsheet directory and binds the
supported `ResourceAction` tables. `action_tables` preserves four action rows,
ten composite rows, storage-first lookup and native-null references. The current
image produces 31 resolved and nine null bindings; these are content diagnostics,
not completion percentages. The app retains the result, and the core's host
dependency gate prevents legal exit until this supported work is ready.

The 301/30-second legal minimum remains an explicit host fallback. The other
original initializers, sixteen resource-queue slots and menu-0f lifecycle are
not implemented; the full native asynchronous loading lifetime remains open.
No legal exit fade is inferred. Fade boundaries stop synthetic elapsed-time
catch-up. Developer stage probes use
`seek_stage` to enter and initialize one stage without claiming skipped boot
transitions or relying on accumulated 21/42-second offsets.

The frontend title adapter binds 13 source scripts to the original mesh geometry
and UVs. `ge-assets` checks command chains, addressed positive-count tracks and
component-five color frames. `ge-core::frontend_animation` evaluates translation,
quaternion, scale and color/alpha tracks, preserves ordered creation/removal and
exposes composed poses without file or rendering dependencies. Its supported
loop-to-zero case is bounded; unsupported curves and reached control commands
produce explicit errors. This is a scoped frontend interpreter, with general
script VM and skeletal animation work still open.

`frontend_segments` takes inclusive ranges from the title's source UI markers:
intro frames 0–29 at 60 fps, a held endpoint, then confirmation starts outro
frames 30–49. Endpoint completion uses a strict `>` time comparison. The host
adapter clamps the displayed root frame rather than replaying the native
controller's overshoot and rewind callbacks. The glint's color animation uses an
independent tagged-child clock during the hold; resumed parent time can reset
and retarget that child without resetting the parent's pose. Color-frame blends
preserve the native byte/256 weights and separately truncated contributions.

The app combines these overlays with the moving THP background and existing
decoded PCM output. Source mesh materials select alpha/additive blending and
clamped 2× texture/raster RGB modulation with singly modulated alpha. Mesh UVs
and colors retain perspective interpolation. `frontend_projection` supplies the
source UI root `(0, -10, 0)`, camera `(0, 18, 0)` and per-vertex depth, including
the engine's actual approximate-pi field-of-view conversion. Native SDK tangent,
matrix rounding and Wii numerical precision remain unverified.

`frontend_sort` orders type-two translucent entities by the original stored
bounds transformed into view Z. All batches of an entity stay grouped; computed
keys use repeated f32 bias increments, and exact negative-key ties reverse input
order. Derived vertex extrema are not substituted for the stored bounds. This
queue contract is separate from a complete native depth-buffer implementation.

The host selects `--title-aspect wide` by default or `--title-aspect standard`.
These select the source full-viewport 16:9/4:3 settings; game assets do not reveal
the original user's runtime display flag. The existing 640×480 capture canvas
and letterboxing remain host presentation choices. The prompt and main-menu
layout retain authored placement. The title audit checks RGB patterns separately
from alpha and mesh population, with a guard against independent RGB tracks in
the reached title cohort. Native GPU depth/filter behavior, the full material
pipeline, original display selection and visual parity remain unverified.
`frontend_title` emits the supported root opcode-five request once at intro
frame 11, using source gain 1.0, and suppresses playback during seek/rebuild.
`frontend_cue` resolves bank/sound/wave identity and runtime class-three metadata
before decoding memory-only PCM. Actual metadata flags `0x1fc` clear owner-release
cancellation: frame-36 slot removal detaches ownership while the nonlooping cue
finishes naturally. It is not a confirmation sound.

`frontend_audio` keeps bounded one-shot voices separate from the primary movie/music
queue and phase. Each voice advances on output-device frames, including while
the title root holds; primary format priming preserves it. Callbacks mark finished
voices and retain their PCM ownership until control-thread retirement. Final
summation, linear resampling and output limiting are host adaptations, not
recovered Wii DSP/mixer behavior. Remaining external control/audio commands are
recorded rather than executed; original input and mixer/synchronization remain
open. See
[frontend scope and startup](main-menu.md). These changes do not complete profile,
save, the remaining submenus or gameplay behavior.

Pure menu navigation lives under `ge-core/src/frontend/`: `model` defines rows,
actions and change effects; `screens` contains the source-backed tree; `settings`
owns bounded slider values and source gain conversion. Device input, timestamps,
preferences, sound submission and layout belong to `goldeneye/src/menu/`.
Keep future frontend features in these focused modules rather than expanding
the crate roots or mixing storage/device ownership into core state.

The actual source main menu is `0x7d000009`; the default frontend entry
`0x7d00000a` is the preceding title menu. Options and Extras preserve native row
order while the host adapts keyboard/mouse navigation and restores selection at
each return depth. Settings persist in a bounded versioned Windows document,
independently of Wii saves. Music/effects gains update existing host playback;
dialog volume is retained for a future speech consumer. Source brightness values
are applied through a host display adaptation. Four cached source effects route
only successful navigation, acceptance, Back and setting changes to independent
voices. Clamped and rejected input produces no effect. Source defaults and
step/clamp behavior do not establish Wii mixer, display or input parity.
`ge-assets::frontend_text` follows the English page mapping and native biased
binary search without sorting or whole-payload string scanning. The app's
`menu::credits` selects the source's 51 text IDs, preserves literal line/font
parsing and supplies content to the host scrolling view. Reference-canvas
clipping, colors, removal of leading blank padding and the adapted scroll clock
remain distinct from original credits presentation. Cheat recognition remains
unimplemented.

The initialized static collision path now runs from source zone/candidate
metadata through component bounds, filtering and recursive geometry collection
into resident mesh queries. `goldeneye::mesh_query` owns asset residency, runtime
batch caches, explicit snapshot intake and provenance identities. `ge-core`
owns coarse-tree traversal, batch/triangle pruning, type-three primitive queries
and contact-result binding without file or rendering dependencies. Collected
records preserve storage order and consecutive frame-key reuse; shared geometry
does not deduplicate contacts. Passing triangles move into the request's query
frame, while the boxes remain in that frame. See [mesh contacts](mesh-contacts.md)
for matrix order, cached versus current state and capacity rules.

`collision-box` selects an explicit initial zone. `goldeneye::world_query` adds
an explicit ordered manager snapshot and opens only the selected world's resource.
Its BSP supplies the initial zone; request bit2 validates that zone's row/group
metadata and evaluates portal shapes lazily, preserving returned order and duplicate
zones. Malformed unused row metadata can still fail the bounded host reader.
`static_collect::append_zone`
adds each zone's geometry and placements to one record buffer, with candidate
ordinals restarting per zone. `mesh_query::query_collection` consumes the combined
list through one inverse cache, resident mesh table and contact buffer.

The typed `grenade-ascent-box` adapter constructs the first-ascent request with
the original start/apex-distance selection values, identity Q, filter slot 1/flags 3,
portal flag 2 and source capacities 64/512. Its `blocked` result follows contact count only.
It does not advance the full arc/action state or spawn/activate a body. Read
[world contact requests](world-contact-requests.md) for complete schemas and limits.
The separate `goldeneye::world_line` adapter now selects an authored world and
binds the full segment to ordered BSP zones before any hit shortening. It uses
line-specific packed candidate gates, current placement transpose frames and
0x601/603/608 recursion. `goldeneye::line_mesh` owns resident mode-two batch caches
and the all-included-triangle plane stream; `ge-core` owns plane construction,
closed triangle predicates, coarse traversal and result-frame math. One mutable
nearest result spans all roots. Equal hits replace, while nonzero early-stop
bytes return before world-normal conversion. Constructor broad bounds remain
separate from caller-mutated displacement until a mesh hit refreshes them.
See [world line queries](world-line-queries.md) for the snapshot and read limits.
`GrenadeWorldLineRequest` prepares the cached-apex request before its later Y lift;
both arc callbacks retain that constructor displacement. `grenade-world-line`
applies the typed request to the static world path, without the required actor
fallback or full action-state transition.
Actor-manager/component line integration, nonzero
position contexts, live manager/membership mutation and cache invalidation remain
open. Authored static queries do not promote reference parity or imply physics response.

`ge-core` now contains actual surface-query runtime operations, with no parser,
graphics or timing dependency. It accepts ordered triangle values and explicit
ray parameters. Each operation preserves its own ordering and endpoint rules.
The app adapter validates asset indices and prepares resident
triangle/group arrays once per loaded navmesh. Surface primitives remain mesh-local;
the separate world binding subtracts an explicit translation after raising Y by one.
It does not infer render transforms or implement player collision/actor simulation.
The current f32 normalization is provisional relative to Wii estimate instructions.
Connected-face lookup uses an explicit stack, preserving current-face priority,
neighbor-slot order, signed depth and callback timing. Its default policy prevents
immediate backtracking; there is no global visited set. An explicit host face-test
budget reports exhaustion as an error. Actor-specific policies remain unimplemented.
Triangle-chain selection keeps observed queue ties, equal-g replacement/reopening,
cost fallbacks and pooled parent-slot identity. It adds an explicit expansion guard;
heap-address reuse after whole-block release and self-edge expansion remain
unsupported errors. See `navigation-paths.md` before extending it.
The alternate selector shares that search machinery but calls a goal predicate
after each queue removal, before edge expansion. Its heuristic receives `None`
for the source -1 destination. The observed actor point heuristic returns squared
3D distance; the CLI uses explicit authored acceptable-face sets and point inputs.
The separate actor diagnostic uses the observed geometric goal callback. See
`navigation-goal-paths.md` before extending this stage.
An optional actor edge policy applies ordered backtracking, destination flag,
stored-vector Y and full-3D target direction gates. Missing vectors remain absent;
the target point is explicit raw context data. Its fixed engine cosine threshold
is statically evaluated; normalization/FP parity remains open. See
`actor-target-edges.md`. `nav-actor-path` instead uses proximity and ordered face
walking from an explicit stored face/origin. Scratch center writes survive rejection;
missing crossings and budgets are errors. See `actor-target-goals.md`. Live caller
integration, storage and movement remain open.
The separate `nav-actor-target` stage now prepares those coordinates from explicit
world snapshots. Its owner-local storage-first ray and height gate precede a lazy
binding reload; the first mesh is retained for the later instance-local connected
lookup. Manager flag transitions differ from both default and actor target edges.
See `actor-target-caller.md`. The external action predicate, live storage and later
action integration remain open. `nav-actor-action` now composes the observed angular
precondition, binding/search and selected-record update over an authored JSON
snapshot. Gate/miss retains state and returns one; success updates only the active
motion plus raw state two. See `actor-target-action.md`. Population, scheduling,
subsequent motion integration and collision remain open.
Alternate world binding now looks up only the start and uses the selected raw
center as its goal value, subtracting/adding translation around waypoint generation.
Persistent state retains lists on failure; predicate exhaustion has status 10.
The app offers authored goal policies through the same dispatch command. See
`navigation-world-goals.md`; actual actor inputs and movement remain unverified.
`WorldPathResult` distinguishes missing endpoints (no search) from a completed
search with no path. Diagnostic surface points do not replace the original world
request positions. World samples use authored frames, not source actor instances.
See `navigation-world.md` for operation order, bounds and remaining actor context.
Waypoint construction is a separate pure stage over shared vertex IDs, centers,
raw optional flags and full neighbor rows. It preserves first-failure order and
the shortcut predicate's distinct portal/fan rules. Host limits report errors;
there is no fallback point list. Local unraised request points become world points
only after reduction. Original CPU numeric parity and actor clearance are unverified.
`WaypointRoute` borrows the resulting world list. Its target is point one; arrival
requires exactly two points and strict horizontal quarter-unit proximity. Corridor
queries use full-3D distance and a separate first-segment forward gate. This is a
read-only stage, without point advancement or actor movement. See
`navigation-route-consumers.md` for the distinct FP orders and limits.
`NavigationRequestState` owns raw phase flags and an explicit cached binding index.
`NavigationRouteState` separately owns retained triangles/world points and wrapper
status. Ordinary misses retain both lists, while successful queries replace them
in source order. The app can replay authored positions across dispatch calls; this
does not simulate movement or imply waypoint popping. Read
`navigation-route-regeneration.md` for status priority and mutation stages.
Acquisition and query callbacks keep loading/policies outside the pure phase.
The app's `nav-request` diagnostic prepares root entities in an authored shared
frame, then executes bounded dispatch calls. It does not establish original manager
membership, scheduling, route lifetime, steering or movement. Read
`navigation-dispatch.md` before extending this state.
Static `--map` mode now supplies initial placed-navmesh frames, independently of
renderer matrices. Native entity references and instance-key lookup have distinct
rules; registrations preserve unavailable bindings and share decoded mesh data.
Loaded-map precedence and dynamic instance state are still open. See
`navigation-placements.md` before extending registration/resolution.
Each resident `PathData` prepares one initial immutable boundary cache. The
optional request diagnostic uses raw input coordinates against local mesh vertices,
independently of world endpoint binding and route success. It preserves collinear
acceptance and cache order. Live action selection and mutable per-wrapper state remain
open. See `navigation-boundaries.md` for construction and numerical contracts.
Four observed actor range predicates now wrap that boundary query with strict
horizontal bands, separate height limits and mode/binding gates. Explicit callbacks
preserve lazy checks and second position resolution. The app offers authored
snapshot diagnostics; live action selection and actor storage are not implemented.
See `actor-boundary-gates.md` before extending this stage.

GPU upload is separate from frame encoding. `GpuScene` owns the pipeline, textures,
model groups, draw ranges and vertex/index buffers. A camera uniform updates once
per frame; immutable model uniforms remain shared. Native surface resizing replaces
depth/swapchain resources without re-reading the disc or rebuilding scene buffers.
Source images are RGBA8 regardless of RGBA/BGRA output. Readback normalizes channel
order to RGBA. The offscreen and native paths execute the same draw encoder.

The app owns winit events, surface acquisition/recovery and inspection input. The
current scene is static, so visible inspection redraws on demand, with delayed
surface retries. Hidden smoke has an explicit bounded frame tick because hidden
windows can receive no paint events. That check does not validate human controls,
original simulation timing, or gameplay input semantics.

`Scene` owns metadata and placements alongside `Geometry`. Geometry retains unique
vertex/index buffers and draw ranges; placements reuse prototype ranges with a
model matrix instead of baking duplicate vertices. Submitted triangle counts
include repeated draws; unique buffer triangle counts do not. The initial model
transform is T * S * Rz * Rx * Ry with the observed order-six polynomial;
live changes and original-runtime comparison remain pending. Strict
placement loading rejects unknown entities; explicit incomplete inspection reports
only typed unsupported entities and still fails on corrupt data.

Navigation meshes are a separate asset type. `Edb::entities()` inventories root type
tags; `entity_graph(hash)` rejects ambiguous hashes, while `entity_graph_at(index)`
selects an exact directory entry. Scene placement status distinguishes geometry
from navigation, and shared `Scene::nav_meshes` retains the latter without GPU
draws or render bounds. `placements_loaded` includes both kinds; separate counters
report render/navigation instances. Scene JSON omits navmesh arrays; `entity-graph`
exports them locally. Original AI and loaded-world collision remain incomplete.
The reader preserves absent arrays and unknown fields,
and reports topology/geometry diagnostics with measured sample counts. The main
resource audit covers every root 0x607 entry, including duplicate-hash entries.

## Data and provenance

- `nod` 1.4.4 reads RVZ and the Wii filesystem. It is MIT OR Apache-2.0 licensed.
- Our Filelist parser reads version 7 big-endian records with relative name
  pointers and encoded strings. It is independently authored from format study.
- `PartitionMeta` in nod internally reads disc metadata, including DOL data; no
  disc code is executed, linked, emitted into source, or packaged.
- The asset API refuses common executable extensions. Other containers are
  unclassified data; an extension allow/deny list is not proof of asset purity.
- The separate typed `weapon_static` adapter hashes the bounded supplied DOL
  against the supported SJBE52 revision-0 profile and retains only DATA section 5.
  It exposes bounded metadata/remap queries, never a public executable-byte or
  arbitrary-address API. Original tables remain supplied data rather than an
  embedded corpus; no original instructions execute or ship in packages.
- No raw archive is ever a distributable artifact. Runtime resource types must
  be classified before caching/export becomes a supported user workflow.

## Checks and evidence

The [loader ownership guide](loader-owners.md) covers fresh map construction,
logical section publication, retained audio providers and the identity-bound
world-to-player boundary. Those operations live in focused `mission/loader`,
`mission/maps` and host `game/world_hooks/loader` modules. The controller keeps
persistent globals and immutable audio caches; mutable mission owners stay on
the worker. Player construction and later camera/view setup remain separate.

`xtask` runs tools serially and records their exit status against a deterministic
hash of Cargo manifests/lockfile, configuration, and all Rust source. Receipts
are local. Generated public summaries include safe aggregate outcomes only.
The dashboard never treats stale receipts as current passes. It is not a signed
attestation: receipts are local development evidence, not tamper-proof security.
The native render receipt is separate from asset parsing, coverage and parity.
CI parses/validates WGSL without requiring a GPU; the local GPU fixture is opt-in
and must report an actual passing test before recording render evidence. All
offscreen frame data remains under ignored `local/`.
