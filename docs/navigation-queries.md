# Navigation surface query

`ge-core::navigation` contains pure runtime surface queries in this reconstruction.
It implements mesh-local ray and segment queries from static SJBE52 revision 0
evidence. It has no asset I/O, window, graphics or wall-clock dependency. It is
not a pathfinder, player collision system, or complete navigation implementation.

## Observed contract

The operations intentionally differ. Treating every operation as nearest-hit
or giving them one shared endpoint policy changes the observed behavior.

| Operation | Original method / virtual slot | Accepted parameter | Selection |
| --- | --- | --- | --- |
| `first_surface_hit` | `0x802e75f0` / `+0x4c` | `0 < t < maximum` | First accepted storage-order face |
| `nearest_surface_hit` | `0x802e7930` / `+0x50` | `0 <= t < maximum` | Least parameter; equal parameters retain the first face |
| `triangle_segment_hit` | `0x802e7c90` / `+0x54` | `0 <= t <= 1` | Only the selected face |
| `connected_segment_hit` | `0x802e7f70` / `+0x58` | `0 <= t <= 1` per face | Current face, then policy-filtered neighbors depth-first |

Virtual offsets are relative to the navmesh table at `0x80590e98`; the related table
at `0x805910b0` shares these methods. The nearest method tightens its upper bound
only after containment succeeds and continues scanning. It includes the origin,
unlike the storage-first method. The segment method takes an origin and full
displacement, not two endpoints; a parallel or coplanar segment is rejected by the
same plane guard. Its original signed face sentinel `-1` returns false; the checked
Rust adapter instead takes an unsigned face index and reports an out-of-range error.

The local DOL identity is recorded in `triangle-graphs.md`. The query at
`0x802e75f0` calls a plane helper at `0x8036d000` and a containment wrapper at
`0x802ac770`, whose primitive is at `0x803ab324`. The installed disassembler's
paired-single mode was enabled for this investigation; scalar comparison fields
were checked separately where decoding was unavailable. Raw code/disassembly
remain under ignored `local/`, outside the independent implementation and packages.
No original program was executed.

| Rule | Reconstructed behavior |
| --- | --- |
| Ordering | Operation-specific selection in the table above; no sorting or tie replacement |
| Group (rays) | Valid signed ID selects its contiguous range; negative/out-of-range IDs use all triangles |
| Vertex addressing | Low 20 bits of each packed index; upper 12 bits preserved by the asset reader |
| Parameter | Operation-specific bounds in the table above |
| Direction | Used as supplied, without normalization; maximum parameter is not necessarily world distance |
| Sidedness | Accept either winding; no backface rejection |
| Parallel guard | Reject absolute plane-direction dot below `1e-10` |
| Small plane | Squared cross magnitude at most f32 epsilon uses plane `(1, 0, 0, 0)` |
| Containment | Reject positive outward half-spaces; zero is accepted, including edges |

The original ray methods return a triangle index. The independent API additionally
reports the calculated parameter, point and normal as diagnostic/runtime data.
It accepts ordinary 3D directions with homogeneous component zero; the origin
corresponds to a point with homogeneous component one. Nonzero direction components
in the fourth lane and original caller/world-transform conventions are not covered.
The original selected-face operation returns success and writes a point on success;
the independent API represents that result as an optional diagnostic `SurfaceHit`.

## Numerical limits and parity

The original plane helper uses Wii reciprocal-square-root/reciprocal estimates
and floating-point state. The current independent implementation uses ordinary
f32 normalization, with explicit multiply/add boundaries and fused operations
where established. It does **not** claim bit-exact Wii arithmetic. Estimate
differences can affect near-parallel, near-degenerate and triangle-edge cases.
Original FPSCR/quantized-load state and actual query outputs have not been compared.
Finite-input/overflow guards return errors for invalid numerical inputs; original
exceptional-input behavior is not established. These gaps must be resolved before
claiming original-runtime parity.

Static startup analysis establishes the ordinary small-data base `0x806f50a0`
at `0x8000654c` and a zero GQR0 initialization at `0x804b2344`. The main text also
contains a GQR0 restore at `0x804acfe0`; these observations support ordinary float
loads but do not prove every live context's register state. The plane helper refines
a reciprocal-square-root estimate, forms a square-root estimate, then uses a
reciprocal estimate for scaling. Substituting a correctly rounded square root does
not establish equivalent boundary decisions. No exact-estimate claim is made here.

