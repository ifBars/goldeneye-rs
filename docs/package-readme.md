# GoldenEye Rust · startup and session checkpoint

This early native Windows build plays startup screens and opens an initial main-menu prototype for the
independent reconstruction of GoldenEye 007 for Wii (2010). Supply your own
SJBE52 revision-zero game image. No game data is included.

Run from PowerShell or a terminal:

```powershell
.\goldeneye.exe play 'D:\your-games\GoldenEye.rvz'
```

Alternatively, place exactly one RVZ or ISO beside the executable or in the
current directory and launch `goldeneye.exe` without arguments. Default startup
shows the strap and Zapper warnings, Activision/Eurocom/MGM movies with audio,
the legal screen and title. Confirm the held title to play its outro and enter
the adapted menu.
Up/Down selects, Enter opens, and Esc returns; mouse movement/clicks also select and open rows.
Continue Game is disabled without a save.

New Game offers Operative, Agent, 007 and 007 Classic. Selecting one admits a
local single-player session and retains Tutorial mission data in this window.
The current checkpoint stops at **Preparing player**. Its live StartPoint selection
and first player constructor/reset prefix now run on the session worker.
The fresh Tutorial path owns its trigger catalog, map initialization and base
publication, audio resource registration and loader readiness delivery. Later
map sections, player creation, the gameplay camera and gameplay remain incomplete.
Esc/Back cancels and returns
to the menu. Loading runs off the window thread, menu music is suspended, and
the accepted effect can finish. No game assets or original code are included.

Options opens Control, Screen and Sound Settings. Left/Right (or the on-screen
arrows) changes volume and brightness; R restores the current settings page's
defaults. Music and sound gains update immediately. Dialog volume is retained
for future speech playback. Windows preferences save automatically under
`%LOCALAPPDATA%\GoldenEyeRewrite\frontend-settings.json`; use `--settings-file`
to select another file. These preferences are separate from Wii saves.
Extras opens Credits, which scrolls text/font assets from your image, and Cheat
Codes, whose gameplay effects remain unavailable. Controller configuration and
gameplay actions still show an explicit unavailable notice.

Warnings accept input after 1.5 seconds or time out at 21 seconds, then remain
visible through a separate 0.5-second fade to black. Legal entry uses a global
0.5-second fade from black, rather than an alpha animation in its asset. The fade
skips advancing on its first UI update, truncates midpoint alpha to 127 and clears
at completion before boot advances. The warning clock rounds in f32 on each
update. After the legal entry fade, a worker loads and retains the supported
`ResourceAction` tables; legal exit waits for that dependency. Its 301/30-second
minimum remains a visual-script-duration host fallback. Other original
initializers and resource/menu lifecycle remain incomplete, so the native loading
lifetime is unverified. No legal exit fade or audio-volume fade is inferred.

The prototype loops the approximately 33-second frontend THP background and uses
your image's font/texture assets. Startup logo PCM and the original title/menu
music and nonlooping title-intro cue are decoded and submitted to native WASAPI;
`--mute` disables audio explicitly. Source navigation, acceptance, back and adjustment
cues use cached local PCM. Main-menu layout and brightness application remain host
adaptations. Profile/save flow, gameplay and remaining submenus remain incomplete;
original mixing, loop-history restoration
and menu/input/audio synchronization parity are unverified.

The title overlay uses source mesh geometry/UVs and 13 scripts with positive-count
pose/alpha tracks and independent glint color-frame animation. Its source intro
plays frames 0–29 at 60 fps and holds; confirmation starts frames 30–49, with
completion strictly beyond the inclusive endpoint. Projection combines the source
UI root `(0, -10, 0)` with the camera `(0, 18, 0)`, retains the engine's
approximate-pi field-of-view conversion and uses actual per-vertex depth.
`--title-aspect wide` is the host default for its 16:9 setting;
`--title-aspect standard` selects 4:3. The original user's display flag is unknown.
The existing 640×480 capture canvas, letterboxing, manually positioned prompt
and main-menu layout remain host adaptations.

