# Native map inspection

The Windows executable now has a native winit/wgpu inspection window. It loads
the user's local map through the same checked asset path as `render-map`. This is
an engine inspection surface; player movement, physics, HUD, audio and original
camera/input behavior are not implemented.

```powershell
.\target\release\goldeneye.exe view-map '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb --placements --eye -252 7 -8 --target -270 3 -27
```

Left-drag orbits the inspection target. Right/middle-drag pans it; the wheel zooms.
Home restores the initial camera and Esc closes the window. Resizing changes the
viewport and depth texture, while scene geometry/textures remain resident. Loss
of focus clears dragging. Static views wait for events instead of running a busy
render loop. Acquisition timeouts/occlusion retry later; zero-sized windows pause.
Unsupported entity omissions stay explicit. The unlit shader/state limitations
in `asset-formats.md` still apply.

All Dam placement records decode strictly: 240 render instances plus one navigation
mesh retained as data. Navmesh vertices do not become visible triangles or alter
the inspection camera bounds. This does not establish full original-level rendering.

## Automated local graphics check

```powershell
cargo xtask viewer '.\GoldenEye 007 (USA) (En,Fr).rvz'
```

This opens a hidden native window, uploads the scene once, submits three frames,
requests a resize from 1280×720 to 960×540, captures the final surface image, and
compares its normalized RGBA SHA-256 against an offscreen frame. Captures/raw
results stay in ignored `local/viewer-smoke`; the dashboard receives aggregate
counts, adapter/backend, format and hashes. The check fails on incomplete frame
counts, missing resize/upload evidence, empty captures or different pixels.

Hidden smoke uses the app's bounded frame tick because a hidden window may not
receive OS paint events. It does not synthesize desktop input. Surface presentation
calls and copied pixels are observed; visible compositor output and mouse/keyboard
interaction are not validated by that check. Human interaction QA remains open.

The separate GPU fixture covers source texture RGBA versus BGRA output, camera
uniform updates and target resize/reset without reuploading the scene, in addition
to texture sampling, alpha, depth, model transforms and row-aligned readback.
CPU tests cover orbit/pan/zoom geometry, clipping bounds and invalid-input recovery.

Manual bounded runs can use `--frames N`, `--hidden`, `--smoke-resize` (N ≥ 3), and
`--capture local/new-file.png`. Existing outputs are preserved. Capture requires
surface `COPY_SRC` support; unsupported adapters fail explicitly. Startup and frame
errors return a failing process exit. Inspection remains distinct from gameplay.

The pinned API references are [winit 0.30.13](https://docs.rs/winit/0.30.13/winit/)
and [wgpu 30.0.1](https://docs.rs/wgpu/30.0.1/wgpu/). No original game code is
executed or included in distributable files.
