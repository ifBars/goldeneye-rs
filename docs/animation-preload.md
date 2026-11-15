# Player animation preload

New Game still stops at **Preparing player**. Tutorial world registration
completes, but player creation, the gameplay camera, movement and combat remain
unfinished.

The initializer's earlier pose helpers and FP/TP transform-cache construction
have returned. Animation preload now runs against those retained owners. Its
current native checkpoint reaches the first temporary animator's script-body
publication dependency.

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

The reached script body belongs to another logical resource section. The
current provider retains its descriptor and stops at
`temporary_script_body_residency`. Body availability remains unknown. Cached
file bytes alone do not prove that this section was published.

## What remains

The next work is to bind the resource owner's fresh base-publication and section
history to the script's runtime body state. A confirmed null body and an unknown
publication state must remain distinct. The descriptor getter itself does not
request a section load.

After that, the actual direct factory, zero-time forward callback, nonempty
evaluation and bind return must run in order. A real script node requires the
nonempty evaluation path even if its body is null. Constructor locals and
allocated matrices are not sampled poses.

Later preload work still needs its remaining temporary operations, teardown,
entry commit and subsequent registry calls. Full player initialization and the
gameplay camera follow those operations. No completed preload, playable mission
or original-game parity is claimed.

## Evidence

Core tests cover narrow storage writes, lazy reads, partial failures, mapping
order, distinct temporary ownership and pending body states. A native hidden
New Game probe retains the registered world, temporary skeleton and cached
script descriptor with no loading failure. Player and camera readiness remain
false.

The progress report keeps the returned initializer snapshot separate from current
preload mutations. See [player pose setup](player-pose-setup.md) for the earlier
checkpoint and [progress definitions](progress.md) for the meaning of each gate.
