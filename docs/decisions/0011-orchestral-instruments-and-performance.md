# ADR 0011: Orchestral instruments: model, sound mapping and performance

- **Status:** Accepted (2026-10-30) by the owner, with the phase-2 contract core (decision 11)
- **Research:** [orchestra-research.md](../orchestra-research.md) and [orchestra/](../orchestra/README.md)
- **Design:** [orchestra-design.md](../orchestra-design.md), its four layer documents and the
  [implementation plan](../orchestra/implementation-plan.md)
- **Amends:** [ADR 0010](0010-sample-based-orchestral-instruments.md) decision 4 (the sampler's
  `instrument` parameter) and two of its consequences: a live graph rebuild reuses an unchanged sampler instead of
  restarting its notes, and the 3 GiB cap applies per device

## Context

Sonara can play SFZ instruments through `sonara.sampler` (ADR 0010) and has orchestration metadata for 16 instruments,
with several ranges and all articulation lists marked as estimates (`sonara_midi::orchestration`). It cannot yet compose
with an orchestra: nothing links a sample preset to an instrument, an agent cannot ask for "horns" and get horns in range,
the sampler cannot change articulation or dynamics except through key-switch notes, and the interface is the same generic
device card for every instrument.

The research mapped 85 instruments with sourced, fact-checked cards, ensemble templates, 67 doubling and balance rules,
the free libraries and their licences, and how the sampler reads them. It found that:

- the sampler diverges from the SFZ format in eleven confirmed places, and seven more findings follow (Q1–Q11, N1–N7 in
  [sfz-conformance.md](../orchestra/sfz-conformance.md)), so key-switch patches, the free grand pianos and several
  saxophones play wrongly or not at all;
- legato is both an engine gap (`trigger=first`/`legato` become attacks) and a sound gap (no free library records
  transitions); dynamics need controller crossfades, which the sampler ignores, and no agent tool writes MIDI CC;
- 49 of the 85 ids have a free file known to play; 36 do not.

The constraints that decide the shape are rules: CTR-01 (57 tools, tool schemas and the `.sonara` schema change only by a
versioned decision; notes have no articulation field; `create_project` has a fixed template enum), GLD-01 (twelve golden
built-ins; the `orchestral` template is golden-covered by the "Ascension" example), RT-01..RT-05 and RT-10 (the audio
path), LIC-01 and ADR 0010 (nothing bundled).

## Decision

1. **One instrument model** in `sonara_midi::orchestration`: the 85 ids with the corrections of
   [model.md](../orchestra/model.md), multilingual aliases, a category with a default member, clefs, a GM voice, an
   ensemble profile, keyboard mechanics and playability facts, with provenance per value; the ensemble templates and the
   doubling and balance rules as pure data; one family enum shared with the catalog.
2. **Articulations:** one primary id per note from a canonical vocabulary of 96 ids in ten kinds (the 15 existing ids
   keep their spelling; the list only grows at the end); no id means ordinario; mute and vibrato are passage states; an
   unrecorded combination falls back by dropping the state and says so.
3. **The sampler carries the track state.** Instrument identity is derived, in this order of trust: *named* (the user's
   `instrument_type` sampler parameter, not `auto`), *declared* (the library's manifest), *profile* (Sonara's shipped
   profile), *inferred* (an alias match on the patch path, or on the track name for Poly Synth and Drum Kit tracks).
   Articulation and dynamics reach the library through two new automatable sampler parameters written with the existing
   `create_automation`: `articulation`, an enum with one frozen, append-only choice list for every preset (`auto`,
   `ordinario`, then the vocabulary), applied inside the sampler by a table lookup without key-switch notes, a choice the
   preset lacks falling back to the nearest articulation; and `dynamics` (0–1, default 1.0), a ratio to the note's
   velocity in the profile's `velocity` and `fixed` modes (1.0 leaves today's render unchanged) or an absolute
   controller value in its `cc` mode (1.0 selects the loudest layer).
   Device parameters are sampled once per block, so a change lands up to one block late and a seek restores the right
   articulation. The sampler is not a golden built-in, so its descriptor may grow. Once decision 11 lands, a note's own
   `articulation` overrides the track's state at that note, CC 1 and CC 11 in clips drive the dynamics, and the
   `dynamics` parameter remains a track-level trim.
