# Native startup and main-menu checkpoint

`goldeneye play` opens the native Windows frontend using a user-owned SJBE52
revision-zero image. Its movies, logo audio, title/menu music, intro cue, game textures and condensed font
come from that image. Default launch runs the startup sequence before the title
and host-adapted menu. Options and Extras now have source-backed page trees;
Sound Settings and Screen Settings have working sliders and reset. Unsupported
actions remain explicit. The checkpoint does not launch missions, create/load
profiles or connect multiplayer.

```powershell
.\target\release\goldeneye.exe play '.\GoldenEye 007 (USA) (En,Fr).rvz'
```

With no disc argument, the launcher looks for one RVZ/ISO beside the executable
or in the current directory. Ambiguous or missing images require an explicit
path. Disc-format/edition support still follows the existing asset reader; an
extension alone does not establish compatibility. No extracted assets, fonts,
original program code or reference screenshots belong in the package.

`--skip-startup` opens the menu directly; `--mute` disables native audio explicitly.
`--title-aspect wide` is the host default for the source 16:9 title projection;
`--title-aspect standard` selects its 4:3 projection. This chooses the camera
aspect independently of the existing 640×480 capture canvas and letterboxing.
The original user's Wii display flag is external state and is not recovered
from the image. Selecting either mode does not establish original display parity.

Developer `--start-at` accepts `strap`, `zapper`, `activision`, `eurocom`, `mgm`,
`legal`, `title`, `main-menu`, `strap-fade` and `zapper-fade`, and requires bounded
`--frames`. Stage entry is explicit and resets its clock and diagnostics; it
does not simulate skipped boot stages through accumulated timeout offsets.
The two fade probes start an accepted warning fade while retaining that warning:

```powershell
.\target\release\goldeneye.exe play '.\GoldenEye 007 (USA) (En,Fr).rvz' --start-at title --frames 45 --hidden --mute --capture local/title.png
.\target\release\goldeneye.exe play '.\GoldenEye 007 (USA) (En,Fr).rvz' --start-at strap-fade --frames 8 --hidden --mute --capture local/strap-fade.png
```

`--smoke-confirm-title` also requires `--start-at title --frames`. It requests
confirmation through the same app handler used by keyboard/mouse once the source
intro permits it. This is an internal developer probe, with no OS input automation
or original-input comparison:

```powershell
.\target\release\goldeneye.exe play '.\GoldenEye 007 (USA) (En,Fr).rvz' --start-at title --frames 240 --smoke-confirm-title --hidden --mute --capture local/title-outro.png
```

The default order is strap warning → Zapper warning → Activision → Eurocom →
MGM → legal → title. Confirming the held title plays its outro before entering
the adapted main menu; the original profile-dependent transition is not
reconstructed. Warning confirmation is accepted only after 1.5 seconds, with a
21-second timeout for each warning. Either acceptance path starts a separate
0.5-second transparent-to-black fade while retaining the current warning; repeated
confirmation cannot restart or shorten it. The warning timer and incoming delta
round to f32 on every update before these thresholds are tested.
The strap artwork follows source frame boundaries at 30 fps: A before frame 170,
B from 170 to 324, A from 325 to 479 and B from 480 onward. The host currently
holds legal for at least 301/30 seconds, using an explicit visual-script-duration
fallback; confirmation does not shorten it. Legal entry has a global 0.5-second
black-to-transparent fade; the legal asset itself has no alpha track. After this
fade, an asynchronous worker loads the supported `ResourceAction` tables and
retains the bound result. Legal exit also requires this real dependency to be
ready. Other native initializers, the sixteen resource-queue slots and menu-0f
lifecycle remain unimplemented, so this does not establish the original loading
lifetime. No legal exit fade is inferred.
The global fade is visual and does not change PCM gain. Logo confirmation
uses the source strict frame>90 limit. Host Enter/Space confirmation remains an
input adaptation, rather than a Wii button-mask implementation.

