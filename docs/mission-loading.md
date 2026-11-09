# Mission residency, map data and StartPoint preparation

The first New Game request is Tutorial (`0x0181016b`, `mt_tutorial.edb`).
The source difficulty menu selects Operative, Agent, 007 or 007 Classic, encoded
as 0 through 3, before requesting that resource. This is separate from the
existing Dam inspection renderer and from a running gameplay session.

Inspect the retained mission context with a local user-owned image:

```powershell
.\target\release\goldeneye.exe mission-context '.\GoldenEye 007 (USA) (En,Fr).rvz' --difficulty 0 --start-point-id 1 --output local/tutorial-context.json
cargo xtask mission '.\GoldenEye 007 (USA) (En,Fr).rvz'
```

The context owns the decoded level registry and each EDB in the discovered
mission-resource chain. Every file is retained once, with the root included in
the same ownership list. Its
receipt contains IDs, counts, hashes and source locators, rather than original
row arrays, coordinates, scripts, asset text or executable code. It also owns
map data instances and a prepared source StartPoint pose. World registration
and player initialization remain false: these data preparations do not start a
mission.

`ge-assets::mission::levels` reads group zero of the two `o_leveld.edb` sheets:
`0x14000011` has 180-byte primary rows; `0x1400003a` has 116-byte tuning rows.
The current image contains 133 and 16 rows respectively. These are diagnostics,
not completion percentages or universal content counts. The reader reuses the
checked spreadsheet implementation and preserves raw words, including float-like
bits whose meaning is not established.

`ge-core::mission::catalog` implements unsigned-key registration. Duplicate
arrivals leave the first record intact and produce explicit diagnostics.
Selection preserves a less obvious source rule: a missing primary row changes
the current requested hash and clears the primary selection, but leaves the
previous tuning selection intact. A successful primary lookup refreshes tuning
from that same hash. Previous-hash updates skip the `0xffffffff` current sentinel.
Initial selection is supplied explicitly; the inspection context does not invent
an original previous-level or profile history.

The app owns resource I/O and retention under `apps/goldeneye/src/mission/`.
Core catalog state has no filesystem, graphics or wall-clock dependency. The
CLI uses the same retained loader intended for later runtime integration.

The tutorial's own map is a logic container with an empty type-603 split and a
coded navmesh placement. Its visible graphics resource is `mg_tutorial.edb`
(`0x018b002a`), whose first map has stored hash `0x05000070`. Native map loading
uses coded reference `0x85000000` to select map index zero, so the coded reference
and stored map hash must remain distinct. The current static renderer can decode
that graphics map, including its placements. Its fitted inspection camera is
not a player spawn or gameplay camera.

The source linked-map helper scans trigger rows in storage order and selects the
first matching linked-map class. Optional file/map properties are read only when
their flags are enabled. Native residency follows the returned file, deduplicates
by file identity and forces map index zero for the next resource. Trigger/widget
scripts are not interchangeable with this map-registration path.

A matching record without its file flag retains `0xffffffff`; that is not the
same as no matching record and must not silently end residency. The subsequent
activation pass may reorder discovered files through runtime preference arrays,
so discovery order alone does not prove final world-manager order.

`ge-core::mission::residency` owns the discovery state machine. The app's
`mission::residency` adapter performs real bounded loading, acknowledges retained
owners before linked-map lookup, and keeps both completion reasons distinct.
Default host limits are 32 files, 64 MiB per file and 128 MiB aggregate payload;
the catalog is separately bounded. Each next header receives the remaining
aggregate allowance before full payload allocation. A failed attempt returns an
error and exposes no completed context.

The current Tutorial chain retains two files and 15,094,304 payload bytes. These
are local diagnostics, not full-game completion measures.

`ge-core::mission::registration` prepares row order and raw bytes separately
from discovery. It takes explicit preference vectors, preserves the input list,
moves the first preferred row to index one, then assigns subtype allowlists.
Null and empty allowlists stay distinct. The app declares a snapshot after one
source preference-initializer call; original virtual-init timing and previous
history remain unverified. Preparing rows alone does not construct map instances.

The app's `mission::world` adapter then constructs owned map data instances in
prepared registration order. Each instance retains source spatial/BSP data,
declared bounds and placement matrices with resolved entity directory indices
and types. It does not construct or execute entity prototypes. The fresh
`ge-core::mission::world` registry seeds a null slot zero, preregisters each
canonical map key and publishes its constructed instance. Consequently the first
real preregistration slot is **1**, while its separate loaded-instance ordinal
is **0**. Discovery source ordinals remain distinct from both. Bounds and
placement matrices stay private; receipts expose their hashes and counts.

`ge-assets::mission::startpoints` reads bounded source StartPoint records using
the shared trigger walker. The app applies the prepared row flags and subtype
allowlists before building a selection catalog. This is an explicit preparation
snapshot, not evidence that live trigger factories populated the original
catalog. `ge-core::mission::spawn` selects the first enabled exact requested ID,
then falls back to ID **0** in the same order. A successful fallback plans an ID
reset to zero; a missing selection retains no pose.

