# Initialized static world line queries

`collision-world-line` evaluates an authored line request against an explicit
ordered world list and initialized static map geometry. It selects one world,
binds the original segment to BSP zones, visits zone geometry before placements,
and retains one nearest-result state and resident mesh table across the sequence.
It uses the line-specific geometry implementations, planes and gates.

The command does not run a live world manager or actor list. An empty static
result does not establish grenade clearance, full state acceptance or readiness
to spawn a body. Original-runtime and Wii floating-point parity remain unverified.

## Snapshot and command

Save this illustrative authored request as `local/world-line.json`. World bounds
are explicit manager-stored values; they are not inferred from map zones.
JSON map IDs and raw flags are decimal numbers. `2231369728` is `0x85000000`.

```json
{
  "worlds": [
    {
      "resource": "m_dam_gfx.edb",
      "map": 2231369728,
      "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
    }
  ],
  "line": {
    "origin": [-252,7,-8],
    "delta": [0,-10,0],
    "constructor_delta": null,
    "maximum_parameter": 1,
    "filter_slot": 0,
    "filter_flags": 0,
    "required_any": 0,
    "excluded_any": 0,
    "stop_first_raw": 0
  }
}
```

```powershell
.\target\release\goldeneye.exe collision-world-line '.\GoldenEye 007 (USA) (En,Fr).rvz' --snapshot local/world-line.json --output local/world-line-result.json
```

The snapshot reader rejects unknown fields and files larger than one MiB. Line
coordinates, optional constructor displacement and maximum parameter must be
finite. The maximum is retained as supplied, without a hidden clamp to one.
`delta` is a displacement, not a unit direction. The derived parameter point is
`origin + delta * parameter`, with separate multiplication and addition.

`constructor_delta` may be omitted or null. When present, it preserves the
displacement used to construct the original world broad bounds before a caller
changed `delta`. The grenade caller adds its lift after construction. Initial
world bounds therefore use `origin + constructor_delta * 1`, while BSP selection,
segment gates and triangle queries use the current `delta`. Changing the initial
maximum does not rebuild constructor bounds. An accepted mesh hit subsequently
refreshes world bounds using the current displacement and shortened maximum.

`filter_slot` and `filter_flags` use the existing component
[geometry filter](collision-collection-records.md). Flag `0x800` skips placements
after zone geometry; `0x400` bypasses component replacement. `required_any` and
`excluded_any` instead gate raw triangle flags: a nonzero required mask needs at
least one shared bit, then any shared excluded bit rejects. They are not material
IDs. `stop_first_raw` is a byte; every nonzero value enables early stop.

## Typed grenade world line

`GrenadeWorldLineRequest::from_target` prepares the source constructor settings
from a cached apex and target. It first stores `constructor_delta = target - apex`
and its broad bounds, then separately adds `0.1` to displacement Y for any nonzero
small-lift byte, or `1` for zero. It uses maximum one, filter slot one, filter
flags zero, required/excluded masks zero and stop-first byte zero. The arc
dispatcher's world and actor callbacks both retain the original
`constructor_displacement` alongside the lifted displacement.

`grenade-world-line` applies that typed request to the same authored world list.
Save this complete snapshot as `local/grenade-world-line.json`:

```json
{
  "worlds": [
    {
      "resource": "m_dam_gfx.edb",
      "map": 2231369728,
      "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
    }
  ],
  "cached_apex": [-252,8,-8],
  "target": [-242,7,-8],
  "small_lift_raw": 1
}
```

```powershell
.\target\release\goldeneye.exe grenade-world-line '.\GoldenEye 007 (USA) (En,Fr).rvz' --snapshot local/grenade-world-line.json --output local/grenade-world-line-result.json
```

The command reports `typed_request`, `world_query` and a world-only `blocked` result. A world
miss still requires the actor pass before full grenade state acceptance. This
command does not advance action state, activate a body or infer live membership.

## Selection and traversal

The world selector opens no resource for an empty list. A single world is selected
without reading its stored bounds. Otherwise it scans indices one onward for the
first inclusive origin containment and falls back to index zero. Only that
resource is loaded. A missing selected resource or corrupt used input is an error.

The BSP uses the full current segment before geometry shortens the maximum.
Near clipped segments precede deferred far segments; ordinary duplicate zones
are suppressed in first-encounter order. Plane ties use the positive side, and
the source's asymmetric epsilon splitting is retained. The signed halfword scan
also retains its extreme zone-32768 comparison behavior. The sixteenth accepted
zone stops traversal and sets `capacity_reached`, even if no additional zone
would exist. At most 32 far segments may be pending. Stack or work-budget
exhaustion is an explicit host error; it does not silently drop geometry. BSP
zones are not recomputed after a nearer hit. Radius-based portal groups and
shapes are not part of line zone selection.

