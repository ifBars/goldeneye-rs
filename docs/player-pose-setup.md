# Player pose helpers and shared transform caches

New Game still displays **Preparing player**. The pose and outer component
helpers now return, and the initializer constructs the shared transform caches.
The next operation is animation preload. No mission is playable yet.

This follows the [constructor cleanup](constructor-continuation.md). Earlier
action, callback and constructor receipts stay immutable as the later helpers
change the live owners.

## Use current component and user state

The pose helper reads the actual first- and third-person component references.
The preceding startup actions changed those references, so their original
constructor value cannot be reused. It updates component enable fields rather
than physical animation-layer flags. In the native Tutorial probe, TP flags
change from 3 to 2 and FP flags remain 3; the stores still occur.

Character scalar reads come from the retained selected row, with matching key
and source index. The reset's semantic character binding takes precedence over
its earlier numeric constructor storage. The first scale uses a separate
startup-owned vector of ones. The outer component helper later overwrites that
scale with the actual character-derived vector and copies the current rotation
back in source order. These operations do not sample animation.

The local-user lookup visits the actual admitted user. Its constructor explicitly
initialized the player association to NULL, before collection insertion. The
association retains session and user identity and stays separate from readiness
and created flags. Publication requires the full player constructor to return;
neither helper publishes a player. Unknown or foreign associations fail the
lookup rather than becoming a missing-user result.

## Keep the unresolved input visible

A reached process byte still has no supplied live value. Two possible branches
pass different arguments to the same attachment helper. With the actual NULL
attachment, that helper returns before reading either argument.

The receipt records the unknown byte, an unresolved argument and the explicit
argument-unread no-op proof. It does not choose a byte value or claim which
original branch ran. A non-NULL attachment needs the real input and its remaining
providers. This equivalence does not establish reference parity.

## Construct the separate caches

After the outer helper returns 1, the initializer writes the item's transform
flags, rotation mode and dirty byte, then six base scalar fields. It resolves FP
before TP from the actual item list. A missing receiver skips cache work and
invalidation; it is distinct from an existing receiver with an unavailable
skeleton.

The controller owns two separate cache headers and their selected-cache state.
Their initial values have checked startup-clear and pre-main-constructor
provenance. They persist across player attempts and cancellation. A selected
cache skips reconstruction; it does not prove that matrices contain a pose.

A fresh build runs FP before TP. Each header receives matrix backing, zeroed
validity bytes, U16 offsets and compact descendant lists in retained child/sibling
order. Temporary storage is released after copying. Matrix payloads remain
unwritten because their source allocation has no element constructor. Allocation
or input failures retain the reached backing and write prefix.

The native Tutorial probe builds 48 FP and 37 TP entries, retains eight backing
allocations and selects FP. Neither cache is associated with a completed player,
and neither has initialized matrix payloads. These are construction results,
separate from evaluated transforms or camera output.

## Continue with real animation preload

The source-owned preload flag is still 1. The next helper first clears 32 exact
flag bytes in the existing raw cache and 34 exact key words in the existing
record cache. It then visits enabled weapon-registry rows, performs six ordered
requests per row and constructs separate temporary animators for real bind,
evaluation and teardown work.

Clearing whole buffers, clearing the flag early or reusing a live FP head would
skip required behavior. Preload must return before later initializer work can
continue. Initializer return, full constructor return, publication, completion
registration and camera readiness remain separate boundaries.
