# Mesh-local box–triangle contacts

`ge_core::collision::OrientedBox::query_triangle` independently implements the
observed SJBE52 revision-zero type-three primitive operation. It accepts ordered
mesh-local vertices, a center, two stored axes and three half extents. The third
box axis is their cross product. Stored axes are not silently normalized or made
orthogonal; this matters for short grenade-ascent boxes.

This is a primitive contact kernel. The separate [initialized static mesh path](mesh-contacts.md)
now supplies record frames, batch/coarse pruning, ordered primitive traversal
and contact-result binding. Live world-manager integration, skinned positions
and physics response remain open. No original game code is included, and no
original-runtime collision comparison has been performed.

## Candidate selection

Local static evidence follows type-three dispatch `802ccdd0` → `803006a0`, ordered
axis selection `802fe2f0`, then contact construction `802ff320`.

The triangle is stored as A and rounded f32 edges E0=B−A, E1=C−A. Contact code
reconstructs B′=A+E0 and C′=A+E1; it does not reach back to the original B/C values.
The result preserves three distinct stages: an early separating-axis tag, a
candidate normal/depth/axis, and an optional final contact.

| Axis ID | Operation |
| --- | --- |
| 1 | Triangle normal E0×E1 |
| 2–4 | Stored box axes zero, one, then their cross product |
| 5–13 | Each box axis crossed with E0, E1, then E1−E0 |
| 99 | Computed f32 triangle-normal length is zero |

The initial triangle-plane test uses radius plus signed projection. It does not
insert an absolute-value/two-sided plane test. Later axes test both interval gaps;
a negative gap separates, while zero continues. Equal candidate depths retain
the earlier axis. Edge axes replace the current choice only when **1.5 times**
their normalized depth is strictly smaller; the stored depth remains unbiased.
Box-axis candidates retain their stored axis length. Cross-axis radii and paired
projections use the native optimized triple-product expressions, including their
rounding order, rather than a generic full-projection SAT replacement.

Tag 99 is not an independent geometric degeneracy test. Rounded products followed
by fused subtraction can leave residual normals even for collinear input. Parser
f64 zero-area accounting stays separate from native query outcomes.

## Contact construction

Triangle-face candidates select a box face using the observed absolute-dot tie
order, construct its four corners, and clip against the triangle plane and three
edge planes. Box-face candidates clip the rebuilt triangle against five planes:
the selected face followed by both planes of each other box axis. An opposite
selected-face plane is not added.

Clipping uses closed inside tests (`distance >= 0`), appends the previous vertex
first, and generates intersections only for strictly opposite signs. Points on a
plane do not create extra intersections. Order and duplicate vertices are kept;
the native nine-vertex capacity is a checked host limit.

Face contacts use a penetration-weighted mean. Zero or tiny total weight below
f32 epsilon yields no contact, including an exactly touching candidate. The
triangle-face branch projects its mean onto the triangle plane; the box-face
branch retains the triangle mean. Candidate depth and final face-contact depth
are separate values.

Edge candidates choose a box support corner and solve closest points on
**unbounded lines**, using the observed triangle anchor for each edge ID. Do not
clamp to finite segments. Determinants below 0.00001 use triangle-line parameter
zero. Native normalization leaves squared lengths <=0.0000001 unchanged.

The wrapper returns a point and that point minus one stored normal vector. This
second output is not an independently clamped witness on the box. Negative final
depth rejects contact. Invalid used values, overflow and capacity exhaustion are
explicit host errors, never clear-space results.

## Numeric and validation scope

Paired-vector dot products accumulate fused Z into rounded X, then add rounded Y
with its zero fourth lane. Scalar clip planes round all products and add X/Y,
then Z, then offset. The line helper instead adds rounded X/Y before fused Z.
These operation orders are distinct. Host sqrt/reciprocal, Wii estimate
instructions, floating-point status and exceptional source behavior remain
unverified. The line solver reconstructs the parameter consumed by this caller;
unused native distance/other-parameter calculations are not modeled.

Authored tests cover face and edge contacts, candidate/contact distinction,
one-sided ordering, the 1.5 edge bias, clipping ties, basis permutations, short
axes, float vertex round trips, invalid values, and independent f64 projections
over deterministic samples. These tests do not establish reference parity.

`scan-edb --decode-collision` also probes at most two enabled triangles per root
mesh. Each receives an authored identity-axis box centered on A with 0.25 half
extents and a second box independently proven disjoint along X. Every distant
box must separate. Returned contact points are checked against an independent f64
plane built from the primitive's rounded f32 edges. Distances <=0.00001 are
counted separately from larger distances within `0.00001 + 8*f32_epsilon*scale`,
where scale is the largest absolute input/contact coordinate, at least one.
The latter are numerically ambiguous, not strict passes. A zero f64 edge normal
cannot validate a contact plane and is counted separately. All query/plane errors
fail the resource gate. Raw vertices and probe details stay in local evidence;
the dashboard exposes safe aggregate counts only.

The local first corpus run exercised 170,828 queries over 85,414 triangles from
42,744 root entries. All 85,414 distant boxes separated. Of 85,410 contacts,
85,306 were within the strict plane tolerance and 104 within the scaled rounding
bound (maximum distance 0.000043908 units). These are directory-entry samples,
not unique world colliders; the live dashboard receipt records freshness. Edge
contacts are covered by authored fixtures; this vertex-centered corpus probe
produced face contacts only.

The [world collection contracts](world-collision-collection.md) retain distinct
world-selection and per-record transform/cache rules. `collision-box` now consumes
the kernel through initialized static records. That snapshot consumer is separate
from live grenade clearance; an unavailable live backend must still return an
error rather than treating this primitive kernel as an empty-world answer.
The separate [world line query](world-line-queries.md) now uses its own resident
plane stream and closed triangle predicate. Its frames, cache margin, tie rules
and disabled-triangle plane reads must not be inferred from this box kernel.