A new fade skips elapsed advancement on its first UI update. Existing fades
update before boot logic; completion clears the overlay before the screen changes
in the same update. Normalized RGBA lanes interpolate with separate f32 operations
and truncate to bytes, so midpoint alpha is 127 rather than 128. Fade boundaries
stop synthetic elapsed-time catch-up; they do not stand in for the original
asynchronous resource/workqueue schedule.

| Movie | Frames | Header rate | Audio |
| --- | ---: | ---: | --- |
| Activision | 311 | approximately 29.97 fps | stereo, 32 kHz |
| Eurocom | 111 | approximately 29.97 fps | stereo, 32 kHz |
| MGM | 326 | approximately 29.97 fps | stereo, 32 kHz |
| Frontend background | 991 | approximately 29.97 fps | none in this THP |

The background loops for approximately 33 seconds. Its THP is video-only;
title/menu music comes from a separate source-selected sound bank. Native WASAPI
output submits decoded logo PCM, the stereo title/menu track and the independent
nonlooping title-intro cue. Exact
outer frame cadence, profile/save flow, original mixing and audio/video synchronization
remain unverified.

## What is reconstructed

Bounded static SJBE52 research identifies boot resource `fe_boot.edb`
(`0x01080001`) and the frontend resource `fe_main.edb` (`0x01080004`). The main
frontend resource identity appears at `80234450`; the transition at `802354c4`
requests that resource. Default frontend setup at `8023c470` reaches menu-manager
`80230a90` with menu ID `0x7d00000a`. Its source definition binds title script
`0x04001bb6` and links to the actual main menu `0x7d000009`, which binds script
`0x04001bca`. Complete profile-dependent transitions remain open.

The implementation keeps responsibilities separate:

- `ge-assets::frontend_font` reads the font directory and sparse glyph pages.
- `ge-assets::frontend_text` resolves bounded English text through the EDB page
  mapping and native lookup order. `goldeneye::menu::credits` loads the source
  credit sequence and parses its literal line separators and font prefixes.
- `ge-core::frontend` keeps the pure menu model, screen tree, settings and tests
  together under `crates/ge-core/src/frontend/`.
- `goldeneye::menu` owns host menu sessions, preferences, cached sound effects
  and reference-canvas layout; file I/O and devices stay outside the pure model.
- `ge-core::frontend_sequence` owns startup stages, source timing gates and the
  host readiness gate for the implemented legal dependency subset.
- [`ge-assets::spreadsheet`](../crates/ge-assets/src/spreadsheet.rs) reads checked twenty-byte directory entries and lazy
  typed rows; [`ge-core::action_tables`](../crates/ge-core/src/action_tables.rs) binds ordered action/composite references.
- [`goldeneye::startup_resources`](../apps/goldeneye/src/startup_resources.rs) starts that worker only after legal entry fades,
  polls completion and retains its tables. Unimplemented initializers stay explicit.
- `ge-core::frontend_fade` owns the global visual RGBA fade, first-update skip,
  truncation and auto-clear; the native frontend composites its overlay last.
- `ge-assets::frontend_script` reads source command chains and addressed tracks;
  `frontend_morph` reads component-five color frames.
- `ge-core::frontend_animation` evaluates bounded commands and positive-count
  pose/color tracks; `frontend_segments` handles inclusive intro/outro timing.
- `ge-core::frontend_projection` binds the source title camera and per-vertex
  depth; `frontend_sort` orders entity groups using their stored bounds.
- `goldeneye::frontend_title` binds the source title geometry, UVs and animation
  to native draw layers.
- `ge-assets::thp` indexes checked frame headers, adapts baseline JPEG scans and
  decodes sequential DSP ADPCM packets without an external video process.
- `ge-assets::frontend_stream` resolves a single-layer resident bank sound and
  decodes planar NGCA DSP; its separate DAT8 reader handles IMA streams.
- [`ge-assets::sound_info`](../crates/ge-assets/src/sound_info.rs) reads checked runtime class-three MUSX metadata and
  preserves the native bounded lookup and owner-release flag.
- [`goldeneye::frontend_cue`](../apps/goldeneye/src/frontend_cue.rs) resolves the actual indexed bank and sound metadata,
  fully decodes the nonlooping intro cue and exposes immutable shared PCM.
