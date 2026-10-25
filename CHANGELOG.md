# Changelog

All notable changes are recorded here. Sonara follows semantic versioning once 1.0 is
reached; until then minor versions may break the API (the project format is versioned
independently, see docs/project-format.md).

## [Unreleased]

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
