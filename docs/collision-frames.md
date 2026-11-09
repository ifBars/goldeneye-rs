# Collision record frames

`ge_core::collision::frame` prepares inverse frames and preserves consecutive
record caching. It is a pure runtime component used by initialized static
collection/contact and [authored world requests](world-contact-requests.md).
Live record population, mutation and original-runtime precision remain incomplete.

Static evidence comes from the user-supplied SJBE52 revision-zero executable,
with private disassembly retained under `local/analysis/`. No original code is
executed, linked or included in the Rust source/package.

## Preparation and cache

Consumer `802ceb00` walks 0x50-byte collected records. The key is the opaque
owner identity at +0x48 and **signed** slot at +0x4c. Its initial key is `(0,-2)`.
Only a changed key reads the matrix and flags and prepares an inverse. Consecutive
equal keys reuse the previous result even if a callback would supply a different
matrix or flag. A sequence A, B, A prepares three times; this is not a global cache.
An unknown initial frame is an explicit error on a matching sentinel, never an
implicit identity. Callers can provide an established preexisting frame explicitly.

Collected flag **0x2** at +0x4e selects general helper `802a4d30`; otherwise rigid
helper `802a5220` runs. Other bits do not choose the helper. The live placement's
engine flag 0x4 and the final collected flag 0x2 belong to different records.
The confirmed map dispatcher maps engine flag 0x4 into argument flag 0x2. Known
0x603/0x608 collectors preserve it and 0x601 append copies it into the collected
flags. [Record contracts](collision-collection-records.md) describe that path;
live population/pruning/contact integration remains incomplete.

The rigid helper transposes the stored 3x3 basis and computes negative basis-dot-
translation, rounding X before fused Y and Z. It forces homogeneous row `(0,0,0,1)`
and does not read the input homogeneous row. Scale is retained without normalization:
on a scaled basis this helper is not a mathematical inverse.

The general helper computes ordered 2x2 minors, its signed determinant, and scaled
cofactor chains over all sixteen matrix values. Exact zero chooses reciprocal
zero, without an epsilon cutoff or identity fallback. The reconstruction preserves
minor scaling before cofactor evaluation and fused operation order. It currently
uses host `1/determinant` as the seed before the observed one-step fused residual
refinement. **Wii `fres` estimate and FP-state precision remain unverified.**
Singular finite affine matrices yield zeros; that is not proof of empty space.

Nonfinite used values and overflow are explicit host errors. Preparation errors
retain the previous valid cache and key so a retry can prepare the failed key.
This transactional host guard is not an established native exception-recovery rule.

## Query origin

Mesh dispatcher `80296160`, at `802961f0..802962d4`, subtracts the **current**
record's translation from the world origin in f32. It multiplies that delta by
the cached inverse with W=0: rounded X, fused Y, fused Z, fused W term. All four
result lanes are retained without a perspective divide. An inverse point with
W=1 can round differently. Reusing an inverse does not reuse the earlier record's
translation. This contract is separate from translation-only navigation frames.

## Validation and limits

Authored tests cover helper flag selection, retained scale, exact singular and
small nonzero determinants, used/unused nonfinite lanes, overflow, operation-order
rounding, current translation, signed cache keys, sentinel reuse and preparation
errors. A deterministic full-projective matrix suite compares the general inverse
with independent f64 partial-pivot elimination and checks its matrix residual.
These checks do not compare with original execution.

`scan-edb --decode-collision` additionally probes **both authored collected-flag
modes for each initial placement**. It does not infer live record flags. Rigid
expectations use independent f64 transposition/dot products; general expectations
use independent partial-pivot elimination. Exact-zero and ill-conditioned general
frames are counted separately and receive no inverse-agreement claim. A stable
general basis has infinity-norm condition estimate at most 10,000.

Inverse coefficients must meet `0.00001 + 32*f32_epsilon*abs(expected)`; strict
frame agreement requires all absolute errors <=0.00001. Each mode also binds five
authored origins: current translation plus origin, the unit axes, and `(1,2,3)`.
The independent query comparator starts after the required f32 origin subtraction,
uses the independent inverse, and bounds each lane by
`0.00001 + 64*f32_epsilon*sum(abs(inverse_basis_term*delta))`. Strict and
rounding-bounded queries, plus unvalidated singular/ill-conditioned probes,
retain separate counters. Failed bounds fail the resource gate.

The first local run covered 21,059 placements in 366 maps, with 42,118 authored
preparations. Of the general frames, 21,058 were validated, one was exact-zero,
and none exceeded the condition limit. Across validated modes, 41,649 inverse
frames met strict tolerance and 468 needed a scaled bound (maximum coefficient
error 0.000499792). Of 210,590 query probes, 210,579 were strict, six needed a
rounding bound (maximum error 0.000016690), and five singular probes were unvalidated.
These are directory-entry counts, not simultaneous live objects.

Initial placement data, authored cache sequences and f64 geometry do not prove
live collection, pruning, world-space contacts, grenade clearance or 1:1 parity.
Pure [bounds operations](collision-bounds.md), initialized record collection and
the [static mesh contact consumer](mesh-contacts.md) now use this component.
Next reconstruct live component/record mutation, manager-owned world population
and dynamic positions before claiming a live loaded-world clearance backend.
