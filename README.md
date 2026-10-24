<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="resources/brand/png/logo-dark.png">
    <img src="resources/brand/png/logo-light.png" alt="Sonara" width="380">
  </picture>
</p>

# Sonara

Sonara is a digital audio workstation for macOS, written in Rust, with a Tauri 2 desktop GUI
(Svelte 5, TypeScript) and a real-time audio engine on miniaudio. It is built for musicians and
for agents: everything you can do in the
GUI is available as a documented, validated tool that Claude Code can call over MCP. Both
interfaces work on the same project with one shared undo history.

> **Status: alpha (0.1).** The foundation is solid and tested, but a lot is still missing,
> notably third-party plugin hosting and time-stretching. See
> [docs/implementation-status.md](docs/implementation-status.md) for the exact state.

## What works today

- **Projects.** The `.sonara` format comes as a bundle directory or a ZIP archive, with
  atomic saves, a manifest with SHA-256 integrity, rotating backups, autosave, crash recovery,
  schema validation and migrations, missing-media detection with relinking, and safe archive
  extraction.
- **Arrangement.** Audio, instrument, bus and folder tracks. Clips can be moved, trimmed,
  split, duplicated and looped, with fades and clip gain. The project has sections, markers,
  tempo changes and ramps, and time-signature changes. All edits are non-destructive and undoable.
- **MIDI.** A piano roll; quantize, swing, humanize, transpose; Standard MIDI File
  import/export; and generators for voice-led chord progressions (Roman numerals or chord
  symbols), arpeggios, basslines, drum grooves (GM map), melodies and Euclidean rhythms.
- **Instruments.** A polyphonic subtractive synth (band-limited oscillators, unison,
  resonant TPT filter, envelopes, glide, 13 presets) and a synthesized GM drum kit (5 kits).
- **Effects.** Parametric EQ, multimode filter, compressor with external sidechain,
  lookahead true-peak limiter, gate/expander, tempo-synced delay, algorithmic reverb,
  convolution reverb, saturation with oversampling, and a utility (gain, width, polarity,
  mono, DC block). Every parameter can be automated.
- **Mixer.** Faders, balance pan, mute/solo/solo-safe, buses, pre/post-fader sends,
  sidechains, a validated routing graph with feedback rejection, sample-accurate volume/pan
  automation and plugin delay compensation.
- **MIDI devices.** Live playing and recording from MIDI keyboards through midir (CoreMIDI)
  into the armed or selected instrument track.
- **Audio engine.** miniaudio device I/O with a lock-free ring into the callback, which never
  allocates, locks or blocks. Lookahead rendering on a render thread in the device's audio
  workgroup, click-free seek/stop, sample-accurate loops, metronome, recording with latency
  compensation, meters, and underrun counters.
- **Analysis.** BS.1770-4 / EBU R128 loudness (integrated, short-term, momentary, LRA),
  oversampled true peak, RMS, crest factor, clipping and silence detection, stereo
  correlation and width, and octave-band balance.
- **Import/export.** WAV/BWF/RF64/W64/AIFF/CAF/FLAC/MP3/Ogg Vorbis/Opus export at 16/24/32-bit
  or float, with TPDF dither, resampling, peak or loudness normalization (never exceeding the
  true-peak ceiling), metadata, BWF `bext` loudness, stems, and multiple formats from one
  render. Import covers WAV, BWF, RF64, W64, AIFF, CAF (PCM, µ-law, A-law, ALAC), FLAC, MP3,
  Ogg Vorbis and Ogg Opus, plus AAC/M4A and others when FFmpeg is installed.
- **Agent interface.** 57 tools exposed through the CLI, local IPC and MCP. They take
  validated JSON Schemas and return structured results, with dry runs, two-phase confirmation
  for destructive actions, file-access sandboxing, background jobs and an audit log.

## Installing the macOS app

The native Rust application ships as `Sonara_<version>_aarch64.dmg` on the GitHub releases page,
for Apple Silicon and macOS 26 or later. Open the DMG and drag Sonara to Applications.

The DMG is signed ad hoc and **not notarized**, so macOS blocks the first launch:

1. Open Sonara. macOS reports that it cannot verify the app; click **Done**.
2. Open **System Settings ▸ Privacy & Security**, scroll down to the message about Sonara, and
   click **Open Anyway**. Confirm with **Open Anyway** and your password.

