# Orchestral instruments in Sonara: design proposal

- **Status:** Proposed (2026-10-30). Nothing here is implemented.
- **Decision record:** [ADR 0011](decisions/0011-orchestral-instruments-and-performance.md) (proposed)
- **Research:** [orchestra-research.md](orchestra-research.md) and [orchestra/](orchestra/README.md)
- **Layers:** (a) [instrument model](orchestra/design/instrument-model.md) · (b) [sound source](orchestra/design/sound-source.md)
  · (c) [performance layer](orchestra/design/performance.md) · (d) [user interface](orchestra/design/ui.md)

This document holds the decisions **S1–S12** that all four layer documents share (the layers call them "the spine").
A layer refines a decision in detail; where it corrects one, it says so in its "Deviation from the spine" section, and
the correction is already folded in below.

## 1. Goal, and what is honestly reachable

Sonara should not only let you write MIDI but compose. When a user or an agent says "the string section plays a legato
melody, horns and timpani underneath", Sonara should know what those instruments are, their ranges, how they are played,
how they sound and how they are controlled, the way GarageBand and Logic Pro do.

The research shows that this sentence is **not deliverable end to end with free libraries and today's sampler**:

- **Horns and timpani are.** VSCO 2 CE and VCSL have solo horn sustains and timpani hits and rolls that the sampler plays
  as is; Sonatina has a horn section that plays partly.
