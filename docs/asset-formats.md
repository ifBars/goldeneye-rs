# Asset format observations

Observed image: SJBE52 revision 0, RVZ; title `James Bond: Golden Eye`. Data
partition: 46 files, 3,802,534,697 file bytes. Original image is unchanged.

| Index | Entries | Observed role |
| --- | ---: | --- |
| Filelist.bin | 919 | 903 EDB resources plus other data |
| File_COM.bin | 1,326 | Common audio; some hash-only entries |
| File_USA.bin | 5,060 | US audio/data |
| File_FRE.bin | 3,585 | French audio/data |
| File_SPA.bin | 3,585 | Spanish audio/data |

These are entries, not necessarily unique resources; duplicate storage locations
reduce optical seek times. Every indexed base range fits its companion `.000`.

## EngineX Filelist v7 (big-endian)

Header: version u32, total index size u32, count u32, packed build flag u16,
archive-count field u16, signed self-relative name-table pointer i32. The observed
archive-count field is zero; it is not used to reject the present archive.

Each entry: base length u32, hash u32, resource version u32, flags u32, location
count u32, then that many `(offset u32, container number u32)` pairs. Name table:
one self-relative i32 pointer per entry. Strings decode bytewise modulo 256 by
adding 0x16 and subtracting the entry index and character index. A decoded zero
terminates the name; empty names are valid for hash-addressed audio.

Virtual paths like `x:\bondx\binary\...` describe the original build layout, not
host paths. Never use them directly for filesystem writes. The parser does not
extract files, execute code, or infer resource types from names alone.

**EDB length caveat:** the index length may cover only a base section. A full EDB
reader must validate its own header/section lengths; index validation is not full
resource validation. Most main EDB entries report version 263; one bundled test
resource uses 262. Their observed metadata layout is shared.

## EDB metadata and texture loading

Big-endian `GEOM`: resource hash at 0x04, version at 0x08, flags at 0x0c, raw
timestamp at 0x10, declared payload size at 0x14 and base size at 0x18. Six
platform words at 0x1c and three reserved words at 0x34 are preserved. Some
platform words point at a relocation trailer beyond the declared payload; we do
not treat that trailer as decoded resource data or execute/relocate disc code.

Sixteen 8-byte directory descriptors start at 0x40: signed count i16, unknown
hash-table-size i16, and signed self-relative pointer i32. Counts and descriptor
targets are checked. **Unknown section record layouts/bodies are not validated**
merely because their descriptor is in range. Texture records (descriptor 15 at
0xb8) are 28 bytes: hash u32, section/debug u16, absolute body address u32, runtime
pointer u32, logical width/height u16, game flags u32, flags u32.

Texture bodies contain dimensions/depth, flags, scroll, frame/image counts,
framerate, mip count, EngineX format, color and external-file hash. For these
versions, 0xffffffff means local; local image pointers at 0x2c are self-relative.
An external record stores **one target texture hash** there, even when its image
count describes a longer animation (observed count 20). The loader follows
file/texture hashes, rejects cycles, limits depth to 16, and checks dimensions.
The full external graph has not yet been audited against original runtime use.

Payloads are checked against the next physical resource/container boundary. One
character EDB declares 95,718,048 payload bytes while its base is only 463,904.
The loader reads 16 MiB chunks with a 256 MiB resource cap and never substitutes
the indexed base length for the declared payload length.

## Wii textures

Local images are TPL containers (`0x0020af30`), including sampler state, storage
dimensions, GX format and image/palette offsets. All mip byte ranges are checked
using padded GX tile sizes. The API decodes I4, I8, IA4, IA8, RGB565, RGB5A3,
RGBA8, CI4, CI8, C14X2 and CMPR; C14X2 currently has synthetic-test evidence only.
Palette formats IA8/RGB565/RGB5A3 are supported.

GX storage uses tile rows, AR/GB planes for RGBA8 and 8x8 macroblocks containing
four CMPR subblocks. CMPR uses big-endian endpoints/selectors and GX's 5/8–3/8
interpolation. The transparent selector retains the endpoint-average RGB with
zero alpha. I4/I8 replicate intensity into alpha as well as RGB; material/channel
operations remain a future rendering responsibility.

EDB logical sizes sometimes differ from TPL storage sizes: e.g. 257x512 stored
as 512x512, or 2x2 stored as 4x4. The reader preserves both. TPL decoding returns
the stored image; EDB PNG export crops to the logical extent at each mip. Future
rendering must establish UV/sampler behavior from geometry/material evidence.

Observed local audit: 903 EDB metadata records, 17,756 texture records, 18,711
local images and 124,164 decoded mip images, with 1,541 external references
counted separately. No unknown resource-body or original-reference parity claim
is implied. Regenerate source-bound evidence with `cargo xtask resources <disc>`.

## Static map geometry (observed Wii v263)

The map directory uses 16-byte common records, with an absolute body address at
+8. Map body type 0x500 has a zone count at 0x84 and 0x8c-byte zone records at
0x88. Each zone starts with a reference-table index, an identifier relative
pointer, and has bounds at +0x68. The reference table also uses common records;
local entries have hash zero and an absolute entity address at +8. Other forms
are rejected until implemented.

Entity types encountered in the Dam's static zone graph:

- 0x608: zone link, following another reference index at +0x58.
- 0x603: split, child count at +0x54 and relative child pointers at +0x5c.
- 0x601: mesh, using separate position/UV/color pools and indexed strips.

Traversal checks cycles, sharing, depth and node budgets. Unsupported types fail
explicitly. Type 0x607 occurs in the root entity directory but is not traversed by
this static-zone path. Static factory/type metadata identifies it as navigation
mesh data. Its arrays decode separately; original traversal behavior and remaining
body fields are unresolved. No actor/script behavior is run.