Each zone visits its geometry reference, then the placement candidates in stored
order. Original root flag `0x100` precedes component filtering. Candidate ordinals
restart per zone; one mutable maximum/result and resident mesh table span all
roots. Candidate hierarchy branches use both broad-bounds overlap and the packed
segment face helper. That helper checks origin containment, then faces in fixed
X/Y/Z minimum/maximum order with its `1e-5` direction threshold. It is distinct
from the segment/AABB SAT helper used for placement, mesh-batch and coarse-leaf
gates. Neither helper is a navigation selected-face segment query.

Zone roots use the original world origin/displacement. Placements subtract the
CURRENT translation and multiply through the CURRENT full matrix transpose.
There is no general-inverse selection, consecutive inverse cache, determinant
test or scale repair in this line path. Working local bounds are refreshed from
that root's coordinates and current maximum.

Geometry `0x603` always checks its parent local AABB, then each child's AABB and
optional line coarse hierarchy before binding the child's optional context. It
dispatches admitted children with flag one. Geometry `0x608` has no parent bounds
gate, skips legitimately unresolved references and dispatches children with flag
zero. Both retain storage order and duplicate references, reload counts at each
loop condition, OR results and stop after a hit only when requested. Geometry
`0x601` always enters its mesh wrapper, whose cache preparation precedes its own
bounds/coarse gates. Unsupported types and used unsupported contexts remain errors.

## Resident mesh query

The line mode-two batch cache is separate from the box-contact cache. It uses
runtime format bit `0x10`, a `0.01` margin and one compact slot per included
batch (`flags & 0x101 == 0`). Enabled triangles (`flags & 0x200 == 0`) contribute
to its extrema. Resident cache preparation/reuse occurs before mesh bounds and
optional coarse-tree rejection. Runtime cache pointers are not serialized asset
offsets. A rejected included batch advances its declared triangle flag count and
the corresponding plane records; excluded batches consume neither stream.

After mesh bounds/coarse admission, the resident plane stream prepares **every
included triangle, including disabled `0x200` triangles**. Its rotating three-slot
strip order, normalization threshold and estimate arithmetic are distinct from
box contact normals. A squared normal at or below f32 epsilon
(`1.1920928955078125e-7`) uses the source plane `[1,0,0,0]`;
no extra area-based triangle rejection is introduced. Unused position-pool slots
stay unread, but malformed XYZ needed by a disabled triangle's plane is an error.

Query-time flag `0x200` rejects before plane intersection. Otherwise the line
tests the plane denominator against `1e-10`, then the closed interval
`0 <= parameter <= current maximum`, then required/excluded flags, then the
three closed oriented triangle edges. Both faces can hit. There is no per-triangle
AABB, barycentric output or extra back-face gate. Query-time XYZ is fetched only
after the earlier gates pass; resident plane preparation may already have read it.

A nearest hit at exactly the existing maximum is accepted and replaces the
earlier result. Thus the last equal-parameter accepted triangle/root wins in
nearest mode. Early-stop mode retains the first accepted hit. Odd strip steps
negate all four stored plane coefficients after acceptance, without repairing
query winding or swapping witnesses.

Zone success copies the local plane into the world-normal result. Placement
success transforms XYZ through the CURRENT placement with input W zero and
normalizes only when its derived length exceeds `1e-5`; shorter normals remain
short. The original plane offset is retained as W. Any-hit early return occurs
before either world-normal conversion, so `world_normal` is intentionally null
when early stop is enabled.

## Output and evidence limits

The top-level loaded-world result reports `selected_world`, `bounds_tested` and
optional `query`. A map query reports `initial_world_bounds`, `zone_binding`, the
final `line`, processed `zones`, per-root `roots` and `resident_meshes`.
`line` contains `maximum_parameter`, refreshed `world_bounds`, optional mesh
`hit`, `world_normal`, derived `parameter_point` and binding
`{zone, placement_index, candidate_ordinal, root_key}`.

Root reports retain working coordinates/local bounds, geometry dispatch counters,
mesh cache/plane/gate probes and child coarse outcomes. Mesh results retain raw
flags/material and triangle, batch, geometry and first-corner identities. These
are resource-local offsets or host indices, not original native pointers, live
residency handles or serialized runtime cache addresses.

Lazy runtime gates do not promise instruction-exact malformed-input reads. Bounded
asset views can validate complete addressed row spans, command metadata or
reachable BSP structure earlier than native traversal would touch each field.
Unused portal inputs stay outside this line adapter; malformed used metadata,
unsupported contexts and budget exhaustion cannot become successful misses.

Authored tests cover order, equal-hit replacement, nonzero early-stop bytes,
cache reuse, plane reads, filters and lazy selection. These are reconstruction
and composition checks. `reference_compared` remains false. Host reciprocal and
reciprocal-square-root seeds substitute for Wii estimates; FP environment,
exceptional values and original numerical parity remain unverified.

Live manager membership/mutation, dynamic/skinned positions and cache invalidation
remain open. The actor-manager entry `80303110`, component virtual slot `0x48`
and actor backend `8027eb20` still require integration. The typed grenade line
constructor is available, while full clearance/action progression remains open. Preserve
world-first, actor-on-world-miss ordering; a static world miss alone never accepts
the full grenade state or activates a body.