`--start-point-id` defaults to **1** as an explicit inspection input. Tutorial's
New Game path supplies that ID in the source; the default does not establish the
original start ID for every other level or saved session. The source initial
pose retains position bits, uses W=0, and preserves rotation Y/W while setting
rotation X/Z to positive zero with rotation mode 3. The owned plan keeps those
bits private. Public `spawn.pose_sha256` hashes the 33-byte encoding
`position_u32be[4] || rotation_u32be[4] || rotation_mode_u8`; it exports no
coordinates and is not a camera frame.

The retained context also binds the supported level row's character request
through `mission::player`. `ge-assets::character` reads group one of
`o_chrctr.edb`'s character sheet and retains the first row for each duplicate
key. This group has a different layout from visual-variant group zero. An
invalid or missing requested key reports unsupported native default/history
fallback; the host does not substitute the first stored character. The selected
row's first-person skin-file field selects a separate retained skin resource.
`cp_bondfp.edb` is the retained animation-component resource, not the skin file.

`ge-assets::skin` resolves the fixed first-person skin reference and reads its
bounded raw hierarchy. `ge-core::animation::skeleton` then constructs owned CPU
bone membership with source-order roots and children, parent links, and the
source four-word range masks. It rejects unsupported forward parents, cycles
and out-of-range references rather than repairing the tree. Local translation
and quaternion constructor values are `[0, 0, 0, 1]`, with identity matrices.
These are constructor values, not the skin rest pose or an evaluated first
animation frame. Range masks come from source range fields, not a recomputed
descendant set; auxiliary hierarchy words remain uninterpreted.

The app resolves camera tag `0x0e000028` against that actual retained skin and
checks its parent chain in the owned skeleton. Tag lookup compares the low
sixteen bits, stops at the sentinel before reading a bone index, and preserves
first-match ordering. `player_components` receipts report IDs, provenance,
counts and hashes; original character rows, rig arrays and transforms remain
private. Camera-tag resolution establishes a dependency, not camera output.
Third-person components, player physics, complete animator state and pose
sampling remain incomplete. Component reference zero
does not justify selecting the first stored animation mode. No sampled pose or
camera readiness is inferred from constructor identity.

An optional animation inspection plan uses that retained first-person component.
Supply all three explicit mode references together; omitting them prepares no
animation plan and invents no initial mode or history:

```powershell
.\target\release\goldeneye.exe mission-context '.\GoldenEye 007 (USA) (En,Fr).rvz' --animation-request 0900071e --animation-previous 0 --animation-fallback 0900065e --output local/tutorial-animation-plan.json
```

These are inspection inputs, not live player creation inputs or a claim about
the final initial first-person request after weapon remapping. Previous reference
**0 is coded directory index zero**, not a missing mode. `ge-assets::animation_modes`
resolves the requested target index without reading its descriptor/body. The
pure `ge-core::animation::mode` selector scans previous-source transitions in
order, uses the first matching target index and tries fallback only after a zero
return. A matched source hash zero can already contribute planned commands;
those effects stay ordered before a fallback attempt. Unmatched command pointers,
later duplicate transitions and zero-count body pointers remain unread.

The app privately owns the resulting transition plan, source groups and child
requests. Supported reached commands are `0x0c000002` StartSet and `0x0c000005`
MaskOperation. Reached other operations or nonzero sections report unsupported
errors; malformed data and budget exhaustion never become ordinary misses.
Plans preserve raw flags, weights and divisors and derive effective flags,
reciprocal-divisor scalars, head-insertion requests and conditional cleanup
intents. Slot/weight requests require an explicit current snapshot. Mask and
cleanup intents do not clear existing layers or establish allocation success.
Receipts publish counts and an opaque ordered-plan hash, not original command
arrays. `initial_request_verified`, `runtime_children_created`,
`animation_sampled` and `camera_frame_ready` remain false: weapon/action
remapping, actual child construction, sampling/blending and camera output are
separate work.

An additional opt-in checkpoint constructs owned first-person animation objects
at the fresh constructor callback boundary:

```powershell
.\target\release\goldeneye.exe mission-context '.\GoldenEye 007 (USA) (En,Fr).rvz' --difficulty 0 --initialize-animation-layers --output local/tutorial-animation-layers.json
```

This flag conflicts with the three manual mode-reference flags. Without it,
`player_components.initial_layers` remains absent or null; ordinary mission
inspection and explicit mode planning retain their existing scope. The opt-in
path owns the supported default inventory record, selected slot and weapon-data
remapping, prepares the two ordered source requests, and executes the supported
layer operations through independently authored core rules. It retains captured
control-node bone locals and actual class04 script children, with repeated clip
references sharing immutable skeletal animation bindings.
Both source post-selector head-blend writes are applied and counted, including
their unchanged zero values. They adjust clip selection only; they do not advance
script position, transition progress or an animation clock. Source virtual18
assigns the supplied position multiplied by script FPS and divided by duration;
it does not accumulate elapsed time.

