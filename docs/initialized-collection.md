# Initialized static collision collection

The native executable now exposes two asset-backed operations:

```powershell
.\target\release\goldeneye.exe collision-candidates '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --zone 0 --origin -252 7 -8 --radius 8 --output local/candidates.json
.\target\release\goldeneye.exe collision-collect '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --zone 0 --origin -252 7 -8 --radius 8 --filter-slot 0 --filter-flags 0 --output local/collected-records.json
```

These use an **initial static snapshot**: stored membership flags, serialized
placement matrices, source component bounds and an explicit selected zone. They
do not establish loaded-map membership, later mutations, triangle contacts,
grenade clearance or original-runtime parity. Output stays local and existing
files are preserved. Query coordinates/radius must be finite; negative margins
retain the native shrink behavior. Filter flags are hexadecimal. Capacity is an
authored host limit (default 4,096), not an inferred original allocation size.

## Candidate initialization

`ge_assets::candidates` decodes all reachable hierarchy records needed for initial
refresh, including disabled entries. It retains flags, unknown leaf bytes,
ordered edges, duplicates, shared nodes and cycles. This bounded initialization
reader is distinct from the runtime query's lazy callbacks. Missing hierarchy
metadata is not an empty-world result.

`ge_core::collision::refresh` visits every leaf entry without membership gates.
The adapter resolves the placement's coded entity reference and transforms its
**base** stored header bounds with the initial matrix. An unresolved entity adds
nothing; an addressed unsupported/corrupt body is an error. Each internal child
starts from its existing packed bounds, grows through recursion, writes its new
XYZ bounds before unioning the child result into the parent, and preserves the
caller's four-lane root accumulator. Refresh never manufactures membership flags.
The adapter mutates an owned hierarchy copy, never the EDB/source image.

Refresh loop counts reload in their source order. Query traversal separately
retains parent/recursive header rechecks and live leaf-count reads. Node/entry
budgets stop cycles with errors. A complete initialized snapshot is not proof of
subsequent live membership or cache invalidation.

## Geometry records

`collision-collect` reads only the selected zone reference, without requiring an
unrelated BSP or other zone's bounds. Zone geometry is processed first. The
original selected entity's flag 0x100 precedes component filtering; a replacement
is not gated again. Unknown referenced tags remain raw until an actual geometry
operation requires support. Local links use the sixteen-byte reference directory,
not the twenty-byte entity directory. Missing references skip; addressed external
resource links remain explicit unsupported errors.

Filter flag 0x800 skips placement processing. Otherwise, initialized candidate
selection runs, full component/shape-expanded instance bounds are tested with
the original query origin/radius, then the entity flag/filter gates precede
recursive geometry dispatch. Candidate-list ordinals become record slots even
when earlier candidates were rejected; global placement indices are different.

`ge_core::collision::dispatch` implements the confirmed 0x601 mesh, 0x603 split
and 0x608 reference-link collectors. Sphere centers use rounded-X/fused-Y/Z/W
point evaluation; sphere distance rounds X/Y squares and their sum before fused
Z. Stored radii are unscaled. Base mesh gates distinguish recursion flags from
collection flags; split sphere gates have their own bypass rules. Bounds updates
use their separate unfused arithmetic. Link/split counts reload after each entry
and at termination. Optional contexts, order, duplicates and OR returns remain
explicit. Full buffers reject appends while traversal continues.

The resulting records contain matrices, opaque resource-local geometry identities,
owner/slot keys and flags. They are **not contacts**. A nonempty record list does
not imply a hit, and an exhausted record buffer does not imply clear space.
See [candidate/filter/record rules](collision-collection-records.md),
[bounds](collision-bounds.md) and [frames](collision-frames.md).

## Diagnostics and next work

The resource audit initializes every available zone hierarchy and samples up to
eight zones per map at stored center/minimum/maximum positions with radius one.
Candidate lists are checked by independent f64 traversal. Near-boundary results
are counted separately rather than claimed as agreements. Stored membership
metadata must be unchanged after refresh.

The audit also collects static geometry at up to eight authored zone centers per
map with radius eight, filter slot/flags zero and capacity 4,096. It counts record
and pruning outcomes and prepares each collected record's mesh-local query origin
through the consecutive-key inverse cache. These pipeline probes do not compare
with original execution or generate triangle contacts.

The separate [static mesh contact consumer](mesh-contacts.md), exposed by
`collision-box`, now adds resident batch caches, coarse sphere-tree pruning,
query-frame triangle/primitive traversal and contact-buffer/result binding.
Its `static_mesh_contacts` resource audit has a separate authored snapshot
denominator; the collection-only probes above still do not generate contacts.
[Initialized world requests](world-contact-requests.md) now append each selected
zone into one shared record buffer before the mesh consumer runs. Per-zone
candidate ordinals restart, while the inverse cache and contact buffer span
the combined sequence. Live loaded-world/actor integration and mutable positions remain open.
Wii reciprocal/reciprocal-sqrt seeds remain provisional and require reference tests.

The first local corpus run refreshed 1,993 trees across 366 maps (14,401 reachable
nodes, 12,408 branch writes, 39,791 leaf entries, no missing placement references).
Of 2,259 candidate queries, 2,256 were stable agreements and three were near a
boundary; 9,964 indices were returned, at most 269 in one query. Separate static
collection completed 753 zone-center queries, collected and prepared origins for
2,737 geometry records with 1,498 inverse preparations, and counted 2,648 sphere
and nineteen geometry-bounds rejections. No sample exhausted its 4,096 capacity;
the maximum record count was 61. These are corpus diagnostics, not gameplay proof.
