# Orchestral instruments: implementation plan

_Status: accepted with ADR 0011 (2026-10-30); every owner decision is taken. Merges the implementation tables of the four layer designs into one backlog. The decisions behind it are in [../orchestra-design.md](../orchestra-design.md) and [ADR 0011](../decisions/0011-orchestral-instruments-and-performance.md); nothing is implemented yet._

## How to read it

- 113 work items: IM (layer [a](design/instrument-model.md), the instrument model), SS (layer [b](design/sound-source.md), the sound source), PF (layer [c](design/performance.md), the performance layer and agents), UI (layer [d](design/ui.md), the interface). Each item's detail, tests and risks are in its layer; SS-36 and PF-23 were added when the owner accepted phase 2.
- Sizes are the layers' relative estimates (S small, M medium, L large); no calendar time is attached. IM-17 and PF-8 are placeholders merged into SS-14 and SS-35.
- **The owner accepted phase 2 together with phase 1 on 2026-10-30:** project schema v2 (a track `part` and a note `articulation`), per-note articulation in the agent API, and a tool that writes MIDI CC. These items moved out of O6 into the milestones that need them. They are versioned changes (CTR-01): project schema 2 with a lowest-version writer, and agent API 1.1 (additive) with golden-delta tests (PF-23).
- **Owner gate**: the decisions each milestone rests on (ids from [open-questions.md](open-questions.md), and P/U from the layers); the owner took all of them on 2026-10-30.
- No item depends on an item of a later milestone (checked when this table was built). Every milestone ends with `scripts/dev.sh rust` and `scripts/dev.sh frontend` passing, the changelog and status table updated (DOC-01), and its own done criteria below.

## The first audible result

The product sentence ("strings play a legato melody, horns and timpani underneath") runs end to end for the first time through the Orchestrate… field (PF-15). That needs **34 items** (O1 12, O2 5, O3 9, O4 7, O5 1), not the whole backlog: the rest of O5 (the Library browser, family panels, lanes) can follow. At that point the planner writes articulations as the sampler's `articulation` state; once PF-18 and PF-21 land in the same O4 it writes them per note and writes dynamics as CC.

Longest dependency chain to it, weighted S = 1, M = 2, L = 4 (weight 32):

IM-1 (M) → IM-2 (M) → IM-5 (L) → IM-8 (L) → IM-9 (M) → IM-18 (S) → SS-13 (M) → SS-26 (M) → SS-15 (M) → SS-16 (M) → PF-2 (M) → PF-7 (L) → PF-13 (M) → PF-15 (S)

The model (IM-1 … IM-18) is the long pole. 5 O2 items are in the set, none on the longest chain, so the conformance work runs beside the model; the other O2 fixes make the free libraries worth hearing and belong in the same release. What it sounds like, and what it does not (recorded legato, swells in timbre), is in [design/performance.md](design/performance.md) section 12.

## Owner gates

| Milestone | Decisions it rests on |
|---|---|
| O1 | all decided on 2026-10-30: shape B with mute and vibrato as passage states (A16), no id = ordinario (A14), `col_legno_battuto` kept apart (A12), only the invented ids a library records in the first list (A13), `tremolo` for mallets and `roll` for drums (A15), exact GM programs with substitutes apart and an orchestral percussion map beside `GM_MAP` (A11), the cards' values, notation, ids, controls and data.rs corrections (A18–A23) |
| O2 | all decided: ADR 0011 accepted, so the SFZ fixes follow ARIA and sfizz where the specification is silent (decision 5, A8); preset ids amend ADR 0010 (A5); schema v2 with a track `part` and a note `articulation` (A1, A3 option b); the code fixes outside the contracts with tests (A24) |
| O3 | all decided: articulation per note plus the sampler's `articulation` state (A3), dynamics in clips as CC plus the sampler's `dynamics` trim (A4), profiles for VSCO 2 CE and VCSL only (A6, A17), overlap legato kept if the listening check B2 passes (A9), coverage reported in both counts (A25) |
| O4 | all decided: staged planner validation with undo rollback (P1), humanization on by default (P2), the AI Composer reference (P3) |
| O5 | all decided: the GUI edits CC 64 and CC 11 (U1); download links only for VSCO 2 CE and VCSL (U2); ensembles in the GUI (A7) |
| O6 | decided to wait: ensemble and orchestration tools only by a later versioned decision (A7); a GM engine decided here, after the recount (A10); plugin hosting (M5) |

