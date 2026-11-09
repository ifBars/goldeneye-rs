# Target action gate and record update

The downstream record consumer is now identified as grenade launching. See
[grenade preparation](grenade-launch.md) for its separate, unintegrated helper
and owning dispatcher. This command exercises the direct target handler.

`nav-actor-action` runs one authored caller snapshot through the observed angular
gate, [caller binding](actor-target-caller.md), actor goal/edge/heuristic search and
selected-record update. It retains state on a rejected gate or completed query
miss. Live record population, active-index scheduling, subsequent integration and
collision remain unimplemented; this is not a moving actor or playable game.

## Static contract

Evidence is the local SJBE52 revision 0 artifact identified in
[triangle graphs](triangle-graphs.md). Original instructions/tables remain private
under ignored `local/`, outside source/packages. The Rust operations and authored
fixtures are independently written.

| Stage | Original evidence |
| --- | --- |
| Active angle limits | Five f32 pairs at `0x8056fff8`–`0x8057001f`; active index at caller `+0xd4` |
| Mode switch | `0x8007e704`–`0x8007e7a4`: zero byte uses lower/upper in order; any nonzero byte swaps them |
| Angular gate | `0x80059480`–`0x80059678`, called with relative=true and offset zero; the caller's `+0xcc` argument is unused in this branch |
| Heading/bearing | Owner yaw at `actor[+4]+0xdc`; bearing is double atan2 of owner-local target X/Z, then rounded to f32 |
| Angle difference | `0x80340800`–`0x8034090c`: separate positive-turn normalization, f32 subtraction, signed half-turn adjustment |
| Strict gate | `0x80059624`–`0x80059640`: accept only `-lower < difference < upper` |
| Selected destination | Accepted goal's scratch center plus the captured instance origin; `0x8007eb84`–`0x8007ec24` |
| Motion calculation | `0x8007ec2c`–`0x8007edb8`: length, fixed exponential scale, reciprocal refinement, separate vector multiplications and half scaling |
| Result mutation | `0x8007edbc`–`0x8007ee00`: state `+0xc0 = 2`, selected vector at `+0x50 + index*16`, then its Y +=1; return 2 |
| Rejection/miss | `0x8007ee08`: return 1 without these state/vector writes |

Pairs 0, 2, 3 and 4 use symmetric f32 limits `0x3f490fdb` (approximately 45
degrees). Pair 1 uses lower `0x3edf66f3` and upper `0x3e5f66f3` (approximately
25/12.5 degrees). Unsupported table indices are checked host errors; there is no
read into adjacent interface data. Source and selected motion records are addressed
with the same active index; unrelated records and all positions remain unchanged.

The gate uses f32 pi/tau, including a double divisor equal to the *promoted f32*
tau rather than mathematical double 2*pi. Caller yaw canonicalization and the
difference helper have distinct operation orders. Positive/negative pi ties swap
sign in the signed adjustment. A negative tiny remainder added to tau can round
to tau; no second clamp to zero is introduced. Y is unused by the angular gate.
The implementation covers this caller's relative/zero-offset case, not all generic
argument combinations of the angular helper.

## Motion arithmetic and limits

World center is formed by **adding** the selected raw scratch center to the
captured instance translation, then subtracting the active world position. There
is no world-goal wrapper subtract/add round trip and no position snapping.
Length uses rounded X/Y squares and sum, then fused Z square. The source scale is
`((5f32 * 0.2f32) * length) / (1f32 - f32(exp(-25f64)))`. The denominator rounds
to one under ordinary round-to-nearest. Each displacement component is multiplied
by scale, then reciprocal length, then 0.5; only afterward is one added to Y.
Do not cancel these stages into half the displacement: intermediate f32 rounding
is observable even when the real-number expressions agree.

The original frees the returned search record list before the state/vector writes.
The independent selector returns owned diagnostic values; no original allocator or
executable is used. Host square root, reciprocal and libm calls are provisional
substitutes for Wii estimates/refinement and its library. FP state, denormals,
fourth components and original execution remain unverified.

Zero/underflow length would reach reciprocal-estimate refinement with zero and
produce nonfinite vector values. The host reports an unsupported motion error.
Other used nonfinite/overflow inputs are errors. Checked errors preserve the prior
snapshot; this does not establish original exceptional-input mutation behavior.
No error is converted into a successful miss or update.

## Snapshot and command

```json
{
  "owner_translation": [0, 0, 0],
  "owner_yaw": 0,
  "instance_translation": [0, 0, 0],
  "state": {
    "state_raw": 7,
    "active_index": 0,
    "mode_raw": 0,
    "target": [0, 0, 10],
    "records": [{"position": [0, 0, 0], "motion": [7, 8, 9]}]
  }
}
```

Save an authored snapshot under `local/`, then use:

```powershell
.\target\release\goldeneye.exe nav-actor-action '<your-disc>' '<resource.edb>' --index 0 --snapshot local/action.json --output local/action-result.json
```

The example defines the schema, not a known valid source-level request. Yaw and
limits are radians. Snapshot size is bounded to 1 MiB. `--max-face-tests`,
`--max-expansions` and `--max-goal-steps` are separate positive host budgets.
Game data loads lazily after a passing angular gate. Existing outputs are preserved.
Output includes before/after state, angular diagnostics, binding/search diagnostics
when evaluated, original return code and motion arithmetic. Return 1 is distinct
from the retained `state_raw`; it does not reset that field to one.

## Evidence

Authored tests cover all five pairs, nonzero-mode swapping, strict limits, pi ties,
large headings, unused Y, lazy rejected/missed queries, selected-record-only writes,
world-coordinate cancellation, multiplication rounding and state retention on
errors. The CLI also checks bounded snapshots and no asset access after rejection.

The initial corpus uses 136 aligned and 136 opposed authored calls: 12 updates,
237 no-ops and 23 explicit unsupported goal-walk errors. Twelve motion vectors
agree with an independent f64 displacement approximation; maximum absolute
difference is `1.78814e-7`. Of 5,440 angular probes, 4,352 agree with independent
f64 bearing/modulo checks and 1,088 near-limit probes remain ambiguous. These are
host diagnostics over authored inputs, not original-runtime or gameplay parity.
