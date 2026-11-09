# Initialized static mesh contacts

`collision-box` connects initialized static map collection to the reconstructed
type-three box/triangle contact path. It reads user-supplied assets and accepts
an authored query frame, boxes, filter and capacities for one explicit zone.
It does not run original game code or establish live world membership.
[World contact requests](world-contact-requests.md) compose this consumer across
BSP/portal-selected zones and bind the typed first grenade ascent. A separate
[world line adapter](world-line-queries.md) now queries initialized static line
geometry; actor integration, a playable level and full grenade state remain incomplete.

The asset adapter can address collected mesh bodies that are not root entity
directory entries. Resident geometry is shared, while each collected record is
queried in order; repeated records can produce repeated contacts. Source
membership flags are retained rather than enabled to obtain a convenient result.
Read [initialized collection](initialized-collection.md) for hierarchy refresh,
filtering and geometry recursion before this stage.

## Run an authored snapshot

Save this JSON as `local/box-query.json`:

```json
{
  "world_origin": [-252, 7, -8],
  "radius": 8,
  "world_from_query": [1,0,0,0, 0,1,0,0, 0,0,1,0, -252,7,-8,1],
  "boxes": [
    {
      "center": [0, 0, 0],
      "axes": [[1, 0, 0], [0, 1, 0]],
      "half_extents": [0.25, 0.25, 0.25]
    }
  ],
  "filter_slot": 0,
  "filter_flags": 0,
  "record_capacity": 4096,
  "contact_capacity": 128
}
```

```powershell
.\target\release\goldeneye.exe collision-box '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --zone 0 --snapshot local/box-query.json --output local/box-contacts.json
```

The example is an authored probe, with no promised contact count. The matrix is
column-major and maps query coordinates into world coordinates. Box centers and
stored axes are in that query frame. `world_origin` and `radius` separately drive
collection and mesh pruning; they are not inferred from the boxes. The explicit
zone bypasses automatic loaded-world/BSP/portal selection for this diagnostic.

All snapshot fields are required. The snapshot and each box reject unknown
fields; JSON flags are numeric words, unlike hexadecimal CLI flag arguments on
`collision-collect`. Query/frame/box values must be finite and half extents
nonnegative. Snapshots are bounded to 1 MiB, at most 256 boxes, and at most
1,000,000 records or contacts. Capacities are authored host limits, not inferred
original allocation sizes. Existing output files are preserved; outputs remain
local game-derived evidence and are excluded from distributable packages.

## Frames and preparation order

Static SJBE52 revision-zero evidence follows `802ceb00` into mesh dispatcher
`80296160`. The request matrix **Q** always uses the rigid transpose/translation
inverse helper, independently of a collected record's general-inverse flag.
The primitive triangle transform is **rigid_inverse(Q) × CURRENT M**, where M is
the current collected record's matrix. Concatenation rounds its first product,
then fuses Y, Z and W in order. Passing vertices transform with W=1 before rounded
triangle edges are formed. Boxes remain in their original query frame.

The [consecutive record cache](collision-frames.md) separately supplies the mesh
inverse used for pruning origin binding. The current record's translation is
subtracted first in f32, followed by that cached inverse with W=0. Equal consecutive
owner/slot keys retain the previous inverse even if the current matrix differs.
Do not replace the current forward matrix with the cached matrix or reuse an
earlier translation. Radius stays unchanged under instance scale.

Each resident mesh prepares/reuses its static batch cache before local-origin
binding and coarse-tree pruning. Collected flag 0x2 bypasses the coarse tree.
Otherwise an absent tree is explicit metadata, while an addressed corrupt or
unsupported tree is an error. Coarse traversal scans spheres in ascending order
and selected children in descending order, retaining closed leaf-margin tests.
The current checked path supports fanout at most sixteen; larger fanout's native
u32/u16 mask asymmetry is explicitly unsupported. Position context keys other than
zero are rejected by this static adapter; dynamic positions are not replaced by
the static pool.

The query-frame product and local-origin-plus/minus-radius cube are prepared
before the reserved-slot capacity gate. Query boxes and triangle data are not
addressed after a failed earlier pruning gate. This order matters for lazy reads,
overflow errors and the capacity behavior below.

