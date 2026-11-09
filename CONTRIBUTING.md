# Contributing

Read [AGENTS.md](AGENTS.md) and the [developer guide](docs/development.md).
Choose a focused runtime or format problem, preserve unknown data, and explain
which supported edition informed the change.

Only submit code and material you have the right to contribute under this
project's license. Do not attach game images, extracted assets, original code,
decompilation, keys, captured game media or private research to commits, issues
or pull requests. Use small synthetic fixtures for tests. Keep local game data
and probes in ignored paths. See [the asset policy](docs/legal.md).

Before submitting a change:

1. Run `cargo xtask check`.
2. Run the checks relevant to the changed behavior, as listed in the developer
   guide. Keep Cargo and coverage tasks serial.
3. Run `cargo xtask report` before testing dashboard changes.
4. Run `python tools/check-source.py --staged` and inspect the staged diff.

Separate build success, test results, compatibility and reference comparisons in
the description. Do not mark gameplay or an entire acceptance criterion complete
because one helper works. Keep dependency notices when reusing third-party code.
