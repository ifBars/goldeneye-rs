# Predicate-based world routes

`select_world_triangle_path_to_goal_with_point` binds the alternate world-wrapper
branch to predicate search. The app now regenerates and retains these routes
through `nav-request`. Goal predicates and heuristic points remain explicit authored
inputs; full original actor policies, scheduling and movement are not implemented.

## Static contract

The original artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Private instructions/probes stay in ignored `local/`. No original program was
executed or embedded; source and fixtures are independently authored.

| Observation | SJBE52 revision 0 location |
| --- | --- |
| Non-null goal policy selects alternate branch | `0x80185c3c`–`0x80185c4c`, request/list field `+0x30` |
| Raised downward nearest lookup for start only | `0x80185c50`–`0x80185c84` |
| Missing start skips predicate search | `0x80185c88`–`0x80185c90` |
| Predicate selector invoked | `0x80185c94`–`0x80185ca0` |
| Selector records accepted face in mesh `+0x60` | `0x802e8b04`–`0x802e8b0c` |
| Success chooses that face's stored center or centroid fallback | `0x80185e18`–`0x80185f3c` |
| Raw center becomes the goal value, with fourth component zero | `0x80185f40`–`0x80185f6c` |
| Wrapper translation subtracted from raw center | `0x80185fa8` / `0x80185fb0` |
| Waypoints built, then translation added to all points | `0x80186044`, `0x80186068`–`0x80186098` |
| Alternate failure status | `0x801860a4`–`0x801860d0` |

The start uses the [ordinary world binding](navigation-world.md): raise world Y
by one, subtract frame translation, query downward nearest over the full mesh with
`0 <= t < 3`. There is **no goal surface ray** in this branch. A missing start
therefore skips the predicate and heuristic entirely. A selected vertical face
can be a successful destination despite admitting no downward goal hit.

## Preserve the raw-center coordinate rule

The chosen face's center comes directly from the mesh's stored centers, or the
ordered `(C + (A+B))/3` fallback. The wrapper does not add instance translation
before treating it as the goal value. It then subtracts translation for waypoint
generation and adds translation to every resulting point.

Preserve that sequence. Do not substitute `center + translation`, snap the center
to a surface, or force the final point to lie inside the selected face. On translated
instances the local waypoint goal can differ substantially from the mesh center.
Whether original actor paths use those translated cases remains unverified.

The result exposes three distinct positions in `selected_goal`:

- `raw_center`: selected stored/prepared mesh center, used as the wrapper goal value.
- `local_waypoint_goal`: raw center minus instance translation.
- `returned_world_goal`: local waypoint goal plus translation, with both f32 rounds.

The subtract/add round trip can lose bits. For example, raw X `-1` with translation
`16777216` returns X `0` under the modeled arithmetic. Do not replace those two
operations with an algebraic identity. Wii FP/GQR/fourth-component parity remains
unverified; authored tests protect the current observed operation order.

## Route state and CLI

Missing start yields wrapper status `0x6`. A completed predicate search finding no
acceptable face yields **`0xa`**, because the wrapper's ordinary goal-face variable
remains -1 in this branch. This differs from ordinary explicit-face no-route status
`0x2`. Both failures retain previous triangle/point lists. Success replaces the
triangle list, clears/rebuilds points and leaves status zero. The same
[persistent route state](navigation-route-regeneration.md) owns both modes.

```powershell
.\target\release\goldeneye.exe nav-request '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --start 0 0 0 --goal 1000000 1000000 1000000 --flags-raw 300 --accept-triangle 100 --heuristic-point 0 0 0 --output local/world-goal-route.json
```

Coordinates/faces are illustrative. Supplying both `--accept-triangle` and
`--heuristic-point` selects predicate route mode. Repeat the accepted-face option
for a membership predicate. The point is mesh-local and uses the observed squared
heuristic with default edge/cost policies. Optional `--actor-edges` applies the
[observed target edge callback](actor-target-edges.md) using that same raw point;
neither mode reconstructs an original actor goal policy.
Accepted IDs are checked against the selected mesh when a query occurs. Acquisition
still defers querying; inactive/unbound calls do not invent a selected goal.

`--goal` remains the nominal input and is recorded, but predicate routing does not
look it up or use it as the successful endpoint. Optional boundary/actor-band
diagnostics still compare the final authored start with that nominal goal separately.
`--next-start` can replay later external positions; it does not simulate movement.
`nav-goal-path` remains a separate mesh-local selector without world binding or
waypoint generation. Existing outputs are preserved.

## Validation and remaining work

Authored tests cover translated raw centers, cancellation, vertical selected faces,
missing-start laziness, completed predicate misses, retained status/list behavior,
inconsistent results and host guards. A synthetic placed-mesh integration test
checks translated waypoint endpoints and later miss retention.

The local corpus audit checks 542 initial authored-frame requests over 68 navmeshes:
541 waypoint routes, one missing start and zero completed predicate misses. Including
541 later start-miss retention checks and 541 recoveries, it executes 1,624 wrapper
calls. Independent topology/cost checks and 25,607 f64 portal checks pass. In 261
successful cases, the returned endpoint differs from its raw center after the f32
round trip. These counts are host diagnostics, not original actor trajectories or
reference parity. Authored tests separately cover completed predicate exhaustion.

The geometric actor goal callback is separately available through `nav-actor-path`;
this world diagnostic retains authored acceptable-face sets. Next establish the
actor caller's coordinate preparation and live inputs, mutable instance state, actual action
scheduling, steering and physical movement, then compare recorded original behavior.
