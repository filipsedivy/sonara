# Changelog

All notable changes are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Sonara follows
[semantic versioning](https://semver.org/) once 1.0 is reached; until then minor versions may
break the API. The project format is versioned independently, see
[docs/project-format.md](docs/project-format.md).

## [Unreleased]

## [0.1.10] — 2026-10-29

### Fixed

- **Meters after a pause:** the mixer meters kept showing the last played block while the
  transport was paused or stopped. The live frame now carries meters only while playing or
  recording, so they fall to silence.

## [0.1.9] — 2026-10-29

### Added

- **Reference audio analysis (`sonara_media::reference`):** a streaming, descriptive analysis of a
  reference recording: tempo with a confidence and the half and double alternatives, the energy
  over time and a segmentation by energy changes, each with explicit limitations. File import is
  `sonara_media::export::analyze_reference_file`. Library API only; no melody or arrangement is
  extracted, and no tool, schema or contract changes. See
  [docs/reference-analysis.md](docs/reference-analysis.md).

## [0.1.8] — 2026-10-29

### Added

- **Conversational orchestration (`sonara_agent::orchestration`):** turns a plain-language request
  (tempo, a track's instrument, deleting a track) into a plan of calls to the existing agent
  tools and a report of what changed. Ambiguous requests only produce questions, destructive
  plans wait for confirmation, and every step is validated with `dry_run` before the first
  commit. Library API with no language model; no new tool, schema or error code. See
  [docs/orchestration.md](docs/orchestration.md).

## [0.1.7] — 2026-10-29

### Changed

- **Licence:** Sonara is released under the BSD Zero Clause License (0BSD), replacing "all rights
  reserved". The About dialog, the bundle copyright, the third-party licence text, the crate and
  package metadata and the README state it.

## [0.1.6] — 2026-10-28

### Added

- **Quality checks (`sonara_app::quality`):** explained findings about a project (a missing
  instrument, duplicate or overlapping notes, notes after a clip's end, empty clips, hot faders,
  uniform velocities, tracks silenced by a solo) and about rendered audio (the analyzer's issues
  with an explanation each), split into errors, warnings and recommendations. Library API only;
  no tool, schema or contract change. See [docs/quality-checks.md](docs/quality-checks.md).
- **Orchestration metadata (`sonara_midi::orchestration`):** pitch ranges (playable and
  comfortable), transposition, family and idiomatic articulations for 16 orchestral instruments,
  each value sourced or marked as an estimate, with range checks that warn instead of clamping.
  Library-independent data; no agent tool, no contract change. See
  `docs/orchestration-metadata.md`.
- **Localized regeneration (SON-P11, `sonara-midi::regenerate`):** replaces the notes of a bounded
  region (beats or a section) of one MIDI clip through a `RegionGenerator` interface. The result
  is a validated patch that names the notes removed and added, applied as one undoable step;
  notes outside the region keep their identity, and a stale or invalid plan changes nothing.
  Library API only; no generator ships, no tool, schema or golden behaviour changes. See
  `docs/regeneration.md`.
- **Song form generator (SON-P10, `sonara-midi::form`):** deterministic templates (verse–chorus,
  pop with pre-chorus, AABA, dance) that plan sections with roles, bar lengths and energy and
  density levels on the project's meter grid; apply them as one undoable step, and regenerate a
  single section without touching the others. Library API only; no tool, schema or golden
  behaviour changes. See `docs/song-form.md`.

### Changed

- **New brand mark:** the "Generative Composition" mark replaces the arc-built "S" with sonar
  arcs: scattered dots that resolve into ordered, ascending notes, as ideas become a
  composition. The app icon, the bundle icons (`Sonara.icns`), the horizontal logos, the About
  dialog and the README logo use it; the wordmark is unchanged, and the colours stay Sonar Blue
  to Violet. A simplified variant serves the 16–32 px icon sizes. See `docs/brand.md`.

## [0.1.5] — 2026-10-28

### Added

- **Theory primitives (SON-P05, `sonara-midi::theory`):** exact step-grid rhythm patterns,
  diatonic motifs with pure transforms, and validators that separate malformed data (errors)
  from valid but unusual choices (advisories). Library API only; no tool, schema or golden
  behaviour changes.
- **Transport tests and timing documentation (SON-P03):** tests that a seek, a resume and a
  stop end the notes that sound, and that the note schedule follows a tempo change; the
  timing rules are written down in `docs/audio-engine.md` ("Transport and event scheduling").
  No behaviour changed.
- **Sample-based instruments (`sonara.sampler`, ADR 0010):** a pure-Rust SFZ sample player
  (velocity layers, round robin, keyswitches, release samples, exclusive groups, loops, 64
  voices) for orchestral libraries that you install yourself; Sonara ships no samples. The
  directories in the settings key `sample_libraries` are scanned for `.sfz` files at startup;
  when any exist, the sampler is offered and its `instrument` parameter lists the patches
  (`list_plugins`, `set_instrument`). The 57-tool contract, the `.sonara` schema and the golden
  plugin catalog are unchanged: without a configured library nothing differs. A missing or
  not-installed patch renders as silence with a warning. See `docs/sample-libraries.md` and
  `docs/orchestral-audio-engine-research.md`. Sound quality of real libraries has not been
  listened to.
- **Instrument catalog (`sonara_app::instruments`):** the instrument presets of the installed
  sample libraries, with a stable ID that does not depend on the install path, a family (declared
  by an optional `sonara-library.json` manifest, else inferred from the patch's path and marked as
  such), declared articulations only, a licence note and availability (available, missing,
  invalid). A broken library, manifest or patch is reported as data, never a crash. A library
  API only: no tool, command or GUI uses it yet, and the 57-tool contract, the `.sonara` schema
  and the golden plugin catalog are unchanged. See `docs/instrument-catalog.md`.
- `sonara-io`: the `AudioBackend` trait, an engine-agnostic seam over the device lifecycle
  (implemented by `Engine`), and `MockBackend` for tests. No user-visible behaviour changes.

### Changed

- CI runs the Rust gate as parallel jobs (lint, tests in two halves, loom, RealtimeSanitizer) and
  runs the tests with cargo-nextest. The development profile keeps line tables only instead of
  full debug info. No user-visible behaviour changes.
- The `golden_export` tests of `sonara-media` use a scratch directory per process, so that they
  also pass when every test runs in its own process (test-only change).

## [0.1.4] — 2026-10-25

### Fixed

- The transport integration test in `sonara-app` is stable (test-only change; no user-visible
  behaviour changes).

## [0.1.3] — 2026-10-25

### Added

- **Piano roll keyboard audition:** clicking (or dragging across) a key in the piano roll's
  keyboard plays that pitch through the clip's track instrument, so the user hears which note a
  row is. GUI-only: it opens the device and builds a graph on a stopped transport too, the same
  as live MIDI input, but adds no agent or MCP tool (the 57-tool contract, CTR-01, is unchanged).

### Changed

- **Sonara is a native Rust application for macOS** (Apple Silicon, macOS 26 or later). The
  Python prototype (PySide6 GUI, NumPy/SciPy DSP, C++/nanobind engine) and its tooling are
  removed. The contracts are unchanged: the `.sonara` format, the 57 agent tools and their
  error codes, the IPC protocol and the `sonara` command tree. Behaviour recorded from the
  prototype stays in `golden/` (Git LFS) as the regression oracle for the Rust tests.
- **The desktop application** is Tauri 2 with Svelte 5 and TypeScript (ADR 0009): a WebGL2
  timeline (100 tracks × 40 clips redraw in 3 ms at p95), track headers, a mixer that can move
  to its own window (meters at 60 fps), the piano roll (drags within two display frames,
  10,000 notes paint in 2 ms), device editors for all 12 built-in plugins, the browser, the
  inspector, every dialog, the native macOS menu with shortcuts, HiDPI, VoiceOver labels, and
  the light theme for the panels. `sonara gui` starts it, and `sonara mcp` attaches to the open
  window with a shared undo history.
- **The audio engine** is Rust: `rtrb` rings between the render thread and miniaudio's
  callback, which never allocates, locks or blocks (checked by RTSan, loom and
  `assert_no_alloc`). The render thread joins the output device's audio workgroup: Ascension
  plays at 2.5–2.8 % render load and Night Drive at 1.5–1.8 %, with 0 underruns (the prototype
  with its GUI: 24.7 % and 14.5 %). Offline renders are about five times faster.
- **Codecs:** decoding through symphonia and Sonara's own CAF reader (PCM, µ-law, A-law, ALAC),
  Ogg Opus import, the FFmpeg fallback as external processes; libsoxr, LAME and libFLAC load at
  run time as separate, replaceable libraries; Ogg Vorbis through `vorbis_rs`, Opus through
  libopus.
- **MIDI devices** through midir (CoreMIDI).
- **Packaging:** `scripts/bundle-macos.sh` builds `Sonara.app` and its DMG (16.0 MiB), signed ad
  hoc and not notarized, with the `sonara` CLI inside, the codec libraries in
  `Contents/Frameworks`, the third-party licences and the LGPL sources. `release.yml` publishes
  them with `SHA256SUMS.txt`.
- **Development:** `scripts/dev.sh` runs the Rust and frontend gates natively on macOS; CI runs
  them the same way (owner decision, 2026-10-24: the Linux Docker development container is
  gone). `.mcp.json` runs `sonara mcp` from the checkout. The project rules in `.claude/rules/`
  describe the Rust application.

### Fixed

Defects of the prototype that the Rust application fixes, each pinned by a test:

- exported MIDI files with non-Latin-1 track names;
- renders that ignored the convolver's "Normalize IR";
- the alignment of recorded takes, also after a play on the same duplex stream, and stopping
  always finishes a recording;
- metronome clicks across block boundaries, held notes and processor state across graph
  rebuilds, and fades across stale playback blocks;
- the Podcast export preset's bit rate, stems overwriting files without asking, one fade curve
  shared by both fades, and editor shortcuts taken by the menu.

## [0.1.0] — 2026-10-20 (unreleased)

First vertical slice through every layer of the PRD, plus the GUI and real-time engine.

### Added

**Foundation**

- Repository scaffold: uv workspace (`sonara` + native `sonara-native`), ruff, mypy (strict),
  pytest, CI workflow, architecture decision records 0001–0006.

**Project model and persistence**

- Immutable domain model: projects, tracks, clips, MIDI notes and controllers, devices,
  automation, media, markers and sections. Tempo maps with linear ramps and time-signature
  maps, with closed-form beat/second conversion. The routing graph has cycle rejection and
  solo/mute semantics.
- Command-based editing: ~45 commands, transactions, dry runs, snapshot undo/redo with
  merge windows, change sets, a thread-safe session and view-state commands that stay out of
  the undo history.
- `.sonara` format in two forms, a bundle directory or a ZIP archive, sharing one manifest
  with SHA-256 checks, required features, JSON Schema validation, a migration framework,
  atomic saves, rotating backups, autosave and recovery, safe extraction, integrity checks,
  relinking and unused-media cleanup.

**Audio**

- Offline render graph:
  - plugin delay compensation, sample-accurate volume/pan/send automation, device-parameter
    automation;
  - sidechains, pre/post-fader sends, buses, isolation rendering for stems;
  - auto tails, note chasing;
  - memory-mapped media decoding with soxr VHQ resampling.
- DSP: parametric EQ, multimode TPT filter, compressor with soft knee and external
  sidechain, lookahead true-peak limiter, gate/expander, tempo-synced delay, algorithmic
  reverb (synthesized IR) and convolution reverb with partitioned FFT convolution,
  oversampled saturation, utility.
- Native C++ kernels with Python reference implementations and equivalence tests:
  attack/release smoother, state-variable filter, gate state machine, band-limited
  oscillator bank.
- Instruments: Sonara Poly Synth (13 presets) and Sonara Drum Kit (GM map, 5 kits).
- Native real-time engine (`sonara_native`, miniaudio 0.11.25):
  - lock-free timeline-tagged playback ring and capture ring;
  - click-free flushes, meters, underrun and load counters, direct monitoring;
  - a null backend for CI.
- Real-time player: lookahead rendering, live graph rebuilds that keep DSP state,
  sample-accurate loops, metronome, recording with latency compensation.
- Analysis: BS.1770-4 / EBU R128 loudness, LRA, true peak, RMS, crest factor, clipping,
  silence, stereo correlation and width, octave balance, and plain-language issue reports.

**Import, export and MIDI**

- Import: everything libsndfile reads; FFmpeg fallback that transcodes to FLAC and keeps the
  original; copy or link.
- Export: WAV/BWF (with `bext` v2 loudness)/RF64/W64/AIFF/CAF/FLAC/MP3/Vorbis/Opus, any bit
  depth, TPDF dither, resampling, peak or loudness normalization that respects the true-peak
  ceiling, metadata, stems, several formats per render, and verification of every output.
- MIDI: theory (keys, chords, Roman numerals, voice leading), generators (chords, arpeggios,
  basslines, drum grooves, melodies, Euclidean rhythms), transforms, and Standard MIDI File
  I/O.

**Agent interface**

- 54 tools served by one registry. They are reachable from Python, the CLI
  (`sonara tool`/`batch`), a token-authenticated local IPC channel (JSON-RPC over a Unix
  socket) and MCP over stdio (`sonara-mcp`, attaching to a running GUI).
- Safety: path sandbox, two-phase confirmations, background jobs with status and
  cancellation, limits, JSONL audit log.
- Reproducible example workflows in `examples/`.

**MIDI devices and recording**

- Vendored RtMidi 6.0.0 in the native module: MIDI input/output ports, virtual ports, and
  hot-plug rescanning.
- Live MIDI reaches armed or GUI-selected instrument tracks, including while the transport
  is stopped (idle rendering at a 25 ms live lookahead).
- MIDI recording into clips; audio recording tests on the simulated device.
- Agent tools `start_recording`, `list_midi_devices` and `set_midi_input` (57 tools in
  total).
- Compressor: new `sidechain_lpf_hz` key filter; the `Sidechain Pump` preset now keys on the
  kick.

**Brand**

- Sonara identity:
  - a geometric "S" mark (two half-waves with sonar arcs);
  - a monoline wordmark built from paths, so it needs no font;
  - an app icon with a small-size variant.
- PNG renders, a macOS `.icns` and a Finder icon on `Sonara.command`. Everything is
  generated by `scripts/build_brand.py`; the guide is `docs/brand.md`.
- A new About dialog with the logo, version, author (Filip Sedivy, mail@filipsedivy.cz),
  copyright, runtime/system info (with Copy) and third-party credits with licences.

**Desktop application**

- Main window with dockable browser, inspector, mixer, piano roll and device editors.
- Virtualized timeline with waveform peak cache, ruler, sections, loop region and drag
  editing.
- Transport bar, export/settings/new-project/recovery dialogs, autosave, and window-state
  persistence.
- `Sonara.command` double-click launcher for macOS.

### Fixed

- **Real-time engine.** Blocks made stale by a flush counted as queued audio, so right after
  a seek the render thread could stall and the device underran. Only current-generation
  blocks are counted now (native ABI 3).
- **Agent tools attached to a running GUI.** Relative paths resolved against the GUI
  process's working directory and were rejected. IPC clients now send their working
  directory and allowed folders in `hello`, and the server applies them for that connection.
  The MCP server and CLI do this automatically. A new test covers MCP `--connect always`
  attached over IPC.

- The compressor filtered its sidechain key after rectification; it now filters the key
  signal itself.
- Bus isolation for stems and per-track levels no longer includes the sources' dry path.
- The true-peak limiter targets 0.1 dB under its ceiling, so BS.1770 meters do not read
  above it.
- The drum kit's default output is 6 dB lower, peaking around −6 dBFS.
- The export dialog crashed on open with PySide6's stricter enums.

### Verified

- **Claude Code over MCP.** A real `claude -p` session over MCP made 49 tool calls with
  0 errors. It produced a −14.0 LUFS / −1.07 dBTP export after fixing true-peak, balance and
  sidechain problems found in its own previews (`examples/claude_code_demo.md`).

- 815+ automated tests: EBU loudness references within ±0.1 LU; EQ response matches the
  design to 0.0 dB; codecs checked by aligned correlation of at least 0.9998; IPC agent edits
  appear in the GUI with a shared undo history.
- Real-time playback on Core Audio (MacBook Pro, 48 kHz, 256 frames) with 0 underruns in
  play, seek and loop tests.
- Benchmarks are in `scripts/bench.py`; numbers are in docs/audio-engine.md.
