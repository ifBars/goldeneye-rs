# Player animation preload

New Game still stops at **Preparing player**. Tutorial world registration
completes, but player creation, the gameplay camera, movement and combat remain
unfinished.

The initializer's earlier pose helpers and FP/TP transform-cache construction
have returned. Animation preload now runs against those retained owners. Its
current native checkpoint completes the first temporary script factory and
zero-time forward callback, then waits for nonempty animation evaluation.

## What runs now

Preload independently reads its shared enable flag. When enabled, it clears
32 individual flags and 34 entry keys in the existing allocations. It preserves
the remaining unwritten bytes, array headers and unrelated cache storage.

The cursor visits the supplied weapon registry in source order and reads each
enable byte before its weapon key. Each enabled row has six ordered mapping
requests: two animation kinds for each of three variants. An unavailable variant
can return before touching the caches; the default variant retains its fallback.

The actual native path now reaches its first mapping and cache entry. It writes
the selected weapon and mapping fields, invalidates the selected FP transform
cache, and extracts the first eligible class-four animation reference. It does
not commit the entry key or clear the shared preload flag at this point.

## A separate temporary owner

Preload constructs a distinct animator through the same component identity
registry. Its new skeleton uses the current FP skeleton's secondary skin pair.
It owns its own bones and auxiliary storage; the native checkpoint has 48 bones
and no auxiliary words.

This temporary is not appended to the player's item list and receives no player
callback table or pose callback. Its direct script factory owns a new header,
slot zero and flags three. It resolves the actual descriptor through the current
FP resource wrapper. It does not reuse a live script head or manufacture a mode
command.

The reached script belongs to another logical resource section. Its descriptor
is retained, and the checked fresh-base callback includes the script-fixup bit.
The same file owner has no section requests or unresolved request history, so
the reached fixup branch establishes a NULL runtime body. Cached file bytes do
not make that body resident, and the metadata getter issues no section load.

The wrapper registry now belongs to the controller. Cancellation, another player
attempt and repeated lookups retain the same wrapper and publication history.
Requests record their prefix before a provider can fail. Failed requests and
unresolved dependency work keep body availability pending; a retained payload
does not prove script publication or completed body initialization.

The NULL-body initializer returns without scanning clips. Its real class-four
node is inserted, and its zero-time forward callback clears the reached phase
fields and returns without reading duration or FPS. Fresh maintenance and reverse
branches skip. The list still contains a node, so this path awaits
`temporary_animation_evaluation`; it cannot use the empty-list refresh.

## What remains

The next work is the nonempty evaluation path and its bind return. A real script
node requires that path even when its body is NULL. Its actual sampling guard,
skeleton preparation, shared scratch stores and release must run in source
order. Constructor locals and allocated matrices are not sampled poses.

Later preload work still needs its remaining temporary operations, teardown,
entry commit and subsequent registry calls. Full player initialization and the
gameplay camera follow those operations. No completed preload, playable mission
or original-game parity is claimed.

## Evidence

Core tests cover narrow storage writes, lazy reads, partial failures, mapping
order, distinct temporary ownership, fresh NULL proofs and pending body states.
Tests also cover retained wrappers across cancellation/reentry and failed request
prefixes. A native hidden New Game probe completes the direct factory and forward
callback without a loading failure. Player and camera readiness remain false.

The progress report keeps the returned initializer snapshot separate from current
preload mutations. See [player pose setup](player-pose-setup.md) for the earlier
checkpoint and [progress definitions](progress.md) for the meaning of each gate.
