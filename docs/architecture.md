# Architecture

Sonara is a Rust workspace with one authoritative project session per process. The desktop
GUI, the CLI, local IPC clients and the MCP server are all interfaces onto that session.

```text
┌──────────────────────────┐ ┌──────────────┐ ┌────────────────────────────────────┐
│ apps/sonara-desktop      │ │ apps/sonara  │ │ sonara-agent                       │
│ Tauri 2 shell (Rust)     │ │ CLI (clap),  │ │ tool registry and handlers,        │
│ + Svelte 5 / TypeScript  │ │ MCP (stdio)  │ │ permissions, audit, IPC server     │
└────────────┬─────────────┘ └──────┬───────┘ └─────────────────┬──────────────────┘
             │ commands, services, events       │ tools          │
             ▼                                  ▼                ▼
┌───────────────────────────────────────────────────────────────────────────────────┐
│ sonara-app   AppContext (composition root), the locked ProjectSession, services:  │
│              projects, media, render/export, jobs, transport, peaks, live feed    │
└──────┬──────────────────┬───────────────────┬────────────────────┬────────────────┘
       ▼                  ▼                   ▼                    ▼
┌──────────────┐ ┌─────────────────┐ ┌──────────────────┐ ┌───────────────────────────┐
│ sonara-core  │ │ sonara-project  │ │ sonara-media     │ │ sonara-engine             │
│ model, time, │ │ .sonara format, │ │ decode, resample,│ │ render graph, offline     │
│ commands,    │ │ store, schemas, │ │ media cache,     │ │ renderer, player loop     │
│ undo, session│ │ migrations      │ │ encoders         │ │ sonara-dsp: kernels,      │
│ (pure)       │ │ sonara-midi     │ │                  │ │ processors, analysis      │
└──────────────┘ └─────────────────┘ └──────────────────┘ └─────────────┬─────────────┘
                                                                         ▼
                                              ┌───────────────────────────────────────┐
                                              │ sonara-rt: rings, flush/fade, meters, │
                                              │ stats atomics                         │
                                              │ sonara-io: miniaudio device callback, │
                                              │ midir MIDI input                      │
                                              └───────────────────────────────────────┘
```

## Crates

| Crate | Responsibility |
|---|---|
| `sonara-core` | The immutable project model (project, tracks, clips, notes, devices, automation, media, markers), the time base (tempo and meter maps), routing rules, plugin descriptors, commands, undo/redo, `ProjectSession`, typed errors. Pure: no I/O, audio devices or GUI. |
| `sonara-project` | The `.sonara` format: serializer, JSON Schemas, migrations, bundle and archive store, atomic writes, backups, integrity. |
| `sonara-midi` | Music theory, generators and transforms, the GM drum map, Standard MIDI File I/O. |
| `sonara-dsp` | The `Processor` contract, filters, dynamics, delay, convolution, reverb, saturation, utility, the instruments (Poly Synth, Drum Kit), and analysis (loudness, true peak, spectrum, stereo). A real-time crate. |
| `sonara-media` | Audio inspection, decoding (symphonia and own demuxers, optional FFmpeg), resampling (libsoxr), the media cache, encoders (WAV family, FLAC through libFLAC, MP3 through LAME, Ogg Vorbis and Opus), dither and normalization. The codec libraries load at run time (RT-10, LIC-01). |
| `sonara-engine` | The render graph, the offline renderer and the player: the render, prefetch and player threads, take writing. |
| `sonara-rt` | Real-time primitives: SPSC rings on `rtrb`, the graph hand-off, flush and fade, meters, stats atomics. |
| `sonara-io` | The miniaudio device engine behind a narrow C shim (FFI), and MIDI devices through midir. |
| `sonara-app` | `AppContext` (the composition root), the locked session, services (projects, media, render/export/analysis, jobs, templates, transport, autosave, recovery), the events pump, the project view and live feed for interfaces, peaks, settings, menus and dialog models. |
| `sonara-agent` | Tool schemas, the tool registry and handlers, permissions and confirmations, the audit log, project-state views, the IPC server and client, the MCP server. |
| `sonara-testkit` | Golden-data loaders and comparisons, and the architecture test. A dev-dependency only. |
| `apps/sonara` | The `sonara` command: CLI, `serve`, `mcp`, `batch`, `gui`. |
| `apps/sonara-desktop` | The desktop application: a Tauri 2 shell in `src-tauri/` and a Svelte 5 + TypeScript page in `src/` ([ADR 0009](decisions/0009-gui-toolkit.md)). |

