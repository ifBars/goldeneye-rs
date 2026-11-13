# GoldenEye Rust

An independent Rust rewrite of **GoldenEye 007 for Wii (2010)**, targeting native
Windows. The long-term goal is to reproduce the original game's behavior.

This is early development. Startup screens, logo movies, title animation, menu
audio, sound/screen settings and Extras navigation work. New Game loads the Tutorial world and reaches
**Preparing player**. Player initialization, the gameplay camera and playable
missions are still incomplete. Asset inspection and rendering tools are also
included.

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
