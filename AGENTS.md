# Working on GoldenEye Rust

## Project scope

Independently reconstruct GoldenEye 007 for Wii (2010) in Rust, targeting native
Windows. The supported image is SJBE52 revision 0. Preserve that target; do not
substitute the N64 game, an emulator or a generic FPS demo.

Read README.md, docs/architecture.md and progress/roadmap.json before changing
runtime behavior. The current New Game path prepares the Tutorial world and the
player constructor through shared allocations and its complete virtual reset,
including scratch, character and default-inventory selection. The captured
per-user flag now permits EXItem/actor registration, physics activation and the
basic initializer's session suffix. The handler scalar retains captured unknown
operands; its getter must fail until those inputs are resolved. TP entry now owns
the fresh cleanup, original inventory slots, animator constructor and actual skin
request. Geometry texture discovery now feeds ordered same-wrapper target
requests; completed target owners survive cancellation. The default TP skeleton
now owns bones, auxiliary storage, animator backlinks and primary/secondary skin
bindings. The zero-time callback now uses the controller-lived aligned animation
arena/context and releases its reservation before the typed TP bind return.
TP now has a persistent host component identity and is attached to its actual
item, with matching skeletal and item backlinks. Visual postbind now reuses the
wrapper, binds the visual secondary skin, repeats ordered dependency requests,
resets the base owner's override list and links it to the item. Four TP headers
are configured. The actual embedded receiver and shared callback table now bind
before the typed TP helper return. FP now constructs its own animator/skeleton
from the retained character file, refreshes through the same scratch owner and
appends after TP. Both helpers return before the real resource-list searches
configure FP masks. The fresh initial action selector now stores its direction
and resolves the actual current inventory. Three mappings run in source order
through retained supplied tables and lazy target availability. TP now performs
its ordinary empty refresh through shared scratch and captures primary locals
and auxiliary storage into four independent physical-slot controls. Active count
stays one. The shared fallback stores bracket this capture. TP now qualifies
through the actual FP wrapper and advances a resumable selector over the logically
resident base. Its tracked branch preserves the actual preclear and snapshot-head
comparison. Live command reads preserve optional gates and group weights, then resume
through real class-four factories and their parent effects. Controls and scripts
share one ID allocator and typed intrusive links. The native first set now
initializes two scripts through the same generation-bound wrapper, copies each
old head's normalized position into the new phase fields, inserts both children
and applies transition/divisor/post-word writes. Source order and failure prefixes
stay retained. Capture preserves its initial active count of one; this command
raises the actual count to four. Both groups return with cleanup skipped by the
actual effective flags. The caller acknowledges the body before the mode cursor
tries its zero-hash fallback. TP selection returns 0x090006c0. The fresh untracked
setter returns without writing player mode history. Its dispatch tail visits
three blend-coordinate helpers, writes two script heads and re-enters both
scripts through initflag1. Rate queries reuse retained clips, and unchanged play
selection skips clip setup. Both updates return before TP dispatch returns;
the native path now completes FP dispatch too. FP qualifies through its actual
resource file, takes the tracked primary branch and retains its own mode cursor.
Its mask3 command captures one control into slot0: the loop follows active count1,
not the highest mask bit. The ordinary refresh and backing return precede list
insertion. A second command initializes one slot1 script with one inline clip,
range76 and FPS30; active count grows to two. Both bodies return before FP
selection returns 0x0900000b. The setter stores the requested generic/mapped
0x0900071e pair, compares the real new primary head and clears secondary history.
Its slot1 coordinate helper returns before FP dispatch returns. The enclosing
initial-action suffix now resolves both ordered animation masks, writes physical
slot masks/flags and applies the fresh FP/TP parameters before returning the first
action invocation. Retained history stays separate from later header changes.
The second action now maps again and executes two ordered TP secondary-intent
setters around a live-position seek and temporary slot override. FP then uses
tracked secondary history, and its second suffix completes 14 parameter writes.
The first action's completed evidence stays immutable. The actual callback tail
now clears the derived timer and retained physics fields, writes handler state
and flags, and returns. Constructor continuation now selects script04001dac
through the retained metadata and current inventory. Its supported cleanup path
preserves NULL skips, word/byte store widths and count-only override removals.
The helper returns1 before the actual signedc7d read. The pose helper now uses
live animator references and semantic enable flags, retained character inputs
and the same admitted local user's constructor-owned NULL player association.
An unknown process byte remains unknown; its selected argument is proved unread
only while the actual attachment is NULL. A distinct outer helper returns after
rotation self-copy and final character scale. The initializer now writes its
item/base prefix and builds both controller-lived transform caches in order.
Matrix payloads stay unwritten. The actual preload flag remains1; full animation
preload is next, not a buffer-clear or flag-clear shortcut. It first writes32
Raw512 flag bytes and34 Array34 key words, then visits actual enabled registry
rows and separate temporary animators. No player/camera readiness is inferred.
See docs/player-pose-setup.md for the current boundary.
Factory and update traces stay separate,
with bounded diagnostic history independent of simulation lifetime. Pose postprocess targets
are assigned, but their body and frame scheduling remain unavailable.
Later initialization and gameplay remain
incomplete. docs/player-creation.md describes the reached boundary.