## Dependency rules

`crates/sonara-testkit/tests/architecture.rs` enforces these over `cargo metadata` (ARC-11).

- `sonara-core` depends on no other workspace crate and uses only a short allow-list of
  external crates (serde, indexmap, regex, thiserror and the like): no I/O, audio or GUI.
- The real-time crates (`sonara-dsp`, `sonara-rt`, `sonara-io`, `sonara-engine`) never
  depend on `sonara-app`, `sonara-agent` or the GUI (RT-01).
- `sonara-app` is the only crate that composes the engine, I/O, media and project crates.
- The desktop application reaches the core only through `sonara-app` and `sonara-agent`, and
  no crate depends on it.
- The GUI and the agent tools change the project only through `ProjectSession::execute`
  (commands), `undo` and `redo`, and through `sonara-app` services. Neither contains editing
  logic of its own.
- There are no cycles.

## State model

[ADR 0002](decisions/0002-state-model-and-commands.md) records this decision.

- The project is an immutable tree behind `Arc`. A `Command` is a pure function from one
  snapshot to the next, plus a `ChangeSet` listing created, updated and deleted object IDs.
- `ProjectSession::execute` validates the command (by running it), commits the new snapshot,
  bumps the version, pushes an undo entry and then notifies subscribers.
- Undo restores snapshots.
- The session sits behind one lock in `sonara-app`. Readers take a snapshot under it and work
  outside it; the render thread never touches the lock and receives built graphs instead.
- The GUI and agents share one session and therefore one undo history: an agent edit can be
  undone from the GUI, and the reverse.

## Interfaces onto the session

- **GUI.** The Tauri shell exposes `sonara-app` services as commands. The `sonara-events`
  pump delivers session events, job changes and status texts to the page in commit order; the
  page then fetches the project view (`sonara_app::view`). Meters, the transport position and
  the waveform tiles arrive over a binary channel and the `peaks:` URI scheme.
- **Agent tools.** `sonara-agent`'s registry holds one handler per tool. It is used by:
  - the IPC server, which runs inside the desktop application or `sonara serve`;
  - the MCP server (`sonara mcp`), which attaches to a running session over IPC or runs
    headless;
  - the CLI (`sonara tool`, `sonara batch`).
- **IPC.** Newline-delimited JSON-RPC 2.0 over a Unix domain socket, authenticated by a token
  in a `0600` discovery file. See [agent-api.md](agent-api.md).

## Threads

| Thread | Work |
|---|---|
| Main thread (Tauri, AppKit) | Window, web view, native menus, dialogs. Commands run edits directly; they are fast. |
| `sonara-events` | Delivers session events, job changes and ready peaks to the page. |
| `sonara-ipc`, `sonara-ipc-client` | Accepts agent connections; one thread per connection runs its tool calls. Commands are serialized by the session lock. |
| `sonara-job-0`, `sonara-job-1` | Renders, exports, analysis, media decoding. |
| `sonara-peaks-*` | Waveform pyramids, off the job workers. |
| `sonara-live` | The live feed: meters and the transport position for the page. |
| `sonara-autosave` | Periodic autosave of a dirty project. |
| `sonara-player` | Builds graphs after edits and drops retired ones (RT-09). |
| `sonara-prefetch` | Reads audio clips ahead of the render thread. |
| `sonara-render` | The real-time player loop: fills the playback ring. It joins the device's audio workgroup. |
| `sonara-takes` | Writes recorded takes from the capture ring to disk. |
| Device callback (miniaudio) | Copies from and to the rings only. See [audio-engine.md](audio-engine.md). |
| MIDI input callback (midir) | Forwards incoming MIDI into a ring for the render thread. |

## Persistence

A `.sonara` project is a bundle directory (the working form) or a ZIP archive (the
packaged form), both with the same members. Saves are atomic, with a manifest hash check,
rotating backups, autosave and recovery. See [project-format.md](project-format.md) and
[ADR 0004](decisions/0004-project-format.md).