- `goldeneye::frontend_media` decodes on a bounded worker; `frontend_audio`
  keeps the primary PCM queue and independent bounded one-shots for WASAPI.
- `goldeneye::frontend_music` selects the title sound and progressively decodes
  it on a bounded worker for the title and main menu.
- `goldeneye::frontend` binds local image/font assets to the reference canvas.
- `goldeneye::frontend_window` adapts native keyboard/mouse events and presentation.
- `ge-render::frontend` draws textured alpha/additive mesh layers and quads on a 640×480 canvas,
  letterboxed into the actual window.

The title overlay uses source mesh geometry and UVs across 13 frontend scripts.
Positive-count tracks drive translation, quaternion rotation, scale and alpha.
Source UI markers provide intro frames 0–29 at 60 fps, held at frame 29 until
confirmation starts outro frames 30–49. Time must pass strictly beyond each
inclusive endpoint to complete that segment. The host clamps the displayed root
frame; native controller overshoot/rewind callbacks remain outside this adapter.
An independent tagged-child clock keeps the glint's color-frame animation moving
while the root holds. This component changes per-corner RGBA rather than replacing
positions, UVs or texture images; its blend uses byte/256 weights and separately
truncated contributions.

Mesh materials use clamped 2× texture/raster RGB modulation before blending;
alpha is singly modulated. UVs and colors interpolate with perspective. The
title uses the source UI root at `(0, -10, 0)` and camera at `(0, 18, 0)` rather
than a fitted pixels-per-unit scale. Its field of view preserves the source
conversion denominator `3.1459` before the SDK half-angle conversion; it is not
silently replaced by mathematical pi. The host selects the source wide/standard
aspect as described above. Native SDK tangent, fixed-point/filter behavior,
depth-buffer behavior and Wii numerical precision remain unverified.

Type-two translucent ordering uses each entity's original stored bounds center
and view Z, with all its batches grouped. Computed keys receive repeated f32
bias increments; exact negative-key ties reverse their input order. This does
not recover every native material or visibility state. The manually positioned
prompt, main-menu layout, capture canvas and letterboxing remain adaptations.
The general script VM and remaining external control/audio cues remain open.
The root opcode-five intro cue is supported as described below; other external
commands remain recorded rather than executed. Unsupported reached commands and
curves fail explicitly. These source bindings are not verified visual or display parity.

The main-menu composition remains authored from a captured Wii menu reference.
Positions, pivots, connecting shapes, colors and row spacing are not yet a
complete interpretation of the original visual scripts. Recovering a source
texture or menu ID does not verify the whole arrangement. The THP background
resource is user-owned disc content from `Movies/US/frontend.THP`, decoded as
moving video. Its selected timeline and texture uploads remain separate from the
title evaluator and the incomplete main-menu visual-script composition.

The supported legal worker reads four action rows and ten composites. Their
forty references currently resolve to 31 action bindings and nine native-null
slots. Storage order, duplicate keys and unresolved references are preserved;
a null reference is not invented work or a failed load. These counts describe
the observed resource, not overall startup completion or compatibility.

## Title music and intro cue binding

Boot code at `8000de9c..8000dec4` sets music mode two after frontend setup and
plays sound `0x2d700109`; the frontend callback at `80244000` also plays it.
The COM master bank-membership table associates that sound with bank
`0x2d500017`, `com_sb_2d500017_sb_lvl_test.sfx`. Its sound descriptor's paired
field-relative pointers select internal wave `0x2da021bf`. Native voice binding
at `8039dc30` distinguishes this internal wave from external streams. The small
external stream in the same bank belongs to a different sound and is not used
as a substitute.

The selected MUSX v10/Wii bank has SBNK version `0x1e`. Its wave stores
3,390,912 sample frames at 31,999 Hz in two channels, approximately 106 seconds.
Each channel has its own 64-byte NGCA header and sixteen signed DSP coefficients,
with a full planar channel stride. Histories persist between bounded chunks;
the DSP prediction includes rounding before saturation, unlike the THP decoder's
truncating variant. Unconfirmed nonzero initial histories, composite sounds and
other codec flags produce explicit errors. Coefficients and decoded PCM stay
out of distributable progress summaries.

