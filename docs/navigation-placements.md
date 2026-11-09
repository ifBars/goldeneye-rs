# Static map navigation bindings

`Edb::map_navigation` reconstructs the initial navigation registration list for one
selected map. `nav-request --map` uses those bindings, sharing decoded meshes while
retaining individual instance frames. This replaces authored translations for this
static single-map scope. Loaded-map precedence, later transform/entity mutations,
actor scheduling and original execution remain unverified.

## Static evidence

The original artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Raw instructions and private game-derived probes stay under ignored `local/`.
No original program was executed, linked or packaged. Rust implementations and
fixtures are independently authored.

| Observation | Original location |
| --- | --- |
| Manager registration, runtime array order / 0x9c stride | `0x80187c00`–`0x80187f50` |
| Source entity reference / instance key | runtime fields `+0x80` / `+0x8c`, used at `0x80187c8c` / `0x80187d34` |
| Raw placement to runtime instance | `0x80316768`–`0x80316874`: raw map `+0x48` count, `+0x4c` pointer, source stride `0x3c` |
| Translation inserted after rotation/scale work | `0x802f3254`, `0x802f32a4`–`0x802f32b0`, `0x802f3350`: raw position `+4/+8/+12` becomes runtime `+0x30` |
| Source entity reference and key copied | `0x80316828`–`0x8031683c`: raw `+0x30` / `+0` to runtime `+0x80` / `+0x8c` |
| Native directory-reference lookup | `0x802e2a00`–`0x802e2bf4` |
| Wrapper lazy binding / frame capture | `0x801857d0`–`0x80185930`, position copy `0x80185844`–`0x8018585c` |
| Global loaded-map search | `0x8032aee0`–`0x8032af64` |
| Per-map instance-key lookup | `0x803187f0`–`0x80318860` |

The runtime construction iterates all raw placement records. Registration checks
the referenced entity's navmesh type; no engine-flags/map-on filter is applied in
the traced loop. Rotation/scale affect other instance fields, but the navigation
wrapper captures translation only. The reader therefore does not depend on the
initial placement matrix construction or its later live changes.

## Entity references and instance keys

Native entity reference resolution differs from the CLI's explicit hash selector:

- If the top byte is zero **or bit 31 is set**, the low 24 bits are a directory
  index, checked against the original signed count. Do not compare the descriptor
  hash instead. Other category bits do not change that index operation.
- Otherwise, use the directory's signed hash-size field. Zero means no searchable
  hash entries; a positive size searches hashes at the record stride, and a
  negative size searches a contiguous u32 prefix immediately before the rows.
- Up to three entries use ordered linear search. Larger lists use the observed
  binary midpoint/update rules. Duplicate keys keep that returned index, which
  need not be the first duplicate. Do not sort data or add a fallback linear scan.

The bounded reader rejects malformed hash ranges/counts. Existing public
`entity_graph(hash)` still requires a unique matching descriptor hash; native
reference lookup is separate and currently used for navigation registration.

Instance keys have a different rule: top byte zero means a placement index;
otherwise scan instance hashes in order and use the first match. Bit 31 does not
switch an instance key to encoded entity-index mode. Duplicate instance keys do
not remove registrations. Multiple registered wrappers can resolve to the same
first placement, including its translation and entity reference.

For each source placement backed by a `0x607` entity, preserve registration order
and resolve its instance key. Ready bindings retain source placement, resolved
placement, entity directory index and raw translation. Missing keys, missing
resolved entities and wrong resolved types remain explicit unavailable bindings;
they do not receive zero frames or alternate meshes. Unresolved source entity
references are counted separately, matching registration's skip behavior.

The native global key search checks loaded maps in order. This reader deliberately
handles one selected map's initial records. Cross-map collisions, runtime changes,
cached-frame lifetime and per-wrapper face-flag groups still require further work.
Registration calls `0x80183250` to populate a separate boundary structure after
capturing the frame. The app prepares its initial immutable cache; see
[boundary rules](navigation-boundaries.md) for raw coordinates and remaining scope.
The asset binding reader does not build that cache.
Only tag `0x607` is supported here; live RTTI/runtime parity is unverified.

## Native diagnostic command

```powershell
.\target\release\goldeneye.exe nav-request '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --start 0 0 0 --goal 1 0 1 --flags-raw 300 --steps 2 --output local/map-navigation-request.json
```

Coordinates are illustrative, not recovered actor spawns. `--map` and
`--translation` are mutually exclusive; exactly one is required. The latter keeps
the existing authored root-entity mode. Static-map mode prepares only ready
registrations, shares meshes by directory index and gives each candidate its own
frame. Deferred queries use the selected candidate's frame.

Output's `map_navigation.registrations` includes unavailable entries.
`candidates` contains usable entries in the same relative order, with their
`manager_slot` pointing back to the registration list. The cached `binding` is a
host candidate index, not an original pointer or necessarily a manager-slot index.
`resident_meshes` reports unique decoded mesh data for that command. Corrupt ready
meshes fail; there is no fallback to an authored context. Existing output files
are preserved. Detailed placements/routes remain local and outside packages.

## Validation and remaining scope

The local metadata audit decoded **366 map directories**, checking **21,059
placement records**. All **68 navigation registrations** were ready, with zero
unavailable bindings and zero unresolved source entity references. Six translations
were nonzero. The sum of per-map unique mesh slots was 68; it does not mean 68
meshes are simultaneously resident in the original game.

Six sampled requests using the translated placements produced routes, including
both independent candidate frames in `mt_ai.edb`. Samples use asset centers as
requested positions, not recorded actor inputs. They check selected-frame use and
returned waypoint endpoints, not original-runtime parity. The dashboard publishes
counts only, with source-bound freshness.

Authored tests cover encoded references that differ from descriptor hashes,
positive/prefix hash tables, midpoint and linear duplicate selection, numeric and
duplicate instance keys, unavailable bindings, malformed data, map ambiguity,
translation-only reads, shared resident data, selecting the second translated
instance and corrupt meshes failing without a fallback context.

Next trace loaded-map precedence and mutable/cached instance state, actor request
scheduling/gate setters, flag groups, route mutation, goal policies and steering.
This stage does not instantiate actors, move them or verify full gameplay parity.
