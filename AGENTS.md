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
configure FP masks. Initial animation modes are next. Pose postprocess targets
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

Keep local work lightweight after the reported CPU issue. Use hosted CI for full
builds and coverage; do not start an unrestricted local rebuild. Run any authorized
local Cargo tasks serially with the default target directories:

```powershell
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