Native codec bit three marks the selected wave for looping and its stored loop
start is sample zero. The current host restarts the complete track with fresh
history. Original hardware loop-state restoration, bus gain, remaining layered
cues, transitions and synchronization remain unverified. Host linear resampling,
device-channel conversion, summation and final output limiting are presentation
adaptations. Focus pause and clip
transitions silence/pause the output; repeated resumes do not restart it.

A bounded native title run confirms decoded music reaches WASAPI with advancing
source/output counters and no reported underrun or device error. These counters
measure host submission, not measured speaker audibility. A separate complete-track
semantic comparison matches an independent DSP decoder, while original runtime
audio comparison remains pending.

The title root `0x04001bb6` requests sound `0x2d7011ad` at intro frame 11
(60 fps), using script gain 1.0, pitch zero and pan zero. This is an intro cue;
confirmation does not play it again, and seek/rebuild suppresses its request.
Bank `0x2d500001` selects resident wave `0x2da01cd0`: 11,256 stereo frames at
22,050 Hz, approximately 0.510476 seconds, without a loop. Runtime sound-info
resource `0x2d200000` comes from the checked `File_USA` index. Its actual flags
`0x1fc` clear stop-on-owner-release, so root slot removal at frame 36 detaches
ownership without stopping the cue.

The immutable cue loader serializes source identity, PCM hash and decode counts;
it never exports or serializes samples. Independent bounded one-shot voices mix
with the 31,999 Hz primary music on the host output clock. Holding root frame 29
does not freeze audio, and priming the primary format does not discard voices.
Callbacks retain completed PCM until a control-thread retirement call. Explicit
transport clear discards both primary PCM and one-shots.

A bounded 120-frame native title probe recorded one cue request, one started and
completed voice, 24,503 one-shot output frames, continued music consumption and
no reported audio error. This proves that host submission path; speaker output,
Wii mixer arithmetic and original timing equivalence remain unverified.

## Font data and layout constraints

The font directory is EDB table twelve. It uses native coded reference lookup and
16-byte descriptors containing hash, section/debug fields, body address and a
raw runtime pointer. The latter is retained as metadata, never dereferenced.
The font body starts with an atlas/material selector, followed by 256 signed
field-relative page pointers. Each nonnull page contains 256 field-relative
glyph pointers, indexed by the high and low bytes of one UTF-16 code unit.
Null pages and glyphs remain absent; the reader does not synthesize replacements.

Each glyph is 24 bytes: four finite UV floats, a width/advance halfword, a height
halfword and two remaining metric words retained as signed raw values.
`801a5130` performs the native two-level lookup; `801a52e0` uses glyph `+0x10`
as the scaled horizontal advance and `801a5320` reads `+0x12` as height.
The source can retry another font on a missing glyph. Its line wrapping,
escape handling and integer truncation of running scaled advances are additional
layout behavior; a basic glyph renderer does not establish all of them.
The meaning of the last two metric words remains unconfirmed, so no extra
bearing, kerning or padding policy is invented from them.

`o_bondfont.edb` (`0x0109002a`) contains font IDs `0x0700002e`, `0x0700002f`,
`0x07000030` and `0x07000033`. The first three contain ordinary ASCII glyphs with
stored heights 27, 32 and 20, respectively; the fourth is a separate symbol
mapping. `o_fonts.edb` (`0x0109000a`) also contains font `0x07000028`.
The checkpoint currently uses the 32-high Bond font, resolving its raw selector
through that resource's texture directory. A selector is not a texture hash.

Decoded atlas UVs run from top-left to bottom-right, matching the decoded PNG
orientation; no vertical flip is needed. A private atlas/UV inspection rendered
an upright phrase using the actual glyphs. Raw UV values are retained rather
than recomputed from a guessed grid. Font work is bounded by 256 pages and
65,536 glyph entries, with checked descriptors/pointers, finite UVs and explicit
errors on consumed malformed data. Unused atlas slots do not become glyphs.

