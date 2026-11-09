# Predicate-based triangle paths

`select_triangle_path_to_goal_with_policy` reconstructs the alternate mesh-local
selector: a policy decides which removed queue record is an acceptable destination.
It shares the ordered queue, replacement rules and logical record pool with the
[explicit-face selector](navigation-paths.md). This does not implement original
live actor input selection or movement. A separate [world predicate stage](navigation-world-goals.md)
now binds this selector to start lookup, selected-center waypoints and route state.

## Static evidence

The original artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Private instructions and probes remain under ignored `local/`. No original program
was executed or embedded; Rust and fixtures are independently authored.

| Observation | SJBE52 revision 0 location |
| --- | --- |
| Alternate search | `0x802e88f0`–`0x802e90f8` |
| Start captured; selected goal initialized to -1 | `0x802e892c`, `0x802e893c`, mesh fields `+0x5c/+0x60` |
| Initial heuristic receives destination -1 | `0x802e89d8`–`0x802e89ec`, cost object `+0x58`, virtual slot `+0x0c` |
| Removed face's open flag cleared before goal callback | `0x802e8acc`–`0x802e8ae4` |
| Goal callback | `0x802e8ae8`–`0x802e8af8`, goal object slot `+8` |
| Accepted face captured; parent chain returned | `0x802e8b04`–`0x802e8bc0` |
| Equal-g replacement comparisons | `0x802e8d4c`, `0x802e8dd8` |
| Neighbor heuristic also receives destination -1 | `0x802e8e50`–`0x802e8e6c` |
| First neighbor sentinel terminates expansion | `0x802e8f6c`–`0x802e8f90` |
| Actor point heuristic | `0x8007f690`–`0x8007f844` |
| Cost table edge-cost/heuristic entries | `0x80570038/0x8057003c`: `0x802e6cc0` / `0x8007fde0` |
| Heuristic thunk adjusts object pointer by -4 | `0x8007fde0`–`0x8007fde4` |

The initial estimate is computed before the first goal test, even if the start
will be accepted. Goal tests occur after removing records, never when discovering
neighbors. A true result stops before expanding that face's edges. Reopened records
are tested again. The first accepted record in queue order wins; it is not the
first discovered acceptable face or necessarily the lowest-cost destination.

Heuristic callbacks receive `None`, representing the original signed -1 destination
argument. The ordinary centroid heuristic indexes that argument as a face; silently
using it with -1 would be incorrect. There is no implicit replacement with zero.
The policy API therefore requires an explicit heuristic, edge filter, edge cost
and goal predicate. The result separately counts goal tests and expanded records.

FIFO score ties, first-sentinel termination, equal-g replacement/reopening and pooled
parent references remain unchanged. No global visited set or immutable face-parent
map is introduced. Existing self-edge/dangling-parent limitations and host expansion
budgets remain errors. Callback failures, nonfinite/negative costs and score overflow
are host errors, separate from normal exhaustion with no acceptable goal.

## Squared point heuristic

The actor heuristic uses the candidate face's stored center, or the same ordered
centroid fallback already prepared by `PathData`. It subtracts that center from
the policy object's point at `+0x1c` and returns **squared 3D distance**:
round X/Y products, add them, then fuse the Z product into that sum. There is no
square root or epsilon cutoff, including tiny nonzero distances.

`squared_goal_heuristic` exposes this arithmetic with explicit positions. The
`select_triangle_path_to_goal_with_point` helper combines it with the default
stored-weight/center-distance edge cost and immediate-backtracking filter. Callers
still supply the point and goal predicate. This helper does not reconstruct an
original actor's point source or full goal predicate at `0x8007eec0`.
The `with_point_and_edge_policy` variant accepts an explicit edge callback; optional
CLI `--actor-edges` uses the [observed target-directed callback](actor-target-edges.md)
with the same authored point. Default edge behavior stays unchanged.
Wii FP/GQR/fourth-component behavior and original-runtime comparison remain open.
The separate `nav-actor-path` command combines this heuristic with the
[observed geometric goal callback](actor-target-goals.md) and actor edges. That
mode uses explicit raw origin/point/start-face values instead of acceptable-face
sets. Its original caller/world setup remains open.

Squared point distance can be inconsistent with stored edge costs; do not claim
shortest-path guarantees or suppress the observed equal-g reopenings to improve
runtime. Callback ordering and selected chains are observable behavior.

## Diagnostic and validation

```powershell
.\target\release\goldeneye.exe nav-goal-path '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --start-triangle 0 --accept-triangle 100 --accept-triangle 200 --heuristic-point 0 0 0 --output local/goal-path.json
```

Face IDs and the mesh-local point are authored examples, not recovered actor inputs.
Repeated acceptable faces form a membership predicate; they do not define priority.
The point and at least one acceptable face are required. IDs are checked against
the selected mesh; duplicate hashes can be disambiguated with `--index`. A completed
search can return no path. `--max-expansions` is a positive host guard, defaulting
to 100,000. Existing outputs are preserved; detailed geometry/routes stay local.

Authored tests cover accepted starts, heuristic absence sentinel, callback timing,
multiple goals, FIFO ties, reopened goal tests, first boundaries, callback/budget
errors, stored/fallback edge costs and squared/fused heuristic arithmetic.

The local corpus audit checks two authored policies per sampled start: a single
farthest-reachable acceptable face and that face plus an immediate neighbor. Across
68 navmeshes, all 1,084 queries found an acceptable face; 542 admitted multiple faces.
There were 227,314 queue removals/goal tests, 150,170 replacements and 135,805
reopenings. Independent topology and f64 edge-cost sums passed, with maximum absolute
cost discrepancy `1.962e-4` and tolerance `1e-4 * (1+abs(sum))`.

All 61,501 centers also passed an independent squared-distance check to an authored
zero point, with tolerance `5e-6 * (1+expected)`. Maximum absolute discrepancy was
`0.02113` at large squared magnitudes. These host diagnostics do not establish Wii
numeric parity, original actor goals or full-game completion.