## Source and asset boundaries

- Keep user-supplied images, extracted assets, original executables, disassembly,
  decompilation, screenshots, audio, keys and private research out of Git.
- Use independently authored code and synthetic fixtures. Do not embed extracted
  game payloads or copy an original program into source or test data.
- Preserve third-party licenses and provenance when reusing code. This project's
  MIT license does not cover game assets or override dependency licenses.
- Put local probes, receipts and research under ignored local/. Keep target/,
  dist/, generated progress reports and node_modules/ untracked.
- Run `python tools/check-source.py --staged` before committing. It checks common
  file and credential mistakes; it does not establish copyright clearance.
- Follow docs/legal.md when preparing a public contribution or distribution.

## Code ownership

- crates/ge-assets: checked, bounded readers. No UI or runtime readiness state.
- crates/ge-core: pure runtime rules. No file I/O, rendering or wall-clock reads.
  Keep each domain and its tests in focused subdirectories.
- crates/ge-render: GPU rendering, upload, presentation and readback.
- apps/goldeneye: native lifecycle, worker ownership, local settings and I/O.
- apps/goldeneye/src/game: retained mission, player and provider bindings.
- apps/goldeneye/src/menu and crates/ge-core/src/frontend: frontend work.
- xtask: validation, source-bound receipts, dashboard generation and packaging.
- tools/dashboard: DOM tests. Use Bun for package management and Node 24 to run
  the test suite.

Keep production tests and fixture builders in tests/ directories or tests.rs
modules. Inline test helpers can distort production coverage measurement.
Preserve unrelated work. Do not register unfinished diagnostic modules as live
gameplay merely to increase coverage or reported completion.

## Commands and evidence

The user requested one published branch, `main`. Preserve merged work and avoid
leaving redundant feature branches after consolidation.

Local CPU use is authorized again. Keep Cargo invocations serial and use two
build jobs by default; do not overlap builds and coverage. Hosted CI verifies
both supported host configurations. Use the default local target directories:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo xtask check
cargo xtask coverage
cargo xtask report
```

The developer guide lists image-dependent checks. Generate a report before
running dashboard tests on a fresh clone. Package generation creates full
host-specific dependency notices and stages an explicit asset-free allowlist.

Builds, unit tests, asset compatibility, native presentation and reference parity
are separate claims. Stale receipts are not current passes. Percentages need a
named denominator; the roadmap is not a measured percentage of the entire game.
Verified parity requires reference inputs, expected results, tolerances and a
passing comparator.

Preserve source ordering, raw values, unknown fields, ownership identities and
partial failure state. Never turn missing providers into success or absent data.
Keep constructor snapshots distinct from original runtime history. Reuse the
retained RNG, resource and manager owners when a later operation shares them.
Cancellation must reject stale worker writes without blocking the window.

Do not use computer control without the user's approval. Do not add agents unless
the user authorizes delegation. Keep game development paused when requested;
publication or documentation work does not resume it.