The current font tests use authored binary fixtures for sparse Unicode pages,
negative relative pointers, aliasing, descriptor identity, coded/hash lookup,
missing entries and malformed/truncated data. These checks do not verify
original rasterization, blending, Wii filtering or full language behavior.

## Text, inputs and state

`o_text.edb` (`0x01090002`) contains UTF-16BE text in spreadsheet sections. Its
full payload is needed; the much smaller base segment excludes the text bodies.
It contains both current Wii strings and older game leftovers. Finding an English
string alone does not bind it to this menu, profile state or action. The bounded
English reader follows the page mapping and native biased binary search;
other languages and live section residency remain separate work.
The source's static twenty-byte menu rows now bind the implemented tree to exact
text IDs. Their sixteen-byte text records use field-relative UTF-16BE string
pointers. This resolves the selected English labels without mistaking legacy
strings for current menu entries; complete production localization and language
selection remain separate work.

The checkpoint shows New Game, Continue Game, Select Profile, Mission Select,
Multiplayer, Options and Extras. New Game is initially selected. Continue Game
is visible but disabled without explicit save availability; the current native
window supplies no save. Up/down selection wraps and skips disabled rows.
Options and Extras enter the source page trees below. Unavailable leaves still
acknowledge their selected action. Back restores the parent page and its selected
row at every depth; back on the root menu does nothing.
The displayed player/profile text is checkpoint presentation, not an authenticated
platform sign-in or loaded game profile.

Keyboard input uses Up/Down or W/S, Enter/Space to select, Left/Right to adjust,
R to reset the current Sound or Screen Settings page, and Escape/Backspace to
return. Mouse interaction selects/activates enabled rows within the adapted
canvas. These controls, hit regions and unavailable pages are explicit host
adaptations. They do not establish Wii Remote/pointer behavior, controller
debouncing, original input timing or complete native menu-stack semantics.

### Options, Extras and preferences

The source order and IDs are:

| Page | Source menu | Rows in order |
| --- | --- | --- |
| Main Menu | `0x7d000009` | New Game, Continue Game, Select Profile, Mission Select, Multiplayer, Options, Extras |
| Options | `0x7d000015` | Control Settings, Screen Settings, Sound Settings |
| Control Settings | `0x7d00007b` | Button Layout, Wii Remote Presets, Controller Settings, ADS Snap |
| Screen Settings | `0x7d000016` | Brightness |
| Sound Settings | `0x7d000018` | Music Volume, Sound Volume, Dialog Volume |
| Extras | `0x7d0000d2` | Credits, Cheat Codes |

Control rows currently lead to explicit unavailable pages; they do not pretend
to change Wii controller state. Cheat Codes acknowledges the source action but
does not accept or activate a code yet.

Credits opens a real scrolling roll from the user-owned text resource. Source
initialization selects 51 text IDs in its original order; the observed target
parses into 1,287 lines, including blank lines. The reader preserves page order,
the native biased lookup, literal `\n` separators and ordered `[font0]` /
`[font1]` prefixes. Two local fonts render the content. The host removes leading
empty padding so text appears promptly and retains internal blank lines and
24-pixel line spacing. The readable canvas, clipping region, colors and adapted
scroll clock remain host presentation choices; source content/order does not
establish original credits visual or timing parity. Back restores Extras and
its selected Credits row without changing preferences.

Sound values range from 0 to 100, change by four and clamp at the endpoints.
Defaults are music 66, sound 100 and dialog 100. The source gain conversion uses
single-precision normalization followed by a double-precision square root and
rounding back to f32. Music and effects gains apply to active host playback;
decoded PCM is immutable. Dialog volume is retained, with no speech stream yet
available to demonstrate its audible application.

Brightness ranges from 25 to 125, changes by five and defaults to 100. Its
linear scale is applied by the host display adapter; original Wii display/gamma
equivalence remains unverified. Reset changes only the current category, retains
the current row and emits an adjustment only when a value changes. A slider at
its endpoint, a repeated hover and root Back do not create navigation sounds.

