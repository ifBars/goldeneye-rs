# Initial action masks and parameters

The native fresh New Game path completes the first action invocation's mask and
parameter suffix. This is part of player initialization. It creates no gameplay
camera and does not finish the enclosing player callback.

## Mask lookup and application

The caller classifies the retained primary mode. The fresh mode `0x0900071e`
selects state 1. Secondary history is read for classification only when the
primary mode does not select a specific state.

FP runs before TP. Each helper finds its receiver through the actual item's
component list and bound file identity. The animator's current resource reference
can now name a script, so it cannot substitute for that file lookup.

Each helper resolves animation A (`0x03000147`), binds its body and converts its
mask before requesting animation B (`0x03000148`). Both conversions precede header
writes. Missing descriptors and missing bodies return without mask writes.
Nonresident data remains pending. A missing kind-three attribute successfully
produces four zero words and still permits the header writes.

The asset reader follows two signed packed offsets. The first is relative to
the attribute field; the second is relative to its table entry. A zero first
offset is an invalid required table. A zero second offset addresses the entry
itself. The reader touches only reached fields in the logically resident base.
It does not require FPS, frame decoding or unrelated attributes.

The independently authored runtime applies these verified slot mappings:

| State | FP slots 0, 1, 2, 3 | TP slots 0, 1, 2, 3 |
| --- | --- | --- |
| 0 | A, A, A, unchanged | A, A, A, A |
| 1 | A, B, A, unchanged | A, A, B, B |
| 2 | A, B, A, unchanged | A, A XOR B, B, B |
| 3 | A, B, A, unchanged | A, A, B, zero |

Each write reads the destination flags first, copies the four mask words, then
stores the flags with numeric 2 set. Inactive physical slots participate. Active
count, header mode, weights and
node membership do not change. Raw animator headers and typed layer state retain
the same masks and flags. Capture/configuration receipts retain their earlier
flags independently of those later mutations.

## Parameter writes

The fresh path re-finds FP, writes its common speed to one, repeats the slot-one
coordinate helper and writes slot-one weight to one. Secondary mode zero skips
the special FP speed branch.

TP then re-finds its receiver and writes common speed to one. Its non-NULL
slot-zero head permits two blend weights and two child-speed calls. The weight
comparisons preserve NaN and signed zero; replacing them with ordinary `min` and
`max` calls would change those cases. The fresh zero blend writes weights zero
and zero. The paired speed helper uses supplied bases 30 and 20 and writes child
speeds one and `20/30`, with each arithmetic operation rounded separately.

The speed call supports both Control and Script heads. A Control stores its
common speed independently of its rate field. A Script synchronizes direction
history before storing the new speed when its old and new signs differ. These
calls do not update the script, resolve FPS or sample a pose.

TP repeats coordinate helpers for slots two and three, then writes slot-one
weight from the retained player field. The native first invocation performs 11
parameter operations, all of which write. Other parameter branches retain an
explicit dependency where required providers or behavior remain unavailable.

## The next action

The first action invocation returns to the enclosing player callback. Four
secondary-mode predicates reject the current zero history. The actual alternate
flag then selects a second action request: fresh zero selects `0x09000765`, with
parameter bits `0x7f7fffff` (`f32::MAX`). A nonzero alternate flag selects
`0x090007a7`.

The first invocation retains that required request separately. The
[repeated-action path](repeated-player-action.md) now executes the second dispatch
and its suffix on the same owners, then returns through the callback's final
state writes. Subsequent constructor and pose work remain incomplete, so these
returns do not advance player or camera readiness.

## Evidence boundaries

Synthetic tests exercise packed-pointer errors, signed offsets, zero-offset
semantics, all mask mappings, early returns, pending-body resumption, partial
write failures and speed-history ordering. Native probes use the user's local
game image. Public reports contain counts, directory indices and state hashes;
mask words and game payloads remain local.

Builds and synthetic tests establish checked behavior for those inputs. They do
not establish full animation parity, playable multiplayer, a complete player or
a complete game.