Title entities sort by original stored bounds with grouped batches, repeated f32
bias and reversed exact negative-key ties. Mesh UVs/colors interpolate with
perspective; materials use clamped 2× RGB modulation with singly modulated alpha.
Native GPU depth/filter behavior, SDK tangent/Wii numerical precision, complete
material behavior, general script VM and remaining external audio/control cues
remain unverified or incomplete. The supported root script plays its intro cue
once at frame 11 with source gain 1.0; confirmation and seeks do not replay it.
Its separate host voice finishes naturally through the held title/outro and
mixes alongside music. Resampling, summation and output limiting are host
adaptations; this does not establish Wii mixer, display, visual or input parity.

Use `--skip-startup` for the direct-menu shortcut. `--start-at` accepts `strap`,
`zapper`, `activision`, `eurocom`, `mgm`, `legal`, `title`, `main-menu`, `strap-fade`
or `zapper-fade` only with bounded `--frames` developer runs, for example:

```powershell
.\goldeneye.exe play 'D:\your-games\GoldenEye.rvz' --start-at title --frames 45 --mute
.\goldeneye.exe play 'D:\your-games\GoldenEye.rvz' --start-at strap-fade --frames 8 --hidden --mute
```

These entries seek directly to a stage and reset its clock and transition
diagnostics. The fade entries start an accepted warning fade; they do not claim
the skipped startup screens were played. Playback phase metadata can distinguish
first, middle and completed fades without automating OS input.

The developer flag `--smoke-confirm-title` requires `--start-at title --frames`.
It calls the same app confirmation handler as keyboard/mouse once the intro
finishes, without automating OS input:

```powershell
.\goldeneye.exe play 'D:\your-games\GoldenEye.rvz' --start-at title --frames 240 --smoke-confirm-title --hidden --mute
.\goldeneye.exe frontend-title-audit 'D:\your-games\GoldenEye.rvz' --output local/title-audit.json
```

The audit runs a deterministic 40-second source-title tick and outro check and
writes metadata. It measures per-mesh RGB patterns independently of alpha,
instance order and duplicate mesh population, and rejects independent RGB
tracks in the reachable title scripts. Neither developer probe compares actual
input, original runtime behavior or original rendered pixels.

- Next checkpoint: activate the retained mission data through base animators and
  enabled trigger factories, then initialize the original player/camera path and
  connect New Game to a running tutorial. Keep the accepted startup presentation
  intact; original behavior comparisons remain open.

The asset inspection and static runtime diagnostics remain available separately.

`goldeneye.exe mission-context <disc> --difficulty 0` retains the checked level
catalog and complete discovered Tutorial resource chain, including the graphics
resource. It prepares registration order/filters from a declared preference
snapshot; original initialization history remains unverified.
Difficulties 0 through 3 match the source choices; `--level` selects another catalog key.
The command binds actual map spatial/BSP data and placements with resolved
entity indices/types, then prepares source StartPoint selection. Real map
preregistration slots start at 1 after null slot 0; loaded ordinals start at 0.
`--start-point-id` defaults to explicit inspection input 1, proven for Tutorial
New Game. Selection falls back to ID 0 when appropriate; no selection yields no
pose. Original pose bits remain privately retained and only their hash is emitted.
This command emits metadata only. Base animators, trigger factories, live catalog
membership and player/camera creation remain incomplete. `world_registered` and
`player_initialized` remain false, and New Game does not yet launch a playable
mission.

The retained context also selects the supported level character row and keeps
its animation component and separate first-person skin. It constructs CPU bone
membership, source range masks and identity constructor locals/matrices, then
resolves the camera tag against that skin. Missing native character-default
fallback is unsupported. Rig arrays and transforms remain private; receipts
contain metadata and hashes. Constructor identity is not an evaluated animation
frame. Third-person components, physics, live animation layers and camera output
remain incomplete, so this does not initialize a playable player or camera.

