# Initial navigation boundary caches

`build_boundary_cache` reconstructs the initial ordered vertex-pair cache.
`BoundaryCache::first_blocker` implements its raw XZ predicate. These operations
do not establish actor movement, physical clearance or original-runtime parity.
Fixtures are authored; private instructions and probes stay under ignored `local/`.

## Static evidence and scope

The artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
No original game program was executed or embedded.

| Observation | SJBE52 revision 0 location |
| --- | --- |
| Initial registration invokes cache builder | `0x80187f24` |
| Ordered vertex-pair construction | `0x80183250`–`0x801837b8` |
| XZ segment predicate | `0x801837c0`–`0x801838c4` |
| First-blocker consumer | `0x80186110` |
| Inspected actor caller, unchanged position arguments | `0x8009d420`–`0x8009d57c`, call at `0x8009d560` |
| Other direct callers | `0x8009d730`, `0x8009d900`, `0x8009dad0` |

The wrapper cache count/pointer are at `+0x64/+0x68`. The builder appends pairs;
the traced registration starts from a fresh empty wrapper. Rust models initial
construction, not repeated appends or later flag mutations. The builder copies
the wrapper translation to a temporary but does not use it. The consumer also does
not subtract translation. The inspected caller passes actor/target coordinates
unchanged. **Raw input XZ is compared against local mesh vertices.** Preserve this
observed rule for translated instances; do not infer a corrected frame conversion.

Initial immutable caches can therefore share resident mesh data. Mutable
per-wrapper flags, invalidation, loaded-map precedence and later binding changes
remain open. Four observed actor distance/height/mode gates are now implemented
over explicit snapshots; see [actor gate rules](actor-boundary-gates.md). Live
actor state and the action selecting each band remain unverified.

## Ordered pair construction

Process faces in storage order, preserving duplicates and orientation. Use low
20-bit vertex/neighbor IDs. Optional face flag bit zero emits all three pairs
`(v0,v1)`, `(v1,v2)`, `(v2,v0)` regardless of neighbors.

For an unflagged face, examine all three neighbor slots and compact valid IDs in
slot order. Sentinels are skipped here; they do not terminate the scan as they do
in ordinary connected-face lookup.

- Zero or three valid neighbors: emit nothing. An isolated unflagged triangle
  therefore does not contribute three boundary edges.
- One neighbor: choose the **last** current vertex absent from that neighbor.
  Emit two pairs starting at that vertex, with the other vertices in face order.
  Missing initialized endpoints produce a host error.
- Two neighbors: collect common IDs in the first neighbor's vertex order. With
  exactly one common ID, exclude it. Otherwise exclude the first current vertex
  matching either of the first two common IDs. Emit the first and last remaining
  current vertices. No exclusion preserves `(v0,v2)` rather than inventing a
  shared edge. Fewer than two initialized endpoints produce a host error.

This is not a generic boundary extractor. Host checks reject invalid indices,
table lengths and allocation overflow. Source-face IDs are Rust diagnostic
metadata; the original pair cache contains only vertex IDs.

## XZ predicate

Y is ignored. Cross products use separate f32 multiplies and subtraction, not
fused operations. The determinant threshold is **f32::MIN_POSITIVE**
(`0x00800000`), not the waypoint portal threshold of `0.001`.

Below that determinant magnitude, both cross numerators must also be below the
threshold. This accepts disjoint collinear segments and degenerate pairs, returning
the initialized zero position. Other intersections require both parameters in
the closed interval `[0,1]`. Position is computed along the input segment with Y
set to zero. The **first cached hit** wins, not the nearest. Nonfinite inputs,
overflow and invalid vertex accesses produce host errors. Wii FP mode, subnormal
behavior and original execution remain unverified.

## Diagnostic use and evidence

```powershell
.\target\release\goldeneye.exe nav-request '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --start 0 0 0 --goal 1 0 1 --flags-raw 300 --boundary-check --output local/boundary-request.json
```

Coordinates are illustrative. The optional check runs only with an acquired binding,
including an acquisition-only call. It is separate from route success. Inactive
requests/acquisition misses have a null boundary result. Output includes per-resident
cache counts and the first blocker. It does not run the full actor clearance logic.

`cargo xtask paths <disc>` audits each root cache once. The local corpus has 68
caches, 39,780 pairs, 1,095 flagged faces, two unflagged isolated faces and zero
missing-exclusion cases. Independent undirected edge incidence agrees on all
61,501 faces under source-specific eligibility rules. Authored tests separately
cover malformed topology quirks.

Of 2,164 sampled first-hit queries, 790 agree with independent f64 cross-product
diagnostics and 1,374 remain numerically ambiguous near parallel lines or closed
endpoints. Ambiguous samples are not passes. The f64 diagnostic declines collinear
cases; authored contract tests cover disjoint collinear acceptance, the minimum-normal
threshold, raw coordinates, height independence, cache order and invalid inputs.
No count contributes to verified parity.
