# GoldenEye Rust

An independent Rust rewrite of **GoldenEye 007 for Wii (2010)**, targeting native
Windows. The long-term goal is to reproduce the original game's behavior.

## Playability at a glance

> **Playable now: startup, menus, settings and Extras navigation.**
>
> **Playable missions: 0.** Choosing **New Game** loads Tutorial world data
> and stops at **Preparing player**. Movement and combat are not available yet.

<!-- Keep this summary aligned with what the native build can actually run. -->

| Part of the game | Current state |
| --- | --- |
| Startup and title screen | **Working**: startup screens, logo movies and title animation. |
| Main menu | **Working**: navigation and menu audio. |
| Settings and Extras | Sound/screen settings and Extras navigation work. |
| New Game / Tutorial loading | **Partial**: world data loads; player initialization is unfinished. |
| Player, camera, movement and combat | **Not playable yet.** |
| Campaign missions and multiplayer | **Unavailable.** |

**Road to the first playable Tutorial:**

- [x] Open the game and use startup, menus and settings.
- [x] Load Tutorial world data through New Game.
- [ ] Finish player initialization and pose setup. **Current work.**
- [ ] Enable the gameplay camera and player controls.
- [ ] Play and complete the Tutorial.

**Latest checkpoint:** both player startup animation actions, their callback and
the following constructor cleanup helpers now complete. Pose setup is next;
the player still needs more initialization before gameplay can start.

A reliable whole-game completion percentage isn't available yet. Progress is
tracked by working features and playable missions; test coverage is separate.

[Detailed checkpoint](docs/constructor-continuation.md) ·
[Build and coverage reports](https://github.com/ifBars/goldeneye-rs/actions/workflows/ci.yml) ·
[Open the local progress dashboard](#development)

## Bring your own game

You need your own legally obtained game image. The supported edition is the USA
Wii release, **SJBE52 revision 0**, in RVZ or ISO format.

This repository contains the rewrite, developer tools and synthetic tests. It
doesn't include a game image, extracted assets, original executable code, or
downloads for any of those files. The program reads assets and selected static
metadata from your local image; it doesn't run the original executable.

Keep game images, extracted files and captures out of commits and pull requests.
The project's MIT license does not grant rights to the original game or its
assets. See [asset and licensing boundaries](docs/legal.md).

## Build and run

Install Rust through [rustup](https://rustup.rs/). On Windows, use the MSVC
toolchain and Visual Studio's C++ build tools.

From the repository root:

```powershell
cargo build --release --locked -p goldeneye
.\target\release\goldeneye.exe play 'D:\Games\GoldenEye.rvz'
```

Use Up/Down and Enter to navigate, or select menu rows with the mouse. Esc goes
back. `--skip-startup` opens the menu directly; `--mute` disables audio.

The app saves settings under `%LOCALAPPDATA%\GoldenEyeRewrite`. They are separate
from Wii saves. For other commands, run `goldeneye.exe --help` or read the
[developer guide](docs/development.md).

## Development

Asset inspection and rendering tools are included alongside the native app.

```powershell
cargo xtask check
cargo xtask report
```

The checks run without a game image. The report creates `progress/index.html`,
which you can open locally. The dashboard tracks coverage and compatibility
separately from reference parity. A passing build doesn't mean the game is playable.

See [CONTRIBUTING.md](CONTRIBUTING.md), the [architecture](docs/architecture.md)
and the [roadmap](progress/roadmap.json). Generated reports and local research
files stay out of Git. Ideas for features beyond the original, such as online
play, bots and replays, are kept in [native extensions](docs/native-extensions.md)
and are not part of the parity roadmap.

For parallel multiplayer foundation work, use the
[networking crate plan](docs/networking-crate-plan.md). It defines crate ownership,
milestones, tests and the current implementation handoff.

## License

The rewrite is [MIT licensed](LICENSE). Dependencies retain their own licenses;
see [third-party credits](docs/third-party.md). This is an unaffiliated project.
Game names, characters and branding remain their owners' property.