Supply `--animation-request`, `--animation-previous` and `--animation-fallback`
together to retain an explicit mode/transition and layer-request plan. Previous
`0` is a coded directory index, not an absent mode; with no inputs, no initial
mode is invented. The supported reached commands are StartSet and MaskOperation;
other opcodes and nonzero sections are unsupported. Plans retain raw source
requests and derived flags/divisor/cleanup intents privately and emit counts plus
a hash. They do not create runtime animation children, sample a pose or produce
a camera frame.

Alternatively, initialize the supported owned animation callback snapshot:

```powershell
.\goldeneye.exe mission-context 'D:\your-games\GoldenEye.rvz' --initialize-animation-layers --output 'D:\your-assets\tutorial-layers.json'
```

`--initialize-animation-layers` conflicts with manual mode-reference flags.
Without it, `initial_layers` remains unset. The optional path owns default
inventory/weapon remapping, control capture and class04 children with shared
clip bindings, then applies and counts both source post-selector head-blend writes.
These affect clip selection only. Source virtual18 assigns a supplied position
conversion; it does not accumulate an elapsed clock.
A typed reader verifies the supplied SJBE52 revision-0 DOL profile
and retains only DATA section 5; it exposes no executable bytes, executes no
original instructions and embeds no original table corpus. General asset reads
still reject executable paths. Public output contains safe counts, identifiers
and digests.

Body-300 PlayAnimation records and body-102 skeletal headers/compressed spans
are bound source data. Animation descriptor +0x1c is an optional same-wrapper
skin reference; the fresh `0xffffffff` value uses the skeleton's secondary skin. Raw
model metadata does not force another skin.

Optionally add `--decode-initial-frame`, which requires the layer initializer:

```powershell
.\goldeneye.exe mission-context 'D:\your-games\GoldenEye.rvz' --initialize-animation-layers --decode-initial-frame --output 'D:\your-assets\tutorial-raw-frame.json'
```

This selects the actual owned slot-zero child's clip/frame and caches raw
polynomial vectors privately. `initial_layers.raw_frame` stays null without it.
Public counts distinguish exact-branch rotations from unverified host-square-root
rotations; exact branch describes evaluated XYZ, not native numeric parity.
Root adjustment, skin mapping, control/layer blending, skeleton commit, animation
tick, player initialization and camera output remain incomplete. A raw camera-tag
vector fingerprint is not a camera frame or the first displayed game pose.


`collision-candidates` refreshes and queries one initial static zone hierarchy.
`collision-collect` additionally collects geometry records through the confirmed
bounds/filter/recursive-dispatch path. Filter flags are hexadecimal; capacity
defaults to an authored host limit of 4,096. These records are not contacts or
live clearance, and membership mutation/reference-game parity remain unverified.
`collision-box` adds static mesh pruning and type-three box contacts for an
authored JSON snapshot. It preserves initialized membership, resident cache reuse
and contact-capacity behavior; it does not supply live gameplay clearance.
`collision-world-box` selects one resource from an authored ordered world list,
then binds BSP/portal-selected zones into a shared record, inverse-cache and
contact sequence. `grenade-ascent-box` prepares the source first-ascent request
and reports a contact-count `blocked` result. `collision-world-line` separately
queries initialized static geometry across segment-selected BSP zones, with
nearest or early-stop results. `grenade-world-line` prepares the typed cached-apex
request and reports world-only blocked state. Actor line integration,
full grenade state progression and live physics remain incomplete.

Additional commands:

