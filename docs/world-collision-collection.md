# World collision collection and live integration

The mesh-local [box/triangle kernel](box-triangle-contacts.md), pure loaded-world
selector, static BSP point binding and ordered portal expansion are implemented.
An [initialized static collector](initialized-collection.md) and [mesh contact consumer](mesh-contacts.md)
now connect asset-backed records to authored box queries. This page records the
surrounding world contracts. [Initialized world requests](world-contact-requests.md)
now compose the selector with BSP/lazy portal binding and one combined zone
record/cache/contact sequence, including a typed first grenade ascent. Live
manager membership, mutable state, actor line integration and full grenade
state progression remain open. Static snapshots do not establish live gameplay clearance.

## World choice

SJBE52 revision-zero dispatcher `8032e430` reads the loaded-world count at manager
`+0x1c` and pointer array at `+0x20`. With no worlds, it calls no collector. With
one world, it selects index zero without testing its bounds. With multiple worlds,
it scans indices **one onward**, in storage order, and selects the first containing
the query origin (`request+0x0c`). If none contains it, it selects index zero.
Index zero is a fallback, not the first tested candidate. It is not a nearest,
smallest-containing-volume or last-loaded selection.

Containment uses `80293060` on the world's stored bounds at `+0x2c`, with margin
exactly zero (`806f21c8`). Minimum and maximum boundaries are inclusive. It tests
rounded f32 differences, minimum X/Y/Z before maximum X/Y/Z. The grenade caller
supplies the original start as this query origin; do not substitute its shifted,
padded oriented-box center. The selection does not inspect the query radius.
The later geometry collector receives the complete request, including its
separate start/apex-distance selection values.

Only the selected world's virtual slot `+0x50` is called, with an identity parent
matrix and final integer argument zero. The wrapper skips a null selected pointer;
that does not justify silently accepting a missing/corrupt world in a host adapter.
Bounds/objects for later worlds must remain lazy after a prior containing match.
This collision manager rule does not by itself establish navigation-manager
membership or precedence. `ge_core::collision::world::select_loaded_world`
implements this selection over an explicit world count and lazy stored-bounds
callback. World counts above 65,536 are an explicit host budget error. No world
objects or membership are inferred from a list of resource files.

## Static BSP point binding

`Edb::map_spatial` reads one unambiguous map directory entry. Map `+4` points
relatively to a BSP whose nodes have a 32-byte stride: a four-float plane at zero
and signed i16 children at `+0x10/+0x12`. Only reachable nodes are read; the last
twelve bytes of each node and unreachable node payloads stay uninterpreted.
Positive children address another node; nonpositive children encode the negative
zone index, including zero for zone zero and -32768 for zone 32768. Leaves must
fit the map's `+0x84` zone count. Zone records retain entity references and packed
source bounds; no rendered mesh or material is needed.

The reader preserves node sharing and cycles. The separate
`ge_core::collision::world::MapBsp::zone_at` starts at node zero, overwrites the
homogeneous point coordinate with one, and selects child one for a strictly
negative plane projection, otherwise child zero. Equality has no epsilon. Paired
arithmetic fuses Z into rounded X and plane W into rounded Y, then adds the lanes;
do not substitute scalar X+Y+Z+W accumulation. A caller-supplied node-test budget
reports exhaustion as an error. Missing roots, invalid addressed nodes/leaves,
nonfinite used values and overflow are errors, never zone-zero defaults.
Absent BSP pointers remain explicit reader metadata and cannot satisfy a query.

The resource audit samples at most eight zones per map, using each stored AABB's
center, minimum and maximum as authored query positions. A zone AABB's center
need not belong to that same leaf; expected IDs come from an independent f64
plane traversal, not from the sample's source zone. A sample is numerically
ambiguous if any independently visited plane has distance within
`0.000001 + 8*f32_epsilon*sum(abs(plane_terms))` of zero. Ambiguous samples retain
their count without claiming agreement; stable samples must agree. The first
local run decoded all 366 map entries, 9,995 reachable nodes and 1,993 zones, then
ran 2,259 point queries: 1,540 stable agreements and 719 near-plane samples, with
at most 14 nodes visited. Counts describe directory entries, not live worlds.
Wii floating-point status/precision and original execution remain unverified.

## Portal expansion and collector identity

Map animator vtable `80591970` installs collector `80315c10` at slot `+0x50`.
Its name getter `80318c80` resolves `EXItemAnimator_Map`; this is distinct from
the serialized map's factory/type table. Request flag `+0xfc & 2` selects the
expanded-zone helper `802f1fb0`; otherwise the collector takes the single BSP
leaf from `802f33c0`.

