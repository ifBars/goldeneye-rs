# Actor boundary range gates

Four actor predicates now have an independently authored pure implementation in
`ge-core::navigation::actor_boundary_gate`. They combine strict range/height gates
with the [raw XZ boundary predicate](navigation-boundaries.md). Action selection,
live actor construction, scheduling, physics and original execution remain open.
Do not assign combat/action names to these functions without further evidence.

## Static contract

The source artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Private instructions remain under ignored `local/`; no original game program was
executed or embedded. The four functions have identical instruction sequences after
normalizing branch destinations and excluding their radius load:

| Function | Radius, stored f32 bits | Half width |
| --- | --- | --- |
| `0x8009d3e0` | 4.5, `0x40900000` | 0.25 |
| `0x8009d5b0` | 3.4, `0x4059999a` | 0.25 |
| `0x8009d780` | 3.45, `0x405ccccd` | 0.25 |
| `0x8009d950` | 3.6, `0x40666666` | 0.25 |

The shared half-width is stored at `0x806ee170`; radius constants follow at
`0x806ee174` through `0x806ee180`. The first function's relevant steps are:

1. Read the component's raw mode at `+0x78c`. Unsigned values above one return false
   before resolving target positions.
2. Resolve the target with `0x8002d830`, read actor position through its parent at
   `+0xf8`, and subtract target from actor. Preserve the Y difference separately.
3. Force horizontal-vector Y to zero. Compute squared XZ distance with a rounded
   X square followed by **fused** Z-square addition. The paired sum of X/Y squares
   occurs before the Z fused operation.
4. Reject distance squared **greater than or equal to** `(radius+0.25)^2`, or
   **less than or equal to** `(radius-0.25)^2`. Both band boundaries are excluded.
5. Reject absolute Y difference **greater than or equal to one**.
6. Reject a missing cached wrapper (`actor +0x5f8`). Do not acquire one here.
7. Resolve positions again and call `0x80186110` using raw coordinates. Do not
   repeat the range/height gates on this second resolution. Return its clear result.

The core exposes lazy position and boundary callbacks, preserving the two reads
and rejection order. The caller supplies mode and binding-presence snapshots;
the primitive does not recreate component pointers or concurrent state changes.
Nonfinite selected positions, arithmetic overflow and callback failures are host
errors. Wii FP/GQR state, fourth components and original numeric/runtime parity
remain unverified.

## Target records

For the modes zero/one admitted by these gates, `0x8002d830` checks target flags
at `+0x98`. When bit zero is set, the byte at `+0x206` selects a record with stride
`0x4c`, using the position at `+0x148` within the target object. With the bit clear,
the base record's `+0x148` position is used, regardless of the active byte.

`ActorBoundaryTarget` accepts a checked host slice of those positions and implements
that selection. It validates only the selected record. It is not a loader for
original live object memory. The target helper's modes two through six perform
other work and are not modeled by this type or admitted by these gates.

## Diagnostic and validation

```powershell
.\target\release\goldeneye.exe nav-request '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --map 85000000 --start 0 0 0 --goal 4.5 0 0 --flags-raw 300 --steps 1 --actor-boundary-band 4.5 --actor-mode 0 --output local/actor-band.json
```

Coordinates are illustrative. `--actor-boundary-band` requires an explicit decimal
`--actor-mode` and accepts only the four observed bands. The diagnostic runs after
the bounded dispatch sequence, using its resulting binding presence and repeated
authored start/goal snapshots. It does not claim those inputs came from an original
actor or target record, and does not schedule an action or mutate request flags.
It can reject an unbound request without inventing an acquisition. Output records
the rejection/outcome, acceptance, position sample count and boundary call count.
The standalone `--boundary-check` remains separate and does not apply these gates.

Authored tests cover all strict thresholds, lazy rejection order, target flags,
unselected records, the second raw position read without re-gating, a one-ULP fused
distance distinction, and host/callback errors. The path audit also composes the
gate with each root cache using authored eligible samples in all four bands. Of
2,164 samples, 486 agree with independent f64 first-hit diagnostics and 1,678 remain
numerically ambiguous near parallel lines or closed endpoints. There are 951
accepted reconstructed outcomes; that count is neither a reference pass count nor
a parity score. None of this verifies actor movement or action selection.