```powershell
.\goldeneye.exe --help
.\goldeneye.exe collision-world-box 'D:\your-games\GoldenEye.rvz' --snapshot 'D:\your-assets\world-box.json' --output 'D:\your-assets\world-contacts.json'
.\goldeneye.exe collision-world-line 'D:\your-games\GoldenEye.rvz' --snapshot 'D:\your-assets\world-line.json' --output 'D:\your-assets\world-line-result.json'
.\goldeneye.exe grenade-world-line 'D:\your-games\GoldenEye.rvz' --snapshot 'D:\your-assets\grenade-world-line.json' --output 'D:\your-assets\grenade-world-line-result.json'
.\goldeneye.exe grenade-ascent-box 'D:\your-games\GoldenEye.rvz' --snapshot 'D:\your-assets\grenade-ascent.json' --output 'D:\your-assets\grenade-ascent-result.json'
.\goldeneye.exe collision-box 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb --map 85000000 --zone 0 --snapshot 'D:\your-assets\box-query.json' --output 'D:\your-assets\box-contacts.json'
.\goldeneye.exe collision-candidates 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb --map 85000000 --zone 0 --origin -252 7 -8 --radius 8 --output 'D:\your-assets\candidates.json'
.\goldeneye.exe collision-collect 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb --map 85000000 --zone 0 --origin -252 7 -8 --radius 8 --filter-slot 0 --filter-flags 0 --output 'D:\your-assets\collected-records.json'
.\goldeneye.exe inspect 'D:\your-games\GoldenEye.rvz'
.\goldeneye.exe archive 'D:\your-games\GoldenEye.rvz' Filelist.bin
.\goldeneye.exe probe 'D:\your-games\GoldenEye.rvz' Filelist.000 --offset 2048
.\goldeneye.exe edb 'D:\your-games\GoldenEye.rvz' fe_boot.edb
.\goldeneye.exe texture 'D:\your-games\GoldenEye.rvz' o_bondfont.edb 86000001 'D:\your-assets\buttons.png'
.\goldeneye.exe scan-edb 'D:\your-games\GoldenEye.rvz' --decode-textures --output 'D:\your-assets\audit.json'
.\goldeneye.exe scene 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb
.\goldeneye.exe entity-graph 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --output 'D:\your-assets\dam-graph.json'
.\goldeneye.exe nav-ray 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --origin 0 5 0 --direction 0 -1 0 --max-t 10
.\goldeneye.exe nav-ray 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --origin 0 5 0 --direction 0 -1 0 --max-t 10 --selection nearest
.\goldeneye.exe nav-segment 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --triangle 0 --origin 0 5 0 --displacement 0 -10 0
.\goldeneye.exe nav-connected-segment 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --start-triangle 0 --depth 2 --origin 0 5 0 --displacement 0 -10 0
.\goldeneye.exe nav-path 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --start-triangle 0 --goal-triangle 100
.\goldeneye.exe nav-world-path 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 82000018 --translation 0 0 0 --start 0 0 0 --goal 1 0 1
.\goldeneye.exe render-map 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 'D:\your-assets\dam.png' --eye -252 7 -8 --target -270 3 -27
.\goldeneye.exe view-map 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb --placements --eye -252 7 -8 --target -270 3 -27
.\goldeneye.exe render-map 'D:\your-games\GoldenEye.rvz' m_dam_gfx.edb 'D:\your-assets\dam-placements.png' --placements --eye -252 7 -8 --target -270 3 -27
```

For `collision-box`, create the authored snapshot file before running the command:

```json
{
  "world_origin": [-252, 7, -8],
  "radius": 8,
  "world_from_query": [1,0,0,0, 0,1,0,0, 0,0,1,0, -252,7,-8,1],
  "boxes": [{
    "center": [0, 0, 0],
    "axes": [[1, 0, 0], [0, 1, 0]],
    "half_extents": [0.25, 0.25, 0.25]
  }],
  "filter_slot": 0,
  "filter_flags": 0,
  "record_capacity": 4096,
  "contact_capacity": 128
}
```

