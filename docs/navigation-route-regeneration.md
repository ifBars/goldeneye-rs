# Persistent ordinary route regeneration

`NavigationRouteState` owns the retained triangle chain, world waypoint list and
last wrapper status. It reconstructs the mutation order observed in the ordinary
world-query wrapper. It does not pop waypoints, select actions, steer or move an
actor. The alternate [world goal branch](navigation-world-goals.md) now shares this
state, with a separate result mapping: predicate exhaustion uses status `0xa`
rather than ordinary no-route `0x2`. Its successful points use the raw selected
center's subtract/add frame sequence.

## Evidence and mutation order

Artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Private instructions/probes stay in ignored `local/`. The original program was
not executed or embedded; the implementation and fixtures are independently authored.

| Observation | SJBE52 revision 0 location |
| --- | --- |
| Actor owns lists/status at `+0x5fc` / `+0x5f4` | arguments at `0x800695c4` / `0x800695c8` |
| Wrapper resets status before checking mesh presence | `0x80185a3c`–`0x80185a60` |
| Missing search result bypasses both list replacements | `0x80185d58`–`0x80185d5c` |
| Successful search clears/replaces triangle count | `0x80185d60`–`0x80185df4`, list count/pointer `+8/+0xc` |
| Waypoint builder invoked only after success | `0x80186044` |
| Builder frees/resets previous waypoint storage | `0x80186c14`–`0x80186c3c`, count/pointer `+0x1c/+0x20` |
| New points translated into world space | `0x80186068`–`0x80186098` |
| Ordinary failure and missing-mesh status | `0x801860a4`–`0x801860dc` |

Each wrapper call first resets status to zero. A missing mesh returns false with
status one and preserves both lists. Otherwise, query both endpoints using the
[ordinary world rules](navigation-world.md). On failure, preserve both lists:

| Result | Status |
| --- | --- |
| Completed search with no route | `0x2` |
| Missing start, including when both endpoints miss | `0x6` (`0x2 | 0x4`) |
| Start found, goal missing | `0xa` (`0x2 | 0x8`) |
| Success | `0x0` |

The start-miss branch has priority; both missing endpoints do not produce `0xe`.
On success replace the triangle chain, then clear/rebuild waypoints and translate
them. This is full regeneration from the current request position. A later query
with start equal to goal produces a single-face chain and two duplicate endpoint
positions; it does not remove a point from an older route.

Wrapper status differs from the [actor dispatch flags](navigation-dispatch.md).
An unsuccessful query can clear actor success bit `0x800` while retaining an older
route and its target. Inactive/acquisition-only dispatch does not enter the wrapper
and leaves route state/status untouched. Retained geometry alone therefore does
not establish success of the current request.

Host query errors reset status but preserve prior lists. Host waypoint errors occur
after triangle replacement and point clearing. Rust leaves that stage visible and
returns an error rather than manufacturing a normal miss or rolling back a route.
Empty successful chains, invalid rebuilt points and inconsistent ordinary results
are host errors. These error guards do not reproduce original allocation failure,
Wii FP state or original-runtime behavior.

## Authored dispatch sequences

`nav-request` now retains route state across its dispatch calls. Output includes
`route_state` at the end and after each call; `last_query` still describes the
latest query, whose waypoints may be absent even while older points are retained.

Repeated `--next-start X Y Z` values supply later authored positions. If used,
provide exactly `steps-1` triples. Otherwise every call repeats `--start`. All
values are validated before asset access. Goal and request flags remain fixed.

```powershell
.\target\release\goldeneye.exe nav-request '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --start 0 0 0 --goal 4 0 0 --flags-raw 300 --steps 4 --next-start 0 0 0 --next-start 1000000 1000000 1000000 --next-start 4 0 0 --output local/route-sequence.json
```

Coordinates are illustrative, not recovered actors or spawns. Supplied positions
are external samples, not simulated movement. Cached binding is preserved across
queries; the diagnostic does not reacquire simply because a later start misses.
Optional boundary/actor-band diagnostics use the final supplied position after
dispatch, still with their own raw-coordinate rules.

## Validation and remaining scope

Authored core tests cover success replacement, all normal status codes, missing
mesh, retained targets, status reset, staged host errors and inconsistent results.
A synthetic placed-mesh integration test verifies acquire → success → endpoint
miss → same-endpoint recovery, cached-frame use and distinct actor/route flags.

The local path audit checks 541 authored sequences across 68 navmeshes: 3,246
wrapper calls, 1,623 successful replacements and 1,623 endpoint misses retaining
both old lists. Each sequence's outside position is independently checked against
all source triangles. There are 541 final duplicate-pair replacements. These
checks augment the existing independent route/waypoint diagnostics; they are not
recorded actor trajectories or original-runtime comparisons. No completed no-route
case occurred in the original sampled corpus; authored tests cover its retention.

Next establish actor action/scheduling integration, the alternate goal policies,
mutable instance state, physical movement and numeric/reference comparison.
Do not infer a generic waypoint popping or constant-speed controller from this state.
