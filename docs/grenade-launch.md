# Grenade launch preparation

`ge-core::grenade` prepares a pure launch packet and angular damping stage.
`grenade-launch` resolves its local script descriptor and runs authored records
through those helpers. It does not spawn an entity, allocate/register
a body, integrate position, detect collisions, or implement lifetime/expiry.
Authored tests are not original-runtime comparisons.

Local static SJBE52 rev0 evidence at `8007d610` identifies the record consumer as
grenade launching: resource `0x0109000f`, coded script reference `0x04000d8e`, and
an `XGrenade` type descriptor precede component initialization at `80060f00`.
The action's stored motion is a grenade linear-velocity input, not evidence of
actor locomotion. Resource/type/component resolution must succeed before packet
preparation. The source's missing-result path may dereference null; a future host
adapter must report an error, never pretend successful spawning. Asset descriptor
lookup is separate from actual runtime typed-component resolution.

The loader at `8027fe80` uses directory `+0x68` with 16-byte descriptors (table
five, named `animation_scripts`), not the 20-byte table-two geometry entity
directory. On the local image this resolves script slot zero. The resource audit
checks this descriptor separately; no script body is interpreted or executed.

The constructor at `8007cf30` clears five position/motion records. Location indices
0..7 map to records `[0,0,1,1,2,2,3,0]`; source kind `0x52000014` instead selects
record four without consulting location. This selector is independent of the
action's active index. Position and motion are world values, with no frame add.
Live bone/animation setup remains open. The dispatcher at `8007d5e0` calls the
target handler for state zero, the analytic handler for state one, and otherwise
returns four. `nav-actor-action` probes a direct handler.

Record one consumes zero RNG draws: yaw is `atan2(motion.x,motion.z)`, Euler Y-X-Z
angles are `[0,yaw,-pi/2]`, spin is `[0,|motion|/0.2,0]`, damping five. Full 3D
length matters and zero motion is permitted. Other records consume eight draws
from a wrapping u32 LCG (multiplier 1664525, increment 1013904223). Unit values
are `f32(state >> 1) * 2^-31`, which can round to one; never clamp them. Axis draws
are Z,Y,X, each remapped with separate operations `-0.9 + 1.8*u`, then biased
by +0.1 for nonnegative values or -0.1 otherwise. Normalize only above `1e-5`.
Draw four selects an axis-angle in `[0,tau]`; draws five through seven construct
the spin axis, draw eight its rate `pi/2 + (3*pi/2)*u`. Damping is twenty.
No quaternion normalization or sign canonicalization follows.

The update at `800614c0` proves component `0x3c4` is angular damping, not linear
drag or a fuse. Only body flag `0x08` enables it: round `time_scale*clock_delta`
to f32, multiply by negative rate, round double exp to f32, multiply each angular
component separately. Flag `0x10` alone does not damp. Surrounding sweep sizing,
counter, grid and lifetime updates remain unimplemented.

Host trig, sqrt and exp remain provisional relative to engine math and Wii FP
state. Used nonfinite values, bounds and overflow return errors atomically for
the passed RNG/spin state. Native exceptional behavior and intermediate RNG
commit timing are not reproduced by that host error contract.

## Authored launch command

```powershell
.\target\release\goldeneye.exe grenade-launch '<your-disc>' --snapshot local/grenade.json --output local/grenade-result.json
```

The strict JSON schema is:

```json
{
  "source": { "entity_kind_raw": 0, "location": 0 },
  "records": [ { "position": [0, 2, 3], "motion": [3, 4, 0] } ],
  "rng_word": 0,
  "body_flags_raw": 8,
  "steps": [ { "clock_delta": 0.016, "time_scale": 1 } ]
}
```

This one-record fixture selects record zero. Native live setup uses five records;
provide all five for other source locations. `steps` is optional and limited to
256 authored angular-only updates. Input size is limited to 1 MiB, unknown fields
are rejected, existing outputs are preserved. JSON words are decimal unsigned
integers. Output identifies `spawned: false` and separates descriptor metadata,
launch preparation, RNG state and spin steps.

## State-one analytic action

