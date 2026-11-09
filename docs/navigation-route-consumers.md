# Waypoint route consumers

`ge-core::navigation::WaypointRoute` provides read-only actor-facing consumers of
a generated world waypoint list: next target, arrival, route length, and proximity
to a route corridor. These independently authored Rust operations reconstruct
observed static contracts. They do not advance points, steer or move an actor.

## Evidence

Artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
The original program was not executed, linked or packaged. Raw instructions and
private probes remain under ignored `local/`; public tests use authored values.

| Operation | Original address |
| --- | --- |
| Actor next-target getter | `0x80068980`–`0x800689ac` |
| Actor route-corridor query | `0x80068ae0`–`0x80068cf4` |
| Point/segment squared-distance helper | `0x802dbf80`–`0x802dc14c` |
| Final-pair arrival predicate | `0x801871b0`–`0x80187284` |
| Ordered route-length sum | `0x80187290`–`0x80187384` |
| Actor request/regeneration caller (partially traced) | `0x80069570`–`0x80069648` |

The actor list is at `+0x5fc`, count at `+0x618`, and four-float-stride point
buffer at `+0x61c`. Arrival/length helpers receive the list object, using `+0x1c`
and `+0x20`. Rust stores three position components; original fourth-component
and FP-state parity is unverified. The other target getter's scripted-path branch
and position fallback are separate actor behavior, outside `WaypointRoute`.

## Contracts

The target getter returns **point one** when at least two points exist; otherwise
there is no target. It does not find the nearest point, skip duplicates or consume
point zero.

Arrival requires **exactly two points** and horizontal distance to point one
strictly below a quarter unit: `dx² + dz² < 0.0625`. Height is ignored. Distance
exactly `0.25` is not arrived. A longer list is not arrived even at its final point.

Length is the ordered sum of adjacent **3D** segments, including height. Only
exact zero bypasses the square-root pipeline; the triangle-path cost's epsilon
cutoff does not apply. Rust uses ordinary f32 square root provisionally. The
original uses `frsqrte` and refinement, so Wii numeric parity is unverified.

The corridor query scans adjacent segments in order. No limit scans all; an
explicit limit scans the first `min(limit, point_count - 1)`. Zero scans none.
Compute full-3D distance with projection clamped to the segment endpoints. Accept
only squared distances **strictly below** `radius * radius`. The radius is explicit,
not an inferred actor setting. Signed values are squared; zero accepts nothing.
Degenerate segments are point tests without division.

Segment zero also rejects positions behind its start, even inside its start end
cap. The start plane itself is accepted. Later segments have both end caps. A
rejection at segment zero continues scanning; a later segment may accept. Rust
returns the first accepting segment index as a diagnostic; the original returns
a boolean.

The distance helper sums x/y products before a fused z product. The first-segment
gate has its own paired-single order: fuse z into the x product, then add y.
An authored cancellation test protects this distinction. Broader GQR/FPSCR and
fourth-component behavior remain unverified. Finite/overflow checks are explicit
host guards.

## CLI

`nav-world-path --waypoints` reports `next_world_target` and `length_3d` with its
local/world lists. An optional explicit world probe reports arrival and the first
accepting corridor segment:

```powershell
.\target\release\goldeneye.exe nav-world-path '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --translation 0 0 0 --start 0 0 0 --goal 1 0 1 --waypoints --probe-position 0 0 0 --corridor-radius 0.5 --output local/world-route-probe.json
```

Coordinates are illustrative. `--probe-position` requires `--waypoints` and explicit
`--corridor-radius`; `--corridor-segments` requires the probe. The point is already
in world space, matching the generated world list. No extra lift, frame subtraction
or snapping applies. Missing endpoints/no-route preserve the request but produce
no probe result, direct-route fallback or invented arrival state. Existing output
files are preserved.

## Validation and remaining work

The local audit checked 541 generated routes across 68 navmeshes. Next-target and
independent f64 length checks passed; maximum length error was approximately
`4.49e-5`. It checked 26,569 arrival samples and 53,138 corridor probes at retained
points, segment midpoints and elevated midpoints, using full and first-segment
scans. Of these, 53,068 corridor probes agreed with an independent f64 projection/
clamp oracle; 70 near-boundary probes were recorded as numerically ambiguous,
not comparator passes. Distance-boundary tolerance is `1e-4`; nonzero forward-dot
tolerance scales by `1e-6` of product magnitude. These sampled checks do not compare
against original execution.

Persistent [ordinary regeneration](navigation-route-regeneration.md) now models
list retention/replacement and wrapper status. It establishes no popping rule.
Next trace actor movement flags, steering and
point-goal policies before adding a controller. Do not invent a waypoint popping
rule, constant-speed follower or actor radius from read-only consumers. Reference
captures and actual collision/movement are still required for 1:1 parity.
