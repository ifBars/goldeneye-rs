# Collision instance bounds

`ge_core::collision::bounds` reconstructs the affine bounds update and ordered
component union used by placement initialization. `ge_assets::entity_spatial`
reads common spatial headers for identified static types 0x601, 0x603, 0x607 and
0x608. The [initialized static collection/contact path](mesh-contacts.md) now
consumes their initial recipes. Live loaded-world membership, dynamic component
binding, physics response and full grenade clearance remain separate work.
[Authored world and first-ascent requests](world-contact-requests.md) now use
these initial recipes without establishing live membership.

The contracts come from static inspection of the user-supplied SJBE52 revision-zero
executable. Original code and private disassembly stay under ignored `local/`.
No original executable code is run, translated into Rust or packaged.

## Placement update

Zone initialization `80316f10` walks its stored u16 placement indices and calls
`80318a50` on each addressed 0x9c-byte record. The update resolves the record's
entity reference at +0x84 using the native entity directory. If it resolves, the
entity's eight stored bounds lanes at +0x10..+0x2c are copied to record +0x40..+0x5c
and transformed by `80292ee0` using the record's initial/live matrix.

The transform uses XYZ endpoints and the matrix's 3x3 basis/XYZ translation only.
Input bounds W and the matrix's homogeneous row stay unread. For each output axis,
each min/max endpoint product rounds separately. A rounded `second-first >= 0`
selects the first product as the lower endpoint, otherwise the second. Translation
is the initial accumulator; the selected X, Y and Z contributions are added in
that order, each rounded separately. There are **no fused multiply-adds** in this
helper. Both output W lanes become positive zero. Do not reuse point-transform
accumulation, normalize scale, assume positive scale or derive bounds from render
vertices. Inverted source endpoints are retained as input to the helper.

The update next visits component slots zero through four in ascending order when
their bits are set in the low byte of entity +0x40. Each component's own stored
header bounds is transformed separately and unioned into the result. Slots five
and seven do not participate in this loop. Component slot six is looked up afterward;
when present, helper `802edb50` first computes its separate shape aggregate, which
is transformed and unioned. It is not an ordinary entity-header bound. The
aggregate helper is reconstructed separately below; live record binding remains open.

The checked component reader follows compact lookup `802e3280`: the low eight
mask bits order the slots; the table address is relative to entity +0x40 using
the sign-extended upper 24 bits. A slot's descriptor rank is the population count
of lower set bits. Its target is relative to the descriptor, again using a signed
upper-24-bit offset. The descriptor's low byte stays raw. Absent slots read no
table or descriptor. Backward links and repeated targets remain intact.

The checked component reader reconstructs compact lookup `802e3280`: the low
eight mask bits order the slots; the table address is relative to entity +0x40
using the sign-extended upper 24 bits. A slot's descriptor rank is the population
count of lower set bits. Its target address is relative to that descriptor, again
using a signed upper-24-bit offset. The descriptor's low byte is preserved raw.
Absent slots read neither table nor descriptors. Backward links and repeated
targets are retained; only addressed known spatial bodies decode.

`prepare_instance_bounds` accepts explicit base bounds/mask and lazy callbacks.
Masked-out ordinary components and absent slot-six shapes remain unread. Missing
required component data and unavailable shape aggregation must return errors;
callers must not invent empty bounds to obtain a successful clearance query.

Union `802cb1a0` subtracts the two endpoints before selecting across **all four**
lanes. Minimum ties choose the incoming operand; maximum ties keep the existing
operand. Signed zeros are therefore observable. Finite inputs whose product,
difference or accumulation overflows produce explicit host errors.

When the base entity reference is unresolved, the native update installs a finite
inverted sentinel: minimum `(1000000,1000000,1000000,+0)`, maximum
`(-1000000,-1000000,-1000000,-0)`. This path reads no matrix or components. It is
not an infinity sentinel and is not proof of clear space.

## Broad-phase margin

The map collector tests each candidate record's +0x40 bounds with `80293060`, the
original query origin and its radius. This helper compares rounded minimum-minus-
coordinate differences against positive margin, then rounded maximum-minus-
coordinate differences against negative margin, minimum XYZ before maximum XYZ.
Equality passes. Negative margins shrink without clamping or endpoint sorting.
W stays unread; later XYZ lanes stay unread after an earlier rejection. The same
helper with margin zero continues to implement loaded-world selection.

The preceding [candidate hierarchy](collision-collection-records.md), its native
membership flags and record indices remain separate from this final stored-bounds
gate. Do not replace that hierarchy with a scan of every serialized placement.

## Slot-six shape aggregate

`ge_assets::shape_bounds` reconstructs helper `802edb50` for this operation only.
Its first signed count is at +0, relative stream pointer at +4, second signed
count at +8 and relative polygon pointer at +0xc. The first pointer word is read
even for nonpositive first counts; the second pointer stays unread for nonpositive
second counts. Unused targets/payloads remain unread.

The observed loops have unusual source accesses. Each positive first outer count
repeats `byte(first)+1` updates, always reading the same first record XYZ at
+4/+8/+0xc. Neither its source pointer nor point offset advances. Each positive
second outer count restarts the first polygon, whose inner loop reads
`max(byte(first)-2,0)` XYZ points at +0x10/+0x14/+0x18 with sixteen-byte stride.
Later records/polygons are not substituted. Point W is positive zero, and the
same signed-zero union ties apply. The aggregate starts at the finite +/-1e6
inverted sentinel; extrema outside those values retain the sentinel's clamping.

Empty aggregates still pass through the affine bounds transform and union.
Skipping them as a neutral empty set changes the native result. Nonfinite used
values, invalid addressed pointers/ranges, overflow, more than one million outer
iterations or total point updates are explicit host errors. This is a bounds
reader, not a complete shape decoder or contact/clearance implementation.

## Diagnostics and limits

`Edb::entity_spatial_at` preserves raw flags, homogeneous-lane bits and the packed
component word without following component pointers. XYZ must be finite. Unknown
entity types fail this reader rather than being treated as empty geometry.

`scan-edb --decode-collision` resolves each initial placement's coded entity
reference. Unresolved references and unsupported types retain separate counts.
For known types it first transforms base header bounds, then decodes addressed
ordinary components and slot-six aggregates, assembling initial bounds when all required inputs
are available. Independent f64 geometry transforms all eight endpoints of every
source box and reduces their extrema. Base and assembled comparisons retain
separate counts. Unsupported component types remain explicitly pending; a base-bound
probe does not claim a complete assembly. Invalid addressed shape data fails the audit.

Strict bound agreement requires all six XYZ endpoint errors <=0.00001. Larger
errors must fit `0.00001 + 16*f32_epsilon*(abs(translation_axis) +
sum(abs(matrix_axis_coefficient)*max(abs(min_endpoint),abs(max_endpoint))))`
and are counted separately. Failed bounds fail the resource gate. Authored tests
also exercise rotated/reflected/collapsed bounds, signed-zero union ties, lazy
components, exact missing sentinels, margin endpoints and 1,000 independent
eight-corner comparisons. These checks are geometry evidence, not Wii execution
or reference-game parity. Original FP-state behavior remains unverified.

The first complete local assembly run checked 21,059 placements across 366 maps,
including 10,716 ordinary component instances and 262 slot-six aggregates with
16,171 addressed point updates. No reference/type/assembly cases were missing.
Of the assemblies, 20,312 met strict tolerance and 747 needed the rounding bound;
maximum error was 0.000137330. Base-only checks counted 20,321 strict and 738
rounding-bounded results. These are initial recipes across directory entries,
not simultaneously resident live collision objects.
