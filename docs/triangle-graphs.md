# 0x607 navigation mesh data

The independent reader decodes triangle/adjacency data from Wii SJBE52 revision 0
EDB v263 assets. Local static type-registration and descriptor evidence identifies
the class as `EXGeoNavMeshEntity`. Links join shared triangle edges, are reciprocal,
and carry centroid-distance weights where weights exist. **Original traversal and
gameplay behavior remain unverified.** Static path contracts have independent
implementations; collision rules, agent clearance and dynamic blockers remain open. Scene loading retains these
placements as navigation data, separate from GPU geometry.

## Static identity evidence

The supplied SJBE52 revision 0 DOL is 6,087,648 bytes, SHA-256
`0d101e5eba1aca38847c03d7a353f5156f3ec2d9025d6c6773d105561532208b`.
The following address chain was inspected in that local artifact:

| Observation | Address / relationship |
| --- | --- |
| Serialized type registration | Record `0x806e9ef8`: fixup/construction entry `0x802e26e0`, type `0x607` |
| Installed virtual table | `0x80590e98` |
| Type-descriptor getter | `0x802e3390` returns descriptor `0x805501f8` |
| Descriptor name | Pointer `0x805501e0` identifies `EXGeoNavMeshEntity` |
| Vertex count | Getter `0x802e3380` reads body field `0x6c` |
| Geometry-processing slot | Virtual offset `0x18` targets an empty-return function at `0x80006800` |
| Group range consumer | `0x802e76b0`–`0x802e76c8` reads 24-bit counts and 20-bit starts |
| Typed navmesh accessor | `0x801857d0` checks that descriptor chain; two direct callers observed |

This establishes type identity and supports keeping navmesh data out of the mesh
draw buffers. It does not establish original path choice, debug drawing, collision
use or runtime query results. Generic PowerPC disassembly does not correctly model
Wii paired-single arithmetic, so arithmetic instructions were not treated as
verified behavioral evidence. No original program was executed. Raw code and
disassembly stay under ignored `local/`; none is included in the independent
implementation, fixtures or packages.

Follow-up analysis enabled paired-single decoding and established the first
[surface-ray query contract](navigation-queries.md). Hardware estimate precision
and original execution comparisons remain open; the initial generic-decoder
limitation above describes the type-identity investigation.

## Reader and commands

`ge-assets::graph` owns the checked data reader. `Edb::entities()` exposes root
entity descriptors (20-byte records). `entity_graph(hash)` requires a unique hash;
`entity_graph_at(index)` resolves an exact directory entry. The source contains
legitimate duplicate hashes, so an audit must not silently select the first.

```powershell
.\target\release\goldeneye.exe entity-graph '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --output local/dam-graph.json
.\target\release\goldeneye.exe entity-graph '.\GoldenEye 007 (USA) (En,Fr).rvz' m_ai_squads.edb --index 1 --output local/squad-graph.json
.\target\release\goldeneye.exe scan-edb '.\GoldenEye 007 (USA) (En,Fr).rvz' --decode-graphs --output local/graph-audit.json
cargo xtask resources '.\GoldenEye 007 (USA) (En,Fr).rvz'
```

Detailed graph coordinates and raw records remain local. The public dashboard
receives counts and diagnostics only. No original game code is executed.

## Observed layout

Offsets below are relative to the entity body; pointer values are signed offsets
relative to the pointer field. The first 0xc0 bytes are retained as raw words.

| Field | Offset | Layout |
| --- | --- | --- |
| Type | `0x00` | `0x607` |
| Stored local AABB minimum / maximum | `0x10` / `0x20` | two vec3 values at 16-byte stride; finite and ordered, not recalculated from vertices |
| Vertex count | `0x6c` | u32 |
| Triangle count | `0x70` | u32 |
| Group count | `0x74` | u32 |
| Vertices | `0x78` | Pointer to count × three f32 positions |
| Triangles | `0x7c` | Three packed u32 indices (low 20 bits, upper 12 preserved) + raw attribute word |
| Neighbors | `0x80` | Three packed u32 indices per triangle; low 20-bit `0xfffff` means absent; upper 12 bits preserved |
| Groups | `0x84` | Pointer to eight-byte tagged count/start records |
| Centers | `0x88` | Optional pointer to three f32 values per triangle |
| Stored face vectors | `0x8c` | Optional pointer to three f32 values per triangle; actor edge callback consults destination Y |
| Weights | `0x90` | Optional pointer to three f32 values per triangle |
| Face flags | `0x94` | Optional pointer to one raw u32 per triangle; bit zero rejects an interior shortcut |

A zero center/vector/weight/flag pointer means absent data. The reader retains `None`, without
manufacturing stored values. Each group packs a raw eight-bit tag in the high byte
and a 24-bit triangle count in its first word. The next word stores the first
triangle index in its low 20 bits; its high 12 bits are preserved separately.
Group ranges must partition the triangles contiguously. Tag/high-bit semantics are unknown.

Counts and byte ranges are bounded before allocation; indices, finite
floats, nonnegative weights and group ranges are checked. Geometric inconsistencies
are reported separately: degenerate triangles, nonreciprocal links, links lacking a
shared edge, self links, neighbor entries after a boundary, connected components,
and maximum centroid/distance discrepancies. The reader preserves entries after a
boundary; the observed default lookup stops at that first boundary instead of
visiting later entries. It must not reorder/compact the stored adjacency records.
Self links are preserved and reported rather than rejected: the observed default
lookup uses depth bounds and immediate-backtracking rejection, not a ban on self links.
See [connected-face lookup](navigation-traversal.md). Upper neighbor-bit semantics
remain unresolved and are not treated as addresses or traversal flags.
Sample counts identify which stored values were available for those comparisons.
Unknown header fields and data reached through other pointers remain unresolved.
The flag reader preserves every bit and reports 59,906 stored records, including
1,095 faces with bit zero set. The actor edge filter rejects destination flags
`& 3 != 0`; other bits remain unresolved. See [waypoint flag use](navigation-waypoints.md)
and [actor edge rules](actor-target-edges.md). The optional vector reader preserves
2,805 raw vectors, including 129 with Y below f32 0.9. It does not normalize or
reconstruct absent vectors from geometry.

## Local evidence

The main archive contains 68 root graph entries across 903 EDB resources. All 68
decode: 48,133 vertices, 61,501 triangles, 147,062 directed links and 37,441 boundary
slots across 318 connected components. There are no nonreciprocal links, links
without shared edges, or degenerate triangles in this observed corpus.

Stored centroids were compared for 61,322 triangles: maximum coordinate difference
`4.070e-5`. Stored weights were compared for 144,522 links: maximum distance
difference `1.819e-6`. These comparisons validate the layout interpretation for
the observed data; they do not compare against original runtime behavior.

The Dam graph has 668 vertices, 960 triangles, 2,484 directed links, 396 boundary
slots and one connected component. It is the previously unsupported placement
167. Strict Dam scene loading now decodes all 241 placements: 240 render instances
and one navigation instance. Shared navmesh arrays are retained without GPU draws,
extra render triangles or render-bound changes. Raw placement/model fields remain
available; original transform and traversal behavior are not thereby verified.

Synthetic tests cover truncation, malformed indices/counts/pointers/floats, absent
arrays, packed multi-group ranges, duplicate-hash selection and diagnostics.
Placement integration tests cover shared navmesh data, unchanged GPU counts/bounds,
strict success and corruption rejection even in explicitly incomplete mode.
No game-derived fixtures are committed. `cargo xtask resources` regenerates the
source-bound local corpus receipt; documentation counts alone are not a fresh pass.