4. **The sampler's `instrument` parameter stores catalog preset ids** (`<library>/<patch path without .sfz>`,
   `<library>/@<set>` for an articulation set); absolute paths from older projects are mapped to ids on load. A library that matches a shipped
   profile takes the profile's id. This amends ADR 0010 decision 4.
5. **Sampler engine work**, pure Rust and real-time safe, no new dependency, each item with its regression test, following
   ARIA and sfizz where the SFZ specification is silent: the conformance fixes first (Q1 with N4, Q2–Q4, the key half of
   Q7, Q11, N1–N3, N5–N7, sfz-conformance C8), with the controller half of Q7 joining the crossfades, then velocity and controller crossfades, `amp_veltrack` and
   `off_mode` (Q6, Q8, Q9), then `trigger=first`/`legato` with an overlap-crossfade legato on sustain samples, then
   multi-patch articulation sets per device.
6. **Library profiles:** `sonara-library.json` version 2 maps each preset to an instrument id, solo or section,
   articulations and how they are reached, dynamics and legato modes, seating and a verdict. Sonara ships profiles (its
   own data, no samples) for VSCO 2 CE and VCSL v1.2.2-RC. A user's manifest overrides a shipped profile.
7. **A degradation chain** in `sonara-app` resolves every request from the exact preset down to silence and reports each
   substitution.
8. **Ensembles** are data applied through existing commands as one undo step. In phase 1 the GUI's New Project, Add
   Ensemble… and Orchestrate… reach them; MCP agents and the AI Composer build ensembles from existing tools, guided by
   the orchestration reference. The golden `orchestral` template stays unchanged.
9. **Agents:** a generated orchestration reference (at most 24,000 characters, tested for size and drift) in the AI
   Composer's system prompt; instrument and ensemble requests in the deterministic planner (English and Czech), with
   a read-only catalog query, and staged validation with undo rollback (through the undo groups of 0.1.12) replacing the
   guarantee that a refusal changes nothing; orchestration findings in
   `sonara_app::quality`, by rule id. The agent tools change only as decision 11 says.
10. **Interface:** a Library browser by family and instrument, family panels, an instrument-aware track header,
    piano-roll range shading, a written/sounding view, articulation, dynamics, CC 64 and CC 11 lanes (the CC lanes edit
    clip events through the core command `SetControlEvents`, before any agent tool can), ensembles in New Project and
    Add Ensemble…, all measured against ADR 0009.
11. **The phase-2 contract core is accepted with this ADR** (owner, 2026-10-30; CTR-01, versioned):
    - **project schema 2** with an optional track `part` (instrument, name, players, solo, the doubling player, a
      notation override) and an optional note `articulation`, written only when a document needs them: the golden
      version-1 bytes stay valid, and an unchanged v1 document opens without a reported migration (GLD-01);
    - **agent API 1.1**, additive: `NoteInput.articulation`, a `part` argument on `create_track`, and a tool that writes
      MIDI CC into clips. The golden 1.0 contract snapshots (`tools.json`, `mcp-tools-list.json`) stay as recorded;
      tests pin 1.1 as 1.0 plus exactly these additions (GLD-01);
    - still separate decisions: ensemble, orchestrate and check tools (A7) and a GM fallback engine (`rustysynth`, A10).
      Plugin hosting (M5) reuses the same articulation maps.
    The [implementation plan](../orchestra/implementation-plan.md) schedules these items in O2 (schema 2), O3 (per-note
    articulation in the engine and the sampler), O4 (API 1.1) and O5 (per-note articulation in the piano roll).
12. **Dependencies:** no new crate enters `Cargo.lock`. New edges to crates the workspace already locks (`serde` and
    `unicode-normalization` in `sonara-midi`, `strsim` in `sonara-agent`, `proptest` as a dev-dependency of
    `sonara-app`) are allowed and recorded in
    `docs/dependencies.md` (QG-10).

## Alternatives considered

