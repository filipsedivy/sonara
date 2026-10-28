# Implementation status

This file records what Sonara can actually do.

| Status | Meaning |
|---|---|
| **done** | Implemented, documented and covered by automated tests, and works through its intended interface |
| **partial** | Implemented, with the stated gaps |
| **planned** | Designed but not implemented |
| **unsupported** | Out of scope for now |

_Last updated: 2026-10-24 · version 0.1.0 · Rust workspace and Tauri desktop application;
behaviour checked against the recorded golden data in `golden/`_

## Milestones

| # | Milestone | Status |
|---|-----------|--------|
| M0 | Repository, tooling, ADRs | done |
| M1 | Headless vertical slice: domain, commands and undo, `.sonara`, offline render through real effects, WAV/FLAC/MP3 export, CLI and MCP | done |
| M2 | GUI: main window, timeline, track headers, transport, mixer, piano roll, device editors, browser, inspector, export and settings dialogs | done (see gaps below) |
| M3 | Real-time playback (miniaudio), meters, transport, recording | done (recording: partial) |
| M4 | Breadth: more DSP (multiband, transient, modulation, de-esser), time-stretch, sampler, MIDI output | planned |
| M5 | Plugin hosting (VST3/AU/CLAP), advanced metering, hardening | planned |

## Open checks

These need hardware or a person and are never CI gates (HW-01). Their results belong in
[audio-engine.md](audio-engine.md):

- the GUI performance sign-off: `cd apps/sonara-desktop && pnpm run perf:signoff`;
- the audio hardware checks listed in [audio-engine.md](audio-engine.md), including recording
  from a real input, unplugging a device and a real MIDI keyboard;
- VoiceOver, the microphone prompt and the native menus in the bundled app;
- dropping a WAV file from the Finder onto the timeline;
- Claude Code against `sonara mcp`.

## Feature matrix

### Project management

| Feature | Status | Notes |
|---|---|---|
| New, open, save, save as, close | done | GUI, CLI, MCP |
| Duplicate (save copy) | done | `save_project as_copy`; File → Save Packaged Copy |
| Templates | done | empty, electronic, band, orchestral |
| Recent projects | done | Settings JSON; File → Open Recent |
| Metadata, audio settings | done | Title, artist and so on; the sample rate is per project |
| Autosave, backup retention | done | Configurable interval; 10 backups by default |
| Crash recovery | done | Autosave in the bundle or scratch workspace; offered on open and at startup |
| Missing media detection, relinking | done | Silence plus a warning; `relink_media`; integrity verification |
| Versioned migrations | done | The framework exists; no migrations yet (schema v1) |
| Media collection, cleanup | done | Embed linked media; remove unused files, sparing those reachable through undo |

### Timeline and arrangement

| Feature | Status | Notes |
|---|---|---|
| Audio, instrument, bus and folder tracks | done | |
| Clips: move, trim, split, duplicate, loop, mute, delete | done | Audio clip looping is planned |
| Multi-selection; copy/paste | partial | Multi-select, multi-move and duplicate work; a clipboard is not implemented |
| Snapping (bars, beats, subdivisions, triplets, off) | done | Sample-level snapping is planned |
| Time-stretching, pitch-shifting | planned | |
| Fades, crossfades (overlap), clip gain | done | Four fade curves |
| Loop region | done | Punch recording is in the format but not wired to the recorder |
| Markers, sections | done | |
| Tempo maps (steps and ramps), time-signature changes | done | The GUI edits the base tempo; changes and ramps are available through the API |
| Undo/redo for all edits | done | Shared by the GUI and agents |
| Responsive large projects | done | WebGL2 timeline: p95 3 ms per frame at 100 tracks × 40 clips |

### Recording and transport