## O1. Instrument model

17 items (S 5, M 8, L 4), in dependency order. **Owner gate:** all decided on 2026-10-30: shape B with mute and vibrato as passage states (A16), no id = ordinario (A14), `col_legno_battuto` kept apart (A12), only the invented ids a library records in the first list (A13), `tremolo` for mallets and `roll` for drums (A15), exact GM programs with substitutes apart and an orchestral percussion map beside `GM_MAP` (A11), the cards' values, notation, ids, controls and data.rs corrections (A18–A23).

| # | Item | Layer | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| IM-1 | `InstrumentType`, `instruments()`, per-family `data/` files, score-order static with const index check | [a](design/instrument-model.md) | sonara-midi `orchestration::{id, data}` | M | – |
| IM-2 | `InstrumentSpec` (with `const EMPTY`) and the extended record; new `InstrumentError` variants and const checks; rewrite the six-argument `try_new` test calls | [a](design/instrument-model.md) | `orchestration::instrument`; `tests/orchestration.rs` | M | IM-1 |
| IM-3 | `Clef` with octave; tenor case per A20 | [a](design/instrument-model.md) | `orchestration::pitch` | S | IM-2, A20 |
| IM-4 | `data/sources.rs`; card S-numbers to full sources; relax `tests/orchestration.rs:78` | [a](design/instrument-model.md) | `orchestration::data` | M | IM-1 |
| IM-6 | Vocabulary with `Kind`, `Axis`, `Mute`, `Vibrato`, `parent`, descriptions: the 15 stable and the approved ids; ids awaiting A12/A13 follow their decision (append-only) | [a](design/instrument-model.md) | `orchestration::articulation` | M | A12, A13 |
| IM-5 | Transcribe the 85 card json blocks (names split from their parentheticals); apply the corrections; per-instrument articulation lists; update the value-pinning tests | [a](design/instrument-model.md) | `orchestration::data::*`; `tests/orchestration.rs` | L | IM-2, IM-4, IM-6, A18, A23 |
| IM-7 | `Request` and `request_availability` | [a](design/instrument-model.md) | `orchestration::{articulation, instrument}` | S | IM-6 |
| IM-8 | `Names`, categories, alias data in five languages (the Czech inflected forms the planner needs, PF-11) | [a](design/instrument-model.md) | `orchestration::{names, data::categories}` | L | IM-1, IM-5 |
| IM-9 | `fold`, alias index, `resolve`; `unicode-normalization` edge (the only folding in the workspace; the planner calls it) | [a](design/instrument-model.md) | `orchestration::names` | M | IM-8 |
| IM-10 | `GmVoice` and data per A11 | [a](design/instrument-model.md) | `orchestration::general_midi` | S | IM-5, A11 |
| IM-11 | `KeyboardMechanics` and the 16 keyboard records | [a](design/instrument-model.md) | `orchestration::keyboard` | M | IM-5 |
| IM-12 | Playability facts | [a](design/instrument-model.md) | `orchestration::playability` | M | IM-5 |
| IM-13 | Ensemble templates and their validation | [a](design/instrument-model.md) | `orchestration::{ensembles, data::ensembles}` | L | IM-1 |
| IM-14 | Rule catalogue, doubling, weight, register and limit tables | [a](design/instrument-model.md) | `orchestration::{rules, data::rules}` | L | IM-1, IM-6 |
| IM-15 | One `Family`: serde derive in sonara-midi; re-export in the catalog | [a](design/instrument-model.md) | sonara-midi; `sonara_app::instruments::family` | S | – |
| IM-20 | Fallback data for the degradation chain (section 11): family neighbours per id, articulation substitutions and emulation kinds | [a](design/instrument-model.md) | `orchestration::{fallback, data::fallback}` | M | IM-1, IM-6 |
| IM-16 | Docs and dependency rows (DOC-01) | [a](design/instrument-model.md) | `docs/` | S | IM-1..IM-15, IM-20 |

