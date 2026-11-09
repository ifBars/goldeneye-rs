# Loader owners and readiness

The native session worker retains the mission inputs, constructed map state,
trigger registry and completed provider work until Back or shutdown. The window
controller retains the engine clock, sparse loader globals, checkpoint wrapper,
resource context masks and audio cache across attempts. Cancelling an attempt
invalidates its generation; it does not erase commands already submitted to the
shared audio service.

`ge-core/src/mission/loader` owns pure phase transitions, bounded work accounting,
checkpoint state and the outer reset/bank rules. `mission/maps` owns mutable map
initialization, resource contexts and fresh section publication. Host archive I/O,
retained payloads and orchestration belong under
`apps/goldeneye/src/game/world_hooks/loader`. Asset readers stay in `ge-assets`.

The audio provider has two distinct operations. One shared job reads and registers
SoundInfo metadata. Ordered bank commands retain checked SBNK payloads and indexes,
then acknowledge their own completion. Having a bank token or cached file does
not complete either callback. Bank residency does not establish playback of every
voice, the original mixer, or native SDK command timing.

Map construction owns zone defaults and mutable state, initially null component
slots, placement-cache words and per-context masks. Each map publication
invalidates caches on the maps already loaded, then executes the supported
publication hook. Fresh base sections are logically published; eagerly cached
nonbase bytes remain unrequested. Zone binding allocation, placement bounds and
hierarchy refresh use the existing checked readers and core geometry kernels.
Later nonbase requests and existing live component histories need their own
providers; they cannot be replaced by an empty-world assumption.

Wait phase 28 first polls actual outstanding work. Only when work and resource
providers both complete does it promote the pending phase. Completion callbacks
must return successfully, and failed work remains retained. The source wrapping
work-counter formula is modeled separately from checked host accounting, where
overflow must never manufacture readiness.

Phase 18 rebuilds the live StartPoint catalog after the intervening barriers.
Phase 19 applies its ordered flag changes to the exact admitted session, peer and
user, updates the local mirror, and queues the player-loading phase. This bridge
does not create a player or camera. The dashboard distinguishes the older
`awaiting_world` checkpoint from `awaiting_player` and reports original timing,
history and SDK dispatch as unverified.

Tests use authored data. Supplied-image probes, static source contracts and
independent fingerprint calculations stay under ignored `local/`. A successful
compile, provider receipt or world flag is separate from playable gameplay and
reference parity.