| Feature | Status | Notes |
|---|---|---|
| Play, pause, stop, seek, loop, return to start | done | Sample-accurate loops; click-free seek and stop |
| Device selection, sample rate, buffer size | done | Settings dialog; applies on restart |
| Input monitoring | partial | Direct (dry) monitoring exists in the engine but is not enabled by the player; monitoring through effects is not low-latency |
| Track arming, multichannel recording | partial | 1–2 input channels per track. The take pipeline is tested on miniaudio's null device; not yet with real input hardware |
| Latency compensation for recording | partial | Uses the backend's latency estimate; a manual offset is planned |
| Record from playhead, count-in, metronome | done | |
| Level meters | done | Device and per-track peak/RMS; 60 fps in the mixer |
| Device error handling, disconnection | partial | Errors are detected, counted and reported; automatic reconnection is planned |

### MIDI and composition

| Feature | Status | Notes |
|---|---|---|
| SMF import/export | done | Type 1, 960 PPQ, tempo and time-signature maps, markers, CC and pitch bend; UTF-8 names |
| Piano roll editing, quantize, velocity | done | Drags within two display frames; 10,000-note clips |
| Piano roll keyboard audition | done | Clicking or dragging a keyboard key plays that pitch through the clip's track instrument; GUI-only, no agent or MCP tool |
| CC automation and pitch bend in clips | partial | Stored, played and exported; the piano roll does not edit CC lanes yet |
| External MIDI devices (input) | done | midir (CoreMIDI). Hot-plug rescans every 2 s. Live playing reaches armed or selected instrument tracks, also while stopped, with a 25 ms live lookahead. Verified with virtual CoreMIDI ports only |
| MIDI recording | done | Armed instrument tracks record into a MIDI clip; timing is mapped through the output-latency estimate |
| MIDI output to external devices | partial | The output port API exists; tracks cannot route to external outputs yet |
| Instruments and presets | done | Poly Synth (13 presets), Drum Kit (5 kits) |
| Chords, scales, arpeggiator, humanize, swing | done | `sonara-midi` and `generate_midi` |
| Programmatic MIDI generation | done | |
| Orchestral ranges, transposition and articulation metadata | partial | `sonara_midi::orchestration`: 16 instruments; several ranges and all articulation lists are estimates; library-independent; no tool or GUI uses it yet ([orchestration-metadata.md](orchestration-metadata.md)) |

### Mixer and routing

| Feature | Status | Notes |
|---|---|---|
| Gain, pan, mute, solo, solo-safe, metering | done | Mute and solo have no render test yet |
| Master, buses, sends (pre/post), inserts | done | A track routed into a bus, and nested buses, have no render test yet |
| Explicit routing graph with feedback rejection | done | |
| Sidechains | done | Compressor (with key high-pass/low-pass) and gate |
| Plugin delay compensation | done | |
| Multichannel (more than 2) buses | unsupported | Stereo mix engine; sources are folded |
| Track and bus grouping (linked faders) | planned | Folders exist for organisation |

### DSP

| Feature | Status | Notes |
|---|---|---|
| Gain/trim, polarity, mono, width, balance | done | Utility |
| Parametric EQ, HP/LP (12–48 dB/oct), shelves | done | |
| Compressor, limiter (true peak), gate, expander | done | The expander mode, the RMS detector and auto makeup have no golden test yet |
| Delay (sync, ping-pong, filtered feedback) | done | |
| Algorithmic and convolution reverb, IR loading | done | Impulse responses load off the render thread |
| Saturation, distortion, clipping (oversampled) | done | |
| Multiband dynamics, transient shaper, chorus/flanger/phaser, de-esser, noise reduction, spectral processing | planned | |
| Parameter smoothing, latency reporting, no clicks | done | |

### Analysis

| Feature | Status | Notes |
|---|---|---|
| Waveforms with cached peaks | done | Peak pyramids on their own worker pool |
| Integrated, short-term and momentary loudness, LRA (BS.1770-4/EBU R128) | done | |
| True peak | done | 4× oversampled estimate |
| RMS, peak, crest, clipping, silence, correlation, width, octave balance | done | |
| Spectrum/spectrogram visualisation in the GUI | planned | The analysis data exists |
| Onset, pitch and tempo estimation | planned | |

