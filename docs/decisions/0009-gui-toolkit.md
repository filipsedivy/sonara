# ADR 0009: GUI toolkit for the Rust application

- **Status:** Proposed (2026-10-21). The spike ran on 2026-10-22 (results below), and the
  owner chose Tauri 2 the same day; the desktop application is built on it. The ADR becomes
  Accepted after the owner's GUI performance sign-off and manual checks (end of this record).
- **Deciders:** Sonara core. The owner asked for Tauri to be analysed against Qt.
- **Context:** Sonara's native Rust application, which replaced the Python prototype and its
  PySide6 GUI.

## Context

- **What the Rust application had to replace.** The PySide6 GUI has about 8.7k lines:
  - main window with docking;
  - timeline with waveforms;
  - track headers and transport;
  - mixer with meters;
  - piano roll and device editors;
  - browser, inspector and dialogs.
- **Criteria:**
  - 100 tracks × 40 clips repaint in about 7 ms;
  - meters at 60 fps;
  - a piano roll;
  - native macOS menu and file dialogs;
  - HiDPI and accessibility;
  - a signed bundle (decided later as ad hoc, without notarization: owner, 2026-10-23).
- **Constraints:**
  - macOS on Apple Silicon is the only product platform;
  - a proprietary licence: no GPL, no static LGPL, MPL by owner decision;
  - an owner fluent in Python, PHP and web technologies but not in C++ or Rust;
  - heavily agent-assisted development;
  - library first.
- **Research** (2026-10-21; sources in the research notes, summarized here):

| Criterion | Tauri 2 (Rust + web UI in WKWebView) | Qt Quick via CXX-Qt | egui (pure Rust) |
|---|---|---|---|
| Version and maturity | tauri 2.12.4, stable 2.x API, large ecosystem | Qt 6.12 LTS is mature; CXX-Qt 0.10 calls itself "early development, API changes frequently" | 0.37, breaking changes each release |
| Licence | MIT/Apache-2.0. Its tree pulls MPL-2.0 crates (`cssparser`, `selectors` through `kuchikiki`) | LGPL-3.0: Qt must stay dynamically linked and replaceable, which conflicts awkwardly with a notarized bundle. GPL-only modules include Qt Canvas Painter, Quick Timeline, Graphs and the Qml Compiler. | MIT/Apache-2.0 |
| Toolchain | Rust plus Node/pnpm/Vite for the frontend | Rust, C++, CMake, a Qt install (aqtinstall), QML | Rust only |
| Owner and AI fit | High (TypeScript/web) | Low to medium (C++ bridge) | Medium (Rust) |
| Big custom canvases | Canvas2D and WebGL2 in WKWebView; WebGPU partial from Safari 26; **no DAW benchmark exists** | GPU scene graph with custom nodes; used by DAWs | Immediate mode on wgpu; plausible but unbenchmarked at DAW scale |
| Core to UI data | IPC: commands, channels, binary responses. Fetch-emulated, no shared memory on macOS; **60 fps meters unmeasured** | In-process through the generated bridge | In-process, direct |
| Native menu, dialogs, multi-window | Yes (menus, dialog plugin, windows) | Yes | Menus need `muda`; dialogs through `rfd` |
| Accessibility | WebKit's own; custom canvases need an ARIA shadow tree | Qt accessibility; custom items need roles | AccessKit (on by default in eframe) |
| Packaging and signing | Built-in bundler: `.app`, DMG, hardened runtime, entitlements, notarization, updater | macdeployqt plus our own signing; bundle size unmeasured | `cargo-packager`/`cargo-bundle`, not verified |
| GUI tests on macOS | WebdriverIO with an embedded driver; Playwright on the frontend | Qt Test; Squish (commercial) | `egui_kittest` (AccessKit queries, snapshots) |
| Real audio apps | Hackathon-grade only | Zrythm 2, Mixxx (both GPL) | Many plugin GUIs (nih-plug); no DAW |
| Minimum macOS | 11 on arm64; configurable | Qt 6.12: 14.4 | 11 on arm64 |

Slint was excluded: its royalty-free licence requires an attribution widget or badge, and its
GUI test tool needs a licence token. iced was excluded: release gaps and no accessibility.
Dioxus inherits the WebView constraints of Tauri without its maturity.

## Decision

**Tauri 2 is the planned GUI toolkit.** It needs a spike first, and egui is the native
contender.

