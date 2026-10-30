# End-to-end smoke test (SON-P20)

`crates/sonara-agent/tests/smoke.rs` drives the main path through the application's composition
root: an `AgentService` on an `AppContext` with the null audio backend, the same wiring that
`sonara` (CLI and MCP) and the desktop app use. It needs no hardware.

```bash
cargo test --locked -p sonara-agent --test smoke
```

## The path

1. `create_project`, two `create_track` calls (drums and bass), `set_tempo`.
2. `generate_midi` into both tracks, `verify_project`.
3. `export_midi`: the file starts with `MThd`.
4. `export_audio` to a 16-bit WAV, verified by the exporter; `analyze_audio` on the written file
   reports the project's length (2 bars at 100 BPM, 4.8 s).
5. `save_project`; `get_history` and the audit log show every call.

The test contains no musical or audio logic of its own: it calls public tools and reads their
replies, so it cannot hide a defect by copying the module's behaviour. A failure points at the
tool in its assertion message.

## What is wired on `main`

The path above uses the project model and commands, the generators, the Standard MIDI File
export, the offline renderer, the WAV exporter and the analyzer, all behind agent tools. The
instrument catalog (`sonara_app::instruments`) is part of the application layer.

## What is not wired

These are library APIs without an agent tool, CLI command or GUI (see
`docs/implementation-status.md`), so the smoke test does not reach them:

- the song form generator and localized regeneration (`sonara_midi::form`,
  `sonara_midi::regenerate`);
- the music theory validators and the orchestral instrument metadata (`sonara_midi::theory`,
  `sonara_midi::orchestration`).

And these are delivered on their own branches, not merged into `main` yet, so they cannot be
wired here without merging, which a task does not do:

- conversational orchestration (`feature/SON-P13-conversational-agent-orchestration`);
- reference audio analysis (`feature/SON-P15-reference-audio-analysis`);
- clipping reports on the live meters (`feature/SON-P17-mix-controls-and-metering`);
- quality checks (`feature/SON-P18-quality-validation-suite`).

Once they merge, wiring means adding agent tools or app services that call their public APIs
(`Orchestrator`, `analyze_reference_file`, `TrackMeter::is_clipping`, `check_project`) and
extending this path; the tools' schemas are part of CTR-01 and change only by a deliberate,
versioned decision, which is why none of them were wired by their own tasks.

## By hand: the AI menu and the AI Composer in the bundled app (owner)

Not a CI gate and not automated (HW-01): it needs the bundled `Sonara.app` started from the
Finder, a signed-in Claude Code and a person who looks at the native menu bar. The automated
parts are covered elsewhere: the menu model's order, with the AI menu between View and Window
(`crates/sonara-app/tests/gui_menus.rs`), the panel's page tests (`tests/page/composer.spec.ts`)
and a run against a fake `claude` (`sonara_app::composer`). What only the bundle can show is that
the native menu is built from that model, and that the run finds `claude` and the bundled CLI
from the Finder's minimal `PATH`.

**Prerequisites.** Apple Silicon Mac with macOS 26 or newer, and Claude Code 2.1.259 or newer,
installed and signed in (`claude --version`, then `claude` once in a terminal). Quit any running
Sonara first, so that the window you see is the only instance.

1. **Build.** `scripts/bundle-macos.sh` (it needs `target/native` from
   `scripts/native/build-codecs.sh`). It checks that `Contents/MacOS/sonara`, the CLI of the run,
   is in the bundle. The app is `target/release/bundle/macos/Sonara.app`.
2. **Start from the Finder.** Double-click `Sonara.app` (or `open target/release/bundle/macos/Sonara.app`).
   Do not start it from a terminal: that would hand it your shell's `PATH`, which hides the
   problem this check is for. The first start of an ad-hoc signed app may need right-click →
   Open.
3. **Menu bar.** Read it from left to right: **Sonara, File, Edit, Track, Transport, View, AI,
   Window, Help**. Open **AI**: it holds one item, **AI Composer…**. Expected: the AI menu sits
   between View and Window.
4. **Panel.** Create or open a project (File → New Project… or Open…), then choose **AI → AI Composer…**. Expected:
   a panel, not a modal dialog (the window and its menus stay usable), that shows the path and
   version of Claude Code and enables Compose. Not expected: "the sonara command line tool is
   missing", "cannot find Claude Code" or "agents cannot attach to this window".
5. **Run.** Prompt: `Add a drum track with a one-bar beat at 100 BPM.` Press Compose. Expected:
   tracks and notes appear on the timeline while the step log fills, and the panel ends with a
   result.
6. **One undo step.** Open **Edit**: the first item reads **Undo AI Composer**. Press ⌘Z once:
   everything the run added is gone and the project equals the one before the run. ⇧⌘Z brings it
   all back. If the run was only a question and changed nothing, there is nothing to undo.
7. **Stop.** Start a longer prompt (`Compose a three-minute song in four sections.`), press Stop
   after a few steps. Expected: the panel reports a stopped run, and **Edit → Undo AI Composer**
   removes the part that was written.
8. **Quit.** ⌘Q during a run. Expected: Sonara quits and
   `pgrep -fl 'claude -p|sonara mcp'` shows no process left over from it (a plain `claude` is your
   own session, not the run's).

Write the result (macOS version, Claude Code version, which steps passed) next to the check in
`docs/implementation-status.md`, under "Open checks", and remove the item when every step passed.