### Plugins

| Feature | Status | Notes |
|---|---|---|
| Plugin contract, registry, presets, state | done | |
| Third-party VST3, AU, CLAP hosting | **unsupported** (planned) | See `docs/plugin-hosting.md`; nothing is faked |

### Format

| Feature | Status | Notes |
|---|---|---|
| `.sonara` bundle and ZIP archive | done | |
| Manifest, integrity hashes, required features | done | |
| Atomic save, interrupted-save recovery, backups | done | |
| Schema validation, unknown-field preservation | done | |
| Linked and embedded media, streaming large media | done | Decode cache with positional reads |
| Safe extraction | done | |

### Import/export

| Feature | Status | Notes |
|---|---|---|
| Import WAV/BWF/RF64/W64/AIFF/CAF/FLAC/MP3/Ogg Vorbis/Opus (+ FFmpeg formats) | done | Copy or link; the original is preserved. CAF covers PCM, µ-law, A-law and ALAC; FFmpeg runs as an external process |
| Inspection, metadata, bit depth, rate conversion | done | libsoxr resampling |
| Preview playback of files in the browser | planned | |
| Export WAV PCM/float, BWF, AIFF, FLAC, MP3, Vorbis, Opus, RF64, W64, CAF | done | |
| Bit depth, dither, rate, channels, bitrate, normalization, range, stems, metadata, multi-format | done | Vorbis bitrate is an approximate quality target |
| Export presets | done | In the export dialog |
| AAF and other session interchange | unsupported | |

### Agent

| Feature | Status | Notes |
|---|---|---|
| CLI, IPC, MCP over one registry | done | 57 tools |
| All required tools | done | |
| Schemas, units, error codes, stable IDs | done | |
| Agent edits visible in the GUI; shared undo | done | Tested through real IPC |
| Path sandbox, confirmations, dry run, jobs, cancellation, limits, audit log | done | |
| Demonstrated with Claude Code | partial | `examples/claude_code_demo.md` records a session against the original prototype; the check against `sonara mcp` is open |

### GUI

| Feature | Status | Notes |
|---|---|---|
| Menus, transport, timeline, headers, mixer, browser, piano roll, device editors, inspector, export and settings dialogs, status/progress | done | Tauri 2 + Svelte 5 + TypeScript ([ADR 0009](decisions/0009-gui-toolkit.md)); the mixer can move to a window of its own |
| Dark theme | done | |
| Light theme | partial | The panels follow the light palette, live from the setting; the canvases stay dark |
| Keyboard shortcuts, context menus, drag and drop, zoom | done | Native macOS menu, HiDPI. A drop from the Finder always snaps |
| Accessibility | done | VoiceOver labels on the AppKit tree (18 of 18 checks) |
| Customisable toolbar | planned | |
| Automation lane editing in the timeline | planned | Automation is available through the API and agent |

### Packaging

| Feature | Status | Notes |
|---|---|---|
| Reproducible setup, CI workflow | done | `Cargo.lock`, the pinned toolchain, pnpm's frozen lockfile and the native macOS CI jobs; `release.yml` has not run on GitHub yet |
| macOS app bundle and DMG | done | `scripts/bundle-macos.sh` builds `Sonara.app` and its DMG with the Tauri bundler and checks them: the hardened runtime, the microphone entitlement, the bundled `sonara` CLI, the codec libraries in `Contents/Frameworks`, the licences and the LGPL sources; 38.6 MiB, the DMG 16.0 MiB |
| Signed, notarised app bundles and installers | unsupported | Owner decision (2026-10-23): ad-hoc signing only, no Developer ID and no notarization; users confirm the first launch in System Settings ▸ Privacy & Security ▸ Open Anyway |
| Windows and Linux | unsupported | macOS (arm64) only; no CI job builds for Linux or Windows |