Later launches open normally. `SHA256SUMS.txt` beside the DMG lets you check the download with
`shasum -a 256 -c SHA256SUMS.txt`.

The `sonara` CLI ships inside the app. After the first launch, add it to Claude Code:

```bash
claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp
```

## Build from source

Requirements: macOS 26 or later on Apple Silicon, the Rust toolchain pinned in
`rust-toolchain.toml` (rustup), the Xcode command line tools, cmake, Node 26 and pnpm 12.

```bash
git clone <this repo> sonara && cd sonara
scripts/native/build-codecs.sh                 # libsoxr, LAME and libFLAC into target/native/lib
cargo build --release -p sonara                # the sonara CLI
(cd apps/sonara-desktop && pnpm install --frozen-lockfile && pnpm tauri dev)   # the desktop app
scripts/bundle-macos.sh                        # Sonara.app and the DMG
```

The CLI:

```bash
sonara --help                                  # overview
sonara tools                                   # list agent tools
sonara batch examples/sidechain_preview.json --connect never
sonara render "My Song.sonara" -o mix.wav -f wav -f mp3 --normalize loudness --target -14
sonara analyze mix.wav                         # loudness / peak / spectrum report
sonara devices                                 # audio devices
sonara serve                                   # headless session for agents (IPC)
sonara gui                                     # the desktop app
```

`scripts/dev.sh` runs the Rust and frontend gates natively; see CONTRIBUTING.md.

## Use it from Claude Code

This repository ships a project-scoped `.mcp.json`, so Claude Code started in this folder
offers the `sonara` MCP server, built from the checkout. With the app installed, use it from
anywhere:

```bash
claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp
```

If the Sonara window is open, the MCP server attaches to it and you watch the agent work: its
edits appear live and can be undone in the GUI. Otherwise it runs a headless session. Ask, for
example: *"Create a four-minute electronic track at 124 BPM with an intro, a build-up, two
drops, a breakdown and an outro; render a preview, check loudness and clipping, and export WAV
and MP3."*

Tool reference, conventions (beats, `bar:beat`, seconds), safety rules and the IPC protocol
are in [docs/agent-api.md](docs/agent-api.md). Reproducible agent workflows are in
[examples/](examples/README.md).

## Documentation

| Document | Contents |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Crates, dependency rules, state model, threads |
| [docs/audio-engine.md](docs/audio-engine.md) | Real-time rules, latency, renderer, DSP, measured performance |
| [docs/project-format.md](docs/project-format.md) | The `.sonara` format, saving, recovery, versioning |
| [docs/agent-api.md](docs/agent-api.md) | CLI, IPC and MCP tools with schemas, examples and safety |
| [docs/plugin-hosting.md](docs/plugin-hosting.md) | Plugin contract and the (unimplemented) third-party hosting plan |
| [docs/decisions/](docs/decisions/) | Architecture decision records |
| [docs/dependencies.md](docs/dependencies.md) | Dependency and licence inventory |
| [docs/implementation-status.md](docs/implementation-status.md) | What is done, partial, planned or unsupported |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Development workflow and quality gates |

## Platforms

Sonara is a macOS application: Apple Silicon, macOS 26 or later, Core Audio and CoreMIDI.
Development and verification happen on macOS 27 and CI. The engine and the CLI are also known to
build and pass their tests on Linux (aarch64, glibc), though CI no longer checks that and Linux
is not a product platform.

## Current limitations

- No VST3/AU/CLAP hosting. Only the built-in instruments and effects are available.
- MIDI input (live play and recording) works through midir. It has been verified with virtual
  ports only, and routing tracks to external MIDI outputs is not wired up yet.
- No time-stretching or pitch-shifting of audio clips.
- Effects on live input are not low-latency. Direct dry monitoring is native and low-latency.
- Multichannel (more than 2) sources are folded to stereo for playback; the original files
  are kept unchanged.
- Measurements of render speed and load are in [docs/audio-engine.md](docs/audio-engine.md).

## Author and licence

Created by **Filip Sedivy** (mail@filipsedivy.cz). Brand assets and usage rules are in
[docs/brand.md](docs/brand.md).

The licence is not yet chosen; all rights are reserved for now (see [LICENSE](LICENSE)). Every dependency is
compatible with permissive, proprietary or copyleft licensing as long as its LGPL obligations
are met (see [docs/dependencies.md](docs/dependencies.md)).
