# Third-party credits

The rewrite uses open-source dependencies. Their licenses apply to those
dependencies; the project's MIT license does not replace them.

Rust versions are pinned in Cargo.lock. Direct dependencies include nod, wgpu,
winit, cpal, jpeg-decoder, png, serde, clap, anyhow, sha2 and pollster. Cargo
downloads their code separately; it is not vendored in this repository.

`cargo xtask notices` collects license and notice texts for the current Rust
build host into THIRD_PARTY_NOTICES.md. `cargo xtask package` regenerates and
includes that file with the executable. The generated document stays out of Git
to avoid duplicating the same license texts throughout source history. Review
the resolved dependency set again when distributing for another platform.

Some published crates omit their workspace-root licenses. The unmodified copies
under [third-party/](../third-party/README.md) fill those gaps. That directory
contains license supplements, not vendored game or library implementations.

The optional dashboard test tools are pinned in tools/dashboard/bun.lock:

| Package | License |
| --- | --- |
| happy-dom | MIT |
| @types/node | MIT |
| @types/whatwg-mimetype | MIT |
| @types/ws | MIT |
| buffer-image-size | MIT |
| entities | BSD-2-Clause |
| undici-types | MIT |
| whatwg-mimetype | MIT |
| ws | MIT |

These packages are development dependencies. No node_modules contents are
committed or bundled into the native executable or generated dashboard.

Format research can refer to other projects, including vgmstream. A format
reference is not a license to copy its implementation; copied code would require
its own provenance and applicable notices.
