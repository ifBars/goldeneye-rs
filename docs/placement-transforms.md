# Native initial placement transforms

`ge_assets::transform::placement` reconstructs the initial matrix builder used
when map placements become live records. The asset inspection renderer and
initial-placement diagnostics share it. Pure [inverse frames](collision-frames.md)
are implemented separately; collision instance population, live bounds and later
mutations remain open.

Static SJBE52 revision-zero evidence follows initialization `803165d0`: each
serialized 60-byte placement calls `802f3070`, whose matrix is copied into the
first 0x40 bytes of its 0x9c-byte live record. Position, Euler angles and scale
come from offsets +4, +0x14 and +0x20. Euler helper `802a54d0` receives order
code six, dispatched to `802a5e58`.

The matrix is column-major and evaluates column vectors. Rotation is
**Rz × Rx × Ry**. Paired concatenation multiplies diagonal scale on the left,
then translation is installed directly: **T × S × Rz × Rx × Ry**. A quarter-turn
Z rotation with scale `(2,3,4)` maps unit X to Y=3 before translation. Scaling
matrix columns instead produces Y=2 and changes 15 matrices in the current
corpus. Raw angles/scale/flags remain available. Negative and zero scales are
retained, without normalization or reordering.
The difference counter uses `abs(new-old) > 0.000001*max(1,abs(old))` for any
basis component, with the same polynomial on both sides to isolate scale order.

Concatenation rounds the first product, then fuses the remaining three column
terms in order. Point evaluation follows rounded X, fused Y, fused Z, then fused
homogeneous translation. Do not substitute independently rounded products or a
reordered sum. This initial model does not establish navigation frames: the
observed navigation binding still uses only its own explicit translation.
Homogeneous zero-lane products are retained too; replacing them all with positive
zero literals changes signed-zero bits for some angle combinations.

## Euler polynomial

Order six uses the observed paired polynomial rather than host `sin_cos`. It
forms f32 `abs(angle)/pi` using a stored reciprocal, rounds through double
add/subtract around 2^52, and obtains integer parity. Fused reduction against
stored f32 pi, sign/parity handling and a half-step shift provide sine/cosine
arguments. A degree-nine odd polynomial uses the observed rounded square/product
and fused coefficient order. The scalar Rust implementation preserves those
operations and does not renormalize the resulting basis.

Nonfinite inputs, overflow and reductions outside signed-integer support are
explicit host errors. Native exceptional conversions, FP status/rounding modes
and paired denormal behavior remain unverified. Authored tests sweep 8,193 angles
over ±tau against independent host f64 geometry, with absolute sine/cosine error
below 0.000001. That is a geometric check, not original-runtime numeric parity.

## Corpus diagnostics

`scan-edb --decode-collision` prepares every map's initial matrices without
loading render textures or prototypes. Five authored points per record (origin,
three unit axes, and `(1,2,3)`) are compared with independent f64 axis operations:
Y rotation, X rotation, Z rotation, componentwise scale, then translation. The
comparator does not reuse production matrix formulas.

Strict agreement means maximum absolute XYZ error <=0.00001. A larger per-axis
error must fit `0.00001 + 32*f32_epsilon*(abs(position_axis) +
abs(scale_axis)*L1(input)*(1+max_abs_angle))`; it is counted separately as needing
a coordinate/angle-scaled rounding bound. A failed bound fails the resource gate.

The first local run checked 21,059 placements across 366 maps: 1,073 unequal-axis
scales, three negative scales and one zero scale. Of 105,295 probes, 105,007 met
the strict tolerance and 288 needed the rounding bound. Maximum error was
0.000030600 units. Engine flag 0x4 occurred in 1,579 records; that is raw metadata,
not proof of populated collision inverse/bounds. Counts describe directory
entries, not simultaneously resident objects. Visibility, live transforms and
original-reference rendering/gameplay parity remain unverified.
