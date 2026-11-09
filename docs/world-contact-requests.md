# Initialized world contact requests

`collision-world-box` binds an explicit ordered list of loaded-world snapshots
to the initialized static mesh contact path. The list and each world's stored
bounds are authored inputs. A resource directory is not evidence that its maps
are loaded, and zone bounds do not stand in for manager-stored world bounds.

Save this JSON as `local/world-box.json`:

```json
{
  "worlds": [
    {
      "resource": "m_dam_gfx.edb",
      "map": 2231369728,
      "bounds": { "minimum": [-1000, -1000, -1000], "maximum": [1000, 1000, 1000] }
    }
  ],
  "request_flags_raw": 2,
  "query": {
    "world_origin": [-252, 7, -8],
    "radius": 16,
    "world_from_query": [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1],
    "boxes": [
      { "center": [-252,7,-8], "axes": [[1,0,0],[0,1,0]], "half_extents": [8,8,8] }
    ],
    "filter_slot": 0,
    "filter_flags": 0,
    "record_capacity": 4096,
    "contact_capacity": 1024
  }
}
```

```powershell
.\target\release\goldeneye.exe collision-world-box '.\GoldenEye 007 (USA) (En,Fr).rvz' --snapshot local/world-box.json --output local/world-contacts.json
```

Map and flag words inside JSON are numeric. The example map value is hexadecimal
`85000000` expressed in decimal. Bounds are illustrative, not recovered world
membership. Fields are required and unknown fields are rejected. Snapshots have
a 1 MiB host limit; world counts are bounded to 65,536. Query/box limits follow
[the static mesh snapshot](mesh-contacts.md). Existing outputs are preserved.

The source manager scans world indices one onward for the first inclusive stored
bounds match at the original query origin, otherwise using world zero. A single
world reads no bounds. No worlds means no resource loads and no map query.
Resources belonging to unselected worlds stay unopened. Host errors for selected
resources or addressed geometry are errors, never clear-space results.

The selected map's BSP supplies the initial zone. Request bit `0x2` expands only
that zone's portal groups using the separate query radius. Strict radius ties
reject. Zone order and duplicates remain intact. Other bits do not enable this
expansion. Without bit `0x2`, portal inputs stay unread. In expanded mode, the
initial zone's full raw row span and group sizes receive bounded validation;
shape data and used indices are evaluated only through each group's first hit.
Unused shapes and other zones' portal rows stay unread. Malformed unused row
spans/group metadata can therefore fail earlier than native execution. The
full-map portal inventory remains a separate audit operation.

Each selected zone collects zone geometry and then placements into one record
buffer. Placement candidate ordinals restart at zero per zone. A full record
buffer rejects further appends while traversal continues. The complete ordered
record sequence then uses one inverse-frame cache, resident mesh/cache table and
contact buffer. Consecutive equal owner/slot keys can span zone boundaries;
do not reset the inverse cache or deduplicate zones to obtain different results.

The report exposes selected world, tested bounds indices, BSP/portal counts,
ordered zones, per-zone record ranges and dispatch counters, followed by contact
probes. These identify which operations ran and where capacities truncated work.
They do not establish live world state or original-game parity.

## Grenade ascent binding

`grenade-ascent-box` prepares one analytic arc and its first-ascent box, then uses
the same authored world list. Its snapshot contains `worlds`, `start`, `target`,
`angle_index` and `small_lift_raw`. Save the following as
`local/grenade-ascent.json`:

```json
{
  "worlds": [
    {
      "resource": "m_dam_gfx.edb",
      "map": 2231369728,
      "bounds": { "minimum": [-1000,-1000,-1000], "maximum": [1000,1000,1000] }
    }
  ],
  "start": [-252,7,-8],
  "target": [-242,7,-8],
  "angle_index": 0,
  "small_lift_raw": 1
}
```

```powershell
.\target\release\goldeneye.exe grenade-ascent-box '.\GoldenEye 007 (USA) (En,Fr).rvz' --snapshot local/grenade-ascent.json --output local/grenade-ascent-result.json
```

The typed source request uses one type-three box, identity query frame, the
original start and apex-displacement length for selection, filter slot 1/flags 3,
portal request flag 2, record capacity 64 and contact capacity 512. The box's
padded center remains separate from the selection origin. Short stored axes and
extents are retained. Ordinary analytic rejection performs no world load/query.
For an admitted ascent request, `blocked` follows the resulting contact count.
Capacity diagnostics remain visible; an internally empty result is not a
reference-verified clearance result.

This command covers the first ascent only. The separate
[world line snapshot adapter](world-line-queries.md) now provides initialized static
BSP-segment traversal and mesh line queries. `grenade-world-line` now prepares the
typed cached-apex request, retaining pre-lift constructor bounds, and reports
world-only blocked state. The actor pass, invoked only on a world miss, remains
separate work. Full state
progression, live manager population, dynamic/skinned positions,
mutable bounds/cache invalidation, body activation and physics response remain
incomplete. No ready-to-launch state is inferred from an ascent box miss.
Host trigonometry, reciprocal/sqrt estimates and Wii FP parity remain unverified.
