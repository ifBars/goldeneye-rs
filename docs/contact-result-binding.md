# Type-three contact result binding

`ge_core::collision::result_binding` reconstructs the append-stage transformation
for accepted oriented-box/triangle contacts. It is independent from the unfinished
mesh traversal and live clearance backend. Private source evidence is retained in
`local/analysis/contact-binding-contract-v1.json` for SJBE52 revision zero.

The triangle and primitive contact must already be in the same query frame.
Static mesh dispatch computes that frame with a rigid inverse of the **request**
matrix multiplied by the current collected matrix; it does not transform the
query box through the cached mesh inverse. The latter serves separate broad-phase
origin pruning. Those full mesh adapters remain pending.

The type-three wrapper maps `point_minus_normal` to witness A and `point` to
witness B. Each is transformed by the request matrix with input W=1, then output
W is forced to positive zero. There is no perspective divide or inferred depth
scale factor. Depth is retained from the primitive.

The stored result normal is recomputed from the query-frame triangle's ordered
edge cross product, **not** taken from its SAT-selected contact normal. Ordered
cross/dot operations, normalization refinement and exact zero/underflow fallback
to +X are retained. The reconstruction uses provisional host reciprocal-square-
root and reciprocal seeds; one fused rsqrt correction and two reciprocal corrections
follow their observed order. Wii estimate/FP-state parity remains unverified.

Normal input W=0 is transformed by the request matrix without post-normalization;
all four output lanes remain intact. Odd triangle ordinal within a strip then
negates this already-transformed normal and swaps provenance indices B/C. Neither
witness is swapped or negated. Collision input strip triples remain consecutive.

Provenance retains opaque collected-record/first-corner identities, raw triangle
flags and material, u16-to-u32 position indices, query-shape index truncated to
u8, and output flag byte zero. The confirmed static path has no separate explicit
index array argument. No original pointers are dereferenced by this pure kernel.

Capacity gates differ. The pre-triangle mesh gate checks
`count < capacity.wrapping_sub(1)` after position/frame/coarse preparation. The
post-hit gate checks `count < capacity`. Capacity zero therefore passes the first
gate at count zero and fails the second; capacity one fails the first. A mesh can
fill the reserved final slot through multiple primitive hits before its next
mesh-level check. Host adapters must use bounded temporary primitive outputs and
must not infer clear space from saturation.

Eight authored tests include 1,000 independent f64 triangle/frame comparisons,
odd-strip provenance, scaled/projective frames, witness ordering, signed zeros,
zero/small normals, explicit nonfinite/overflow errors and capacity boundaries.
These validate a reconstruction component, not original execution or live physics.