| Alternative | Why not now |
|---|---|
| Phase 1 only, with the contract changes deferred to a later ADR | The design's first recommendation; the owner chose to take the contract core now, so per-note articulation and CC writing do not wait for O6 |
| Key-switch notes written into clips as the authoring path | Works under today's contracts, but clutters clips, breaks range checks, depends on per-library key numbers and loses the switch on a seek (C11); kept only for imported MIDI |
| Replacing the `orchestral` template with a real orchestra | It is golden-covered (Ascension, its golden workflow and a render-load baseline); GLD-01 forbids it |
| Working around the SFZ divergences in the profiles | Leaves about 75 key-switch files and every free grand piano broken for users without profiles |
| An FFI engine: sfizz (BSD-2-Clause, archived) or FluidSynth (LGPL-2.1, only as a replaceable dynamic library with its source shipped, LIC-01) | Both need an RT-10 amendment; unchanged from ADR 0010 |
| A GM fallback engine first: `rustysynth` (MIT, pure Rust, on the real-time path, so RT-02..RT-05 must be shown) | Adds a second engine for low-quality voices before the conformance work shows how many ids still lack a free SFZ |
| Commercial libraries through plugin hosting (M5; the VST3 SDK and CLAP are MIT) | The quality ceiling, but a separate milestone with its own sandboxing design ([plugin-hosting.md](../plugin-hosting.md)) |
| Bundling a library | ADR 0010 and LIC-01; shipped profiles give most of the benefit without redistributing samples |

## Consequences

- Two versioned contract changes: project schema 2 (lowest-version writer) and agent API 1.1 (additive). The sampler's
  descriptor grows (`instrument_type`, `articulation`, `dynamics`), and its `instrument` values change form, which
  projects that stored absolute paths still accept.
- The sampler's real-time code grows (crossfades, legato, several banks per device); every addition carries
  `assert_no_alloc` tests, and memory per device must be measured (HW-01).
- The track-level articulation state is one per track; per-note articulations let notes on one track differ.
  Ensembles still place divisi parts on separate tracks.
- The articulation choice list is frozen once projects store its indices: the vocabulary decisions come before the first
  release that writes `articulation` automation.
- The planner's documented guarantee changes from "a refusal changes nothing" to "a failed step is rolled back with
  undo" (`docs/orchestration.md` must say so when O4 lands).
- Legato built from sustain samples is not recorded legato. The release notes and the AI Composer's reference must say
  so; plugin hosting is the route to recorded transitions.
- Sound quality has not been judged. The first step after the conformance work is a listening test with VSCO 2 CE and
  VCSL.

## Owner decisions (2026-10-30)

Decided:

1. Decisions 1–12 are accepted as written, and phase 2's contract core is taken now (decision 11) instead of being
   deferred.
2. Preset ids in the sampler: yes; ADR 0010 decision 4 is amended (A5).
3. Library profiles ship for VSCO 2 CE and VCSL only; none for Sonatina or CC BY libraries (A6, A17).
4. Articulation shape B: one primary id per note, mute and vibrato as passage states (A16); no id means ordinario
   (A14); `col_legno_battuto` stays apart from `col_legno` (A12).
5. The remaining recommendations are accepted (second round, the same day): ensembles in the GUI now and agent tools
   only by a later versioned decision (A7); overlap legato on sustain samples, kept if the listening check B2 passes
   (A9); a GM engine decided in O6 after the recount (A10); GM data keeps exact programs and substitutes apart, with an
   orchestral percussion map beside `GM_MAP` (A11); the cards' range values, notation (tenor at transposition 0 with a
   treble-8 clef, Wagner tuba tenor at −2), ids, families and controls, and the data.rs corrections (A18–A23); the code
   fixes outside the contracts, each with a test (A24); coverage reported in both counts (A25); humanization on by
   default (P2); download links only for VSCO 2 CE and VCSL (U2).
6. Of the 34 invented articulation ids, only those that at least one library records enter the first frozen list;
   the others are appended later (A13). Mallet rolls use `tremolo` and drums and timpani use `roll`, as the existing ids
   define them (A15).
7. The owner asked for the full implementation of the plan.

The options and the recommendation for each are in section 5 of [orchestra-design.md](../orchestra-design.md).

The detailed questions, with options and the research's recommendations, are A1–A25 in
[open-questions.md](../orchestra/open-questions.md).