- **String legato is not.** It is an engine gap (the sampler turns `trigger=first`/`legato` into attacks and chokes the
  old voice after 10 ms; Q5 and Q9 in [sfz-conformance.md](orchestra/sfz-conformance.md)) and a sound gap (no free library
  records legato transitions; Sonatina's legato reuses its sustain samples).
- **Dynamics are not.** Swells on held chords, timpani roll crescendos and string phrasing need controller-driven
  crossfades, which the sampler ignores, and no agent tool writes MIDI CC.

So the first orchestral release can demonstrate: the right instruments, chosen by id and alias (also in Czech), in
range, in score order, with section sizes; horns and timpani from free libraries; a string melody with an
**overlap-crossfade legato built from sustain samples**, not recorded transitions; and dynamics where a library has
velocity or controller layers. Recorded legato and the top of the quality range need commercial libraries through plugin
hosting (milestone M5). The performance layer walks through the sentence step by step, tool call by tool call
([performance.md](orchestra/design/performance.md)).

## 2. The four layers

```text
 (a) instrument model         sonara_midi::orchestration: 85 ids, aliases, ranges, GM voices, ensembles, rules
        │ ids, ranges, articulation vocabulary
        ▼
 (b) sound source             sonara-library.json v2 profiles → catalog → sonara.sampler (preset ids;
        │                     instrument_type, articulation and dynamics parameters) → degradation chain
        │ playable articulations and dynamics per preset
        ▼
 (c) performance layer        sonara_midi::performance: articulation inference, dynamics shaping, humanization,
        │                     checks by rule id, orchestration from a reduction; planner and AI Composer use existing tools
        ▼
 (d) user interface           Library browser by family, family panels, track header, piano-roll range shading,
                              articulation, dynamics and CC lanes, ensembles in New Project
```

Each layer reads only the layers above it. The model and the performance logic are pure Rust in `sonara-midi` (no I/O,
not real-time; ARC-11, RT-01). Profiles, the catalog mapping, ensembles and the degradation chain are application
services in `sonara-app`. Only sample playback runs on the audio thread (RT-02..RT-05).

## 3. Decisions

Each decision has a recommendation, its alternatives, its contract impact and who decides. **Recommended** means it stays
inside the existing contracts (CTR-01, GLD-01) and can proceed when ADR 0011 is accepted. **Owner** means it changes a
contract, a licence position, a documented guarantee or an earlier ADR. Ids A1–A25 refer to
[open-questions.md](orchestra/open-questions.md); P1–P3 and U1–U2 are raised by the performance and interface layers.

### S1. One instrument model in `sonara_midi::orchestration` (recommended)

The 85 researched ids become the dataset ([model.md](orchestra/model.md)), held by a typed `InstrumentType` enum in score
order so that ensembles and rules cannot name a misspelt id; the string id stays the external form. The record gains
names in five languages and the score abbreviation; aliases for the deterministic planner, stored folded, with Czech
plural and case forms written out, matched longest phrase first, ambiguous words (`alto`, `bass`) answered with every
candidate and a question; a category with a selection rule (default member, all, or ask); clefs; a GM voice that keeps
exact programs and substitutes apart (`Program`, `Percussion`, `Substitute` with its fit, `NoEquivalent`); an ensemble
profile (solo, section, players by era); keyboard mechanics (action, pedals and their controllers, resonance, noises,
decay) and playability facts for the rules. Provenance stays per value. The corrections of model.md are applied
(14 of the 16 existing ids, 21 values). The data stays compile-time-validated Rust statics, split by family. The catalog
re-exports the model's family enum, which gains a serde derive, so the catalog's JSON is unchanged. Ensemble templates and
the 67 doubling, balance and playability rules are typed data in the same module (DES-01). Detail:
[layer (a)](orchestra/design/instrument-model.md). Owner: A18–A23.

### S2. Articulations: one primary id per note, plus mute and vibrato as states (recommended; points owner)

The vocabulary grows from 15 to 96 ids (95 if `col_legno_battuto` merges), each with one of ten kinds; the 15 existing ids
keep their spelling, and the list only ever grows at the end. A request is one primary id (none means ordinario: the
instrument's ordinary sound) plus the passage states mute and vibrato, as players and libraries treat them;
`con_sordino` stays as an id. A combination a library did not record falls back by dropping the state, and the user is
told. Owner: the merges and invented ids (A12, A13), the ordinario rule (A14), mallet rolls (A15), shape B against a flat
list (A16).

### S3. Where identity lives in a project

- **Phase 1 (recommended):** identity is derived, in this order of trust: a phase-2 `part` > the sampler parameter
  `instrument_type` named by the user > the library's own manifest > Sonara's shipped profile > a guess from the track
  name > none. The sampler is not one of the twelve golden built-ins (ADR 0010 decision 4), so its descriptor can grow;
  the application passes the id choices in, because `sonara-core` may not depend on `sonara-midi`. Part names ("Violins
  I") live in track names. Poly Synth and Drum Kit tracks carry no identity; a match by track name is shown as inferred.
- **Phase 2 (owner, CTR-01):** project schema v2 adds an optional track `part` (instrument, name, players, solo) and an
  optional note `articulation`, with a v1 → v2 migration and the required features `parts` and `note-articulations`. The
  writer writes the lowest version a document needs, because the bytes of the rich test project and all five golden
  workflows are version 1 (GLD-01). Owner: A1, A2.

### S4. Articulation and dynamics reach the library through sampler parameters first

- **Phase 1 (recommended):** two automatable sampler parameters, written with the existing `create_automation`.
  - `articulation` is an enum with **one frozen, append-only choice list for every preset**: `auto`, `ordinario`, then the
    vocabulary. A per-preset list is impossible, because a parameter belongs to the plugin descriptor and automation
    stores choice indices. The sampler applies a change in `set_params` by a table lookup in its prepared setup: a forced
    key switch, a velocity zone, a member patch of an articulation set, or a controller value, with no allocation. A
    choice the preset lacks falls back as S2 says and is reported. Clips hold no key-switch notes, and because device
    parameters are evaluated at every block's position, a seek or loop wrap restores the right articulation (fixes C11).
  - `dynamics` (default 1.0, no change) is a ratio applied to velocity where the profile has velocity layers, and an
    absolute controller value where it has controller crossfades.
  - Timing: the engine samples device parameters once per block, so a change takes effect at the next block start, up
    to one block late (`crates/sonara-engine/src/automation.rs`). The performance layer writes changes two blocks early.
    One state per track: divisi parts with different articulations go on separate tracks.
- **Phase 2 (owner, CTR-01):** a per-note `articulation` in the schema and in `NoteInput`, and a tool that writes MIDI CC
  into clips. Owner: A3, A4.

### S5. The sampler stores preset ids (owner: amends ADR 0010 decision 4)

The `instrument` parameter's choices become catalog preset ids (`<library>/<patch path>`, and `<library>/@<set>` for an
articulation set), resolved to files by the application through a resolver it gives the media provider. Absolute paths
from older projects are mapped to ids when a project loads. A library that matches a shipped profile takes the profile's
id instead of its folder slug, so renaming a folder no longer changes ids (C4); this lands in O2, before any project
stores a preset id. Discovery skips include fragments (C8). Owner: A5.

### S6. Sampler engine work (recommended; pure Rust, real-time safe, no new dependency)

In priority order, each with the regression test named in [sfz-conformance.md](orchestra/sfz-conformance.md); where the
SFZ specification is silent, Sonara follows ARIA and sfizz (A8):

1. **Conformance** that unlocks existing files: key switches (Q1, Q11; about 75 files), includes and defines (Q2–Q4: the
   grand pianos and the saxophones), `-1` keys and CC gates (Q7), release samples on pedal up (N1), a missing sample no
   longer fails a patch (N3), unsupported values reported (N2), include fragments (C8).
2. **Expression:** velocity crossfades (Q6), `amp_veltrack` (Q8), controller crossfades driven by CC 1, CC 11 or the
   `dynamics` parameter, `off_mode=normal` (Q9).
3. **Legato:** `trigger=first`/`legato` and an overlap-crossfade legato on sustain samples.
4. **Articulation sets:** one device holds the preloaded banks of several patches of one instrument, chosen by
   `articulation` (the 3 GiB cap applies per device).

Everything on the audio path follows RT-02..RT-05 with `assert_no_alloc` tests. No maintained, permissively licensed SFZ
crate exists (QG-13, ADR 0010); sfizz and FluidSynth stay deferred (RT-10). Detail:
[layer (b)](orchestra/design/sound-source.md). Owner: A8, A9.

### S7. Library profiles: Sonara ships mapping data, never samples

`sonara-library.json` version 2 adds per preset: the orchestration id, solo or section and players, the articulations and
how each is reached (patch, key switch, velocity, controller), the dynamics mode, the legato mode, whether the library was
recorded in its seating position (then pan stays 0), a verdict (plays as is, partly, not) and the licence. Precedence,
lowest first: the scan, a structural check of the SFZ file, the shipped profile, the user's manifest (field by field).
Version 1 manifests keep working, and an older Sonara ignores a version 2 manifest without failing. Sonara ships
profiles for **VSCO 2 CE** and **VCSL v1.2.2-RC** (CC0), embedded in `sonara-app` at build time: its own data, no samples,
LIC-01 clean. Owner: shipping profiles at all (A6); no profile for Sonatina (a retired licence and Philharmonia-derived
samples whose terms exclude sampler instruments) or for CC BY libraries; users can write their own (A17).

### S8. A degradation chain that always says what it did

An application service resolves a request in this order and reports every substitution (ADR 0010 soft failure): the
exact preset, the nearest articulation, a solo or section swap, a neighbour of the family within range, (a GM program),
a Poly Synth preset or Drum Kit note, silence with a warning ([sound-mapping.md](orchestra/sound-mapping.md), section 4).
It runs off the audio thread. Owner: a GM fallback engine (`rustysynth`, registered by the application like the sampler,
never a golden built-in, with a user-installed MIT SoundFont); the recommendation is to decide in O6, after counting
again the ids that no free SFZ plays (36 of 85 today). Owner: A10, A11.

### S9. Ensembles

The ensemble templates of [ensembles.md](orchestra/ensembles.md) are data applied by one application service
(`sonara_app::ensembles`, shared by the planner and the GUI) through existing commands as one undo step, into a new project
or an open one ("Add Ensemble…"): folders per family, one track per part in score order, section sizes, pan only for
libraries not recorded in situ, family buses. The existing `orchestral` template is covered by golden data and stays
exactly as it is (GLD-01). Phase 1: the GUI offers ensembles, and agents reach them through the planner as a sequence of
existing tools. Owner (CTR-01): extend the `create_project` enum or add an ensemble tool (A7).

### S10. Agents learn the orchestra without new tools first

- The AI Composer's system prompt gets an orchestration reference generated from the model (ids, ranges, roles, the key
  doubling and balance rules, the sampler parameters and how to automate them), at most 24,000 characters, with a size
  test and a drift test so it never disagrees with the data. Owner: whether every turn carries it (P3).
- The `sonara_agent::orchestration` planner learns instrument and ensemble requests in English and Czech. It needs two
  changes: a later step can use an id an earlier step returned, and validation becomes staged, with a failed step rolled
  back through undo. That replaces today's documented guarantee that a refusal changes nothing (owner, P1).
- `sonara_app::quality` reports orchestration findings by rule id; a first wave of 19 rules needs only the instrument,
  notes, tempo and lanes. Humanization is on by default when the layer writes parts (owner, P2).
- New tools (orchestrate, check, a CC writer, an ensemble tool) are phase 2 and need the owner. Detail:
  [layer (c)](orchestra/design/performance.md).

### S11. An instrument-aware interface (recommended)

A Library tab in the Browser by family, instrument and source with availability, losses and licence; the track header and
Inspector with the instrument, where its identity comes from, the part, solo or section, the articulation at the playhead
and range warnings; family panels on the sampler card (strings, keyboards, percussion, woodwinds and brass, voices, harp,
organ), where a control the engine or the source cannot honour is listed as not available with the reason; a piano-roll
lane selector (velocity, articulation, dynamics, CC 64, CC 11), range shading and a written/sounding view; score-order and
family-folder actions; ensembles in New Project and Add Ensemble. The GUI may edit the CC 64 and CC 11 lanes of clips
through the existing core command `SetControlEvents` before any agent tool can (owner, U1); links to library download
pages use an allow-list (owner, U2). Every new canvas element is measured against ADR 0009 (timeline p95 ≤ 7 ms; meters
≥ 58 fps, p99 ≤ 50 ms; piano roll ≤ 2 display frames). Detail: [layer (d)](orchestra/design/ui.md).

### S12. Milestones

All except O6 belong to milestone M4 ("breadth") and change no contract.

| # | Milestone | Delivers | Contract impact |
|---|---|---|---|
| O1 | Instrument model | 85 ids with corrections, vocabulary with kinds, aliases, GM voices, ensembles and rule tables as data; one family enum | none |
| O2 | Sampler conformance | S6.1, preset ids and profile ids (S5) | ADR 0010 amendment (owner) |
| O3 | Sound mapping and expression | Library profiles, catalog mapping, degradation chain; `instrument_type`, `articulation`, `dynamics`; S6.2–S6.4 | none |
| O4 | Performance and agents | Articulation inference, dynamics shaping, humanization, checks; ensembles service; planner and AI Composer knowledge; quality findings | none (P1 changes a documented guarantee) |
| O5 | Interface | Library browser, family panels, track header, piano-roll lanes and shading, ensembles | none |
| O6 | Contract v2 (owner) | Schema v2 `part` and note `articulation`, a CC-writing tool, an ensemble tool, orchestrate and check tools; GM fallback; plugin hosting (M5) drives the same articulation maps | CTR-01, versioned |

The order follows the gaps: O2 unlocks files users already have, O3 makes the product sentence audible, O4 makes it
reachable by words, O5 makes it visible. Each layer document lists its work items with sizes, dependencies and tests
(about 100 in all).

## 4. Contracts and rules

| Rule | How the proposal holds it |
|---|---|
| CTR-01 | O1–O5 add no tool, tool field, schema field, IPC message or CLI command; everything that would is in O6 and needs the owner |
| GLD-01 | The twelve golden built-ins, the golden `orchestral` template and the golden transforms are untouched; the sampler is not golden; schema v2 writes version 1 whenever it can, so golden bytes stay valid |
| RT-01..RT-05 | Only sample playback runs on the audio thread; maps and banks are built off it and preloaded; every new real-time path has `assert_no_alloc` tests |
| RT-10 | No `unsafe`, no FFI engine |
| LIC-01, ADR 0010 | No samples are bundled; shipped profiles are Sonara's own 0BSD data; share-alike and non-commercial libraries are only documented |
| QG-10, QG-13 | No new crate in `Cargo.lock`. New edges to crates the workspace already locks (`serde` and `unicode-normalization` in `sonara-midi`, `strsim` in `sonara-agent`) are recorded in `docs/dependencies.md`; the owner confirms this reading of "no new dependency" |
| ARC-11, MOD-09 | Pure data and logic in `sonara-midi`, services in `sonara-app`, playback in `sonara-dsp`/`sonara-media`; no new crate |
| ADR 0009 | Every new GUI element is measured with `pnpm run perf:signoff` |
| DOC-01 | Each milestone updates the changelog, the status table and the affected guides |

## 5. Owner decisions

| Question | Recommendation | Ids |
|---|---|---|
| Accept phase 1 (S1–S11) with no contract change | Yes | ADR 0011 |
| Schema v2 with a track `part` and a note `articulation` | After O1–O5 prove the model; versioned; lowest-version writer | A1, A2, A3 |
| Dynamics through a sampler parameter or a CC tool | The parameter first; a CC tool with schema v2 | A4 |
| Preset ids in the sampler (ADR 0010 decision 4) | Yes, with old paths mapped on load | A5 |
| Ship library profiles; which libraries | Yes for VSCO 2 CE and VCSL; not for Sonatina or CC BY libraries | A6, A17 |
| Ensemble templates for agents | Planner first; the `create_project` enum or a tool later, versioned | A7 |
| Fix the SFZ divergences; follow ARIA/sfizz where the spec is silent | Yes, in O2, with the named regression tests | A8 |
| Legato | Overlap crossfade on sustains first, after a listening check; recorded legato through M5 | A9 |
| A GM fallback engine | Decide in O6 with a fresh count | A10, A11 |
| Articulation vocabulary | As in S2 | A12–A16 |
| Range semantics, notation (tenor at transposition 0 with a treble-8 clef), new ids, controls | As layer (a) proposes | A18–A22 |
| Apply the data.rs corrections | Yes, in O1 | A23 |
| Staged planner validation with undo rollback | Yes | P1 |
| Humanization on by default; the AI Composer reference on every turn | Yes | P2, P3 |
| GUI editing of CC 64 and CC 11 before an agent tool; download-page allow-list | Yes | U1, U2 |
| "No new dependency" means no new crate | Yes | QG-10 |

## 6. Risks

- **Sound quality is unknown.** Nothing was listened to; the free libraries may disappoint even when they play
  correctly. The first step after O2 is a listening test (open question B1).
- **Legato built from sustains** may not satisfy "legato melody"; the honest fallback is plugin hosting (M5).
- **Track-level articulation** (phase 1) cannot give divisi different articulations on one track, and a change lands up
  to one block late; the ensemble service puts divisi on separate tracks and the performance layer writes changes early.
- **The frozen articulation list** cannot shrink once projects store its indices; the vocabulary decisions (A12–A16) must
  come before the first release that writes `articulation` automation.
- **Memory:** articulation sets hold several patches per device; the 3 GiB cap and the RAM needs per profile must be
  measured (HW-01, recorded in `docs/audio-engine.md`).
- **Licence drift:** library licences and provenance can change; profiles record the licence they were written against.