These are illustrative query values, with no promised contact count. The
column-major matrix maps query coordinates to world coordinates; boxes are in
the query frame. World origin/radius independently drive collection and pruning.
All fields are required, unknown fields are rejected, and JSON filter flags are
numeric. Inputs must be finite and half extents nonnegative. The snapshot limit
is 1 MiB and 256 boxes; each capacity is at most 1,000,000. An empty or saturated
result is not proof of live clearance. Capacity one skips mesh triangle work;
capacity zero can still query primitives but appends no successful contacts.
Coarse fanout above sixteen and nonzero position contexts are unsupported.
Result mesh/corner offsets are resource-local and collected-record keys are
host identities. Live world/actor-manager binding, mutable geometry, physics
response, full grenade clearance/state integration and original Wii numeric/runtime
parity remain open.

For `collision-world-box`, save this complete snapshot as
`D:\your-assets\world-box.json` before running its command:

```json
{
  "worlds": [{
    "resource": "m_dam_gfx.edb",
    "map": 2231369728,
    "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
  }],
  "request_flags_raw": 2,
  "query": {
    "world_origin": [-252,7,-8],
    "radius": 16,
    "world_from_query": [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1],
    "boxes": [{ "center": [-252,7,-8], "axes": [[1,0,0],[0,1,0]], "half_extents": [8,8,8] }],
    "filter_slot": 0,
    "filter_flags": 0,
    "record_capacity": 4096,
    "contact_capacity": 1024
  }
}
```

For `grenade-ascent-box`, save this complete snapshot as
`D:\your-assets\grenade-ascent.json`:

```json
{
  "worlds": [{
    "resource": "m_dam_gfx.edb",
    "map": 2231369728,
    "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
  }],
  "start": [-252,7,-8],
  "target": [-242,7,-8],
  "angle_index": 0,
  "small_lift_raw": 1
}
```

The world list and bounds are illustrative authored inputs, not recovered live
membership, and no contact count is promised. JSON map and flag values are
numeric; map `2231369728` is hexadecimal `85000000`. Fields are required, unknown
fields are rejected, and the snapshot size limit is 1 MiB. Only the selected
world's resource opens. One world reads no bounds; multiple worlds test indices
one onward for the first containing match and fall back to world zero.

Request flag 2 expands only the initial BSP zone's ordered portal groups,
preserving duplicate zones. Without it, portal inputs stay unread. Expanded mode
validates the initial zone's row span/group metadata, then evaluates shapes lazily
through each group's first hit. Malformed unused row metadata can still fail the
bounded host reader; exact raw-row load laziness is not claimed.
Each zone collects geometry before placements. Candidate ordinals restart per
zone, but the complete sequence shares one record capacity, inverse cache,
resident mesh cache and contact buffer. Capacity diagnostics stay visible.

The first-ascent command uses the original start/apex-distance selection values,
identity query frame, filter slot 1/flags 3, portal flag 2 and source record/contact
capacities 64/512. These differ from the authored capacities in the world-box
example and resource audits. Ordinary arc rejection performs no world query.
An admitted ascent's `blocked` result follows contact count only; a miss does
not establish readiness to launch, subsequent grenade world/actor clearance or full
state progression. No physics body is spawned or activated.

For `collision-world-line`, save this snapshot as `D:\your-assets\world-line.json`:

```json
{
  "worlds": [
    {
      "resource": "m_dam_gfx.edb",
      "map": 2231369728,
      "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
    }
  ],
  "line": {
    "origin": [-252,7,-8],
    "delta": [0,-10,0],
    "constructor_delta": null,
    "maximum_parameter": 1,
    "filter_slot": 0,
    "filter_flags": 0,
    "required_any": 0,
    "excluded_any": 0,
    "stop_first_raw": 0
  }
}
```

