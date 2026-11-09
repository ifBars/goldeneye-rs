# Actor target-directed navigation edges

The optional `--actor-edges` policy independently reconstructs the edge callback
at `0x8007f850`–`0x8007fdc8` in the local SJBE52 revision 0 executable. It runs in
both `nav-goal-path` and predicate-mode `nav-request`. Acceptable faces and the
heuristic/target point are still explicit authored inputs. The full actor goal
callback at `0x8007eec0`, actor storage, scheduling and movement remain open.
Default CLI queries keep their immediate-backtracking-only edge filter.

## Ordered contract

1. Read the destination from the source face's neighbor slot. Reject an immediate
   return to the previous face.
2. Reject destination face flags `& 3 != 0`. Source flags do not gate this edge;
   other destination bits are retained but do not reject it.
3. If the optional per-face vector array at body `+0x8c` exists, reject destination
   Y below f32 `0.9`. Equality passes. Missing data skips this check; geometric
   normals are never manufactured, and stored vectors are not normalized.
4. Obtain source/destination centers from `+0x88`, falling back to the existing
   ordered centroid calculation. Form outgoing = destination − source and
   target = explicit context point − source. These are full 3D raw values, with
   no frame conversion or Y flattening.
5. Compute each squared length as rounded X/Y products and sum, then fused Z.
   Exact zero bypasses the square root. Normalize only when length is strictly
   greater than f32 `1e-5`: multiply every component by a separately calculated
   reciprocal length. Short/zero vectors remain unchanged.
6. Compute the dot product in the same X/Y-then-fused-Z order. Admit only a dot
   **strictly greater** than the fixed engine cosine threshold.

The engine calls `0x80298fc0` with f32 angle bits `0x3ff5be0b` (approximately
110 degrees). Independent exact-rational evaluation of its fixed-angle arithmetic
under round-to-nearest gives threshold bits `0xbeaf1d42`
(`-0.34202009439468384`). Standard cosine of that rounded angle instead gives
`0xbeaf1d43`; substituting it changes the equality boundary. Only the resulting
behavioral constant is in the Rust source; original polynomial code/tables stay
in private local analysis. Original FP mode and Wii reciprocal-square-root
estimate/refinement precision remain unverified. Rust square root is provisional.

The asset reader validates bounds and all three components' finiteness. The pure
edge predicate lazily consults only Y, preserving rejection order before unused
center/target values. Invalid used inputs and arithmetic overflow are host errors.

## Evidence and limits

Authored tests cover rejection order, low flag bits, absent/nonunit stored vectors,
the inclusive normal threshold, full 3D direction, short vectors, strict dot
threshold, malformed inputs and persistent route retention after a rejected path.

`cargo xtask paths <disc>` separately checks the local source corpus. The initial
audit saw 2,805 stored vectors (129 below the Y threshold), 147,062 directed edges
toward authored point zero, and 68 authored constrained searches. Independent f64
checks agreed on 147,061 edges; one near-threshold edge stays numerically ambiguous.
All 68 search outcomes agreed with conservative/optimistic independent
reachability: 17 paths and 51 expected misses. These are host diagnostics, not
original actor trajectories, execution comparisons or verified gameplay parity.

Use `--actor-edges` alongside `--accept-triangle` and `--heuristic-point`; predicate
`nav-request` still uses start-only world binding and raw selected-center waypoint
rules documented in [world goal binding](navigation-world-goals.md).
