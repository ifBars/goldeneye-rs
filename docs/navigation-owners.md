# Retained navigation owners

The native mission worker now initializes a fresh navigation component after
complete trigger constructor and reference passes. It retains each real wrapper,
its level/entity binding, fifteen runtime face-category lists and the existing
[ordered boundary cache](navigation-boundaries.md). This does not complete the
remaining world-manager hooks or create a player or camera.

The source loader calls the navigation hook once with the first registration-map
instance and the selected level resource. Registration visits that map's placement
records in source order, without an engine-flags or map-on filter. A missing or
non-navigation initial entity skips registration. Each admitted placement then
uses its instance key to search all loaded maps in order. A key with top byte zero
is a placement index; all other keys are hashes, including `0xffffffff`.

The first found placement supplies the cached raw entity reference and translation.
Entity revalidation still uses the original level resource, even if another map
supplied that placement. Initial placement owner, found placement owner and level
entity owner are distinct identities. The cached reference is not a directory
index. Translation preserves XYZ bits and forces W to positive zero; it does not
move the boundary cache's local vertex indices or geometry into world space.

The pure implementation belongs under `ge-core/src/navigation/runtime`, and the
retained asset adapter under `game/world/navigation`. Immutable decoded graphs are
shared by exact entity directory index within the level owner, while duplicate
registrations retain separate wrappers and caches. Missing required binding,
invalid topology or exhausted host budgets fail the load rather than producing
an empty initialized wrapper. No native allocation-failure or previous-manager
history is inferred from this checked construction.

Runtime categories come from each face's `(flags & 0xf00) >> 8`: zero appends
nothing, and values 1–15 select lists 0–14. Face order and absent flags are
preserved. These lists differ from the serialized contiguous partition groups.
The existing asset reader additionally checks optional fields that this source
initialization does not consume; passing it is checked-input evidence, not exact
source read-order parity.

The local Tutorial check binds one owner with 28 vertices, 28 faces and 28 ordered
boundary pairs. All fifteen runtime categories are empty. These counts describe
that supplied level, not a game-completion denominator. Scalar receipts and
separate binding/category/cache fingerprints are checked against an independent
raw-data evaluator. Original arrays and poses remain local.

The earlier mission-context map-binding receipt still reports navigation as
uninitialized: it describes that earlier preparation stage. The later
`world_initialization.navigation` receipt describes the actual retained component.
Neither receipt establishes original manager history, navigation query precision,
world/user readiness, physics, player creation or gameplay.
