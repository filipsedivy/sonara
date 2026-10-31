<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="resources/brand/png/logo-dark.png">
    <img src="resources/brand/png/logo-light.png" alt="Sonara" width="380">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/filipsedivy/sonara/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/filipsedivy/sonara/ci.yml?branch=main&label=CI"></a>
  <a href="https://github.com/filipsedivy/sonara/releases"><img alt="Release" src="https://img.shields.io/github/v/release/filipsedivy/sonara?include_prereleases"></a>
  <img alt="macOS 26+" src="https://img.shields.io/badge/macOS-26%2B-black">
  <img alt="Licence: 0BSD" src="https://img.shields.io/badge/licence-0BSD-brightgreen">
</p>

# Sonara

Sonara is a music production app (a digital audio workstation) for macOS. You can work in the
desktop window, or let an AI agent such as Claude Code work in the same project. Both use the
same tools and share one undo history.

It is written in Rust. The window is built with Tauri and Svelte.

> **Alpha.** Sonara is at version 0.1. The core works and is tested, but plugin hosting
> (VST3, AU, CLAP) and time-stretching are not there yet.
> See [implementation status](docs/implementation-status.md).

## What you can do

- Record and arrange audio and MIDI on tracks, with undo for every edit.
- Play instruments built in: a synthesizer and a drum kit.
- Mix with effects, buses, sends and automation. Built-in effects include EQ, compressor,
  limiter, delay and reverb.
- Generate chords, basslines, drums and melodies.
- Play and record from a MIDI keyboard.
- Measure loudness and clipping.
- Export WAV, FLAC, MP3, Ogg and more.
- Control everything from an agent: 58 tools over MCP, a command line and local IPC.

## Install

1. Download `Sonara_<version>_aarch64.dmg` from the
   [releases page](https://github.com/filipsedivy/sonara/releases). It needs an Apple Silicon
   Mac with macOS 26 or later.
2. Open the DMG and drag Sonara to Applications.
3. The app is not notarized, so macOS blocks the first launch. Open **System Settings ▸ Privacy &
   Security** and click **Open Anyway** next to the message about Sonara.

To check the download, run `shasum -a 256 -c SHA256SUMS.txt` next to the DMG.

## Use it with Claude Code

The app includes a command line tool, `sonara`. Add it to Claude Code once:

```bash
claude mcp add sonara -- /Applications/Sonara.app/Contents/MacOS/sonara mcp
```

Then ask in plain language, for example:

> Create a four-minute electronic track at 124 BPM. Render a preview, check the loudness and
> export WAV and MP3.

If the Sonara window is open, you see the agent's edits appear live and can undo them. If not,
Sonara runs without a window.

The tools are described in [docs/agent-api.md](docs/agent-api.md). Ready-made workflows are in
[examples/](examples/README.md).

## Command line

```bash
sonara --help                                  # all commands
sonara tools                                   # list the agent tools
sonara render "My Song.sonara" -o mix.wav -f wav -f mp3 --normalize loudness --target -14
sonara analyze mix.wav                         # loudness, peak and spectrum report
sonara devices                                 # list audio devices
sonara gui                                     # open the desktop app
```

## Build from source

You need an Apple Silicon Mac with macOS 26 or later, [rustup](https://rustup.rs) (the
toolchain version is pinned in `rust-toolchain.toml`), the Xcode command line tools, cmake,
Node 26 and pnpm 12.

```bash
git clone https://github.com/filipsedivy/sonara.git && cd sonara
scripts/native/build-codecs.sh                 # builds libsoxr, LAME and libFLAC
cargo build --release -p sonara                # the command line tool
(cd apps/sonara-desktop && pnpm install --frozen-lockfile && pnpm tauri dev)   # the app
scripts/bundle-macos.sh                        # Sonara.app and the DMG
```

To contribute, read [CONTRIBUTING.md](CONTRIBUTING.md).

## Documentation

- [Architecture](docs/architecture.md): crates, threads and data flow
- [Audio engine](docs/audio-engine.md): real-time rules and measured performance
- [Project format](docs/project-format.md): the `.sonara` file
- [Agent API](docs/agent-api.md): tools, CLI and IPC
- [Plugin hosting](docs/plugin-hosting.md): what exists and what is planned
- [Decisions](docs/decisions/): architecture decision records
- [Dependencies](docs/dependencies.md): every dependency and its licence
- [Brand](docs/brand.md) and [Changelog](CHANGELOG.md)

## Known limitations

- No VST3, AU or CLAP plugins. Only the built-in instruments and effects.
- No time-stretching or pitch-shifting of audio clips.
- MIDI input is tested with virtual ports only. Sending MIDI to external devices is not
  supported yet.
- Effects on live input add latency. Direct monitoring does not.
- Files with more than two channels play as stereo. The files themselves are not changed.

## Licence

Copyright © 2026 Filip Sedivy. Sonara is free software under the
[BSD Zero Clause License (0BSD)](LICENSE): use, copy, modify and distribute it for any purpose,
with no attribution required.