For Wii mesh type 0x601, the relevant relative pointers are +0x54 material remap,
+0x58 strip batches, +0x5c positions, +0x60 UVs and +0x64 colors. Batch count is
at +0xa4, position count +0xa8, and the raw index-format/UV-scale word +0xb0.
Only the observed low-bit format 0x203 is accepted. UVs are signed i16 pairs
scaled by 2^(16 - ((word >> 28) & 7)). Positions occupy 16 bytes: three floats
and one preserved packed-normal/unknown word. Normal indices remain preserved;
normal decoding and material lighting are not implemented.

Each batch has a 32-byte header and a declared stream byte length. The observed
stream uses 16-bit 0x0098 commands, a corner count, then four u16 indices per
corner: position, normal/unknown, color and UV. Position/UV/color indices are
independent. Zero padding must remain zero; unknown commands are rejected.
Strips become alternating-winding triangle lists without dropping degenerates;
decoded counts must match each batch's declared triangle count.

Batch texture IDs address the EDB texture table directly, optionally after a
per-mesh u16 remap (entity flag bit 0). The Dam has 104 textures and 103 separate
material-reference records; a placed prop uses valid texture index 103. This
corrects the earlier preview's mistaken material-reference indirection. A synthetic
regression covers unequal table lengths and nonidentity references.

Material-reference common records point to observed 12-byte bodies: source file
hash (0xffffffff local), texture index and an unknown word. Reference 74 targets
texture 75, but this is not a remap for batch 74. References are retained separately
as `MaterialReference`; their full runtime purpose remains unresolved. Batch
flags/transparency and unknown words are retained for later state decoding.

The tested Dam map resolves 20 zone links and 20 splits into 132 mesh leaves,
69,368 expanded corners, 51,808 triangles and 381 batches. Nine material images
are used by this static graph. Skies, effects, collision, actors and draw-state
parity remain separate work.

## Map placements (observed Wii v263)

Map fields +0x48/+0x4c provide the placement count and relative record pointer.
Each 60-byte record preserves hash (+0), position (+4), flags (+16), Euler rotation
(+20), scale (+32), engine flags (+44), map-on (+46), object hash (+48), light set
(+52), signed group (+54), and unknown word (+56). Object hashes resolve against
the root entity directory, not the zone reference table.

The Dam has 241 placements. Strict `--placements` loading decodes all records:
240 resolve to mesh/split prototypes; record 167 resolves to a 0x607 navigation
mesh retained separately. `placements_loaded` counts both kinds; `render_placements`
and `navigation_placements` distinguish them. Navmesh instances retain model/status
records and share graph data, without adding GPU triangles or changing render
bounds. Corrupt known navmeshes and missing hashes still fail even in explicitly
incomplete mode. No original navigation behavior, collision or actor logic is implemented.

See [triangle graph layouts and diagnostics](triangle-graphs.md) for the separate
0x607 reader, static type-identity evidence and full root-directory corpus audit.
Static classification does not establish original navigation/traversal behavior.

Prototype vertices/indices are decoded once per object hash and reused by draws
with model matrices. Initial T * S * Rz * Rx * Ry and its Euler polynomial now
follow static native builder evidence; see [placement transforms](placement-transforms.md).
Original-runtime rendering, later mutation and visibility parity remain unverified.
Raw flags, angles and scale remain available. This path submits 213,085 triangles
in 1,996 draws from 135,003 unique buffer triangles, 177,811 corners and 91 textures.
These counts include the static zones and supported placements, not a complete level.

## Native inspection rendering

`ge-render` uploads decoded corner attributes, triangle indices and padded TPL
RGBA images through wgpu. The present WGSL shader samples the texture and
multiplies by vertex color. It uses a depth buffer, alpha blending/discard and
disabled culling. Sampler wrap modes are read from TPL, while exact filter/mip,
GX/TEV, lighting, fog and transparency ordering behavior remain pending.

The local RTX 3060 Ti/Vulkan run confirms texture sampling, depth occlusion,
alpha discard, shared geometry with distinct instance transforms, and padded
readback with a synthetic GPU regression, separately
from a real-map render. No original-reference image comparator has run.

The [native inspection window](native-viewer.md) and PNG path share a persistent
`GpuScene` encoder. View/projection and model uniforms are separate so camera
changes do not recreate model/geometry buffers. Decoded textures always upload as
RGBA8; BGRA swapchain targets are handled independently, with normalized RGBA
readback. The synthetic GPU check exercises that channel-order distinction.

The fitted overview includes a large black surface with material 0, batch flags
0x190 and geometry extending down to y=-224.631. Its original draw/cull treatment
is unresolved. It is preserved in previews rather than hidden to improve their
appearance. The documented close camera is an inspection viewpoint, not an
inferred original spawn/cutscene camera.

## Format references

- [nod API](https://docs.rs/nod/1.4.4/nod/)
- [Dolphin RVZ specification](https://github.com/dolphin-emu/dolphin/blob/master/docs/WiaAndRvz.md)
- [EngineXFilelistUtil](https://github.com/BrinchEbsen/EngineXFilelistUtil), Filelist reader
- [Swyter's Filelist description](https://github.com/Swyter/sphinxtools/blob/master/sphinx_filelistbin.bt), earlier versions
- [Eurochef metadata descriptions](https://github.com/eurotools/eurochef/tree/main/eurochef-edb/src), independently studied layout references
- [Dolphin GX format reference](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/VideoCommon/TextureDecoder_Generic.cpp), hardware color/tile behavior; no source copied
