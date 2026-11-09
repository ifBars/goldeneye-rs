# Asset and licensing boundaries

This repository distributes independently authored rewrite code and synthetic
test fixtures. It does not distribute the original game, extracted game assets,
original executable code, decompiled source, game captures or game-key files.
The runtime reads the supported game image supplied by the user.

The [MIT license](../LICENSE) applies to this project's code. It does not license
GoldenEye 007, its assets, characters, names or branding. Dependencies keep their
own terms; [third-party credits](third-party.md) explain the notice workflow.
This project is not affiliated with or endorsed by the game's rights holders.

## Using a game image

Use only a copy you are legally entitled to use. This project does not provide
game downloads or instructions for obtaining unauthorized copies. Keep your image
and any extracted data local. Do not upload them to this repository, its issues,
pull requests or build artifacts.

Bring-your-own-assets packaging separates this code from game content. It does
not guarantee that every use, method of obtaining an image or reverse-engineering
activity is lawful. Copyright and access-control rules depend on the facts and
jurisdiction. The U.S. Copyright Office distinguishes program expression from
[functional aspects](https://www.copyright.gov/register/tx-programs.html).
[17 U.S.C. § 1201](https://www.govinfo.gov/content/pkg/USCODE-2024-title17/html/USCODE-2024-title17-chap12-sec1201.htm)
also imposes access-control rules with specific exceptions. These references are
general information, not legal clearance for a particular activity.

## Contributions and distributions

- Contribute independently authored code and small synthetic fixtures.
- Keep extracted payloads, original code, disassembly, decompilation, keys and
  captured game media out of source control and public attachments.
- Preserve licenses, copyright notices and attribution for reused code.
- Run the source-tree check and inspect the complete staged diff before pushing.
- Generate dependency notices for each distributed binary's target platform.
  `cargo xtask package` does this for the current build host and uses a fixed
  asset-free package allowlist.

The automated source check catches common file and credential mistakes. It
cannot determine authorship, rights ownership or legal compliance by itself.
