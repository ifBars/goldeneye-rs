# Navmesh acquisition and actor request dispatch

The runtime now has `acquire_navigation` and `NavigationRequestState::dispatch`.
Together they reconstruct the acquisition/query phase ahead of steering. Actor
scheduling, policy setup, route mutation and movement remain incomplete.

## Evidence

Artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
The original program was not run, linked or packaged. Raw instructions and probes
remain under ignored `local/`; tests use authored data.

| Contract | Original location |
| --- | --- |
| Actor dispatch wrapper / virtual reference | `0x80055280`, table reference at `0x8056ed64` |
| Actor request dispatcher | `0x80069570`–`0x80069648` |
| Explicit binding refresh | `0x80068930`–`0x80068974` |
| Manager acquisition | `0x80187f60`–`0x801883e8` |
| Vertical broad phase | call at `0x8018810c` to `0x802932b0` |
| Nearest surface lookup | virtual slot `+0x50`, call at `0x801881a0` |
| Center-height score / strict replacement | `0x801882e8`–`0x80188374` |
| In-place constructor preserves AABB | factory `0x802e26e0` uses placement helper `0x80028260` (returns input pointer); initializer `0x802e74d0` leaves `+0x10`/`+0x20` intact |

## Acquisition

Candidates have an explicit translation frame, stored local AABB, checked faces,
and prepared centers. The candidate list/order is supplied by the caller. This API
does not infer the original manager's instance list from renderer placements.

For each candidate, subtract translation from the world position **before** adding
one to local Y. This is a different operation order from ordinary world endpoint
binding, which raises world Y first. Preserve the distinction under cancellation.

Broad-phase a vertical segment with displacement `(0,-distance,0)` against the
stored AABB. A starting point inside the box and segment endpoints are inclusive.
Outside the box, a displacement smaller than f32 `1e-5` is not plane-tested. The
Rust operation specializes the observed helper to this vertical direction.

After a broad-phase pass, run a full-mesh downward nearest ray with `0<=t<distance`.
Rank a hit by `abs((face_center_y + translation_y) - original_world_y)`, keeping
the first candidate on equal scores. This is not a nearest-hit-distance or nearest
3D-center selector. Stored centers and the existing ordered centroid fallback
remain distinct. The actor dispatcher passes distance **30**, observed at
`0x80069600`/`0x80069610`; ordinary endpoint binding still uses distance 3.

The AABB reader decodes body `+0x10` minimum and `+0x20` maximum, preserving raw
header words. Do not replace the stored box with calculated vertex extents. Some
boxes do not enclose every vertex. In the local Severnaya interior sample, face
63's stored center is about 0.7 units beyond the stored minimum X. A direct surface
query can hit it while acquisition rejects it. An authored regression protects
this distinction. This observation does not establish original live behavior.

Finite inputs, positive distance, valid bounds/face IDs and arithmetic overflow
are checked host constraints. Original FP/GQR/FPSCR behavior remains unverified.

## Dispatch state and callbacks

`flags_raw` preserves unrelated actor flags. `binding` is an explicit host candidate
index in place of the original cached wrapper pointer. The acquire and query
callbacks connect this phase to resident game data without file I/O in `ge-core`.

1. Clear bit `0x800` on **every** dispatch, including inactive calls.
2. Process only when both gate bits `0x100` and `0x200` are present.
3. With a binding, set `0x400`, query that binding, and set `0x800` only on success.
4. Without a binding, call acquisition and cache its result. Do not query on that
   call, even when acquisition succeeds. Set `0x400` if acquisition misses.
5. Preserve other flags, including an existing `0x400`; gate bits remain set.

The original boolean return indicates processing, not route success. Both failed
acquisition and a completed query with no route are processed requests. A later
call retries missing acquisition; a cached binding queries without acquiring.
The broader meanings/setters of the gate and marker bits are still under study.
Host callback errors propagate rather than becoming normal misses, preserving
mutations already performed before the error.

## CLI

```powershell
.\target\release\goldeneye.exe nav-request '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --translation 0 0 0 --start 0 0 0 --goal 1 0 1 --flags-raw 300 --steps 2 --output local/actor-request.json
```

Coordinates are illustrative. This diagnostic prepares root `0x607` entries in
directory order, preserving duplicate hashes, with one **explicit shared frame**.
That candidate context is authored, not recovered manager membership or actor
spawn context. Core acquisition supports independently translated candidates.
There is no render-model rotation/scale inference.
Alternatively, `--map HASH` replaces `--translation` and supplies recovered initial
per-instance frames using the [static map binding rules](navigation-placements.md).
That mode preserves registration order and unavailable bindings while sharing
decoded meshes. Global loaded-map precedence and dynamic state remain unverified.

`--flags-raw` is explicit hexadecimal input. `--steps` is a host dispatch-count
guard in `1..=16` (default 2), not game ticks or a recovered scheduling frequency.
One active call acquires only; a second can query. Output records each event,
processing boolean, before/after flags, cached binding, latest acquisition and
latest world query/waypoints. Failed/missing phases do not fabricate paths.
`--max-expansions` defaults to 100,000 and must be positive. Waypoint host limits
are 100,000 shortcut tests and 1,000,000 fan steps. Exhaustion is an error. Existing
output files are preserved. Game-derived output stays local.

## Validation and remaining scope

Authored tests cover candidate scoring/ties, translation-before-lift, stored boxes,
inclusive broad-phase versus strict ray limits, invalid data, inactive calls,
success-bit clearing, deferred querying, cached binding reuse, misses/retries,
unrelated flags and host callback errors.

The local audit uses each root graph as a separate authored candidate context:
542 requests across 68 graphs, with two dispatch calls per request. It completed
1,084 calls: 543 acquisitions, including two stored-AABB misses from the same
retried sample; 541 deferred queries produced 540 routes. Both AABB misses excluded
a surface available to the separate direct endpoint query. The query with no route
retains the existing endpoint-miss result. Independent f64 interval/surface checks
validate acquisition misses, and deferred queries replay the separately validated
ordinary world query. These counts do not prove original actor compatibility.

Initial single-map bindings now have a separate source-backed reader. Next establish
global loaded-map precedence, mutable/cached instance state, scheduling and gate
setters, actor-specific callbacks/point-goal variants, route mutation, steering and
movement/collision. No waypoint popping or constant-speed follower is inferred
from this dispatcher. Reference captures and numeric comparisons remain required.
