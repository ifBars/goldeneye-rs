# Repeated player action and callback return

Native New Game now completes both startup animation actions and returns the
initial player callback. The enclosing player constructor is still unfinished.
The game continues to show **Preparing player**, with no gameplay camera or
playable mission.

## A second invocation on the same player

The first action's completed evidence is captured before the repeat changes
live animation heads or history. It remains a historical snapshot. The repeat
has its own mappings, setter calls, command traces and suffix receipt.

The fresh callback requests generic mode `0x09000765` with `f32::MAX`. It resolves
the current inventory again, then maps FP, TP and TP-secondary requests in order.
The native results are `0x09000b70`, `0x09000b70` and `0x09000b73`.

The repeat uses the existing components, controls, scripts, file wrappers and
scratch owner. It does not reset the player or repeat the fresh control capture.
Incoming `f32::MAX` is retained as caller evidence. The supported opcode 2/3/5
class-four paths do not consume that float argument; other command paths remain
explicit dependencies.

## Ordered setters and seek

TP reads the live slot-three animation head's position before its first setter.
The getter accepts animation instances. Snapshot controls fail that type gate
and return source zero without reading their position field.

Both TP setters separately query the actual FP mapped-mode availability. On the
native path they are untracked and use primary history as their previous mode.
Each executes one real set command, initializes and inserts a script, acknowledges
the returned body and returns `0x0900000b` without writing player mode history.

Between those calls, TP writes slot-zero coordinate and seeks its new head to
one quarter of the saved position. A script seek writes its two phase fields,
then performs a real initflag-one update through the retained resource, cache,
rate, selection and position providers. The native seek returns at position zero.
The shared forced-slot override is `1/1` around the second TP setter and is reset
to `0/0` only after that call returns.

FP then directly qualifies through its own file and takes the tracked secondary
setter. It reads secondary mapped history after both TP calls. A nonzero return
stores the requested generic/mapped pair into secondary history, without a
primary-head comparison or primary-history writes. Its command also creates a
script and returns `0x0900000b`.

FP applies a separately rounded, ordered coordinate calculation using retained
controller globals. The repeated suffix re-applies both mask helpers and performs
14 parameter operations on this native path. The extra secondary operations
write FP slot zero and TP slots zero and one. They do not replace child speed.

## Returning the callback

After both action invocations return, the callback executes its final writes:

1. The verified logging callee returns without reading format data or doing I/O.
2. Read the branch byte and clear the derived timer to positive zero.
3. Write handler state 2 on the fresh branch, or 3 on its nonzero branch.
4. Find the actual item's retained physics owner. Clear the specified flag bits,
   then its motion scalar and halfword, in source order.
5. Read handler flags before writing the acknowledgement word and final flag OR.

These operations mutate the existing base, reset and physics owners. They do not
construct replacement physics state. Missing required fields preserve the reached
prefix; an unknown old handler state remains an optional diagnostic and cannot
block its unconditional overwrite.

The native receipt records handler state `0 → 2`, acknowledgement word 1,
physics flags `0x4000 → 0x4000` and handler flags `0x17 → 0x17`. Stores still occur
when the value is unchanged. Only this separate receipt reports callback return;
the two earlier action snapshots keep their original completion scope.

## Remaining constructor work

Callback return does not publish a completed player. Subsequent constructor
selection, cleanup and pose helpers still require implementation, followed by
later loader work, camera setup and gameplay. No mission has become playable.

Synthetic tests cover secondary history ordering, command acknowledgement,
animation type gates, seek fields, scalar rounding, callback branches, field
widths and partial failures. Native probes validate the reached local-image path.
They do not establish complete animation or full-game reference parity.