The source documentation's `docs/world-line-queries.md` explains the full contract.
The last equal-parameter hit wins in nearest mode. Every nonzero stop-first byte
retains the first hit and leaves `world_normal` unpopulated. Optional constructor
displacement preserves broad bounds before a caller changes the actual segment.
Line mode-two caches and all-included-triangle planes are distinct from box
contacts; malformed consumed disabled-triangle XYZ remains an error. A static
world miss does not establish actor clearance or full grenade state acceptance.

For `grenade-world-line`, save this typed-request snapshot as
`D:\your-assets\grenade-world-line.json`:

```json
{
  "worlds": [
    {
      "resource": "m_dam_gfx.edb",
      "map": 2231369728,
      "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
    }
  ],
  "cached_apex": [-252,8,-8],
  "target": [-242,7,-8],
  "small_lift_raw": 1
}
```

The typed constructor preserves target-minus-apex broad bounds before adding
the Y lift (`0.1` for any nonzero byte, `1` for zero). It uses filter slot one,
flags zero, maximum one and zero required/excluded/stop fields. `blocked` describes
only the static world query. Actors, full action-state acceptance and body
activation remain separate; a false value does not authorize launching.

The locally tested source is SJBE52 revision 0 in RVZ format. Other editions and
formats are unverified. EDB v262/v263 metadata and local GX/TPL texture frames/mips
are readable. `texture` resolves external links with cycle checks and exports a
logical-size PNG. `scan-edb` audits local images; it reports external references
separately. Unknown resource bodies, gameplay, and reference parity are unverified.
Reads never execute game code.
`render-map` requires a native graphics driver. The tested host uses Vulkan on
an RTX 3060 Ti. It renders static zones with an unlit inspection shader. Optional
`--placements` requires complete decoding. All 241 Dam records decode: 240 render
instances and one separate navigation mesh (0x607), identified through static
factory/type metadata. `--allow-incomplete` remains available for other unsupported
types and never hides corruption. Original rotation/visibility, actors, skies, lighting/TEV and exact
material states remain pending.
`entity-graph` separately reads 0x607 navigation mesh arrays; original path/traversal
behavior remains unverified. Use `--index N` instead of a hash for
duplicate-hash entries. `scan-edb --decode-graphs` audits all root graph entries.

