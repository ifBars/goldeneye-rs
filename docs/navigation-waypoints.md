# Centroid-seeded greedy waypoints

`ge-core::navigation::build_waypoints` reconstructs the observed waypoint builder
and shortcut rules for SJBE52 revision 0. It consumes a triangle chain, explicit
local start/goal positions and checked navmesh tables. It returns retained seed
points and diagnostic source indices. Read-only [route consumers](navigation-route-consumers.md)
now expose target, arrival, length and corridor probes. Actor steering, dynamic collision and
original-runtime numerical/clearance parity remain unverified.

## Static evidence

The original artifact hash is recorded in [triangle graph evidence](triangle-graphs.md).
No original game code was executed, linked or bundled. Raw instructions and local
asset/path dumps remain under ignored `local/`; public source contains independent
Rust implementation, authored fixtures and semantic/address notes only.

| Operation | Original location |
| --- | --- |
| Waypoint builder | `0x80186be0` |
| Interior center seed loop | `0x80186ccc`–`0x80186e90` |
| First-failure greedy reduction | `0x80187054`–`0x8018715c` |
| Shortcut predicate | `0x80187390` |
| Interior face flag check | `0x80187468`–`0x801874b0`, optional array at body `+0x94`, bit zero |
| Shared vertex-ID pair | `0x80187950` |
| World translation after generation | `0x80186068`–`0x80186098` |

Given a present mesh, the seed line contains the original start, centers of chain
faces at indices `1..len-1` (exclusive upper bound), then the original goal. Stored
centers are used when available; otherwise the ordered f32 centroid fallback is
used. Zero/one/two-face chains still produce two endpoint seeds. Duplicate
endpoints are retained. The ordinary request pipeline calls this only after a
successful route; a missing endpoint or no-route result does not create waypoints.

From each retained seed, test candidates starting two positions ahead. Each
successful candidate extends the span. The first failure ends that scan; retain
the immediately preceding successful candidate, or the next seed if none passed.
Do not continue searching farther candidates after a failure.

## Shortcut decisions

For a span from seed index `p` to `q`, inspect portal pairs
`chain[i] -> chain[i+1]` for `p+1 <= i < q`. Preserve that offset: the portal at
index `p` is not inspected. Adjacent seed points need no shortcut test.

Each inspected interior face rejects a shortcut if flag bit zero is set. Other
bits remain uninterpreted. Find shared vertex IDs in the first face's vertex
order, stopping after two matches. Do not compare positions or reorder IDs.
Fewer than two matches skip that portal constraint, as the original predicate does.

The candidate segment and shared portal must intersect in XZ. Reject absolute
determinants below f32 `0.001`; for both segment parameters require `0 < t <= 1`.
The lower endpoints are excluded and upper endpoints included. Separate f32
multiply/subtract/divide operations preserve the observed boundaries.

For each portal endpoint, project onto the full 3D candidate segment. If its
projection parameter is in `[0,1]` and squared distance is at most `0.25`, require
a closed vertex fan. This is a half-unit proximity rule from the shortcut code,
not a verified actor-body radius. The check includes height and uses known fused
dot-product boundaries; projection multiply and addition remain separate.

The fan walk starts in the portal's destination face, with its source face as the
previous face. Scan neighbor slots in order, skip boundaries/previous face, and
take the first neighbor containing that vertex. Continue until returning to the
portal's source face. A missing eligible neighbor rejects the shortcut. Unlike
the connected-face surface query, this scan skips boundary slots instead of
terminating at the first one. It does not apply the ordinary edge-policy callback
or inspect support-face flags. Nonclosing cycles can exhaust the host guard.

Original FP state and actual boundary decisions have not been compared. These
rules are not a guarantee of collision-free movement or an optimal corridor.

## CLI, coordinates and work guards

```powershell
.\target\release\goldeneye.exe nav-world-path '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --translation 0 0 0 --start 0 0 0 --goal 1 0 1 --waypoints --output local/world-waypoints.json
```

The coordinates/frame are illustrative, not an original spawn/instance. Start and
goal are translated to local coordinates without the endpoint query's Y lift;
projected surface diagnostics do not replace them. After reduction, frame
translation is added to each point. Output includes both local and world points.

`--max-shortcut-tests` and `--max-fan-steps` require `--waypoints`. Their defaults
are 100,000 and 1,000,000, applied after parsing. These are independent host work
guards, not recovered game settings. Exhaustion is an explicit error, never a
rejected shortcut or a fallback point list. Table/index/finite-input and overflow
errors are also explicit. Files preserve existing output and remain local when
they contain game-derived positions.

## Evidence and remaining scope

The optional face-flag reader decoded 59,906 flag records across 68 root navmeshes;
1,095 had the shortcut-blocking bit. Missing flag arrays remain `None`, with their
runtime shortcut value treated as zero.

The sampled world-route audit generated 541 waypoint paths: 34,296 seed points,
9,217 retained points, 33,214 shortcut tests, 97,994 fan steps and 383 flag-based
rejections. One world request lacked an endpoint and generated no path/waypoints.
Seed/subsequence/frame invariants and 25,079 independent f64 XZ portal-crossing
checks passed. The f64 checker uses parameter tolerance `1e-4`; it does not replay
the original program or independently establish physical vertex-fan clearance.
Point-count reduction is an algorithm outcome, not a parity or progress score.

Authored tests cover seed endpoints/duplicates, the skipped first portal,
first-failure order, shared-ID ordering and missing edges, strict/inclusive portal
parameters, determinant cutoff, 3D half-unit proximity, closed/open fans, boundary
slots, ordered nonclosing cycles, invalid data and explicit budgets. Source-bound
receipts still declare `reference_compared: false`. Next trace actor consumers,
goal-predicate variants, source-instance binding and actual movement/collision.
