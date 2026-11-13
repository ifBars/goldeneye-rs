# First-person mode dispatch

The native New Game path now completes FP dispatch and waits for the enclosing
initial-action suffix. It still shows **Preparing player**. This does not complete
the player, camera, pose evaluation or playable mission.

## Receiver and history

FP uses its existing animator, masked headers, skeleton and resource wrapper.
Its resource identity qualifies it directly for the tracked primary setter,
without the separate availability query used to qualify TP. Entry stores the
shared fallback in order and retains its own cursor, previous history and empty
primary-head comparison. Earlier TP capture, command and dispatch receipts remain
historical when this later call changes the shared fallback and player history.

The component bridge selects an exact receiver identity for command preparation,
factory construction and parent effects. It represents FP's already constructed
empty lists from their actual header values. It does not apply TP's four-control
capture or replace FP's masks, mode bytes, flags or weights.

## Mask command and script

The first reached command is opcode 5 with raw mask 3. It allocates a control header,
performs the ordinary empty FP refresh through the shared arena, then captures
the actual FP skeletal locals and auxiliary words. The refresh receipt survives
a later backing failure. Selection and list effects wait for backing return.

The mask helper follows current active count. Here that count is one, so only
slot zero is visited and receives the original control. Slot one stays empty;
there is no clone or active-count increase. Raw mask, selected mask, reached
clear mask and actual insertion count remain separate evidence. A required
nonempty-list destructor remains an explicit dependency on other paths.

The second command starts set 9 and constructs one class 4 script in slot one.
It binds one inline normal clip, a positive range of 76 frames and a 30 FPS animation from
the same file generation. Position zero skips root motion. The previously empty
slot needs no old-head copy. Insertion raises active count to two; actual flags
skip cleanup. Only its real return lets the mode cursor acknowledge that command.

## Dispatch return and remaining work

The cursor returns 0x0900000b after both commands. The tracked setter writes the
requested generic and mapped modes, both 0x0900071e, before reading the new primary
head. Its identity changed from NULL to a captured control, whose comparison
word is zero, so secondary history is cleared. The slot 1 coordinate helper
then uses the actual reset input and FP dispatch returns.

The report retains the earlier mask receipt even when the latest command is the
set. Its prior active count of one must not be replaced with the later count of
two. Hashes include private control backing and typed node links without exporting
pose arrays. Authored tests cover mask bounds, inactive bits, independent clones,
partial allocations, refresh/backing order and acknowledgement guards. Native
probes check the reached image path; original timing and numerical parity remain
unverified.

Next, the enclosing callback must execute its real packed animation-mask lookups
and FP/TP parameter writes before returning. Later player completion, frame
scheduling, camera setup and mission execution remain required.
