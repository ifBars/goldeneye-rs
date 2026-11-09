# Collision candidates and collected records

`ge_core::collision::candidates` and `collector` reconstruct pure candidate
traversal, geometry-filter selection and fixed-capacity append contracts. They
operate on explicit callback state. Initialized hierarchy refresh and recursive
geometry dispatch now have a checked asset adapter. The separate [mesh contact consumer](mesh-contacts.md)
adds static traversal, capacity gates and result binding for authored snapshots.
Live membership/mutation and grenade clearance integration remain incomplete.
Static evidence is SJBE52 revision zero; original
code and detailed disassembly stay private under ignored `local/`.

## Candidate hierarchy

The map collector resolves a zone's +0x28 collection, then its +8 relative hierarchy
pointer, and calls `802efa70`. Each node has child count byte zero, flags byte one
and u16 leaf-entry count at +2. Flag 0x08 admits a node. An internal node's records
start at +4 with 0x1c stride: a signed relative child link at zero and packed
minimum/maximum XYZ bounds at +4. The bounds-plus-margin test occurs before
addressing the child pointer. A zero raw relative link means that record's own
address, not a null child.

A leaf's four-byte entries start at +4: u16 placement index at zero, flags byte
+2, and an uninterpreted byte +3. Only flag 0x08 entries append. Disabled entry
indices stay unread. Storage-order depth-first traversal retains duplicates and
shared nodes. Explicit node/entry/output budgets report cycles or excessive work
as errors, never successful partial/empty lists. Query origin/radius stay unread
if no branch bound is tested. The runtime trait splits these reads so an adapter
can preserve their order and laziness.
Internal children retain the parent's flag/count precheck and the recursive
helper's second flag/count read. Inline leaf children have one header check.
Leaf counts reload before every entry and the final termination check. Diagnostic
`nodes_tested` counts header checks, including repeats, rather than unique nodes.

Mutable refresh `802efbe0` is now implemented as a separate pure contract. It visits
all leaf indices, builds initial placement matrices and transforms entity base
bounds without component expansion. Internal records start their accumulator
from the **existing** packed child bounds, recurse, overwrite the packed bounds
and union upward. This can grow conservative seeds; do not rebuild smaller bounds
from rendered vertices or substitute current live component bounds. It does not
read/write membership flags. Live membership mutation remains unresolved, so a
serialized tree alone is not an established live clearance backend.

## Filtering and append

The caller first applies the entity's separate flag 0x100 gate. Filter `802cd850`
then bypasses on request-filter flag 0x400, without reading component headers.
Otherwise it tests the exact current slot against the low-byte component mask,
then descends through permitted lower slots using request flag `2 << slot`.
The first match replaces the selected entity through compact component lookup.
With no match, current slot zero keeps the original entity; other slots use
request flag bit zero as their fallback. PowerPC shift behavior, callback order
and explicit host work limits are retained.

Root type 0x601 uses vtable `8058b9d8`, collector slot +0x38 -> `802e49c0`.
The append portion copies the full context matrix into a 0x50-byte record, with
geometry identity +0x40, optional context +0x44, owner identity +0x48, low sixteen
bits of the context slot at +0x4c and low sixteen flag bits at +0x4e. A full
fixed-capacity buffer returns zero without growing or changing count. Preparing
the append payload stays lazy after the capacity check. This does not imply that
all earlier native geometry pruning was skipped or has been implemented.

For the map path, owner identity is zero. Zone geometry uses slot -1; placements
use the **ordinal in that zone's returned candidate list**, not the addressed
global placement index. Different matrices in consecutive zones can therefore
share a cache key. [Frame caching](collision-frames.md) must preserve that reuse,
while query binding still subtracts the current record's translation.

Map engine flag 0x4 ORs argument flag 0x2 before dispatch. The known 0x603 split
and 0x608 link collectors preserve those argument flags, and the 0x601 append
copies them unchanged into the collected u16 flags. Split recursion has a
separate r6 gate; it is not a change to collected flags. A 0x608 link collector
walks its count byte +0x54 and 0x14-byte rows at +0x58, resolving the owning file's
reference directory (stride sixteen), retaining order/duplicates and skipping
missing references. Do not reuse the renderer's current first-link traversal
as proof of complete collision recursion. The confirmed recursive collectors now
operate through a checked local-resource adapter in the [initialized snapshot path](initialized-collection.md).
Static mesh-contact consumption is available separately; live map membership
and manager-owned runtime integration remain pending.

The remaining pruning contracts are located too. The base collector skips sphere
and AABB pruning for recursion-gate bits r6&3. Otherwise argument flag r7&2 skips
only the sphere, while the transformed stored-AABB margin test still runs. The
sphere uses a transformed stored center and **unscaled** stored radius, with
strict-greater distance rejection and its own ordered square accumulation. The
split collector has parent/per-child sphere gates and does not honor that r7&2
sphere bypass; it passes r6|2 onward and visits all children even after capacity
is exhausted. Keep these distinct when implementing dispatch.

Authored tests verify traversal laziness/order/budgets, filter ordering and shifts,
raw append/truncation, capacity rejection and repeated-ordinal frame reuse. They
do not establish live membership, contact results or original-runtime parity.
