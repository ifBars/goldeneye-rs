# Connected-face segment lookup

`ge-core::navigation::connected_segment_hit` implements an observed navmesh surface
lookup. It tests a displacement segment against a starting triangle, then searches
connected triangles. It returns a face hit, not a movement path or shortest route.
This is an independent Rust operation informed by local static evidence; original
runtime comparisons, Wii numerical precision and actor behavior remain unverified.

## Static contract

The original artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Raw instructions and analysis remain under ignored `local/`, outside the source
and executable packages. No original program was executed for this investigation.

| Evidence | SJBE52 revision 0 location |
| --- | --- |
| Connected lookup | `0x802e7f70`, navmesh virtual slot `+0x58` |
| Current-face segment test | Calls navmesh slot `+0x54`, targeting `0x802e7c90` |
| Default policy installation | Constructor `0x802e74d0`, field assignment at `0x802e750c` |
| Policy field | Mesh body `+0x54`, holding the callback object |
| Default callback object | `0x806ec334`; table `0x80591110` installed at `0x802e944c` |
| Default edge predicate | Callback slot `+0x08`, targeting `0x802e73c0` |
| Policy replacement/reset | `0x802e80e0`; a null replacement restores the default |

The default predicate reads the selected neighbor's low 20-bit ID and compares it
with the previous face. It accepts only when those IDs differ. It does not inspect
the preserved upper bits or other actor metadata. The callback receives the mesh,
current face, previous face, neighbor slot and opaque caller context; its default
implementation ignores that context. Actual installed actor overrides are not yet
traced or implemented.
The separate [target caller binding](actor-target-caller.md) now supplies the
manager-installed flag policy through the fallible-policy variant. Other callers
and the default filter retain their own contracts.

The lookup follows these rules:

1. Test the current face using the inclusive segment operation, `0 <= t <= 1`.
2. Return immediately on a hit, before considering the depth or any edge predicate.
3. On a miss, stop if the signed remaining depth is zero or negative.
4. Visit neighbor slots `0`, `1`, `2`. Stop at the first low 20-bit boundary sentinel
   `0xfffff`; do not skip it to visit later slots, even if those contain IDs.
5. Invoke the policy immediately before entering each eligible child. Finish that
   subtree before checking the next slot.
6. Descend with depth minus one and the current face as the child's previous face.
7. Return the first successful subtree result. There is no global visited set,
   nearest-hit reduction, group restriction or distance-based ordering.

Immediate backtracking is prevented by the default policy. Longer cycles can
revisit faces until the depth is exhausted. Adding a global visited set or sorting
neighbors could change which face wins and how often a stateful policy is called.

## Independent API and host guard

`SegmentSearch` takes mesh-local origin/displacement, a starting face, an optional
previous face, signed depth and an explicit maximum number of face tests. Depth
zero or negative still permits the initial face test. The starting/previous IDs
must be valid table positions; invalid input reports an error.

The Rust implementation uses an explicit stack to preserve recursive visit and
callback order without consuming the host call stack. Its face-test budget is a
separate host guard, not a recovered game setting. Exhausting it returns an error;
it never returns a successful miss or falls back to another search algorithm.
A search that completes on its last allowed face test still succeeds normally.

`connected_segment_hit_with_policy` replaces the default predicate with a Rust
callback. The immediate-backtracking filter is not pre-applied to that replacement.
Caller-owned policy context can be captured by the callback. This extension models
the observed callback boundary; it does not supply unimplemented actor policies.

`SegmentSearchResult` adds diagnostic counts for face tests, policy checks and
maximum depth reached. These counts extend the original index/point result.
The same provisional plane/containment math as the other surface queries is used;
see [numerical limitations](navigation-queries.md#numerical-limits-and-parity).

```powershell
.\target\release\goldeneye.exe nav-connected-segment '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --start-triangle 0 --depth 2 --origin 0 5 0 --displacement 0 -10 0
```

The example coordinates and depth are illustrative, not an original actor setup.
Use `--previous-triangle N` to supply an existing previous face. `--face-limit N`
sets the host guard (CLI default: 100,000). A null result hit is a completed miss;
invalid data or an exhausted host budget exits with an error. Outputs preserve
existing files and must remain local when they include game-derived coordinates.

## Evidence and remaining scope

Authored tests distinguish depth-first/slot order from breadth-first or nearest
selection, cover first-boundary termination, depth zero/negative values, callback ordering and replacement,
immediate/two-face/longer cycles, invalid topology and work-budget exhaustion.

The local corpus sampled 2,094 connected-face queries across 68 navmeshes, starting
at reciprocal neighbors with depth one. All returned a neighboring face hit, using
5,820 face tests and 3,726 policy checks; no reciprocal-neighbor sample was skipped.
Independent f64 geometry checks validate returned points. This is not an original
runtime comparator and does not test arbitrary deep routes, policy changes or actors.
It contributes no verified parity score. Full path selection, world transforms,
agent clearance, dynamic blockers and movement remain pending.