`ge-core::grenade_arc` implements candidate math and the owning dispatcher. It
tests angles in order 45, 55, 35, 65, 25 degrees, with target Y lifted by 0.1 for
any nonzero small-lift byte or by one otherwise. It preserves separate f32
multiply/divide boundaries, rejects tangent-minus-elevation <=0.001, and rejects
an apex at or beyond horizontal arrival time. Normal failures retain intermediate
displacement or solved motion already written to the selected slot. Host zero
horizontal distances and overflow remain explicit unsupported errors.

Clearance is two-stage: an accepted ascent-box query caches the world apex,
clears the prepare flag and returns zero. A subsequent line query starts at that
cached apex and adds the current lift to target-minus-apex Y. A clear result
writes raw state three and returns three. The backend must preserve the original
loaded-world-first, actor-list-if-clear ordering for that line stage. The source
calls `8032e700` for the loaded world, then `8027eb20` only on a miss. The latter
walks the actor list without an excluded actor; do not implicitly ignore the owner.

A blocked query or normal candidate rejection increments the angle index. Below
five it sets the prepare flag. At five, a nonzero small-lift byte becomes zero
and the index resets, preserving the prepare flag even if it was zero in the
line stage. Exhausting the larger lift returns one without changing raw action
state. A rejected angular gate addresses neither records nor the backend. Other
raw dispatcher states return four without consulting either callback.

`GrenadeArc::clearance_volume` now reconstructs that first-stage type-three
oriented box, independently identified by the axis accessor `800bca50`, extent
accessor `802f7a90`, and triangle shape dispatch `802ccdd0` to `803006a0`. It is
not a curved primitive. The callback receives both the arc and the constructed
volume. Axis zero follows the start-to-apex chord; axis one points toward the
parabolic bulge. Axis two is the stored axes' cross product, without normalization.

The 0.3 padding is asymmetric: near faces remain at the start and chord. Long
extent is `0.3 + chord_length/2`, normal extent is
`(midpoint_sagitta_allowance + 0.3)/2`, and depth extent is 0.3. The center adds the long-axis offset
then the normal-axis offset in separate f32 operations. Axis-zero normalization
fuses Z into X before adding Y and occurs only for squared length >`1e-7`; the
separate chord length uses rounded X/Y sum then fused Z. Short axes stay short.
The perpendicular uses the captured horizontal apex distance with separate
multiply/reciprocal stages. Host estimates remain provisional.

Loaded-world selection separately receives the original start and apex distance,
not a fitted sphere around the padded box. [Static collision mesh inputs](collision-inputs.md)
now decode independently of rendering. Initialized static instance transforms
and the [authored mesh contact consumer](mesh-contacts.md) are available. Live
manager population, skinning and actor line integration remain open. The independently authored
[box/triangle contact kernel](box-triangle-contacts.md) is now available, with
ordered axis candidates, face clipping and unbounded edge lines. Its sampled
mesh-local checks alone do not supply a loaded-world grenade clearance backend.
The separate [typed first-ascent adapter](world-contact-requests.md), exposed by
`grenade-ascent-box`, now binds an analytic ascent to an explicit authored world
list, BSP/lazy portal selection and static mesh contacts. It preserves identity
query frame, original start/apex-distance selection, filter slot 1/flags 3, portal
flag 2 and source record/contact capacities 64/512. It reports only whether that
admitted first-ascent query produced contacts; it does not advance full arc/action
state, test the subsequent grenade world/actor line or establish readiness to launch.
The separate [world line snapshot command](world-line-queries.md) now reconstructs
the initialized static world segment path, preserving pre-lift constructor bounds,
later displacement and nearest-hit shortening. `GrenadeWorldLineRequest::from_target`
now preserves constructor displacement/bounds before adding the lift. Both line
callbacks carry that original constructor displacement. `grenade-world-line`
queries this typed request against the static world only. The
actor-manager/component backend remains separate work; a static world miss alone
does not accept the action state.
The API requires a fallible callback and never assumes an empty world or turns
an unavailable backend into a pass. Authored callbacks verify scheduling only.
No original game instance, live collision, bone setup, or body tick is proved.
