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
