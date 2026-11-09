# Actor proximity and face-walk goals

`nav-actor-path` uses the observed actor goal, squared heuristic and target-directed
edge callbacks together. It replaces authored acceptable-face sets with a geometric
acceptance test. Origin, target point and start face remain explicit diagnostic
inputs; the original caller's live state is not yet wired into the independent
runtime. This does not simulate actors or movement.
The separate [caller binding stage](actor-target-caller.md) now prepares the raw
inputs from explicit world snapshots and distinct owner/instance translations.
Live record loading and the surrounding action are still unimplemented.

## Static contract

Evidence is the local SJBE52 revision 0 artifact identified in
[triangle graphs](triangle-graphs.md). The goal callback is
`0x8007eec0`–`0x8007f688`; the goal-interface thunk at `0x8007fe00` adjusts the
object pointer by -8. Private instructions remain in ignored `local/`.

| Stage | Observation |
| --- | --- |
| Candidate center | Stored `+0x88` center, else ordered centroid fallback; copied to policy scratch `+0x2c` before the distance gate |
| Distance gate | Raw target point at context `+0x1c`; full 3D squared distance, rounded X/Y products and sum then fused Z; admit `<=4` |
| Walk inputs | Raw origin at `+0x0c`, stored start face at `+0x3c`; direction = center − origin, then force Y to zero |
| Normal gate | Each processed face's optional `+0x8c` vector Y must be `>=0.9`; absent skips; no geometric reconstruction |
| Edge scan | Three vertex pairs in storage order, including pair 2→0; raw XZ ray/edge intersection |
| Intersection limits | `abs(det)>=f32 0.001`; `abs(u-0.5)<=0.5`; forward parameter strictly greater than the current threshold, with no maximum of one |
| Crossing choice | On the stored start face take the first eligible pair. Else store the first eligible pair/parameter; the first later greater parameter replaces it and ends the scan. Threshold resets to zero for each face |
| Neighbor scan | All three slots, skipping sentinels; skip immediate previous face; reject neighbor flags bit zero; choose the first face with exactly two vertex occurrences matching either selected edge ID |
| Completion | Compare the newly entered face with the candidate **after** transition. No acceptance shortcut at loop entry; the arrived candidate's normal is not checked |

The intersections use direction crossed with vertical up and the same operation
order as the paired-single arithmetic. Y does not influence these intersections;
it does influence the initial proximity gate. No normalization, placement transform,
global visited set or nearest-crossing sort is introduced. A neighboring face's
bit one is allowed by the goal walk, although the separate actor edge callback
rejects either of its low two bits. Keep those policies distinct.

The policy retains the last tested center even for a rejected candidate. Reopened
search records repeat goal tests, and the first accepted removed queue record
wins. [Predicate search](navigation-goal-paths.md) still owns queue/pool semantics.

## Unsupported cases and numerical limits

If no edge qualifies, the original leaves its selected slot at -1 and reads before
the current face records. The independent implementation reports an unsupported
crossing error; it does not manufacture a miss or infer a fallback edge. In
particular, a same-face test with origin exactly at its center has zero direction
and can reach this case. The original has no observed global walk guard. The host
uses an explicit per-goal face budget; exhaustion is an error. Search expansion
exhaustion is separately reported, retaining equal-g reopening behavior.

Wii FP state, paired-single denormal behavior, original execution and numeric
parity remain unverified. A successful host diagnostic does not establish physical
clearance, actual actor inputs, world binding, scheduling or movement.

## Diagnostic

```powershell
.\target\release\goldeneye.exe nav-actor-path '<your-disc>' '<resource.edb>' --index 0 --start-triangle 0 --origin 0 0 0 --point 10 0 10 --max-expansions 100000 --max-goal-steps 100000 --output local/actor-path.json
```

Coordinates and face IDs above are placeholders for explicit mesh/context values,
not a known valid game request. The command does not transform world coordinates
or look up an origin face. Output includes the chain, goal rejection counts, last
scratch center, last walk and separate work counts. Existing files are preserved.

Authored tests cover distance/normal equality, full 3D distance versus horizontal
walk, missing vectors, start/candidate flag differences, crossing order, no ray
maximum, closed edge parameters, sentinel skipping, duplicate vertex occurrences,
previous-face skipping, post-transition completion and error propagation.
`cargo xtask paths <disc>` additionally checks adjacent-face goal samples and
bounded full searches against independent f64 XZ geometry and reachability.
Ambiguous and unsupported samples remain distinct from confirmed outcomes.

The initial corpus audit checks all 147,062 adjacent-face samples: 135,824 agree
with independent f64 geometry, 10,914 are numerically ambiguous and 324 have
unsupported walks. The 68 bounded searches yield four paths, 62
completed misses and two expansion-budget exhaustions. Of the 66 completed
outcomes, 65 are independently confirmed and one is ambiguous. These are host
diagnostics over authored inputs, not original runtime comparisons. The two
budget cases retain the actual search error; reopening rules remain unchanged.
