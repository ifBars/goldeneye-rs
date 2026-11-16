# Player animation preload

New Game still stops at **Preparing player**. Tutorial world registration
completes, but player creation, the gameplay camera, movement and combat remain
unfinished.

The initializer's earlier pose helpers and FP/TP transform-cache construction
have returned. Animation preload now runs against those retained owners. Its
current native checkpoint completes 162 registry calls, including 32 temporary
animation setups, bone-output copies, explicit destructors and cache-entry
commits. The next temporary reaches a resident animation and waits for frame
sampling during its bind evaluation. The shared preload flag remains enabled.

## What runs now

Preload independently reads its shared enable flag. When enabled, it clears
32 individual flags and 34 entry keys in the existing allocations. It preserves
the remaining unwritten bytes, array headers and unrelated cache storage.

The cursor visits the supplied weapon registry in source order and reads each
enable byte before its weapon key. Each enabled row has six ordered mapping
requests: two animation kinds for each of three variants. An unavailable variant
can return before touching the caches; the default variant retains its fallback.

For each new entry, the native path writes the selected weapon and mapping
fields, invalidates the selected FP transform cache, and extracts the first
eligible class-four animation reference. The entry key is committed only after
that call's temporary operations return. Other early-return paths retain their
own source effects. The weapon key is read again before each call.

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

## The caller's second evaluation

After bind return, the caller reads the actual slot-zero head and checks its
class through the same animator's node registry and script store. For a matching
head, it writes 0.5 through the header's constructor-owned parameter pointer into
that header's coordinate word.

It then reads the persistent process scalar. Its startup value is known, but its
current value is not proved. The read therefore captures an unknown current value
with its owner, sequence and revision. The ordered clamp is retained as an
expression in the script phase field, replacing its stale known zero. Numeric
phase reads fail while that expression is unresolved. No startup-value fallback
is used.

The second evaluator uses a new scope in the same shared arena. It repeats the
required skin preparation, mask clears and actual node visit. The reached MAX
clip guard returns before reading either updated field, so the unknown phase
does not prevent this evaluation from returning. Its scope releases with zero
samples and no pose commit. The first bind's history remains unchanged. A NULL
head skips both writes and the scalar read, while still running its own second
evaluation.

## Bone outputs, destruction and continuation

After the second evaluation, the output helper reads six supplied tag hashes
lazily. It looks up each tag through the same primary skin, then copies the
actual bone record's index, local translation and quaternion into the selected
cache entry. These are current locals, not matrix calculations or proof of a
sampled pose. A missing tag writes only its missing index. A zero hash preserves
the source's unchanged hash cursor.

The explicit destructor unlinks each child before invoking its cleanup, then
releases the skeleton and temporary owner in source order. A later unresolved
cleanup retains the prefix already performed. Cancellation and Rust memory
cleanup do not count as this source destructor. Diagnostics are captured before
destruction; a tombstone carries the full call binding to the final entry-key
store. The cursor then continues with a fresh temporary identity.

The native checkpoint has copied 192 found bone outputs into 32 committed
entries, with no missing tags. A bounded history keeps the latest 16 completed
temporary receipts and aggregate counts. The first completed snapshot is kept
separately, so its original bind and caller evidence survives history eviction.

## What remains

Call 163 reaches a resident script with four clip records. Its initializer and
zero-time forward callback return, but the bind evaluator now reaches the actual
frame-sampling provider. It retains its own live scratch lease at that boundary.
The caller's parameter updates, second evaluation, outputs and teardown have not
run for this temporary. Earlier completed calls do not supply its pose.

Frame sampling, the remaining preload calls and the final shared flag clear
remain. Full player initialization and the gameplay camera follow those
operations. No completed preload, playable mission or original-game parity is
claimed.

## Evidence

Core tests cover narrow storage writes, lazy reads, partial failures, mapping
order, distinct temporary ownership, fresh NULL proofs and pending body states.
Tests also cover retained wrappers across cancellation/reentry and failed request
prefixes. Tests cover pending evaluations, exact-owner cancellation and immediate
reentry. Native hidden New Game probes complete the direct factory, forward
callback, nonempty evaluation and bind without a loading failure. A direct
native probe also completes 32 caller suffixes, output copies, destructors and
entry commits before reaching the resident sampling boundary. Completed scopes
release with no sampled pose or commit. Shutdown cleans the pending evaluation
after joining workers. Player and camera readiness remain false.

The progress report keeps the returned initializer snapshot separate from current
preload mutations. See [player pose setup](player-pose-setup.md) for the earlier
checkpoint and [progress definitions](progress.md) for the meaning of each gate.