No direct branch to the storage-first method was found in either main text segment;
its address occurs in the two tables above. This does not establish that it is
unused: indirect call and original caller/world-transform tracing remain open.
The neighbor operation at `0x802e7f70` calls the selected-face slot before visiting
neighbors through a policy callback. Its default contract now runs independently;
see [connected-face lookup](navigation-traversal.md) for policy provenance, ordering,
cycle behavior and the explicit host work guard. Actor-specific policies remain open.

Instruction semantics were studied from the [IBM Gekko manual](https://datasheets.chipdb.org/IBM/PowerPC/Gekko/gekko_user_manual.pdf)
and [Dolphin's paired-single implementation](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/PowerPC/Interpreter/Interpreter_Paired.cpp).
These are analysis references; no implementation source was copied. The
[Capstone documentation](https://github.com/capstone-engine/capstone) describes
the disassembly framework; live decoding was checked against the installed build.

## Commands and local checks

```powershell
.\target\release\goldeneye.exe nav-ray '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --origin 0 5 0 --direction 0 -1 0 --max-t 10
.\target\release\goldeneye.exe nav-ray '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --origin 0 5 0 --direction 0 -1 0 --max-t 10 --selection nearest
.\target\release\goldeneye.exe nav-segment '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --triangle 0 --origin 0 5 0 --displacement 0 -10 0
.\target\release\goldeneye.exe nav-connected-segment '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --start-triangle 0 --depth 2 --origin 0 5 0 --displacement 0 -10 0
.\target\release\goldeneye.exe scan-nav '.\GoldenEye 007 (USA) (En,Fr).rvz' --output local/navigation-probe.json
cargo xtask navigation '.\GoldenEye 007 (USA) (En,Fr).rvz'
```

Coordinates are mesh-local. The sample ray is illustrative, not an original spawn.
Use `--index N` instead of a hash for duplicate-hash root entries; `--group N`
selects a group. A null hit means no accepted triangle for that ray/range. Outputs
preserve existing files. Detailed query positions and errors stay local.
`nav-ray` defaults to `--selection first`. `--selection nearest` explicitly chooses
the other original contract. `nav-segment` tests only `--triangle N`; it does not
search the mesh or apply a group. Both segment endpoints are included.

The sampled audit chooses at most 32 ordinary triangle-centroid positions per root
navmesh, offset along the geometric normal, and probes full/valid/invalid group
selection for both ray methods, plus one selected-face segment per position. A
connected-face probe starts at a distinct reciprocal neighbor and allows one
transition toward the sampled face. Samples without a reciprocal neighbor are
counted separately. The audit
independently checks returned points using f64 plane distance and
barycentric containment. Small sample faces are counted separately; fallback-plane
hits are distinguished from ordinary geometry checks. It does not exhaust every
point, group, edge or runtime state.

The observed corpus produced 16,752 queries across 68 navmeshes: 6,282 storage-first,
6,282 nearest, 2,094 selected-face segments and 2,094 connected-face lookups, with no misses,
small samples skipped or fallback hits. Maximum independent plane discrepancy was
`2.532e-5`. The source-bound receipt must be regenerated after code changes;
documentation counts alone are not current proof. Every report explicitly records
`reference_compared: false`; this audit contributes no verified game-parity score.
The short corpus rays did not produce different storage-first/nearest face IDs;
authored stacked-face tests separately exercise that distinction and equal-distance
ties. These samples do not prove global nearest selection for arbitrary geometry.

Authored tests cover storage-order versus nearest selection, group fallback/ranges,
non-unit directions, strict distance-parameter limits, both windings, inclusive
edges, outside points, parallel thresholds, the small-plane fallback and invalid
numbers. Nearest-query tests cover origin inclusion, strict maximum, ties and
outside-face candidates that must not tighten the bound. Selected-face segment
tests cover inclusive endpoints, reverse direction, displacement length and misses.
Connected tests cover current-face priority, ordered depth-first descent, signed
depth bounds, callback order/override, immediate backtracking, longer cycles and
explicit budget exhaustion. Future work includes original numeric precision,
caller transforms, custom actor policies, path selection and agent behavior.
