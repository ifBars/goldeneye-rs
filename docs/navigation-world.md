# World endpoint binding

`NavigationFrame`, `locate_world_endpoint` and `select_world_triangle_path` bind
world-position requests to the reconstructed explicit-face selector. They implement
the translation-only coordinate operations observed in the ordinary branch of the
SJBE52 revision 0 navigation wrapper. A separate optional waypoint stage follows
this binding; actor movement remains unimplemented.
Original execution, source-placement binding and exact Wii arithmetic remain unverified.
Initial single-map placement-to-frame rules are now reconstructed in a separate
[static binding stage](navigation-placements.md); global/dynamic instance state and
live source-placement comparisons remain open.

## Static evidence

The local DOL identity is recorded in [triangle graph evidence](triangle-graphs.md).
No original program was executed. Original instructions and raw probe geometry
remain in ignored `local/`, outside independent source and executable packages.

| Observation | Address / relationship |
| --- | --- |
| Typed frame origin | `0x80185844`–`0x8018585c` copies the referenced record's `+0x30` position into the wrapper's first vector; fourth component is zero |
| World navigation wrapper | `0x80185a20`, start/goal inputs in its second/third arguments |
| Policy installation | `0x80185a7c` / `0x80185a88` install edge/cost callbacks from request fields `+0x28` / `+0x2c` |
| Lift before translation | `0x80185abc` / `0x80185ad8` add the `(0,1,0)` vector before `0x80185b40` / `0x80185be8` subtract the frame origin |
| Ordinary start lookup | Nearest-surface slot `+0x50`, call at `0x80185ce0` |
| Ordinary goal lookup | Same slot, call at `0x80185d2c`; both lookups occur before checking for missing faces |
| Face-to-face selector | `0x80185d50` calls `0x802e8100` when both face IDs exist |
| Later waypoint processing | `0x80186044` calls `0x80186be0`, then adds frame translation to produced points |

The lookup ray is constructed in this order, with separate f32 operations:

1. Copy the requested world position and add one unit to Y.
2. Subtract the wrapper frame's translation, component by component.
3. Query all faces using direction `(0,-1,0)` and maximum parameter `3`.
4. Use the nearest-surface contract, including `t=0` and excluding `t=3`.

Do not reorder the lift and subtraction: large coordinates can round differently.
Do not replace this query with storage-first selection, a closest-centroid search,
an infinite downward ray or a renderer's rotation/scale matrix.

The ordinary branch queries both endpoints even if the first misses, and calls the
path selector only when both succeed. A missing endpoint is distinct from a
completed path search that finds no route. `WorldPathResult::path_search` is absent
in the first case; in the second, it contains a result with no path.
The separate [persistent route state](navigation-route-regeneration.md) retains
older lists for either failure, recording wrapper status independently of the
latest query result and actor dispatch flags.

Projected local/world surface points are diagnostics. The request's original
world positions remain unchanged; the original wrapper also passes unraised
positions into later waypoint processing. This API does not snap an actor to a
surface or substitute those diagnostic points for movement targets.

## Explicit frame and CLI

```powershell
.\target\release\goldeneye.exe nav-world-path '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --translation 0 0 0 --start 0 0 0 --goal 1 0 1 --output local/world-path.json
```

These coordinates and translation are illustrative, not an original spawn or
recovered instance setup. `--translation`, `--start` and `--goal` are required.
The command does not infer a navmesh instance or reuse an unverified render model
matrix. `--index N` disambiguates duplicate hashes. `--max-expansions` controls the
same explicit host guard as `nav-path`; exhaustion reports an error.

Finite-input, topology-ID and overflow checks remain separate from geometric
misses. Plane/containment arithmetic shares the [provisional numeric implementation](navigation-queries.md#numerical-limits-and-parity).

## Sampled checks and remaining work

`scan-paths` and `cargo xtask paths <disc>` now include a world-binding check for
each sampled face pair. It uses an authored translation `(11,-7,5)` with asset
centers as input positions, not original actor requests or placement transforms.
Independent reachability checks verify completed route/miss outcomes. Endpoints
receive f64 plane checks; missing endpoints are checked against every triangle
using an independent f64 vertical-ray/containment calculation. Found chains receive
the existing topology and independent edge-cost-sum checks.

The observed corpus completed 542 world requests across 68 navmeshes: 541 grounded
pairs produced paths; one request had a missing endpoint; no completed search
misses occurred. The missing sample's intended face was vertical. An independent
check of its 327-triangle graph found no admissible downward hit in the window.
No small-plane sample checks were skipped. Maximum endpoint-plane discrepancy was
`9.123e-6`; maximum route cost-sum discrepancy was `9.645e-5`.

Authored tests cover operation order under cancellation, translated coordinates,
nearest stacked-face selection, inclusive/exclusive ray bounds, unchanged input
positions, endpoint misses versus disconnected routes, invalid inputs and the
independent miss verifier rejecting a hidden hit. These checks contribute no
reference-verified parity score.

The alternate branch at `0x80185ca0` calls `0x802e88f0` with a goal predicate
instead of a goal face. Actor-specific callback thunks at `0x8007fde0` and
`0x8007fe00` adjust their object pointer before calling `0x8007f690` and
`0x8007eec0`.
The mesh-local selector and squared point heuristic are now available through
[predicate path APIs](navigation-goal-paths.md). The alternate [world branch](navigation-world-goals.md)
now uses start-only lookup and the selected raw center. Full actor goal/edge
policies remain unimplemented. Next trace those policies,
mutable instance binding and actor movement.