1. **Tauri 2 wins against Qt for Sonara on these points:**
   - The licence is clean, with no LGPL relinking duties inside a notarized bundle and no GPL-only
     modules to avoid.
   - There is no C++ toolchain.
   - It has a complete macOS bundler with signing, notarization and an updater.
   - The owner can read and steer the frontend.
   - The macOS-only target removes Tauri's worst platform risk, WebKitGTK on Linux.
2. **Qt keeps one advantage: proven DAW-scale rendering.** It remains the fallback if both
   Tauri and egui fail the spike.
3. **The spike decides before any GUI parity work.** A thin Tauri shell and a thin egui shell share one Rust
   fixture: 100 tracks × 40 clips with precomputed peaks, a 60 Hz meter source on a real thread,
   and a 2,000-note clip. Measurements use release builds on Apple Silicon at 2× HiDPI.

| Test | Pass |
|---|---|
| Timeline full repaint while scrolling and zooming | p95 ≤ 7 ms, no frame > 16.6 ms over 10 s |
| Meters: 200 values per frame from a Rust thread | ≥ 58 fps, dropped frames < 1 %, p99 latency from Rust sample to pixel ≤ 50 ms |
| Waveform peaks: 10 MB in 4 MB tiles | ≤ 50 ms per tile at p95 |
| Piano roll with 2,000 notes: drag, resize, lasso | pointer to pixel ≤ 2 display frames at the display's refresh rate (restated by the owner on 2026-10-22; was ≤ 16 ms, see the spike results) |
| Native menu with accelerators, open/save dialogs, a second window | all work |
| VoiceOver on a button, a slider and the track list | label, role and value announced |
| Signed DMG with hardened runtime and microphone entitlement (ad hoc; no Developer ID, owner 2026-10-23) | launches; the microphone prompt appears |
| Licences: `cargo-deny`, plus a pnpm licence allow-list for the frontend | no GPL and no static LGPL; MPL reviewed by the owner |

**If Tauri passes,** this ADR becomes Accepted with Tauri. **If it fails,** the outcome depends
on egui:

- if egui passes, egui is chosen;
- if both fail, Qt via CXX-Qt is re-evaluated with its own spike.

**Shape of the Tauri application** (`apps/sonara-desktop/`):

- **Process model.**
  - The Rust process hosts the whole core: `sonara-app`, the engine, and the agent IPC server.
  - As in the prototype, the MCP server is a separate stdio process (`sonara mcp`). It forwards
    to the running desktop session over IPC, or runs headless.
  - The WebView only renders.
  - Edits go through Tauri commands into `sonara-app` services and commands (ARC-11).
  - **Decided later:** the bundle ships two executables in `Contents/MacOS`: `sonara-desktop`
    (the GUI, the main executable) and the `sonara` CLI; `sonara gui` starts the GUI.
- **Data to the frontend.**
  - Meters and transport status stream through `Channel`s as packed binary frames.
  - Waveform peaks come through binary responses or a custom URI scheme.
- **Frontend stack.**
  - TypeScript with Vite and one small framework, chosen in the spike.
  - `pnpm` with a frozen lockfile, plus a licence allow-list in CI next to `cargo-deny`.
  - The timeline and piano roll are canvas-based.
- **Tests.** Playwright on the frontend with mocked commands. End to end on macOS, Sonara's
  own harness in the `bench` build replaced WebdriverIO, whose npm tree would have needed a
  licence decision.

## Consequences

- **The repository gains a frontend toolchain** (Node, pnpm), which the development container and
  CI must provide. Frontend dependencies get the same licence and advisory scrutiny as crates
  (LIC-01).
- **MPL-2.0 crates enter through Tauri** (`cssparser`, `selectors`). The owner accepted unmodified
  MPL-2.0 dependencies on 2026-10-21. `deny.toml` lists each as an explicit exception.
- **A TypeScript frontend is accepted** within the "completely Rust" goal (owner, 2026-10-21). The
  core, engine, agent and MCP are Rust; only the UI rendering in the WebView is TypeScript.
- **The desktop build needs Rust and Node on the macOS host.**
- **The minimum macOS follows the spike's WebView needs.** OffscreenCanvas needs Safari 17
  or later; WebGPU is partial from Safari 26 on. The spike measures with and without WebGPU so
  that the minimum macOS can be set deliberately (it is macOS 26).
- **Custom-drawn views need an accessibility layer of their own:** an ARIA or DOM shadow tree
  for the timeline and piano roll.
