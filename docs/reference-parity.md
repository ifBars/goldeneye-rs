# Frontend reference comparisons

The dashboard separates working features from measured matches to the original
game. A passing menu smoke test proves that our menu works through the tested
route. Reference comparisons check its output against a captured original.

The first comparison set covers sampled warning artwork, startup movie frames
and the initial main-menu view at 640×480. These are three finite visual cases.
They do not measure full-game completion, audio, timing or every menu action.
The original game must actually reach each recorded reference state; an adapted
Rust menu cannot stand in for a missing original capture.

## What counts as verified

Each case in [parity-cases.json](../progress/parity-cases.json) lists every required
sample, its dimensions, any fixed crop, signal checks and numeric tolerances.
The comparator reads both PNGs and computes their errors. It does not accept a
caller-supplied pass flag, resize images, search for a better alignment or relax
tolerances after a failure. Every required sample must pass before its case
contributes one verified result.

Images need visible content and spatial variation. Matching empty frames cannot
verify a startup screen. A missing capture, unsupported image or failed sample
contributes no verified case. The denominator retains all declared cases.
Missing samples do not prevent other complete cases from being measured, but
the command fails until every declared case passes. Undeclared captures are
rejected rather than silently ignored.

Reference packs identify the original edition, image digest, Dolphin executable,
settings and input contract. Candidate packs identify the Rust source and actual
executable, settings and matching observation contract. Captures, profiles,
recordings and manifests containing local paths stay under ignored `local/parity`.
Only independently authored comparison code, case definitions and synthetic tests
belong in Git.

## Refreshing a comparison

Build the native executable before capturing its current output:

```powershell
$env:CARGO_BUILD_JOBS = [string][Math]::Max(1, [Math]::Floor([Environment]::ProcessorCount * 0.75))
cargo build --release --locked -p goldeneye
```

Bounded native probes can request an exact physical canvas. For example:

```powershell
.\target\release\goldeneye.exe play 'D:\Games\GoldenEye.rvz' --start-at strap --title-aspect standard --capture-size 640 480 --frames 60 --hidden --mute --capture local/parity/strap.png
```

Starting inside a stage measures that view. It does not prove the complete
boot sequence or its timing. Keep the capture's actual state and configuration
alongside it. The local pack schema is defined in
[manifest.rs](../xtask/src/parity/manifest.rs).

Run the offline comparison against the two local packs:

```powershell
cargo xtask parity progress/parity-cases.json local/parity/reference/pack.json local/parity/candidate/pack.json
```

The command records the latest result and refreshes the dashboard, including
when comparisons fail. Its exit status is successful only when the entire case
set passes. Original media stays local. Reference collection through Dolphin
uses a separate profile and does not alter the user's normal saves or settings.

## Keeping results current

Report generation rehashes the case definitions, executable files, packs,
configuration, input contracts and captured images. Changed source or evidence
makes prior verification stale and removes it from the current verified count.
The previous metrics remain historical. A newer failed attempt never falls
back to an older passing result.

For a phone dashboard that recomputes freshness on each page request:

```powershell
cargo xtask report
python tools/dashboard/serve.py --bind <computer-LAN-IP> --port 8080
```

The server serves only public progress summaries, disables browser caching and
runs the compiled report tool before responding. If regeneration fails, it
returns an error instead of serving old verified counts. Reload the page to
check the current state. Stop the server with Ctrl+C.

The broad roadmap remains a separate, incomplete inventory. Its implementation
states cannot be edited into verified parity. A frontend case does not verify
an entire broad rendering, audio or campaign criterion.
