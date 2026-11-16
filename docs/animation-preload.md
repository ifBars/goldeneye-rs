# Player animation preload

New Game still stops at **Preparing player**. Tutorial world registration
completes, but player creation, the gameplay camera, movement and combat remain
unfinished.

The initializer's earlier pose helpers and FP/TP transform-cache construction
have returned. Animation preload now runs against those retained owners. Its
current native checkpoint completes the first temporary script factory,
zero-time forward callback and nonempty evaluation, then returns from the bind.
The caller's post-bind work remains unfinished.

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
branches skip. The list still contains a node, so this path runs the nonempty
evaluator rather than the empty-list refresh.

That evaluator reserves the existing shared scratch arena and stores its context
regions. It resolves the temporary skeleton's current primary skin before clearing
the two shared masks. It then visits the real script node. The constructor's
current clip reference is MAX, so the node returns zero before reading animation
coordinates or sampling frames. One nonempty layer is visited, with zero samples,
blends or pose commits. The actual reservation releases before the bind returns1.

Pending evaluations retain their scope through a cancellation lease. The lease
checks the actual arena and context instances, even when semantic IDs match.
New scratch operations reap stale attempts; shutdown aborts remaining scopes
after workers join. Cleanup never counts as a completed evaluation or bind.

## What remains

The next work is the caller's parameter write and ordered clamp, another
evaluation, six bone outputs and teardown. The mutable process scalar's current
value remains unresolved. Its startup value alone cannot supply a current value.
Constructor locals and allocated matrices are not sampled poses.

Later preload work still needs its remaining temporary operations, teardown,
entry commit and subsequent registry calls. Full player initialization and the
gameplay camera follow those operations. No completed preload, playable mission
or original-game parity is claimed.

## Evidence

Core tests cover narrow storage writes, lazy reads, partial failures, mapping
order, distinct temporary ownership, fresh NULL proofs and pending body states.
Tests also cover retained wrappers across cancellation/reentry and failed request
prefixes. Tests cover pending evaluations, exact-owner cancellation and immediate
reentry. Native hidden New Game probes complete the direct factory, forward
callback, nonempty evaluation and bind without a loading failure. Their scopes
are released, with no sampled pose or commit. Player and camera readiness remain
false.

The progress report keeps the returned initializer snapshot separate from current
preload mutations. See [player pose setup](player-pose-setup.md) for the earlier
checkpoint and [progress definitions](progress.md) for the meaning of each gate.