Expanded lookup first obtains the same BSP leaf, then considers **only that
initial zone's** grouped portal rows. It does not recursively flood through newly
selected zones. The rows are eight bytes, with the first row's signed target
zone at `+2`, per-row signed portal index at `+4`, and extra-row count at first
row `+6`. The first portal distance strictly below the query radius appends that
target once and ends that group; equality rejects. Negative target groups add
nothing. Group/storage order and duplicate target occurrences are retained.
Portal distance uses `802f3c80`, including its triangle/quad handling. The checked
group reader, alternating-fan distance and expansion are now independently
implemented; read [portal contracts and numeric limits](portal-expansion.md).
Do not replace them with a zone-AABB sphere overlap or generic graph flood fill.

Initialized expanded requests use a bounded query view that validates the
initial zone's full raw row span/group metadata before evaluating shapes lazily
through each group's first hit. Malformed unused row metadata can therefore
fail host reading; exact raw-row load laziness is not claimed. This view remains
separate from the full-map portal inventory audit.

The map collector separately filters zone geometry and placed objects, with the
entity's flag `+4 & 0x100` and reference/filter helper `802cd850` before geometry
dispatch. The latter can replace the selected entity pointer. Placed-instance
matrices come from live `0x9c`-byte records, not directly from the serialized
60-byte placement table or an assumed inspection transform convention. The
initialized snapshot adapter now reconstructs those initial matrices/bounds and
dispatches the confirmed static collectors. Live animator population, scheduling
and subsequent record mutations remain separate work.

The initial population path is now located: map animator binding `80314810`
calls `803165d0` for its `+0x114` runtime state. That routine walks the serialized
`+0x48/+0x4c` placement array in 60-byte steps and creates 0x9c-byte live records.
It calls `802f3070` to build each initial matrix, copies that matrix to the live
record's first 0x40 bytes, and preserves reference/hash/flag fields afterward.
The initial matrix builder is now reconstructed as T*S*Rz*Rx*Ry, with Euler
helper `802a54d0` order six, its polynomial and paired accumulation order.
Read [placement transform contracts](placement-transforms.md); live mutation
and original execution are still unverified. The collector maps
live engine flag `+0x90 & 4` to argument bit two
before geometry dispatch. The pure [bounds update](collision-bounds.md) now
preserves unfused endpoint arithmetic, ordered component unions and the final
candidate's radius-expanded stored-bounds gate. Initial component/shape bounds
are now reconstructed and audited. Pure [candidate/filter/record contracts](collision-collection-records.md)
are available. Initialized hierarchy refresh and geometry collection are
implemented separately from live membership, later mutation and manager-owned
record population.

## Collected records and pending work

The subsequent `802ceb00` consumer iterates collected records in `0x50` byte
steps, invoking `80296160` for each. It caches matrix preparation across consecutive
records sharing the pointer at record `+0x48` and signed short at `+0x4c`; a changed
key triggers preparation. This is not global geometry deduplication. Record flags
`+0x4e & 2` select different matrix helpers (`802a4d30` versus `802a5220`). These
implement a general matrix inverse and a rigid transpose/translation inverse respectively.
Pure [frame preparation and cache semantics](collision-frames.md) are now
implemented, with a provisional host reciprocal seed in the general helper.
Do not normalize scale or substitute one helper for the other. Consecutive key
reuse skips both matrix and flag loading; the current record's translation is
still subtracted before inverse multiplication with W=0. Live record binding
and original FP estimate precision remain open.

`80296160` takes mesh data from record `+0x40` and another context pointer from
`+0x44`. It prepares positions, transforms the query origin into the record's
frame, applies mesh/batch pruning, consumes the collision input streams, then
dispatches triangle queries. The [static mesh contact path](mesh-contacts.md) now
preserves batch-cache/coarse pruning, query-frame triangle transforms, primitive
order, contact capacity and world-space result binding. It accepts explicit
initial records and rejects nonzero position contexts. Do not simply loop over
every rendered triangle with renderer matrices: membership, transform/cache rules,
excluded batches, disabled flags and raw strip order remain separate contracts.

`collision-world-box` now binds these initialized static contacts to an authored
ordered manager list and BSP/portal-selected zones. Duplicate zones remain
ordered, candidate ordinals reset per zone, and one record buffer and later
inverse cache/contact buffer span the complete sequence. `grenade-ascent-box`
uses the source first-ascent request, not a full grenade-clearance state update.
Read [world request contracts](world-contact-requests.md) before extending it.

The separate [static world line adapter](world-line-queries.md) now reconstructs
`80314a40` with segment BSP traversal, line-specific gates, transpose placement
frames and resident plane queries. It does not use this box record/contact buffer.
Next reconstruct actor-manager `80303110`, component virtual slot `0x48` and
backend `8027eb20`, before connecting the full clearance state and typed body
activation. Manager-owned live population, mutation/cache invalidation, skinning
and original numeric/runtime comparisons remain open.
