# Live animation-script initialization

New Game retains its first class-four animation child in the TP animator. The
initializer now selects a play command and constructs its clip references. The
native Tutorial probe visits four command headers, selects offset zero and binds
seven normal clips from the same resource wrapper. Clip setup returns with
index 6 and a 30-frame range. The selected animation then resolves descriptor 22
through the same wrapper, binds a resident header and reads 30 FPS. Zero timing
flags and position zero let the initializer and factory return. Parent child/list
effects now finish for both native set groups. The command and TP selector return
before the setter tail; the player and camera remain incomplete.

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
The next animation lookup and position operations use the same retained child.

The dashboard distinguishes file lookup, unknown clip range, frame-count lookup
and animation preparation. A script with no selected play can return its
initializer without constructing clips; this still does not insert a layer or
return its parent command. Reports contain counts, typed states and state hashes,
with no pose arrays, executable code or game assets.


## Selected animation and position

`ge-core/animation/script_child/position` continues after clip setup without
repeating selection or allocating another child. A bound clip uses its actual
file generation. A cache miss resolves the directory index, stores the typed
cached descriptor and then updates the generation when found. A genuine miss
stores NULL and leaves the old generation. A later body or timing failure keeps
these writes. A matching but unknown cache pointer remains unknown.

`ge-assets/skeletal_animation/live` reads the logically resident base. Index
lookup does not read an unnecessary descriptor hash. Body lookup distinguishes
NULL from an unrequested section. FPS is a separate scalar read; neither a full
animation header nor compressed frame/channel data is preflighted. The app
borrows this view from the same file owner rather than reloading a second EDB.

The live timing reader follows only the selected table entry and reads its flags.
Only the low two bits request timing curves; other bits do not force option or
curve reads. Required curves stop before FPS conversion and position writes.
A resident animation reads its FPS before the script FPS; an absent animation
skips both. The source's separately rounded conversion, bounds or double
remainder, history write and normalized position write stay in order. A
nonfinite normalized result retains those writes before the explicit host limit.

A nonzero position with child bit 2 set still needs the source's clip selection
and root-motion lookup. Position zero skips that branch. Initializer return does
not sample a pose, insert an intrusive layer node, execute parent post-words or
acknowledge a mode command. The dashboard keeps those boundaries separate.


## Parent effects and command continuation

The existing head's virtual getter reads its normalized position. The class-four
setter copies that scalar to phase fields `+0x80`/`+0x84` without invoking the initializer.
Captured controls now retain their common constructor position explicitly; cloning
constructs a fresh header and copies only its backing.

The shared node owner stores typed previous/next links for controls and scripts.
Parent insertion raises the actual active count before publishing the list head,
updates the old head's previous link and preserves the list tail. These typed
roles replace native pointers. Allocation, factory return and membership remain
distinct facts.

After insertion, the parent zeroes transition progress and stores one divided by
the unsigned divisor, with divisor zero selecting one. Only then does it read and
store the serialized child post-word. That word remains raw, even when its bits
look like a nonfinite float. A later read failure retains insertion, active count,
phase copy and scalar writes, and prevents replay.

The child cursor resumes at the next ordinal and group without re-reading the
command operand prefix or rewriting an existing group weight. Effective flags
control the final cleanup gate. The native first set uses two groups, inserts two
script children and raises the actual slot count to four. Its flags skip cleanup,
so the command returns and is acknowledged before the mode cursor continues.
The permitted zero-hash fallback then returns 0x090006c0. The TP setter preserves
its own history gates before the dispatch tail re-enters the retained scripts.
See [script updates](animation-script-updates.md) for that separate continuation.