**Done when:**

- `crates/sonara-midi/tests/orchestration.rs` checks every invariant of the 85 ids (unique ids, aliases unique or marked ambiguous, written = sounding − transposition, comfortable inside playable, provenance on every value, every ensemble part and rule names an existing id) (TST-07); invalid data fails to compile
- the 14 corrections of [model.md](model.md) are in the data; `docs/orchestration-metadata.md` describes the new record, and model.md is retired
- no golden file changes (GLD-01)

## O2. Sampler conformance, preset ids and project schema v2

13 items (S 8, M 4, L 1), in dependency order. **Owner gate:** all decided: ADR 0011 accepted, so the SFZ fixes follow ARIA and sfizz where the specification is silent (decision 5, A8); preset ids amend ADR 0010 (A5); schema v2 with a track `part` and a note `articulation` (A1, A3 option b); the code fixes outside the contracts with tests (A24).

| # | Item | Layer | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| IM-19 | Schema v2 `part` and note `articulation`: `project-v2.schema.json`, relabel migration, `migrated_from` only for content changes, features, lowest-version writer; dataset check in the app *(moved from O6: phase 2 accepted on 2026-10-30)* | [a](design/instrument-model.md) | sonara-project `schemas`, `migrations`, `serializer`, `store`; sonara-core `model`, `midi`; sonara-app load path | L | A1, A16, CTR-01 |
| SS-01 | Q1 and N4: switch key set (union), last `sw_default` | [b](design/sound-source.md) | `sonara-media::sfz`, `sonara-dsp::{bank, processors::sampler}` | S | — |
| SS-02 | Q11: `<control>` resets the control scope only | [b](design/sound-source.md) | `sonara-media::sfz` | S | — |
| SS-03 | Q2: directives anywhere in a line | [b](design/sound-source.md) | `sonara-media::sfz` | M | — |
| SS-04 | Q3 defines in include paths; Q4 main-file directory | [b](design/sound-source.md) | `sonara-media::sfz` | S | SS-03 |
| SS-05 | Q7: `−1` keys; CC-triggered regions not key-triggered | [b](design/sound-source.md) | `sonara-media::sfz`, `sonara-dsp::bank` | S | — |
| SS-06 | N3 generators and missing samples per region; N2 reporting | [b](design/sound-source.md) | `sonara-media::sfz`, `sonara-dsp::bank` | M | — |
| SS-07 | N1: release samples on pedal up | [b](design/sound-source.md) | `sonara-dsp::processors::sampler` | S | — |
| SS-08 | C8: skip `includes/`; region-less patches invalid | [b](design/sound-source.md) | `sonara-app::samples`, `::instruments::discovery` | S | — |
| SS-09 | `sfz::inspect` structural report | [b](design/sound-source.md) | `sonara-media::sfz` | M | SS-01..SS-06 |
| SS-10 | Preset-id choices, instrument resolver, path compatibility; shipped-profile identity (`id`, `detect`) so library ids are final before any project stores one (2.4) | [b](design/sound-source.md) | `sonara-core::plugins::builtin`, `sonara-media::provider`, `sonara-engine::graph`, `sonara-app::{samples, instruments::profiles}` | M | SS-08; owner (A5) |
| SS-28 | Docs: ADR 0010 amendment, sample-libraries, instrument-catalog, CHANGELOG, status | [b](design/sound-source.md) | `docs/` | S | each item |
| UI-1 | Sampler `instrument` combo shows preset names grouped by library, not paths | [d](design/ui.md) | desktop `devices/`; sonara-app `instruments` | S | SS-10 (A5) |

**Done when:**

- every Q/N fix has the regression test named in [sfz-conformance.md](sfz-conformance.md) section D, passing, with `assert_no_alloc` on the real-time paths (RT-02..RT-05)
- a project that stores an absolute path still opens and plays; preset ids survive a renamed folder for profiled libraries; ADR 0010 carries the amendment
- schema v2: migration tests, `project-v2.schema.json`, and the guarantees that an unchanged v1 project saves byte-identical and opens with `migrated_from_schema: null` (golden coverage step 164); `docs/project-format.md` describes v2 and its required features
- the human render check B1 confirms the fixed verdicts; `docs/sample-libraries.md` drops the divergences that are fixed

