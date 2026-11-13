# Working in this project

Open the repository folder in your editor. Read `AGENTS.md`, the current
[architecture](architecture.md), and `progress/roadmap.json` before implementation.
Run `cargo xtask report` to create the offline dashboard at `progress/index.html`.
The generated `progress/latest.json` contains the same state for tools. These
files are local output and are not committed.

| Work | Command | Evidence |
| --- | --- | --- |
| Normal change | `cargo xtask check` | Format, Clippy, authored tests, native release build |
| Fast authored tests | `cargo test --workspace --locked` | No game image or graphics adapter required |
| Coverage | `cargo xtask coverage` | Production LLVM line/function totals and local HTML; 50% line floor |
| Refresh progress | `cargo xtask report` | Current source freshness, declared scope, observed history |
| Asset compatibility | `cargo xtask smoke <disc>` | Disc/index intake only |
| Resource compatibility | `cargo xtask resources <disc>` | Bounded EDB, texture, graph and collision-input audit |
| Navigation changes | `cargo xtask navigation <disc>` / `cargo xtask paths <disc>` | Sampled geometric/runtime diagnostics |
| Rendering changes | `cargo xtask render <disc>` / `cargo xtask viewer <disc>` | Explicit GPU and hidden-window checks |
| Frontend changes | `cargo xtask menu <disc>` / `cargo xtask startup <disc>` | Native menu, startup media/audio, mesh GPU fixtures, source-title loop and outro checks |
| Distribution | `cargo xtask package` | Asset-free native file allowlist and checksums |

Run Cargo tasks serially. They share build output and coverage profiles. Each
recorded gate regenerates the dashboard. A source/configuration change marks old
receipts stale; the displayed prior result is useful history, not a current pass.
Local evidence and original game data stay under ignored `local/` or as ignored
user-owned images. CI runs synthetic checks without uploading those files.

For dashboard development only, install Bun and Node 24. First run
`cargo xtask report` from the repository root, then run from
`tools/dashboard`:

```powershell
bun install --frozen-lockfile
bun run test
```

Tests use authored scenarios and the generated report. They work on a fresh
checkout without private receipts. Regenerate with `cargo xtask report` after
editing the template. DOM tests check interactions and data, not rendered layout;
browser visual/input inspection remains separate and requires user approval for
computer control. The normal Rust build and offline HTML have no JS dependency.

To view progress from a phone on the same Wi-Fi, serve only the generated
`progress` directory from the repository root:

```powershell
python -m http.server 8080 --bind <computer-LAN-IP> --directory progress
```

Open `http://<computer-LAN-IP>:8080/` on the phone. Keep the terminal running;
Ctrl+C stops the server. Each report refresh appears after reloading the page.
The server root contains public summaries rather than the image, private evidence
or coverage source listings. This serves the dashboard only; it does not run the
game. The computer and phone must be able to reach one another on the local network.

New Game currently stops at **Preparing player** after the constructor cleanup
helper returns. Its receipt keeps selected metadata, current inventory, ordered
NULL skips and narrow stores separate from the completed action/callback
snapshots. The next pose helper has not run. See
[constructor continuation](constructor-continuation.md) for the source order and
remaining providers. A new native receipt must keep player and camera readiness
false until their real completion paths return.

VS Code's **Run Task** menu exposes these routine commands; Rust Analyzer and
TOML extension recommendations are included. Generated/private directories are
excluded from editor search and watching. Avoid parallel task launches.

Choose one acceptance criterion or observed defect, write a focused authored
fixture, implement within its owning crate, and record the relevant checks.
Report build success, test success, asset compatibility and reference parity
separately. Do not mark an entire criterion implemented because one helper works.
Keep source readers in `ge-assets`, pure membership/animator rules in their
`ge-core/mission` subtrees, and source/runtime lifetime binding in
`apps/goldeneye/src/game/world`. See [world initialization](world-initialization.md).
The acceptance inventory can grow, so its denominator and declared implementation
percentage may change. Verified parity requires a reference-comparison runner;
until one exists the verified count remains zero.