- **Plugin editors (M5) are separate native windows** owned by the plugin-host process in every
  option. A WebView cannot embed a native view.

## Spike results (2026-10-22)

The spike was a Cargo workspace and pnpm project of its own, removed once the desktop
application took its place:
- a shared fixture;
- a Tauri 2.12 shell with Svelte 5, Vite 8 and TypeScript;
- an egui 0.37 shell on wgpu;
- benchmark and licence scripts.

The measurements ran on an Apple M5 Pro with macOS 27, WebKit 22625 (Safari 27), as release
builds at 2× HiDPI. They are provisional. The Tauri numbers come from 2 of 3 runs, the egui
numbers from one shortened run, because WebKit stops drawing while the screen is locked and the
unattended series stalled.

| Test | Tauri 2 | egui |
|---|---|---|
| Timeline, 100 × 40 clips | WebGL2: p95 2 ms, max 5–7 ms, pass. Canvas2D: p95 18–89 ms, fail. | p95 0.9 ms, pass |
| Meters, 200 values at 60 Hz | 59.9 fps, 0.25 % dropped, p99 44.5 ms with a two-frame queue, pass | 60.3 fps, 0 % dropped, p99 16 ms, pass |
| Waveform peaks, 4 MB tiles | p95 2 ms, custom URI scheme or binary command, pass | 0.5 ms in process, pass |
| Piano roll, pointer to pixel | about 33 ms: event to drawn about 16 ms, plus one 60 Hz frame | about 16–17 ms at 120 Hz |
| Second window, menus, dialogs | work; the menus and dialogs await the owner's manual check | work |
| Signed DMG, hardened runtime, microphone entitlement | 2.7 MB DMG; `codesign --verify --deep --strict` passes; no JIT entitlement needed | 4.8 MB DMG; passes |

- **The piano-roll threshold as first written cannot be met.** "Pointer to pixel" was measured to
  the start of the frame after the draw, so its floor is one display frame: 16.7 ms at 60 Hz.
  WKWebView runs `requestAnimationFrame` at 60 Hz even on a 120 Hz panel. The owner restated
  the criterion in display frames on 2026-10-22. That corrects the definition; it does not lower
  the bar. Tauri's figure is a lower bound, because the synthetic events skip WebKit's input
  pipeline.
- **Licences:**
  - Tauri links `option-ext` (MPL-2.0) into the app, and builds with `cssparser`,
    `cssparser-macros`, `dtoa-short` and `selectors` (MPL-2.0). Vite 8 builds with
    `lightningcss` (MPL-2.0).
  - The owner accepted these as unmodified MPL-2.0 (2026-10-22). Each one is listed as a
    `deny.toml` exception.
  - egui's bundled fonts are OFL-1.1, which is not on the allow-list; that no longer matters for
    the chosen direction.
  - No GPL, AGPL or LGPL anywhere.
- **Owner decision (2026-10-22): Tauri 2 with TypeScript.** It keeps the language goal (Rust + TypeScript +
  C/C++ for native libraries only) and a frontend the owner can read. It accepts a piano-roll
  response of about two 60 Hz frames and less headroom on the meters than egui.
- **Consequences for the desktop application:**
  - the timeline is WebGL2; Canvas2D is unusable at timeline scale;
  - meters use a two-frame queue;
  - the app opts out of App Nap (`NSProcessInfo.beginActivity`);
  - every channel message passes the Rust main thread;
  - the Linux development container cannot compile the desktop crate without WebKitGTK, so its
    gates run on macOS;
  - the minimum macOS is the oldest macOS that can be tested. The spike needs only WebGL2
    and Canvas2D, and was tested on macOS 27 only.
- **Measured in the desktop application** (2026-10-22 to 2026-10-24, unattended): timeline p95
  3 ms at 100 × 40 clips, meters 60 fps with a p99 of 38 ms, piano-roll drags within two display
  frames and 10,000 notes painted in 2 ms, VoiceOver checks on the AppKit tree 18 of 18, the DMG
  16.0 MiB.
- **Before Accepted (the owner, attended):**
  - the GUI performance sign-off on the real application
    (`cd apps/sonara-desktop && pnpm run perf:signoff`, see `docs/audio-engine.md`), which
    replaces the spike's rerun;
  - the microphone prompt;
  - VoiceOver on the button, slider and track list;
  - the menu shortcuts and dialogs.

