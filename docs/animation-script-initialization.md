# Live animation-script initialization

New Game retains its first class-four animation child in the TP animator. The
initializer now selects a play command and constructs its clip references. The
native Tutorial probe visits four command headers, selects offset zero and binds
seven normal clips from the same resource wrapper. Clip setup returns with
index 6 and a 30-frame range. Animation-body lookup and position preparation are
next. The initializer, factory and command have not returned; the player and
camera remain incomplete.

These operations are independently authored from static investigation of the
supported SJBE52 revision 0 image. Synthetic tests verify the reconstructed
rules. The local native probe verifies that our program reaches this boundary
using user-supplied data. Neither establishes original runtime timing or
numerical parity. Private binary analysis and probe captures stay under ignored
`local/`; they are not distribution inputs.

## Owners and reads

`ge-core/animation/script_child/initialization` owns the pure selection and
clip-setup rules. It uses the same header allocated by the pending command;
continuation does not allocate another child or repeat the metadata cache step.
The player bridge supplies the actual animator speed. The app supplies the
controller clock's last host frame duration scaled by the source baseline of 60.
The clock survives cancellation, while stale attempts cannot read it.

`ge-assets/frontend_script/live` supplies bounded scalar reads. The app's source
view borrows the same logically resident base and retained file wrapper. It
does not use the inspection reader that copies a whole command or preflights
every clip payload.

The fresh inline clip has no animation resource and its rate getter returns one.
Direction changes update the child sign and history before duration or command
reads. Script duration is signed. Selection visits storage order and remembers
the last play command at or before the current frame, stopping at the first
future play or END. Other command payloads stay unread, including unknown
opcodes. The native next-byte read occurs even on the stopping record; its
target remains unread. Host traversal budgets reject nonadvancing/malformed
streams without replaying the retained prefix. Initialization skips event
callbacks and does not read their script mask.

## Clip setup

The caller reads the first normal animation reference before clip counts, then
stores the selected command offset. Total requested count zero or one uses the
header's inline clip storage. Larger counts allocate exactly that many physical
records. Heap records start unknown; the source does not initialize their cached
pointer field. Inline fields retain constructor values until actually written.

A local-file sentinel reuses the existing wrapper. A confirmed missing external
file skips that row without reading its other fields. An unavailable external
registry provider remains pending. Successful rows pack together in normal then
secondary order; secondary source addressing uses the raw normal count, not the
successful count. Each write survives failure of the next scalar read.

After those loops, the helper clamps the initial index to the successful normal
count itself. It may therefore select a secondary or unwritten physical record.
Unknown storage remains an explicit dependency. A nonpositive signed range
requires the selected animation's frame-count provider. A positive range writes
the frame count and returns clip setup. The caller then overwrites the index
with the raw serialized byte and applies the play flag to child bit `0x80`.
The next animation lookup and position operations remain required.

The dashboard distinguishes file lookup, unknown clip range, frame-count lookup
and animation preparation. A script with no selected play can return its
initializer without constructing clips; this still does not insert a layer or
return its parent command. Reports contain counts, typed states and state hashes,
with no pose arrays, executable code or game assets.
