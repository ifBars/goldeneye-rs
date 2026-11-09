# Portal distance and ordered zone expansion

`Edb::map_portals` and `ge_core::collision::portal` independently implement the
static portal inputs and observed distance/group operations used by the map
collision collector. They extend [BSP point binding](world-collision-collection.md).
They do not collect mesh geometry, infer live map membership, bind collision
instance transforms or provide a complete grenade-clearance backend.

## Inputs and addressing

The map's portal count is at `+0x38`, with a field-relative descriptor pointer at
`+0x3c` and 0x44-byte descriptors. Each zone's `+0x20/+0x24` supplies the count and
relative pointer for eight-byte grouped rows. The first row supplies a signed
target zone at `+2` and extra-row count at `+6`; group length is that count plus
one. Every row supplies its own signed portal index at `+4`. Other target/count
words and unknown prefix/tail bytes remain raw; they do not split the group.

Negative target groups retain their rows but address no portal shapes. Used
target/index/range errors fail explicitly. Only descriptors referenced by
nonnegative groups are decoded; an unused corrupt descriptor is not addressed.
This inventory prepares all potential shapes for those groups, independently
of a particular query's first-hit short circuit.

A zero descriptor relative pointer at `+0x10` selects four inline XYZ vertices
at `+0x14`, `+0x20`, `+0x2c`, `+0x38`. Otherwise it points to linked polygon data:
vertex count at linked `+8`, then vertices at `+0x10` with a 32-byte stride. Only
XYZ is interpreted, not W or the remaining attributes. Linked polygons require
3–65,536 vertices. Row and vertex totals each have a one-million host budget per
map. Missing used pointers, truncated data, bad counts and nonfinite used XYZ
values are errors, never absent portals.

## Distance semantics

Static SJBE52 revision-zero evidence follows `802f3c80` and its triangle helper
`80378c30`. Both storage forms use an alternating origin-anchored fan:
`[0,1,2], [0,3,2], [0,3,4], ...`. Do not replace it with renderer strip winding
or reorder triangles. Rounded f32 edges and origin-minus-query positions feed
the triangle primitive. Its dot products sum rounded X/Y products before fused
Z; this differs from the BSP paired-lane projection.

The primitive computes an unclamped barycentric projection, selects its face,
edge or corner region, then evaluates squared distance through a quadratic
expression and takes its absolute value. A determinant <=f32(0.0001), including
negative cancellation residuals, returns the squared-distance sentinel with bits
`0x5f7fffff` and zero parameters. It does not fall back to a segment or point.

The polygon wrapper begins with the same sentinel as its linear best distance.
Each triangle whose squared result is <=best² writes a reconstructed point
**before** comparing distances. Equal-distance triangles can therefore overwrite
the point while retaining the earlier best scalar. Reconstruction separately
multiplies both edges, adds them together, then adds the origin. A squared result
<=f32 epsilon becomes linear zero; otherwise the wrapper takes a square root.
Strictly larger squared results skip the point write. Host sqrt remains
provisional relative to the Wii estimate/refinement sequence.

Preserve cancellation in the quadratic expression. For example, the authored
triangle `(0,0,0), (1000,0,0), (0,1000,0)` and query `(250,250,0.01)` have a
geometric distance of 0.01, while the reconstructed f32 quadratic returns zero.
Replacing it with the distance to the reconstructed point changes radius gates.
Invalid used values, nonfinite results and exhausted host budgets are errors.

## Expansion semantics

`portal::expand` accepts an explicit initial BSP zone, its stored groups, radius
and fallible distance callback. It emits that initial zone first. It scans only
those groups, in order; the first distance **strictly below** the radius appends
the group's target and ends that group. Exact equality rejects. Negative target
groups invoke no callback. It retains repeated target zones and does not recurse
into the groups of newly appended zones. Output and portal-test budgets are
checked when the corresponding operation is needed. An unavailable distance
backend must return an error; it cannot stand in for clear space.

## Evidence and limits

Authored tests cover regions, alternating fan/tie writes, native degeneracy and
small-distance gates, cancellation, malformed inputs, strict radius limits,
duplicate outputs, lazy callbacks and budgets. Deterministic random triangles
are checked against an independent f64 plane/segment construction rather than
the production barycentric-region classifier.

The resource audit uses the existing BSP center/min/max samples and authored
radii 0.25, 3 and 20. Independent f64 geometry begins after rounded f32 edge and
origin-difference preparation. It uses plane projection plus finite edge tests.
Strict distance agreement means absolute error <=0.0001. A larger error must fit
`0.00001 + sqrt(64*f32_epsilon*scale)`, where scale is the greatest squared
relative point/edge length, at least one. Near-degenerate geometry is classified
separately. Ordered zone-list comparison is stable only when both BSP selection
and every tested portal radius gate are outside their ambiguity bounds.

The first local corpus run decoded 3,777 groups / 6,370 rows across 366 maps,
with 1,298 inline quads and 1,887 linked polygons. It made 6,777 expansion queries:
4,563 stable agreements and 2,214 ambiguous cases. Its 16,430 distance calls tested
32,716 fan triangles; 54 calls contained determinant-rejected triangles. The live
dashboard reports strict, bounded-rounding and near-degenerate diagnostics
separately. These are directory-entry samples, not unique live portals or
original-runtime comparisons. [Initialized world requests](world-contact-requests.md)
now use a bounded portal query view: single-zone requests read no portal inputs;
expanded requests validate the initial zone's complete raw row span and group
boundaries, then evaluate shapes lazily through each group's first hit. Malformed
unused row metadata can still fail this host reader; exact raw-row load laziness
is not claimed. That query view is separate from this full-map inventory audit.
Live world membership and mutable manager state remain open.

The final source's distance diagnostics recorded 16,340 strict agreements, 36
larger errors within the stated rounding bound, and no additional near-degenerate
unvalidated calls. The maximum bounded distance difference was 0.011045 units;
it is not evidence of original CPU numeric parity.