## Batch cache and triangle traversal

The mesh's native +0x70 field is a resident absolute runtime cache, not a serialized
relative pointer. Preparation skips on runtime format bit 0x8 before inspecting
an existing cache; that skip does not clear old cache data. An established cache
is reused without rebuilding it from later records. Keep runtime format changes
separate from the checked serialized corner-format reader.

Only batches with `flags & 0x101 == 0` have compact cache entries. Bounds begin at
minimum XYZ `999999` and maximum XYZ `-999999`, use enabled triangles only, and
retain strict endpoint comparisons. The per-strip visited-corner mask shifts even
for disabled triangles and resets for each new strip; it is not global vertex
deduplication. An included empty/disabled batch still has a sentinel entry.
No included batches sets the runtime skip bit. Cache generation has explicit
host work budgets and the observed included-entry limit of 0x2aa.

Exact cached minimum X `999999` rejects a static batch. Other cache tests compare
minimum XYZ against the query cube's maxima, then maximum XYZ against its minima,
with inclusive ties. Excluded batches consume neither a compact cache slot nor
triangle flags. A rejected included batch advances **declared** triangle-count
flag words without parsing its strips. Actual strip traversal consumes actual
counts; the asset reader validates their equality rather than conflating the
two cursor rules.

For an addressed triangle, flag 0x200 rejects before XYZ reads. X, Y and Z extrema
are tested against the same closed cube in order; later axes remain unread after
an earlier rejection. Only passing triangles are transformed into query coordinates
and tested against all query boxes in storage order. Raw consecutive strip
corners are retained, including odd and degenerate triangles. Rendering's odd
winding swap must not be applied to contact input.

## Contact results and capacities

The [primitive kernel](box-triangle-contacts.md) must produce a final contact;
a candidate alone does not append. The result binder stores the primitive's
`point_minus_normal` as witness A and `point` as witness B. Both transform through
Q with input W=1, then their output W lanes become positive zero. Depth remains
in query units without an invented scale correction.

The stored result normal is recomputed from the query-frame triangle's rounded
edges and normalized before transformation through Q with input W=0. It is distinct
from the primitive's selected SAT normal and is not normalized again afterward.
Its full four-lane transformed value is retained. Exact squared length zero,
including underflow, uses the source +X fallback. Odd triangle ordinals within a
strip negate the already transformed normal and swap provenance indices B/C;
the witnesses do not swap.

The reserved-slot mesh gate is unsigned
`count < capacity.wrapping_sub(1)`. Capacity one skips triangle work at count zero.
Capacity zero passes that initial gate but cannot append a successful contact.
Within an admitted mesh, multiple boxes can fill the reserved final slot. A later
primitive may still be queried; if it reports a contact when the buffer is full,
the current mesh ends without appending. Later collected records continue their
frame/cache/coarse preparation. Host temporary primitive outputs avoid speculative
out-of-range writes while preserving the query/check order. Record collection's
separate capacity does not use this contact-slot rule.

Result provenance uses resource-local mesh/corner offsets and host one-based
collected-record keys, not original process pointers. It retains raw triangle
flags and material, position indices, and the query-shape index's low byte.
These identities support local diagnostics; they do not prove live residency.

## Remaining integration and evidence

Authored tests exercise the static asset-to-contact path, frame/corner binding,
cache reuse, pruning laziness, odd-strip provenance and separate capacity gates.
Independent f64 geometry checks and asset-backed snapshots remain distinct from
original-runtime comparisons. Host reciprocal/reciprocal-sqrt seeds, Wii FP state
and exceptional behavior remain provisional or unverified.

Initialized authored world-manager/BSP/portal binding and first-ascent requests
now use this consumer. Live loaded-world membership, mutable hierarchy/bounds/cache
invalidation, dynamic/skinned positions, actor line integration, full grenade
clearance/state integration, physics response and body activation remain open.
An empty or saturated authored result must not be promoted to verified gameplay
clearance or 1:1 parity. No acceptance criterion is promoted by this operation.
