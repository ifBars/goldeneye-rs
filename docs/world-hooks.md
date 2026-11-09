# Persistent world-hook owners

The native controller now owns the clock-reset and final cleanup managers across
startup, menus, mission attempts and cancellation. Pure reset rules live under
`ge-core/src/mission/world_hooks`; the real frame clock, synchronization and
attempt lifetime live under `game/world_hooks`. The worker executes this segment
after navigation and the selected-definition/resource hook.

The cached host clock starts at controller construction and advances from actual
native-window frame durations. Each update performs a separate single-precision
scaled step and addition, using the source initial scale of 1. Tutorial does not
reset this clock. This is an explicit host frame-clock adaptation; the original
engine's earlier frame history, scheduling and numerical precision are unverified.

Constructing the reset owner performs sixteen actual getter calls once. Each
world reset performs another sixteen calls, in source order. A lease of the real
cached word keeps the frame sample stable while the CPU worker holds the manager
lock. The first member of each time pair adds positive zero; the second copies
the raw sample. Pure provider failures preserve reached writes and cleanup rather
than replacing the owner or reporting completion.

The reset clears four of six owned buffers and preserves the other two. Final
hooks then reach two literal no-ops, write only the known sparse leaf fields,
clear five top-level and eleven record buffers, and clear two further buffers.
All clears release actual owned backing allocations while retaining growth words.
The 24-buffer count describes this scoped model, including its two untouched
buffers; unrelated manager fields and vectors are not invented or zeroed.

A bound item is retained separately from the manager's optional link. Release
writes its byte41 to 0, then byte40 to 1, before clearing that link. The item stays
owned for later consumers. The last hook installs a typed world callback without
executing it. Unknown binding history cannot be substituted with a null link.

Cancellation changes the active attempt identity without joining a worker or
recreating managers. A waiting worker checks its identity again after taking the
manager lock. Already-started operations retain their reached side effects, but
an old attempt cannot overwrite a newer completed hook or publish stale results.
Per-operation diagnostics clear separately from persistent source state.

Receipts expose scalar counts and fingerprints, not timer arrays or backing bytes.
Clock samples, frame counts and reset history can differ between real processes
and attempts. Each snapshot's reset fingerprint is independently calculated;
different clocks are not forced equal to make comparisons pass. Buffer byte
counts describe actual host allocation capacity, not original native heap sizes.

This reset segment emits no readiness. The separate [outer loader](loader-owners.md)
now executes the supported fresh Tutorial hooks, provider waits, phase18 catalog
rebuild and identity-bound phase19 readiness delivery. Player and camera
construction remain later dependencies. Keep these two receipt scopes distinct.