`nav-ray` defaults to the first accepted storage-order triangle, strictly between
zero and `--max-t`. Explicit `--selection nearest` instead returns the least
nonnegative parameter below the maximum; ties keep the first storage-order face.
Directions are not
normalized; `--max-t` bounds the ray parameter. Optional `--group N` selects a
group; invalid IDs fall back to the full mesh. The example is an illustrative ray,
not an original spawn. `nav-segment` tests only the selected `--triangle N`, using
origin plus the full displacement vector, and includes both endpoints.
`nav-connected-segment` tests the current face, then connected faces depth-first
in neighbor-slot order. The default policy prevents immediate backtracking;
longer cycles are bounded by `--depth`. `--face-limit` is an independent host
guard whose exhaustion is an error. It returns a surface hit, not a movement path.
`scan-nav` runs sampled geometric smoke probes for all four operations. These
commands do not verify original execution, Wii estimate precision, pathfinding or physics.
`nav-path` selects a triangle chain using reconstructed default costs and queue
rules, including equal-cost record replacement and reopening. It is not a movement
path or shortest-path guarantee. `--max-expansions` is a host guard; exhaustion,
dangling pool parents and self-edge expansion report errors. Original heap reuse,
Wii precision and actor overrides remain unverified. `scan-paths` runs sampled
topology/cost checks with no original-runtime comparison.
`nav-world-path` requires an explicit translation frame and world start/goal
positions. It raises each endpoint by one unit, subtracts translation, and casts
downward using nearest selection with `0<=t<3`. Missing endpoints skip path search;
a completed search can separately find no route. It does not infer placement
rotation/scale or snap actors. The example frame and
coordinates are illustrative. The audit includes authored-frame world requests,
independent geometry/reachability checks and explicitly reported endpoint misses.
Add `--waypoints` to construct the observed centroid-seeded greedy point list,
including local and translated world points. It preserves the original unraised
start/goal, interior shortcut flags, portal rules and half-unit vertex-fan check.
This does not move actors or prove collision-free clearance. Optional positive
`--max-shortcut-tests` / `--max-fan-steps` require waypoint mode (defaults 100,000 /
1,000,000); exhaustion is an error. Missing endpoints/no-route produce no waypoints.
Waypoint output also includes the next world target and ordered 3D route length.
Add `--probe-position X Y Z --corridor-radius R` in waypoint mode to check arrival
and corridor proximity at an explicit world position. `--corridor-segments N`
limits the scan (zero scans none). Arrival requires exactly two points and strict
horizontal quarter-unit proximity. These queries do not advance or move actors;
original numeric/runtime parity remains unverified. Missing routes have no probe result.
`nav-request <disc> <resource> --translation X Y Z --start X Y Z --goal X Y Z
--flags-raw 300 --steps 2` probes acquisition followed by a deferred route query.
Flags are hexadecimal; steps are bounded dispatch calls (1..=16), not game ticks.
Its candidate list contains root navmeshes with the explicit shared frame, not
recovered manager instances. Stored-AABB misses and processed requests without
routes remain visible. This does not steer or move actors.
Use `--map HASH` instead of `--translation X Y Z` to recover static placed-navmesh
bindings for one selected map. Each ready candidate uses its own initial translation;
meshes are shared, and unavailable registrations remain in the diagnostic output.
Loaded-map precedence, dynamic changes and original execution remain unverified.
Optional `--boundary-check` checks the acquired binding's initial ordered edge
cache. It compares raw input XZ against local vertices, with no frame conversion
or height test; disjoint collinear segments count as intersections. An unbound
request has no result. This diagnostic is separate from route success, physical
clearance and the actor caller's full gates.
`--actor-boundary-band 4.5 --actor-mode 0` separately applies one observed actor
range predicate after dispatch. Other bands are `3.4`, `3.45` and `3.6`. Both
options must be supplied together. Start/goal are repeated authored position
snapshots; this does not load actor records or schedule actions. The diagnostic
reports mode/range/height/missing-binding rejections and boundary acceptance,
preserving strict band/height limits and second position resolution.
Route state persists across dispatch calls. Ordinary query misses retain the old
triangle/point lists with a separate status; success replaces them. Repeated
`--next-start X Y Z` supplies one authored position per later call: exactly
`steps-1` triples are required when used. Otherwise the initial start repeats.
These are external samples, not movement simulation. Boundary diagnostics use
the final supplied position. Output includes each dispatch's retained route state.
`nav-goal-path <disc> <resource> <entity-hash> --start-triangle N
--accept-triangle M --heuristic-point X Y Z` probes the alternate mesh-local
selector. Repeat `--accept-triangle` to admit multiple faces. The accepted-face
set and point are authored inputs; goals are tested after queue removal. Its
observed actor heuristic is squared 3D distance, with default edge/cost policies.
Optional `--actor-edges` on either predicate command applies ordered destination
flag, optional stored-normal Y and full-3D target-direction gates using that point.
Missing normals skip the gate. The fixed engine cosine threshold is statically
evaluated; Wii normalization and FP state remain unverified.
It does not reconstruct a full actor goal predicate,
guarantee shortest paths, generate waypoints or move actors. `--index` can replace
the hash; a positive `--max-expansions` bounds work. Existing outputs are preserved.
The same `--accept-triangle` and `--heuristic-point` pair on `nav-request` enables
alternate world routing after acquisition. Only the start is surface-bound. The
selected raw mesh center is frame-subtracted for waypoints, then translation is
added back; no goal ray or prior center translation is applied. The nominal
`--goal` is recorded but does not choose the route endpoint. Completed predicate
exhaustion uses status 10 and retains old lists. These remain authored policies;
full actor behavior, movement and original-runtime parity are unverified.
`nav-actor-path <disc> <resource> --index N --start-triangle M --origin X Y Z
--point X Y Z` combines the observed proximity/face-walk goal, squared heuristic
and actor edge callbacks. Inputs are raw explicit values; no world conversion or
origin lookup is applied. Separate positive `--max-expansions` and `--max-goal-steps`
bound work. No forward crossing reports an unsupported error rather than a miss.
Output includes rejection counts, last scratch center and last walk. Actual actor
inputs, caller coordinate preparation, FP parity and movement remain unverified.
`nav-actor-target <disc> <resource> --index N --owner-translation X Y Z
--instance-translation X Y Z --target X Y Z --active-position X Y Z` reconstructs
caller binding after its separate external action gate. It uses owner-local
storage-first ray 0<t<1.25 and original target height, then instance-local
depth-five connected lookup with manager flag rules before actor policy search.
Snapshots are explicit. Positive `--max-face-tests`, `--max-expansions` and
`--max-goal-steps` bound separate work. No state mutation or movement follows.
`nav-actor-action <disc> <resource> --index N --snapshot local/action.json` adds
the observed angular gate and selected-record update. The authored JSON contains
`owner_translation`, `owner_yaw`, `instance_translation`, and `state` with
`state_raw`, `active_index`, `mode_raw`, `target`, and `records` of `position`/`motion`
vec3 values. Limits/yaw are radians. Snapshots are bounded to 1 MiB. A rejected gate
loads no game data. Gate/miss returns 1 and preserves state; success returns 2,
sets raw state 2 and updates only the selected motion. Positions stay unchanged.
Host numeric estimates remain provisional; zero/overflow motion is an error.
Record population, scheduling, integration/collision and gameplay remain unverified.
The generated image is not a full-level/parity result. Outputs are local
game-derived assets and must remain separate from redistributable files.

