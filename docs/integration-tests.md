# Integration and contract tests (SON-P19)

`crates/sonara-agent/tests/contracts.rs` checks the planning contracts (`contracts/*.md` of the
Sonora task plan) against the public interfaces, one test per contract. A failing test name
says which contract broke:

```bash
cargo test --locked -p sonara-agent --test contracts      # all contract tests
cargo test --locked -p sonara-agent --test contracts contract_project_commands
scripts/dev.sh rust-check                                 # also runs them (and CI does)
```

They run on the null audio backend, need no hardware and no network, and are part of
`cargo test --workspace`.

## Coverage

| Contract | Test | What it checks |
|---|---|---|
| agent-tools | `contract_agent_tools` | 57 tools, each titled and described and classed read, write or destructive; calling every read tool leaves the project version unchanged |
| project-commands | `contract_project_commands` | an invalid operation is refused and leaves version and tracks as they were; a valid one reports `committed` and `changes.created`; `undo` restores the tracks |
| song-project | `contract_song_project_round_trips` | save, open again: the same tracks; the manifest carries a version |
| orchestration-metadata | `contract_orchestration_metadata` | unique ids, a source or an estimate on every record, an articulation `Supported` only when a library declared it, an unknown one never |
| instrument-catalog | `contract_instrument_catalog` | an empty scan is an empty catalog; a preset without a manifest declares no articulations or licence |
| mix-control | `contract_mix_control_ranges` | gain −144 to +12 dB and pan −1 to +1 are enforced at the edges, NaN refused |

Checked elsewhere:

- **audio-engine:** `crates/sonara-io` (the `AudioBackend` trait, the null backend and the
  `EngineError` variants) and `crates/sonara-engine/tests/player`, `live`.
- **editor-state:** the frontend, `apps/sonara-desktop/tests` (unit and page tests): every editor
  gesture is one command and the UI keeps no copy of the song.
- **reference-analysis:** not on `main`. It is delivered on
  `feature/SON-P15-reference-audio-analysis` (`sonara_media::reference`); add a contract test
  after it merges.

## Order of merging the task branches

Each branch is based on `main` and independent in code. Merge in any order. The text that two
branches both edit is `CHANGELOG.md` (the `Unreleased` / `Added` list) and the table in
`docs/implementation-status.md`, in P13, P15 and P18: keep every entry when resolving. After the
last merge, run `scripts/dev.sh rust` and `scripts/dev.sh frontend`.

| Branch | Touches |
|---|---|
| SON-P07, SON-P08 | frontend tests only |
| SON-P12 | `sonara-agent/tests/change_safety.rs` |
| SON-P13 | `sonara-agent/src/orchestration.rs`, `lib.rs`, docs |
| SON-P14 | `sonara-midi` tests and the `smf` module documentation |
| SON-P15 | `sonara-media/src/reference.rs` and the export file analysis, docs |
| SON-P16 | `sonara-agent/tests/render_tools.rs`, `docs/audio-engine.md` |
| SON-P17 | `sonara-engine` meters (`is_clipping`) |
| SON-P18 | `sonara-app/src/quality.rs`, docs |
| SON-P19 | `sonara-agent/tests/contracts.rs`, this file |

## Reporting a contract failure

A contract test never fixes the code under test. When one fails, open a task for the module's
owner with the test name, the assertion and the contract it cites.