The app saves a versioned, bounded Windows preferences document at
`%LOCALAPPDATA%\GoldenEyeRewrite\frontend-settings.json`; `--settings-file <path>`
selects a separate document for tests or another configuration. Writes are
debounced and flushed on orderly exit. Missing files use source defaults;
malformed, oversized or unsupported documents produce an explicit error and are
preserved. This is host persistence, not reconstruction of Wii save/profile data.

### Menu effects

Confirmed source menu effects use generic bank `0x2d500001` and checked runtime
metadata `0x2d200000`:

| Accepted change | Sound | Wave | Stereo frames at 32 kHz |
| --- | --- | --- | ---: |
| New selected row | `0x2d70044a` | `0x2da01128` | 4,480 |
| Accepted action | `0x2d70044d` | `0x2da01125` | 23,184 |
| Return to parent | `0x2d700449` | `0x2da01127` | 9,016 |
| Changed setting | `0x2d70044e` | `0x2da01126` | 2,688 |

These nonlooping effects preload once into bounded shared PCM and use the
independent voice transport, preserving the music clock and existing tails.
Only successful state changes request them. At the host's eight-voice limit a
new menu event is dropped and counted; native priority/voice stealing is not
claimed. Original Wii input dispatch, mixer behavior, timing and speaker output
remain separate verification work.

## Evidence and remaining work

```powershell
cargo xtask menu '.\GoldenEye 007 (USA) (En,Fr).rvz'
cargo xtask startup '.\GoldenEye 007 (USA) (En,Fr).rvz'
.\target\release\goldeneye.exe frontend-title-audit '.\GoldenEye 007 (USA) (En,Fr).rvz' --output local/title-audit.json
```

`frontend-title-audit` runs a deterministic 40-second source-title tick and
outro check, writing metadata to the requested new JSON file. It exercises the
held root, independent glint clock and title transition without a native window
or OS input automation. Its RGB-pattern metric ignores alpha, instance order
and duplicate mesh population; a guard rejects independent RGB tracks in the
reachable title scripts. Intentional gaps between glints remain valid. It is an
asset/runtime diagnostic, with no comparison
against original execution, rendered pixels or actual input behavior.

The `menu` task builds the native executable, opens the muted direct-menu
shortcut, requests three hidden frames and a smoke
resize, and saves a local capture plus a source-bound `menu` receipt. It checks
the initial main-menu/New Game state, one asset upload, required texture set,
renderer identity, capture metadata and matching PNG/header dimensions. Failures
remain failed receipts. It does not inject input, perform a second pixel decode
or compare the capture against the original game. A fresh passing gate is a
checkpoint presentation result, separate from visible inspection and parity.

The separate `startup` gate decodes every indexed video frame in the four clips
and every startup audio packet, checks sample totals against the movie headers,
and records RGBA/PCM hashes, including the full resident title-music wave.
Bounded hidden native probes start each logo and the title with audio. They require advancing video, at least two decoded
frames, capture metadata, and nonzero WASAPI source-frame consumption/output
submission without reported device errors. Separate first/last title captures
must differ. A longer Eurocom probe must transition into MGM after consuming
its audio tail. These counters prove host PCM submission, not measured speaker
audibility or agreement with original timing. The initial menu checkpoint keeps
its separate denominator of one launch check.

Bounded warning-fade and legal-entry probes sample first, middle and completion
phases. Playback metadata includes black RGBA bytes, active/pending flags,
elapsed/duration/remaining seconds and warning-fade state. These checks require
the warning to persist during its fade and the next stage to appear after
completion. Captures and host phase diagnostics remain distinct from an original
pixel comparator or proof of the original asynchronous loading lifetime.

A native frame/capture demonstrates the named renderer/backend and current
checkpoint state. A glyph/UV inspection demonstrates only the inspected font
mapping and orientation. Authored state tests demonstrate their supplied
selection rules. Keep these claims separate from interactive input testing,
original game comparisons and full gameplay compatibility.

Complete native rendering states and script control, remaining cues/mixing/transitions,
profile/save conditions, full localization, remaining submenus and mission transitions
remain open. This is a reference-informed startup/main-menu checkpoint,
not full frontend parity or proof that the game is playable. No roadmap parity
criterion is promoted by decoding a font or presenting the menu.
