# Constructor continuation

After the startup animation callback returns, the player constructor selects a
script and runs its immediate cleanup helper. Pose setup follows that helper.
These are separate steps from the completed startup actions and callback.

## Keep the saved state separate

The constructor reads handler state `704` before entering the callback. Only a
saved value of 5 causes it to restore 5 afterward. The fresh path saves 0 and
keeps the callback's resulting state 2.

That saved entry value has its own receipt. The callback's optional diagnostic
old value cannot replace it. Completed action and callback evidence stays
immutable while constructor continuation changes the live owners.

## Select through the retained inventory

The selector sign-extends metadata slot `c98` and resolves the actual selected
row. A negative slot is a known missing row. An unavailable positive row remains
a provider dependency.

The selector reads the metadata type first. The default type `0x4b200000`
selects script `0x04001dac` directly. It does not read the fields used by other
type branches. This metadata type is unrelated to the unresolved physics
handler kind.

The following helper resolves current inventory slot `c96`, retaining its actual
key and flags. Its variant comes from the reached alternate-byte and record-flag
branches. It then reads metadata inventory key `0x02000830`, which takes the
immediate cleanup path. Selecting the script does not load its resource on this
path.

Metadata primary key, metadata inventory key and current inventory record key
remain separate values, even when two happen to match. The app checks the
selected row against the retained inventory and loadout owners.

## Preserve the cleanup prefix

Cleanup reads each link at its source position. Known NULL links skip their
release calls and stores. Unknown or unsupported owners stop at the reached
dependency without erasing earlier effects.

The common path reads `af8` before clearing word `b84`. A non-NULL `af8` therefore
retains that clear while waiting for its real release provider. The next store
clears byte `bd1`; adjacent bytes stay unchanged. A NULL base `570` skips its
nested work.

Two visual override removals follow, in order: `0x0600056d`, then `0x0600067b`.
The source first reads only the signed count. A nonpositive count returns
without reading entries, backing, capacity or growth, and without changing the
generation. This operation does not reset the container. A positive count needs
its actual entries and removal behavior.

NULL `bb0` and `bb4` links skip their release calls and subsequent stores. The
helper then returns literal 1. Only after that return does the constructor read
and sign-extend byte `c7d` for the next pose helper.

## Evidence and limits

The native Tutorial probe reaches this boundary with metadata slot 23, current
inventory key `0x02000830` and variant 0. All five cleanup links are NULL, both
override counts are zero, and the helper returns 1. The next pose argument is
the actual signed byte value 1. The app still displays **Preparing player**.

The continuation has a separate receipt for its actual reads, skipped branches,
ordered stores, override results and helper return. Private owner state is
hashed; original payloads and pointers stay out of reports and synthetic tests.

The following [pose helpers and transform-cache construction](player-pose-setup.md)
now execute too. Animation preload remains pending. These helper returns do not
complete the player constructor or publish a live player; camera setup, movement
and combat remain separate milestones.
See [player creation](player-creation.md) and the
[repeated startup action](repeated-player-action.md) for the preceding owners.
