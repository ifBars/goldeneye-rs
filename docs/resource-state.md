# Retained resource state

`ge-core/mission/resource_state` owns bounded, ordered 16-bit key/value records.
The source constructor starts with an empty store; this is distinct from an
unavailable provider or an unknown original runtime snapshot.

Lookup returns the first matching low-16-bit key, or the caller's full 32-bit
fallback. Token zero returns that fallback without scanning. Prefix validation
belongs to the setter and trigger callers. Setting a valid token reuses the first
matching row or appends a row, truncates the value to 16 bits, and tracks source
capacity growth separately from the Rust allocator's capacity.

Reset clears a bounded prefix of values while retaining keys, count and capacity.
Capture keeps nonzero rows in source order and stops at the source cap of 500.
Restore checks capture/generation admission before resetting or reading rows.
Accepted imports skip zero keys and apply repeated keys in saved order to the
first existing record. Host budget/allocation failures leave the owner unchanged.
These operations implement the record region, rather than a disk save format or
the rest of a captured engine/player state buffer.

The app's `game/resource_states` module retains one owner on the native window
controller and shares it with CPU mission workers. Only this pure state crosses
threads; source partitions remain on their loading worker. Cancellation and
re-entry retain the same owner. Lookup counts belong to each attempt, so cancelled
workers cannot contaminate a later attempt's diagnostic counts. Future writes and
snapshot admission must update this same owner and refresh affected evidence.
Each typed New Game flag1 handoff performs the source value-prefix reset on its
CPU worker before mission loading. This preserves keys, capacity and origin;
cancellation itself does not reset the store.

The current host begins from the explicitly labelled constructor snapshot and
initial reset-limit input. Both actual Tutorial Radius03 queries miss in that
owned empty store and return their caller fallback. Original prior setters,
capture/import, profile history and the original values at New Game remain
unverified. A supplied snapshot has a separate origin; no missing owner is
replaced by an assumed zero result. Public evidence exports counts, origin,
lifetime and a private-state digest, without keys or values.