## O3. Sound mapping and expression

26 items (S 9, M 13, L 2), in dependency order. **Owner gate:** all decided: articulation per note plus the sampler's `articulation` state (A3), dynamics in clips as CC plus the sampler's `dynamics` trim (A4), profiles for VSCO 2 CE and VCSL only (A6, A17), overlap legato kept if the listening check B2 passes (A9), coverage reported in both counts (A25).

| # | Item | Layer | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| IM-17 | (Merged into layer b's SS-14, which builds `SamplerChoices` from `InstrumentType::ALL`; no separate work, kept for traceability) | [a](design/instrument-model.md) | – | – | IM-1 |
| IM-18 | Catalog inference through `resolve()` (A24 keywords; camel-case split, `Lang::En`): the `Inferred` claim of layer b's SS-13 | [a](design/instrument-model.md) | `sonara_app::instruments::family` | S | IM-9, IM-15 |
| SS-11 | Manifest v2 reader (per-entry parsing), validation, merge | [b](design/sound-source.md) | `sonara-app::instruments::{manifest, profiles}` | M | SS-10; IM-1, IM-6, IM-15 |
| SS-12 | Shipped profiles VSCO 2 CE and VCSL v1.2.2-RC | [b](design/sound-source.md) | `crates/sonara-app/profiles/`, `::instruments::profiles` | L | SS-11 |
| SS-13 | Catalog mapping: claims with the `Profile` source, ensemble, articulations, verdict, `PatchFacts` | [b](design/sound-source.md) | `sonara-app::instruments::{model, family, discovery}` | M | SS-09, SS-11; IM-18 (`Inferred` claims) |
| SS-14 | Parameters `instrument_type`, `articulation`, `dynamics`; `SamplerChoices` (absorbs layer a's IM-17); unknown stored choices fall back to `auto` | [b](design/sound-source.md) | `sonara-core::plugins::builtin`, `sonara-app::samples`, `sonara-engine::graph` | S | IM-1, IM-6 (`InstrumentType::ALL`, `Articulation::ALL`) |
| SS-18 | Sampler-only chase: `KeySwitch` event, CC 1/11/64, event buffer +4 | [b](design/sound-source.md) | `sonara-engine::{schedule, graph}`, `sonara-dsp::{processor, processors::{sampler, synth}}` | M | SS-01 |
| SS-19 | Q6 velocity crossfades; Q8 `amp_veltrack` | [b](design/sound-source.md) | `sonara-media::sfz`, `sonara-dsp::{bank, sampler}` | S | — |
| SS-20 | Controller table, gates, CC crossfades, `gain_cc` | [b](design/sound-source.md) | same | M | SS-05 |
| SS-22 | Q9 `off_mode` normal and time | [b](design/sound-source.md) | `sfz`, `bank`, `sampler` | S | — |
| SS-23 | Q5 `first`, `legato`, `release_key` | [b](design/sound-source.md) | `sfz`, `bank`, `sampler` | S | SS-22 |
| SS-26 | Degradation chain service; the step-1 and step-6 tables | [b](design/sound-source.md) | `sonara-app::instruments::chain` | M | SS-13; IM-20 |
| SS-15 | `SamplerSetup` in `DeviceState`; compile with step-2 fallbacks | [b](design/sound-source.md) | `sonara-dsp::{bank, processor}`, `sonara-app::instruments::compile`, `sonara-engine::graph` | M | SS-10, SS-13, SS-14, SS-26 |
| SS-16 | Sampler applies targets, key offset; `rest()` re-applies | [b](design/sound-source.md) | `sonara-dsp::processors::sampler` | M | SS-15 |
| SS-17 | Live rebuild reuses an unchanged sampler | [b](design/sound-source.md) | `sonara-engine::graph` | S | SS-15 |
| SS-21 | `dynamics` drive (velocity, cc, fixed) | [b](design/sound-source.md) | `sonara-dsp::processors::sampler` | M | SS-14, SS-15, SS-20 |
| SS-24 | Overlap-crossfade legato | [b](design/sound-source.md) | `sonara-dsp::processors::sampler` | M | SS-16, SS-23 |
| SS-25 | Multi-patch sets, one loader per device, memory cap | [b](design/sound-source.md) | `sonara-app::instruments`, `sonara-media::{sfz, provider}`, `sonara-dsp::{bank, sampler}` | L | SS-15, SS-16 |
| SS-27 | Sampler bench, `assert_no_alloc` and RTSan coverage | [b](design/sound-source.md) | `sonara-dsp` benches and tests, `sonara-engine` tests | S | SS-16..SS-25 |
| SS-29 | N5, N6, N7 (latent) | [b](design/sound-source.md) | `sonara-media::sfz` | S | — |
| SS-33 | Event-accurate enum automation for samplers (optional) | [b](design/sound-source.md) | `sonara-engine::graph::process`, `sonara-dsp::processor` | M | SS-16 |
| SS-34 | `mute` and `vibrato` parameters (optional, 5.7) | [b](design/sound-source.md) | `sonara-core::plugins::builtin`, `sonara-app::instruments::compile`, `sonara-dsp::processors::sampler` | M | SS-15, SS-16; owner (A16) |
| SS-35 | Decoded facts at `verify`: onset per articulation (`attack_ms`) and longest unlooped sample (`max_hold_s`) into `PatchFacts`; the profile's `attack_ms` override (absorbs layer c's PF-8) | [b](design/sound-source.md) | `sonara-media::sfz` (`PatchReport`), `sonara-app::instruments::{verify, model}` | M | SS-09, SS-13 |
| SS-36 | Per-note articulation in the engine and the sampler: a note's `articulation` travels with its note-on event, and the sampler applies that note's compiled switch at the note, sample-accurately, over the track's `articulation` state *(added with the phase-2 decision)* | [b](design/sound-source.md) | `sonara-engine::schedule`, `sonara-dsp::processors::sampler` | M | IM-19, SS-15, SS-16 |
| PF-8 | (Moved to layer (b)'s SS-35, which measures onsets and the longest unlooped sample at `verify`; kept for traceability) | [c](design/performance.md) | – | – | – |
| PF-9 | Catalog → `SoundCaps` adapter and the planner's catalog seam | [c](design/performance.md) | `sonara-app` `instruments`; `sonara-agent` `orchestration` | S | SS-09, SS-13, SS-35 |

**Done when:**

- profile validation and merge tests; the degradation chain has a test per step with its user message (TST-03)
- sampler tests for per-note articulation (a note's switch wins over the track state, at the note's frame), the `articulation` and `dynamics` parameters, a seek and a loop wrap restoring the articulation, CC crossfades, overlap legato and articulation sets, all inside `assert_no_alloc`
- memory per profile measured and recorded in `docs/audio-engine.md` (HW-01); the listening check B2
- `docs/instrument-catalog.md` and `docs/sample-libraries.md` describe profiles and the new parameters

## O4. Performance layer, agents and agent API 1.1

18 items (S 4, M 11, L 3), in dependency order. **Owner gate:** all decided: staged planner validation with undo rollback (P1), humanization on by default (P2), the AI Composer reference (P3).

| # | Item | Layer | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| PF-1 | Types, `perform()`, per-part streams (`PyRandom::with_key`) | [c](design/performance.md) | `sonara-midi` `performance::{mod, seed}`, `pyrandom` | S | IM-1, IM-6 (85 ids, vocabulary) |
| PF-2 | Articulation inference and passages, choice-index points | [c](design/performance.md) | `performance::articulate` | M | PF-1; SS-14, SS-16 (`articulation` parameter) |
| PF-3 | Level table, phrases, arcs, swells, roll crescendos, written velocities and lane ratios | [c](design/performance.md) | `performance::dynamics` | M | PF-1; SS-14, SS-21 (`dynamics`, layer (b) 5.4) |
| PF-4 | Humanization per family, attack compensation | [c](design/performance.md) | `performance::humanize` | S | PF-1, PF-9 |
| PF-5 | Voicing and doubling helpers by family | [c](design/performance.md) | `performance::voicing` | L | IM-13, IM-14 (ensembles, `orchestration::rules`) |
| PF-6 | Wave-1 rule predicates; harp-pedal and timpani-tuning derivations (section 6) | [c](design/performance.md) | `performance::check` | M | IM-12, IM-14 |
| PF-7 | Orchestrate from a reduction (skyline, chord match, key) | [c](design/performance.md) | `performance::reduction`; `theory` (`CHORD_QUALITIES` to `pub(crate)`) | L | PF-2-PF-6 |
| PF-10 | Orchestration findings | [c](design/performance.md) | `sonara-app` `quality::orchestration` | M | PF-6, PF-9 |
| PF-11 | Grammar over layer (a)'s `resolve()` mentions, suggestions (`strsim`), questions | [c](design/performance.md) | `sonara-agent` `orchestration::grammar` | L | IM-8, IM-9 |
| PF-12 | Step references, staged validation, rollback, report | [c](design/performance.md) | `sonara-agent` `orchestration::refs`, `mod` | M | — |
| PF-14 | Composer reference renderer, profile block, size and drift tests, generator example | [c](design/performance.md) | `sonara-midi` `orchestration::reference`; `sonara-app` `composer::invocation` | M | IM-1..IM-14, SS-12 |
| PF-17 | Listening checks of thresholds, overlap and crossfade times (B2), recorded locally | [c](design/performance.md) | `docs/performance.md` | S | SS-24, PF-2-PF-4 |
| PF-20 | Ensemble plan service (spine S9): parts in score order, family folders, sources through the chain, pan unless in situ, family buses; one plan for the GUI's `Transaction` and the planner's steps | [c](design/performance.md) | `sonara-app` `ensembles` | M | IM-13, SS-26 |
| PF-13 | `Orchestration` → steps (tracks, instrument, clips, lanes, routing, pan) | [c](design/performance.md) | `sonara-agent` `orchestration::ensemble` | M | PF-7, PF-9, PF-12, PF-20, SS-26 |
| PF-16 | Docs (DOC-01) | [c](design/performance.md) | `docs/` | S | PF-1-PF-14, PF-20 |
| PF-23 | Agent API 1.1: the additive tool changes (`NoteInput.articulation`, a `part` argument on `create_track`, the CC-writing tool) with tests that pin 1.1 as golden `tools.json` 1.0 plus exactly the recorded additions, and the same for `mcp-tools-list.json` and the IPC `hello`; golden files are not re-recorded (GLD-01) *(added with the phase-2 decision)* | [c](design/performance.md) | `sonara-agent::contract`, `crates/sonara-agent/tests` | M | IM-19 |
| PF-18 | Per-note articulation: `NoteInput.articulation` and a `part` argument on `create_track` (golden `tools.json`), per-note inference output *(moved from O6: phase 2 accepted on 2026-10-30)* | [c](design/performance.md) | `performance::articulate`; `sonara-agent` contract | M | IM-19; owner (A1, A3 b, A16); PF-23 |
| PF-21 | CC-writing tool and CC output of `perform()`; controller events in looping clips *(moved from O6: phase 2 accepted on 2026-10-30)* | [c](design/performance.md) | `sonara-agent` contract; `performance::dynamics`; `sonara-engine` `schedule` | M | owner (A4 a); PF-23 |

**Done when:**

- determinism tests with fixed seeds (TST-04); a test per shipped rule id with its oracle (TST-10)
- agent API 1.1: the golden-delta tests pass; every new error path of the changed tools has a test (TST-03); `docs/agent-api.md` is regenerated
- the walkthrough of [design/performance.md](design/performance.md) section 12 as an integration test against the null backend (TST-02), writing per-note articulations and CC once PF-18 and PF-21 are in
- the AI Composer reference passes its size and drift tests; `docs/orchestration.md` and `docs/ai-composer.md` describe the new behaviour and the changed planner guarantee

## O5. Interface

33 items (S 13, M 17, L 3), in dependency order. **Owner gate:** all decided: the GUI edits CC 64 and CC 11 (U1); download links only for VSCO 2 CE and VCSL (U2); ensembles in the GUI (A7).

| # | Item | Layer | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| PF-15 | Orchestrate field, undo-group wrapping, measured plan wall time (QG-11) | [c](design/performance.md) | `apps/sonara-desktop` (`src-tauri`, Svelte) through `sonara_app::edit` | S | PF-13; the undo groups of 0.1.12 |
| UI-2 | `Edit::SetAutomation`; `TrackView.automation` in the view and `types.ts` | [d](design/ui.md) | sonara-app `edit`, `view`; desktop `types.ts` | M | core `SetAutomation` |
| UI-3 | Core `SetControlEvents` gains an explicit (kind, number) target list; `Edit::SetControlEvents`; `MidiClipView.controls` | [d](design/ui.md) | sonara-core `commands::clip`; sonara-app `edit`, `view` | M | none |
| UI-4 | `Edit::CreateTrack` with `params`, `index` and `parent_id`; `Edit::SetSource`; each one `Transaction`; the preset-id parser | [d](design/ui.md) | sonara-app `edit`, `instruments` | M | SS-10 (A5) |
| UI-5 | Instrument context service and `track_instrument_contexts` command | [d](design/ui.md) | sonara-app `instrument_view`; src-tauri `commands.rs` | L | IM-1..IM-14, SS-13, SS-14 |
| UI-6 | `library_tree` and `instrument_detail` | [d](design/ui.md) | sonara-app `browser`, `instrument_view`; src-tauri `dialogs.rs` | M | IM-9, SS-12, SS-26 |
| UI-7 | Library tab: tree, alias search, filter, activation, context menu, row cap | [d](design/ui.md) | desktop `browser/` (new `library.ts`, `Library.svelte`) | M | UI-4, UI-6 |
| UI-8 | Library detail pane | [d](design/ui.md) | desktop `browser/` | S | UI-6 |
| UI-9 | Sample Libraries sheet (own commands, restart notice), outside the Settings dialog | [d](design/ui.md) | sonara-app `samples`; src-tauri `dialogs.rs`; desktop `dialogs/` | S | none |
| UI-10 | In-app install sheet from the profile; download links in the `open_link` allow-list; `reveal_preset` | [d](design/ui.md) | sonara-app `about`, `instruments`; src-tauri `dialogs.rs` | S | S7 profiles; U2, A17 for non-CC0 libraries |
| UI-12 | `orchestra_findings` service and command | [d](design/ui.md) | sonara-app `quality`, `instrument_view` | M | PF-10 |
| UI-11 | Track header chip, identity detail, badges, menu items, ARIA label | [d](design/ui.md) | desktop `timeline/headers.ts`, `Headers.svelte`, `a11y/labels.ts` | M | UI-5, UI-12 |
| UI-13 | Inspector Instrument group (Track group rows unchanged) | [d](design/ui.md) | desktop `inspector/` | S | UI-5, UI-12 |
| UI-14 | Family panel framework: binding model, gating, "Not available" list, duplicate-parameter hiding | [d](design/ui.md) | desktop `devices/family/` (new), `DeviceCard.svelte` | M | UI-5, SS-14 |
| UI-15 | Strings panel and `Edit::SplitDivisi` | [d](design/ui.md) | desktop `devices/family/`; sonara-app `edit` | M | UI-14 |
| UI-17 | Percussion pads and kit map, audition | [d](design/ui.md) | desktop `devices/family/` | M | UI-14, SS-12 (`keymap[].name`) |
| UI-18 | Woodwind and brass panel | [d](design/ui.md) | desktop `devices/family/` | S | UI-14, UI-12 |
| UI-19 | Voices panel | [d](design/ui.md) | desktop `devices/family/` | S | UI-14, A16 |
| UI-20 | Harp pedal diagram and timpani tuning views; harp panel | [d](design/ui.md) | sonara-app `instrument_view`; desktop `devices/family/` | S | UI-14, PF-6 (derivations) |
| UI-22 | Lane selector and Articulation lane | [d](design/ui.md) | desktop `pianoroll/paint.ts`, `gestures.ts`, `model.ts`, `PianoRoll.svelte` | L | UI-2, UI-5, SS-14 |
| UI-23 | Dynamics, Pedal and Expression lanes | [d](design/ui.md) | desktop `pianoroll/` | M | UI-2, UI-3, UI-22 |
| UI-16 | Keyboards panel (type comparison, pedal gating, lane shortcuts) | [d](design/ui.md) | desktop `devices/family/` | M | UI-14, UI-23 |
| UI-21 | Organ panel (registration, swell) | [d](design/ui.md) | desktop `devices/family/` | S | UI-14, UI-23 |
| UI-24 | Range shading and out-of-range outlines | [d](design/ui.md) | desktop `pianoroll/paint.ts` | S | UI-5 |
| UI-25 | Written/sounding toggle (labels, readout, ARIA) | [d](design/ui.md) | desktop `pianoroll/`, `a11y/labels.ts` | S | UI-5 |
| UI-26 | `keyLabels` (key switches, percussion), hatched key-switch notes | [d](design/ui.md) | desktop `pianoroll/` | S | UI-5 |
| UI-27 | Arrange in Score Order, Group by Family, folder collapse | [d](design/ui.md) | sonara-app `edit`; desktop `timeline/` | M | UI-5 |
| UI-28 | New Project `ensemble` field (defaulted; oracle unchanged) and preview | [d](design/ui.md) | sonara-app `projects`; desktop `dialogs/NewProject.svelte` | M | PF-20 (S9 service) |
| UI-29 | Add Ensemble… dialog, `Edit::ApplyEnsemble`, Track menu item, fixture regenerated | [d](design/ui.md) | sonara-app `edit`, `menus`; desktop `dialogs/` | M | UI-28 |
| UI-30 | ARIA layers for lanes and pads; a11y e2e checkpoints | [d](design/ui.md) | desktop `pianoroll/`, `bench/e2e-a11y.ts` | M | UI-17, UI-22 |
| UI-31 | Bench fixture library and orchestral scenarios in all perf:signoff parts | [d](design/ui.md) | desktop `scripts/bench-timeline.mjs`, `src/lib/bench/` | M | UI-22-UI-24, UI-11 |
| UI-32 | Docs (DOC-01) | [d](design/ui.md) | `CHANGELOG.md`, `docs/` | S | all |
| UI-33 | Phase 2: per-note articulation in the roll, note inspector, part editing in the Inspector *(moved from O6: phase 2 accepted on 2026-10-30)* | [d](design/ui.md) | desktop `pianoroll/`, `inspector/`; sonara-app `view`, `edit` | L | IM-19, PF-18 (A1, A3 b) |

**Done when:**

- page and unit tests for every new view; `pnpm run perf:signoff` meets ADR 0009 (timeline p95 ≤ 7 ms, meters ≥ 58 fps with p99 ≤ 50 ms, piano roll ≤ 2 display frames with 2,000 notes)
- VoiceOver labels for the new controls; the menu oracle (GLD-01) unchanged except by decision

## O6. Open decisions and plugin hosting

6 items (S 2, M 2, L 2), in dependency order. **Owner gate:** decided to wait: ensemble and orchestration tools only by a later versioned decision (A7); a GM engine decided here, after the recount (A10); plugin hosting (M5).

| # | Item | Layer | Crate / module | Size | Depends on |
|---|---|---|---|---|---|
| SS-30 | GM review spike and decision record | [b](design/sound-source.md) | none (spike) | S | SS-01..SS-12 |
| SS-31 | `sonara.gm` engine, if accepted | [b](design/sound-source.md) | `sonara-dsp::processors::gm`, `sonara-media::soundfont`, `sonara-core::plugins::builtin`, `sonara-app::samples` | L | SS-30; owner (A10) |
| SS-32 | Articulation translator for hosted instruments | [b](design/sound-source.md) | `sonara-engine::articulation` | M | M5 host |
| PF-19 | `orchestrate` and `check_orchestration` tools, golden contract update | [c](design/performance.md) | `sonara-agent` contract | L | owner (S10) |
| PF-22 | Ensemble tool or a wider `create_project` enum | [c](design/performance.md) | `sonara-agent` contract | M | PF-20; owner (A7 c) |
| UI-34 | GM sources in the Library | [d](design/ui.md) | sonara-app `browser` | S | A10 |

**Done when:**

- each open decision is recorded before its items start; tool additions follow the API 1.1 pattern of PF-23