`view-map` opens a native asset inspection window. Left-drag orbits,
right/middle-drag pans, wheel zooms, Home resets and Esc closes. Geometry/textures
remain resident across redraws/resizes. These are inspection controls; gameplay
is unimplemented. The automated local check covers hidden presentation, resize
and pixel agreement with offscreen rendering; human interaction remains unverified.

`inspect` and `archive` print JSON. Add `--output your-new-file.json` to save it.
`probe` prints hex by default; `--output your-new-file.bin` saves local asset bytes.
Outputs do not overwrite existing files. Keep image/probe files private and separate
from this tools folder when redistributing the executable.

`SHA256SUMS` lists the exact package contents and checksums. `LICENSE` applies to
our code. Dependency licenses are in `THIRD_PARTY_NOTICES.md`.

For the supported source reset's fixed 0.2-second animation prewarm, use:

```powershell
.\goldeneye.exe mission-context 'D:\your-games\GoldenEye.rvz' --initialize-animation-layers --evaluate-prewarm-pose --output 'D:\your-assets\tutorial-prewarm.json'
```

This conflicts with `--decode-initial-frame`. Its separate `prewarm_pose` receipt
records both child clock updates and 48 committed local bone poses, including
root bias and skin translation fallback processing. Original vectors stay private.
Host quaternion precision is unverified; this provides no external scheduler
step, rebuilt matrices, live player/physics, world activation or gameplay camera.

Add `--evaluate-cached-camera-bone` to that command to evaluate the actual camera
tag from the committed locals through a fresh FP actor-bone cache. It owns 48
slots, builds three transforms and performs two matrix products plus a parent hit.
The alternate `--evaluate-uncached-camera-bone` flag explicitly selects a separate
inspection branch; the flags conflict. Both publish only counts/fingerprints and
keep matrices private. Owner placement, full native cache history, later camera
adjustments, native numeric parity and playable gameplay remain incomplete.
