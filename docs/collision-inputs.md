# Static collision mesh inputs

`Edb::collision_mesh` and `collision_mesh_at` decode mesh-local CPU collision
inputs from root `0x601` entries. Literal hash selection rejects duplicates;
directory selection retains them. No renderer texture/material state is required.
This is not a loaded-world collider, skinned pose, broad phase or contact result.

Local SJBE52 static evidence at `80295860` and `80296160` reads positions through
the relative `+0x5c` pointer, batches through `+0x58`, and per-triangle u16 flags
through `+0x68`. Position records have a 16-byte stride; only XYZ is interpreted.
The fourth word is preserved elsewhere as packed rendering data, not a float W.
The current reader supports the observed Wii corner format `0x203` with its high
format bits retained. Bounds, allocation, count and stream budgets are explicit.

Each batch has a 32-byte header. Flags `&0x101` exclude it from this collision
operation. Excluded batches consume **no** triangle flags and do not require
their corner commands, position pool or flags to be interpreted. Included batches
consume one flag per strip triangle, even for disabled and degenerate triangles.
Flag `0x200` disables that triangle; its raw indices are kept without addressing
their positions. Other bits are retained without inventing surface semantics.

The native loop stops at the declared triangle count. The primitive command word
is retained but not used by this collision consumer; trailing bytes are not read
as extra primitives. Corners have an eight-byte stride. Collision order is always
consecutive strip corners `[k,k+1,k+2]`, including odd triangles. Rendering's odd
winding swap must not be reused. Skinning, per-instance matrices, bounding caches,
batch-bounds pruning and loaded-map selection are separate runtime operations.

The reader keeps sparse decoded positions: enabled triangles resolve and validate
their used XYZ values; unused pool slots stay absent. Disabled triangles return
no vertices. An attempted enabled lookup with missing data is an error, never a
clear-space result. Hash lookup and directory lookup retain their distinct read
contracts. Corrupt used data and unsupported formats fail explicitly.

The separate `Edb::collision_line_mesh_at_address` view prepares positions for
all included triangles, including disabled `0x200` triangles, when the admitted
line mesh needs its resident plane stream. This does not change the ordinary
reader's sparse enabled-only contract. The line wrapper prepares its mode-two
batch bounds before mesh gates, while all-triangle plane reads follow admission.
See [world line queries](world-line-queries.md); do not reuse box cache/normal
rules or treat malformed disabled-triangle XYZ as clear space there.

`scan-edb --decode-collision` audits root mesh entries, and `cargo xtask resources
<disc>` includes it alongside existing metadata/texture/navmesh checks. The local
image has 43,239 decoded root mesh entries: 4,266,516 enabled and 738,390 disabled
triangles in included batches, plus 196,178 declared triangles in 3,957 excluded
batches. Directory entries can share geometry; these are not unique world or
simultaneous-residency counts. The 82 exact-zero-area enabled triangles reported
by f64 diagnostics are retained, not treated as parser corruption or native
contact outcomes. No original-runtime collision comparison has been performed.

The observed box/triangle query dispatch is `802ccdd0` type three to `803006a0`.
That path first tests axes at `802fe2f0`, then constructs contacts at `802ff320`.
A separating-axis candidate alone must not be promoted to a final native contact.
The independent [box–triangle kernel](box-triangle-contacts.md) now implements
ordered candidates, face clipping and unbounded edge-line contacts. The audit
samples authored boxes over decoded inputs. The separate [initialized mesh contact path](mesh-contacts.md)
now supplies static record transforms, pruning and result binding. Live world
integration, skinning, original numeric precision and reference comparisons remain open.