`ge-assets::weapon_metadata` reads the required `o_guns` spreadsheet fields.
The separate typed `weapon_static` adapter verifies the supplied SJBE52 revision-0
DOL hash and DATA profile, retaining only DATA section 5. It exposes bounded
weapon queries rather than executable bytes or arbitrary addresses. The general
asset API still refuses executable reads. No original code executes, no original
table corpus is embedded in source, and no original bytes or table arrays enter
public receipts or packages.

`frontend_script` now types opcode 02 PlayAnimation lists in body-300 scripts.
`skeletal_animation` checks table-three/body-102 headers, sparse channel indices
and separate compressed payload spans. Those are bound source records, not a
pose evaluation. The host also reads the actual skin auxiliary count
lazily and rejects unsupported nonzero auxiliary capture. Raw model-pair metadata
does not force a different live skin. Animation descriptor +0x1c is an optional
same-wrapper skin reference, distinct from the body +0x1c owning-wrapper field.
The fresh `0xffffffff` descriptor reference selects the skeleton's secondary skin;
conditional skin mapping remains separate.

Optionally decode the raw frame selected from the actual owned slot-zero script:

```powershell
.\target\release\goldeneye.exe mission-context '.\GoldenEye 007 (USA) (En,Fr).rvz' --initialize-animation-layers --decode-initial-frame --output local/tutorial-raw-frame.json
```

`--decode-initial-frame` requires `--initialize-animation-layers`. It selects the
owned child's clip and frame through the source rules and retains the decoded
polynomial vectors in a bounded owned cache. `initial_layers.raw_frame` stays
null without this flag. Public evidence contains counts, cache activity and
fingerprints, including separate exact-branch and unverified host-square-root
rotation counts. Exact-branch labels describe the evaluated XYZ branch; they
do not verify Wii numeric parity. Original vectors and compressed data stay local.

`initial_layers` receipts publish inventory/profile identity, object and clip
counts, capture counts and digests. `post_selector_scalars_applied` is true with
`post_selector_head_writes` equal to two after the actual setters execute.
Animation tick and pose sampling remain false, including when raw decoding is
requested. Root adjustment, skin mapping, control/layer blending and skeleton
commit are separate prerequisites. Player initialization and camera output also
remain false. A raw camera-tag vector fingerprint does not supply a camera frame,
and this owned callback snapshot is not the first displayed pose or a playable
mission.

`map_instances.bound` reports these actual data bindings, while
`world_registered` and `player_initialized` remain false. Base animators,
navigation registration, trigger factories, live StartPoint catalog membership
and player/camera creation are still incomplete. The first-person skeleton
constructor does not complete the native animator or `PlayerNew`. No mission script or gameplay
loop runs, and a prepared spawn position does not supply the camera eye.

The separate `--evaluate-prewarm-pose` flag requires owned layers and conflicts
with raw constructor decoding. It executes only the supported fresh reset's
fixed 0.2-second prewarm, producing `initial_layers.prewarm_pose`; otherwise that
field is null. It updates both active children in source order, including the
zero-weight layer, before sampling the control and selected script. Root bias,
6 animation translations and 42 skin fallback translations prepare 48 local
poses. Script success supplies the scratch commit gate, then local T/Q are copied
to the owned skeleton and source dirty flags cleared. Tick/sample flags are true
only for this explicit evidence. Matrix rebuild, external update, physics, live
player/world state, native numeric parity and the displayed camera remain false.

```powershell
.\target\release\goldeneye.exe mission-context '.\GoldenEye 007 (USA) (En,Fr).rvz' --initialize-animation-layers --evaluate-prewarm-pose --output local/tutorial-prewarm.json
```

Remaining work includes live world activation, actual player/camera
setup, mission scripts and live gameplay. All tracked
parity remains unverified until an original-runtime comparator proves it.

`--evaluate-cached-camera-bone` requires prewarm, binds tag `0e000028` to the
retained bone 47 and constructs a fresh 48-slot FP actor-bone cache. Its first
request builds the three-bone chain parent-first; root local is stored directly,
then two parent-cache products build the requested transform. The separate parent
output is served from cache. This is the actual source cache-query behavior,
without claiming full native FP/TP global setup or its intervening history.

`--evaluate-uncached-camera-bone` requires the same committed pose and explicitly
selects the source's different uncached inspection branch. It preserves the
ancestor identity products and performs three products. It conflicts with the
cached flag. `camera_bone_transform` stays null unless one is requested. Matrices
and the cache stay private; only provenance/counts/fingerprints are exported.
Both paths retain unverified numeric provenance and leave owner/world, cached
hierarchy refresh, full camera, native parity and displayed-frame flags false.
