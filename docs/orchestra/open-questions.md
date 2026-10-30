# Orchestra research: open questions

_Triage of 2026-10-30 of every open question in the orchestral instrument research. Status: research input
for the orchestra design; nothing here is decided or implemented._

## How to read this file

This file collects, in one place, the open questions from:

- every card in [cards/](cards/): the 15 instrument cards, [cards/free_libraries.md](cards/free_libraries.md)
  and [cards/apple.md](cards/apple.md). Each card's section "Open questions and disagreements between
  sources" and the remaining concerns of its fact-check log were read, along with its consistency review;
- the open-question sections of [ensembles.md](ensembles.md), [doubling-and-balance.md](doubling-and-balance.md),
  [sound-mapping.md](sound-mapping.md) and [coverage.md](coverage.md);
- section 3.6 ("Questions for the owner") of [articulation-vocabulary.md](articulation-vocabulary.md);
- sections D and E of [sfz-conformance.md](sfz-conformance.md).

The same disagreement often appears in several of these files. Here it appears once, with links to every
place that discusses it. Each item is in exactly one class:

- **A. Owner decisions:** product, contract (CTR-01, GLD-01), licence (LIC-01, ADR 0010) or architecture
  choices. Each gives the question, the options, the research's recommendation with its reason, and where
  it is discussed.
- **B. Human checks:** what only a person can settle: a render or listening test, a printed score or page
  scan, a check inside an app, a question to a player, or a source that could not be opened.
- **C. Data disagreements:** sources disagree about a value. One row each, with the value the cards
  adopted and why, grouped by family.
- **D. Research follow-ups:** what more reading or file inspection could settle.

Conventions:

- The ids (A1, B1, C1, D1) are stable. When an item is settled, it is marked as resolved; it is never
  renumbered.
- "OQ n" is item n of the linked file's open-question list. "FC" is the remaining concerns in the card's
  fact-check log. Q1-Q11 and N1-N7 are row ids of [sfz-conformance.md](sfz-conformance.md); its claim
  rows C1-C24 are always written with the file name ("sfz-conformance C22"), to keep them apart from
  this file's C ids.
- MIDI note numbers use C4 = 60 and give sounding pitch unless a value is called written, as in
  [model.md](model.md).
- `[Estimate: reason]` marks a judgement made in this triage. The adopted values in section C carry their
  provenance in the cards.

## A. Owner decisions

**Decided on 2026-10-30.** The owner accepted [ADR 0011](../decisions/0011-orchestral-instruments-and-performance.md)
together with the phase-2 contract core, and decided A1, A3, A4, A5, A6, A12, A14, A16 and A17 (marked below). The other
A items stay open; [implementation-plan.md](implementation-plan.md) shows which work items wait for which of them.

### A1. Where an instrument's identity and part live in a project

**Decided 2026-10-30 by the owner:** accepted with ADR 0011: project schema v2 with an optional track `part`, written only when a document needs it (milestone O2).

- **Question.** Project v1 tracks have no field for the orchestration id, the part (Violins I, Horn 3), the
  player, solo or section, divisi, mute, role or notated dynamic. Range, doubling and balance checks need
  them, also after a project is saved and reopened.
- **Options.** (a) Keys in the track's `extra` map. They are kept on a round trip, but `create_track` has
  no parameter for them, so an agent cannot write them. (b) A versioned change to
  `project-v1.schema.json` that adds a track `part` object (`instrument_id`, `part`, `player`,
  `ensemble_id`) written by `create_track` (CTR-01). (c) No project data: the arranging layer derives the
  identity in memory from template data and the track name.
- **Recommendation.** None final. Templates can be applied today without a schema change (c), but the
  part then survives only in the track name. (b) is the route if the checks must work on reopened
  projects.
- **Discussed in.** [doubling-and-balance.md](doubling-and-balance.md) OQ5;
  [ensembles.md](ensembles.md) 4.3; [coverage.md](coverage.md) gap rank 6.

### A2. A doubled instrument: its own track or a switch

- **Question.** When a player doubles (flute and piccolo), is the second instrument a track of its own or
  an articulation-like switch on the player's track?
- **Options.** Its own track; or one track with a switch, which matches how the part is printed.
- **Recommendation.** Its own track, because the sampler holds one patch per device and its `instrument`
  parameter is not automatable (sfz-conformance C2).
- **Discussed in.** [ensembles.md](ensembles.md) OQ7.

### A3. How articulations reach a library

**Decided 2026-10-30 by the owner:** both: the sampler's `articulation` parameter as the track state, and a per-note `articulation` in schema v2 and agent API 1.1 on top (option c with option b).

- **Question.** A note carries no articulation, and no tool writes program changes or CC into a clip.
- **Options.** (a) The planner or the agent writes key-switch notes into clips with today's note tools.
  They appear in the user's MIDI, and the switch note must be held through the passage, because a seek or
  loop wrap while playing keeps the last switch (sfz-conformance C11). (b) A per-note articulation field
  (a CTR-01 change to `project-v1.schema.json`; notes do not keep unknown keys) and an articulation lane
  in the piano roll. (c) A new automatable enum parameter `articulation` on `sonara.sampler`, driven by
  `create_automation`. This needs no schema change, because the sampler is not one of the 12
  golden-pinned built-ins, but it gives one state per track, applied at the start of the audio block that
  holds its beat. No enum parameter is automatable today.
- **Recommendation.** (a) now, with the switch keys in the library mapping data (A6); (b) later, by a
  versioned decision, once the vocabulary shape (A16) is chosen. Reason: (a) is the only route the 57
  tools drive today, and a per-note field should not be designed before its shape.
- **Discussed in.** [sound-mapping.md](sound-mapping.md) OQ4 and 3.3; [coverage.md](coverage.md) gap
  rank 7; [articulation-vocabulary.md](articulation-vocabulary.md) 3.1 and 3.4.

### A4. Dynamics: an automatable sampler parameter or a CC-writing tool

**Decided 2026-10-30 by the owner:** both: a CC-writing tool (agent API 1.1) for dynamics in clips, and the sampler's `dynamics` parameter as a track-level trim.

- **Question.** Libraries put dynamics on CC1 crossfades (Spitfire, Sonatina Performance). The sampler acts
  on CC 7, 11 and 64 only, and no tool writes CC into a clip.
- **Options.** (a) A CC-writing agent tool and a CC lane (CTR-01). (b) An automatable `dynamics` float on
  `sonara.sampler`, targeted by `create_automation` as `<device_id>:dynamics`: no schema change, but per
  track and per audio block. (c) Today's workaround: velocity on the Notation patches plus track-volume
  automation, which changes level but not timbre. (a) and (b) both need CC and velocity crossfades in the
  sampler first (A9). What `dynamics` drives for each preset (velocity, CC1 or CC11) belongs in the
  mapping data (A6).
- **Recommendation.** None. [coverage.md](coverage.md) offers (b) as the route that avoids a contract
  change and ranks dynamics as gap 2, after legato. Dynamic shapes (swells, niente) are part of A16.
- **Discussed in.** [coverage.md](coverage.md) OQ5, gap rank 2; [cards/strings.md](cards/strings.md) OQ13;
  [cards/clarinets.md](cards/clarinets.md) OQ14; [articulation-vocabulary.md](articulation-vocabulary.md) 3.3.

### A5. The sampler's `instrument` parameter: absolute paths or preset ids

**Decided 2026-10-30 by the owner:** yes: preset ids, with absolute paths of older projects mapped on load; ADR 0010 decision 4 is amended.

- **Question.** The parameter's choices are file paths, the configured library root joined with each
  patch (absolute when the setting is), and the project stores that path (ADR 0010 decision 4). A project
  therefore does not play on a machine whose library sits elsewhere.
- **Options.** Keep paths; or store the catalog preset id (`<library id>/<patch path without .sfz>`) and
  resolve it to a path on load. That changes how the value is interpreted, not the schema, and ADR 0010
  decision 4 would need amending.
- **Recommendation.** Preset ids with a resolver; all mapping data is keyed by preset id. Caveat: an id
  survives a move only with a manifest `id` or an unchanged folder name (sfz-conformance C4).
- **Discussed in.** [sound-mapping.md](sound-mapping.md) OQ5 and Summary; [coverage.md](coverage.md) gap
  rank 4; [sfz-conformance.md](sfz-conformance.md) C1, C4.

### A6. Library mapping data: where it lives, what it holds, whether Sonara ships it

**Decided 2026-10-30 by the owner:** Sonara ships its own profiles for VSCO 2 CE and VCSL v1.2.2-RC (no samples).

- **Question.** No manifest field maps a preset to an orchestration id, solo or section, part, key
  switches, CC or GM program, so an agent cannot find "the horn section, legato".
- **Options for the place.** (a) A `sonara-library.json` v2 field (the manifest is not on the CTR-01 list,
  but it is a published v1 format). (b) A separate per-library file. (c) Data shipped in Sonara for known
  libraries.
- **Fields the research asks for.** Instrument id; ensemble and part; switch keys; how each articulation
  is reached; what dynamics drives; the legato kind; a play verdict; the licence. Also: a key offset
  (VSCO and Sonatina map the mallets one or two octaves below sounding pitch, and the parser does not read
  `octave_offset` or `note_offset`); whether a patch is keyed at written or sounding pitch (the VSCO
  piccolo is keyed written, the Sonatina piccolo sounding); whether it was recorded in situ (for pan); a
  velocity policy (organs, A9); the tuning (harpsichords recorded at A = 415); and a note remap
  (VSCO `GM-StylePerc.sfz` is not a GM map despite its name).
- **Recommendation.** Data keyed by preset id (A5) for VSCO 2 CE, VCSL and Sonatina, shipped as data
  ([coverage.md](coverage.md) gap rank 4). The place is the owner's choice. Whether shipping such data
  for Sonatina counts as distribution is A17.
- **Discussed in.** [sound-mapping.md](sound-mapping.md) OQ3 and 3.2; [coverage.md](coverage.md) gap
  rank 4; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ10; [cards/flutes.md](cards/flutes.md)
  OQ12; [ensembles.md](ensembles.md) OQ5; [cards/historical_keys_organs.md](cards/historical_keys_organs.md)
  OQ13; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ14.

### A7. Ensemble templates: GUI only, or the `create_project` enum

- **Question.** `create_project.template` is a fixed enum pinned by `golden/contracts/tools.json`, while
  the GUI's New Project dialog lists the backend's `TEMPLATES`. The `orchestral` template is golden-covered
  (the Ascension workflow, `golden/workflows/cinematic_orchestral`) and a measured render-load baseline
  (PERF-10).
- **Options.** (a) New entries in `TEMPLATES`: the GUI shows them, the agent cannot choose them. (b)
  Ensembles as data, applied after `create_project` with `template: "empty"` through existing tools, by
  the planner (a new plan kind) or the AI Composer. (c) A wider enum, or new `apply_ensemble` and
  `list_ensembles` tools, by a versioned CTR-01 decision.
- **Recommendation.** Keep `orchestral` byte-identical, use (b) now, and (c) only by a versioned
  decision. Reason: GLD-01 forbids re-recording Ascension, and (b) needs no contract change.
- **Discussed in.** [ensembles.md](ensembles.md) OQ10, 4.3 and 5.

### A8. Fix the SFZ parser and sampler divergences, or work around them

- **Question.** Code reading, confirmed by a second reading, found eleven behaviours that differ from the
  SFZ spec (Q1-Q11) and seven more (N1-N7); four other claims are only partly right (sfz-conformance
  C4, C11, C22, C24).
  How audible the losses are is still open (B1).
- **Options.** Fix them in `crates/sonara-media/src/sfz.rs` and the sampler, each with a fixture test that
  reproduces the real library's header layout (TST-06), keeping the sampler allocation-free and panic-free
  (RT-02, RT-05). Or document the gaps and steer users to single-articulation files.
- **Recommendation.** Fix, in order of reach: Q1 and Q11, the key switches (about 75 files: the Sonatina
  `KS` files, three VSCO files and Salamander), with Q11 following ARIA and sfizz because the spec does
  not settle it; Q2, Q3 and Q7, without which no grand piano plays as is (Sonatina Grand, Splendid,
  Salamander); Q4 (the MTG saxophones, four ids); N1 (release samples lost under the sustain pedal); N2
  (the ignored-opcode warning undercounts) and N3 (one missing sample fails a whole patch);
  sfz-conformance C8 (leave `includes/` fragments and files without `<region>` out of discovery and out
  of the `instrument` choices). For sfz-conformance C11, document "hold the switch note" until the chase
  re-sends switches. Section D of
  [sfz-conformance.md](sfz-conformance.md) gives each row's one-line fix and pinning test.
- **Card questions this answers.** Every card's "does the sampler honour this opcode" question is now
  settled as a code question; only the audible result is left (B1): acoustic_pianos OQ14 (Salamander);
  clarinets (Sonatina Performance and legato); double_reeds OQ11; flutes OQ14; high_brass OQ13;
  historical_keys_organs OQ8; horns OQ10; low_brass OQ14; strings OQ12; tuned_percussion OQ11;
  untuned_percussion OQ13; voices OQ13; free_libraries OQ11.
- **Discussed in.** [sound-mapping.md](sound-mapping.md) OQ1 and OQ6; [sfz-conformance.md](sfz-conformance.md)
  D and E; [coverage.md](coverage.md) gap ranks 3, 5 and 13.

### A9. Sampler features: legato, crossfades, velocity and pitch bend

- **Question.** Which playing features the sampler gains, and whether true legato waits for plugin
  hosting.
- **Options.** (a) `trigger=first` and `trigger=legato` plus an overlap rule that crossfades into the new
  note on sustain samples ("fake legato"). (b) Recorded legato transitions only through milestone M5
  (plugin hosting) or sfizz, which needs an RT-10 change (ADR 0010 decision 6: owner decision pending).
  Further candidates: velocity and CC crossfades (`xfin_*`, `xfout_*`, `gain_ccN`; needed by A4 and by the
  timpani, harp, celesta and Performance choir patches); `amp_veltrack` (organs and harpsichords follow
  velocity today), or else one fixed velocity for organs in the mapping data; `off_mode=normal` and
  filters (Q9); a pitch-bend range other than the fixed ±2 semitones (a trombone slide glissando spans 6).
- **Recommendation.** [coverage.md](coverage.md) ranks legato first and dynamics crossfades second, so
  (a) and the velocity crossfades come first, allocation-free (RT-02 to RT-05). `amp_veltrack` is the
  smallest fix with the largest effect on harpsichord and organ. Whether (a) sounds good enough for a
  legato melody is a listening test (B2).
- **Discussed in.** [coverage.md](coverage.md) OQ7, gap ranks 1, 2 and 8; [sound-mapping.md](sound-mapping.md)
  OQ11; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ8;
  [cards/low_brass.md](cards/low_brass.md) OQ7.

### A10. A General MIDI fallback engine

- **Question.** ADR 0010 deferred `rustysynth`, so Sonara plays no SF2, and the one SFZ GM bank found is
  mostly stubs. About 36 ids have no free SFZ that plays (recounted in coverage.md), so the degradation chain falls from a family
  neighbour straight to Poly Synth or Drum Kit.
- **Options.** Add `rustysynth` (MIT, pure Rust) after a QG-13 check that includes its real-time
  behaviour (not checked yet), with a user-installed MIT SoundFont (MuseScore_General, FluidR3_GM); let
  users convert MuseScore_General to SFZ (untested, and its licence notices would have to travel with
  it); or no GM step.
- **Recommendation.** None. The gain is a uniform, low-quality voice for about 36 ids.
- **Discussed in.** [coverage.md](coverage.md) OQ9, gap rank 12; [sound-mapping.md](sound-mapping.md)
  OQ10, sections 4 and 5; [cards/free_libraries.md](cards/free_libraries.md).

### A11. GM numbers for ids without a GM1 program, and the Drum Kit map

- **Question.** The cards are not uniform. Some store a substitute in `gm_program`: the six solo voices
  54 Voice Oohs; cornet, flugelhorn and piccolo trumpet 57 Trumpet; the studio grand 2, a Sonara choice.
  Others store null and name a fallback in the prose: alto and bass flute 74, oboe d'amore 69,
  contrabassoon 71, upright, felt and prepared piano 1, clavichord 7 or 8, lute 25, mandolin 26.
  `gm_percussion_notes` for the cymbals and the snare drum is a substitution policy too (suspended cymbal
  49, snare drum 40 "Electric Snare").
- **Options.** (a) `gm_program` and `gm_percussion_notes` hold exact GM instruments only, with a separate
  fallback field. (b) They hold the best substitute, marked as such by the provenance. (c) Exact values in
  the model, fallbacks only in the table of [sound-mapping.md](sound-mapping.md) section 5.
- **Drum Kit.** `GM_MAP` lacks the GM1 triangles (80, 81) and 11 other GM1 notes, and the Drum Kit's
  sounds are golden-pinned (GLD-01). `shaker` in `DRUM_NAMES` means GM 69/70, while GS has a Shaker on 82
  and the card's `shaker` is an instrument class; renaming it changes the generator API.
- **Recommendation.** [Estimate: triage judgement] exact values only, with substitutes kept apart as
  sound-mapping.md section 5 already does. For percussion, an orchestral map beside `GM_MAP`, not a
  change to it (the untuned-percussion card's proposal).
- **Discussed in.** [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ11;
  [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ7; [cards/plucked.md](cards/plucked.md)
  OQ10; [cards/double_reeds.md](cards/double_reeds.md) OQ10; [cards/flutes.md](cards/flutes.md) OQ15;
  [cards/high_brass.md](cards/high_brass.md) OQ14; [cards/voices.md](cards/voices.md) OQ10;
  [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ10-OQ12 and FC.

### A12. Articulation vocabulary: the merges and `col_legno`

**Decided 2026-10-30 by the owner:** `col_legno_battuto` stays apart from `col_legno`; the five applied spelling merges stand.

- **Question.** Confirm the five merges already applied to the card json blocks: `multitongue`,
  `double_tonguing` and `triple_tonguing` into `multi_tongue`; `muted_string` into `palm_mute`;
  `pitch_bend` into `portamento`. Decide the sixth, `col_legno_battuto` into `col_legno`, which is not
  applied.
- **Options for `col_legno`.** Keep its doc comment ("struck") and merge `col_legno_battuto` into it,
  dropping a preferred spelling; or widen `col_legno` to "struck or drawn" and keep `col_legno_battuto` as
  a refinement, which changes the meaning of an id that manifests may already declare.
- **Recommendation.** Confirm the five merges. For `col_legno`, none: the vocabulary file leaves it to
  the owner. The cards' requests to choose one spelling for multiple tonguing (clarinets OQ5, flutes OQ11)
  are settled by the applied merge; the card prose keeps the old spellings, and the json is
  authoritative.
- **Discussed in.** [articulation-vocabulary.md](articulation-vocabulary.md) 1.2, 1.4, 1.6 and 3.6 Q1;
  [cards/clarinets.md](cards/clarinets.md) OQ5; [cards/flutes.md](cards/flutes.md) OQ11;
  [cards/acoustic_pianos.md](cards/acoustic_pianos.md) FC.

### A13. Articulation vocabulary: the 34 invented ids and the candidate merges

- **Question.** Approve or reject the ids the cards coined (status "invented" in
  [articulation-vocabulary.md](articulation-vocabulary.md)): `fingered_tremolo` (the two-note wind and
  string tremolo, which four card groups ask to split from `tremolo`), `multi_tongue`, `detache`,
  `martele`, `loure`, `sautille`, `ricochet`, `left_hand_pizzicato`, `slap_pizzicato`, `su_una_corda`,
  `behind_the_bridge`, `cluster`, `scoop`, `subtone`, `bells_up`, `snares_off`, `rute`, `bowed`, `scrape`,
  `thumb_roll`, `shake_stroke`, `rasgueado`, `strum`, `golpe`, `jet_whistle`, `whistle_tone`,
  `tongue_ram`, `wah_wah`, the four `mute_*` ids, `text` and `whisper`. Also decide the two candidate
  merges kept apart, `loure` into `portato` and `choke` into `damped`, and whether `snares_off` and
  `con_sordino` stay two ids for the snare drum.
- **Recommendation.** The vocabulary file defines each id and proposes refinement fallbacks (3.5), but
  recommends nothing per id. Note that the guitar mappings to `palm_mute`, `bartok_pizzicato` and `golpe`
  rest on video headings only, and that `key_click` (a preferred id) also stands for VSL's key slap.
- **Discussed in.** [articulation-vocabulary.md](articulation-vocabulary.md) 1.4, 3.5, 3.6 Q2 and Q3;
  [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ15; [cards/clarinets.md](cards/clarinets.md)
  OQ4; [cards/double_reeds.md](cards/double_reeds.md) OQ9; [cards/flutes.md](cards/flutes.md) OQ11;
  [cards/horns.md](cards/horns.md) OQ9; [cards/high_brass.md](cards/high_brass.md) OQ10;
  [cards/low_brass.md](cards/low_brass.md) OQ12; [cards/plucked.md](cards/plucked.md) OQ11;
  [cards/saxophones.md](cards/saxophones.md) OQ10; [cards/tuned_percussion.md](cards/tuned_percussion.md)
  OQ12; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ18; [cards/voices.md](cards/voices.md)
  OQ14.

### A14. Articulation vocabulary: the ordinary note, and ids used in two meanings

**Decided 2026-10-30 by the owner:** no id means ordinario, the instrument's ordinary sound; `sustain` keeps its meaning as the long patch.

- **Question.** Several cards use `sustain` (pianos, harpsichord, celesta, harp, guitar, lute, mandolin)
  or `staccato` (xylophone, marimba) for the instrument's ordinary note, so that library default patches
  match; tuned percussion writes `let_ring` instead. `damped`, `tremolo` and `staccato` also carry a
  different meaning in some cards (the Clavinet mute slider, the triangle, the piano and harp
  alternations, the glockenspiel's damped note).
- **Options.** Keep `sustain` and `staccato` as the ordinary note; add an `ordinario` id; or let "no
  articulation" mean the instrument's ordinary sound, as Logic separates "no ID" from ID 1.
- **Recommendation.** None for the default. For the usage, the vocabulary file proposes card fixes:
  two-note alternations become `fingered_tremolo`, the Clavinet's dull tone `palm_mute`, the triangle's
  mute stroke `dead_stroke` or `con_sordino` and its choke `choke`. The tuned-percussion card asks
  whether the glockenspiel's short note should be `damped` rather than `staccato`, and notes that the
  data.rs `MALLETS` list's `marcato` has no source for any mallet instrument.
- **Discussed in.** [articulation-vocabulary.md](articulation-vocabulary.md) 1.5, 3.3 and 3.6 Q4;
  [cards/plucked.md](cards/plucked.md) OQ12; [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ15;
  [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ13; [cards/electric_keys.md](cards/electric_keys.md) FC.

### A15. Articulation vocabulary: mallet rolls as `tremolo` or `roll`

- **Question.** Every library calls a mallet tremolo a "roll", while the vocabulary keeps `roll` for drums
  and timpani and `tremolo` (doc comment: "bowed or mallet") for mallets.
- **Options.** Keep the split; accept `roll` as an alias for mallet instruments; or merge the two existing
  ids (a stable-id change).
- **Recommendation.** None. The cards use `roll` for timpani and `tremolo` for mallets until decided.
- **Discussed in.** [articulation-vocabulary.md](articulation-vocabulary.md) 1.4 and 3.6 Q5;
  [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ12.

### A16. Articulation vocabulary: flat list or separate axes (mutes, beaters, vowels, shapes)

**Decided 2026-10-30 by the owner:** shape B: one primary id per note, with mute and vibrato as passage states.

- **Question.** One articulation per note (Logic's flat list), or a primary articulation plus states such
  as the mute? The brass cards ask for a mute field (straight, cup, harmon, plunger, bucket) instead of
  `mute_*` ids, the low-brass card treats the mute type as a parameter of `con_sordino`, the
  untuned-percussion card asks for a beater field (stick, soft and hard mallet, brush, rute, hand, bow),
  and the voices card says `vowel` needs a parameter (ah, oh, oo, ee, mm). Dynamic shapes (niente, swells,
  fp) and contact point (sul ponticello, sul tasto) are further candidate axes.
- **Options.** Shape A, the flat list extended with all new ids and a refinement fallback table; shape B,
  one primary id per note plus a few states (mute, vibrato, contact point) that change at a time point;
  shape C, every axis as its own note field. In any shape, per-track states can be automatable enum
  parameters on the sampler without a schema change (A3); per-note data needs the CTR-01 decision.
- **Recommendation.** None; the vocabulary file sets out the options and costs and asks for a choice
  before any per-note field is proposed under CTR-01. Instrument controls (harp pedals, organ stops,
  Leslie speed, vibraphone motor) are not articulation ids in any shape (A22).
- **Discussed in.** [articulation-vocabulary.md](articulation-vocabulary.md) 3.3, 3.4 and 3.6 Q6;
  [cards/high_brass.md](cards/high_brass.md) OQ9; [cards/low_brass.md](cards/low_brass.md) OQ12;
  [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ17; [cards/voices.md](cards/voices.md)
  OQ14; [cards/clarinets.md](cards/clarinets.md) OQ14.

### A17. Licences: recommending and supporting Sonatina-, Iowa- or Philharmonia-derived libraries

**Decided 2026-10-30 by the owner:** no shipped profile for Sonatina or for CC BY libraries; users can write their own manifest.

- **Question.** Does recommending a user-installed library make it a dependency under LIC-01? Could Sonara
  ship a downloader or mapping data for Sonatina without that counting as distribution under ADR 0010
  decision 3? Sonatina is under CC Sampling Plus 1.0, a retired licence that limits whole-library
  redistribution to non-commercial use; some of its samples are of unknown origin, and some derive from
  Iowa MIS and Philharmonia recordings. Iowa MIS says nothing about redistribution, and the
  Philharmonia's terms exclude building sampler instruments from its samples. VPO3 adds share-alike
  components.
- **Options.** Treat a recommendation and mapping data as documentation (no dependency); restrict support
  to CC0 libraries (VSCO 2 CE, VCSL); or seek a legal opinion first.
- **Recommendation.** The research assumes that recommending a user install is not a dependency: the
  starter kit is links, install steps and mapping data, with no samples. Sonatina is acceptable as a user
  install and never bundled; VPO3 and Philharmonia-derived instruments are never bundled. Whether
  redistributing the Iowa and Philharmonia derivatives is lawful is beyond Sonara's control and unresolved
  (no legal opinion was read).
- **Discussed in.** [coverage.md](coverage.md) OQ6 and section 4; [cards/free_libraries.md](cards/free_libraries.md)
  OQ8, OQ14 and FC; [sound-mapping.md](sound-mapping.md) OQ7.

### A18. What `playable` and `comfortable` mean

- **Question.** The cards follow different conventions, so one written definition is needed for each
  field.
  - `comfortable`: the horn's 41-72 covers any horn part, Forsyth's 48-65 is safe for all four players;
    the saxophones use the traditional keyed top (written F6), Widor's advice suggests about written C6;
    the pianos set comfortable = playable while the alternative is null; the Rhodes uses its 73-key span
    as comfortable.
  - `playable`: the widest professional range (B♭ trumpet 91) or an orchestral cap (87); the stopped
    range (violin 105) or harmonics (108); the envelope of the largest current model (celesta 113) or the
    standard instrument (108); retuned strings in (harp C♭1 = 23) or out (harp G♯7 = 104); pedal notes in
    (bass trombone, tuba, cimbasso, euphonium) or out (tenor trombone).
  - Profiles: one range for amateur and professional players (Wikipedia's typical solo voice ranges
    against Rimsky-Korsakov's outer limits), and choirs with or without Russian-style octavist basses.
- **Recommendation.** The cards' current values follow their own stated rules; the research asks the
  owner to confirm them: horn comfortable 41-72, B♭ trumpet playable 91 with comfortable 82, keyboards
  comfortable = playable, Rhodes comfortable 28-100, harp playable 23-103, voices from Rimsky-Korsakov's
  professional limits. Per-family profiles are a later option.
- **Discussed in.** [cards/horns.md](cards/horns.md) OQ6; [cards/high_brass.md](cards/high_brass.md) OQ1;
  [cards/saxophones.md](cards/saxophones.md) OQ2; [cards/acoustic_pianos.md](cards/acoustic_pianos.md)
  OQ1; [cards/electric_keys.md](cards/electric_keys.md) FC; [cards/strings.md](cards/strings.md) OQ0;
  [cards/plucked.md](cards/plucked.md) OQ2; [cards/low_brass.md](cards/low_brass.md) OQ3;
  [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ5 and OQ17;
  [cards/voices.md](cards/voices.md) OQ5 and OQ9.

### A19. Range model extensions: extensions, gaps, variants and a third tier

- **Question.** One contiguous playable range cannot express: low extensions on some instruments
  (bassoon A1 = 33, contrabassoon A0 = 21, English horn E♭3 = 51, bass clarinet low C); gaps (the tenor
  trombone's pedals E1-B♭1 below a gap, gaps on three-valve bass Wagner tubas); per-instance choices (the
  double bass's lowest string, the tenor trombone's F attachment, the 97-key Bösendorfer Imperial, the
  piccolo's low C); a third voice tier, the tessitura that Rimsky-Korsakov's Table F prints; and the
  sounding extent of a registered instrument (the stored key compass at 8′ against 17-113 for a
  harpsichord with 16′ and 2′ choirs, 12-120 for a large organ, 24-114 for the Hammond's tonewheels,
  26-110 for a large accordion).
- **Options.** Optional fields (`extension_low`, a gap list, `tessitura`); variant ids (A21); a
  project-level setting (lowest string); a registration model with stop footages.
- **Recommendation.** The cards lean to optional fields over new ids for extensions, a per-project lowest
  string later, and a separate registration model before any sounding-pitch check on organs and
  harpsichords. Until then the stored values are those in section C.
- **Discussed in.** [cards/double_reeds.md](cards/double_reeds.md) OQ8; [cards/clarinets.md](cards/clarinets.md)
  OQ2; [cards/low_brass.md](cards/low_brass.md) OQ2 and OQ3; [cards/horns.md](cards/horns.md) OQ3;
  [cards/strings.md](cards/strings.md) OQ8; [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ3;
  [cards/flutes.md](cards/flutes.md) OQ4; [cards/voices.md](cards/voices.md) OQ3;
  [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ1.

### A20. Notation: one transposition per instrument, and clefs

- **Question.** Each instrument has one transposition and a list of clefs, but parts are written in
  several ways: the bass clarinet in French treble notation (−14) or German bass-clef notation (−2); the
  tenor Wagner tuba in B♭ (−2, Strauss and VSL) or in F like the bass (−7, modern parts), and in older
  notations (E♭ −9, B♭ basso −14, bass clef an octave low); band parts for euphonium (−14 or −2), E♭ tuba
  (−21 or −9), B♭ tuba (−26 or −2) and treble-clef tenor trombone; the xylophone at pitch (Forsyth) or
  +12; the glockenspiel +24 or +12 (Wagner); the lute at −12 or 0; the tenor voice in treble-8vb clef
  (−12) or on a shared bass-clef staff (0); Strauss's F and C saxophones.
- **Options.** One default per instrument plus a per-part transposition and clef override on import and
  export (a "notation context"); or separate ids per notation.
- **Recommendation.** Defaults plus a per-part override; the default for `wagner_tuba_tenor` is −2 as
  stored (Strauss, VSL), but the owner may prefer −7 to match modern parts. The clef slug `treble_8vb`,
  which the voices card defines, needs a shared clef vocabulary.
- **Discussed in.** [cards/clarinets.md](cards/clarinets.md) OQ3; [cards/horns.md](cards/horns.md) OQ1;
  [cards/low_brass.md](cards/low_brass.md) OQ11; [cards/tuned_percussion.md](cards/tuned_percussion.md)
  OQ5 and OQ6; [cards/plucked.md](cards/plucked.md) OQ9; [cards/voices.md](cards/voices.md) FC;
  [cards/saxophones.md](cards/saxophones.md) OQ11.

### A21. Instrument ids: what to add or split, and families

- **Question.** Which ids to add, split or merge, and how to classify borderline instruments.
  - Missing ids found while building templates: natural horn and natural trumpet, alto trombone, E♭
    trumpet, bass trumpet, contrabass trombone, C clarinet, cowbells, hammer, drum kit, jazz or electric
    guitar, electric bass. The Rute is modelled as the proposed articulation `rute` of `bass_drum`.
  - Variants: tuba sizes (`tuba_f`, `tuba_cc`, against one composite `tuba`); a tenor trombone with F
    attachment; `piano_felt` as its own id or a "muffler on" state of `piano_upright` (libraries ship it as
    a separate instrument); a 97-key Imperial; a tuned gong (`gong_tuned`) beside `tam_tam`; the alto
    flute in F (Rimsky-Korsakov glosses his alto flute as F and G); the F and C saxophones. A double horn id is not recommended unless libraries ship F-side
    and B♭-side patches that users ask for by name.
  - Families: the accordion (and bandoneon, concertina, harmonica) as keyboards or other; the carillon
    as percussion or keyboards; the temple blocks as unpitched with five ordered keys or as pitched; a
    flag for keys with an unreliable pitch (prepared piano).
- **Recommendation.** The cards' proposals: accordion keyboards, carillon percussion, temple blocks
  unpitched with five ordered keys, prepared piano pitched; separate tuba sizes "would be more exact";
  the other missing ids are the owner's choice.
- **Discussed in.** [ensembles.md](ensembles.md) OQ6; [cards/low_brass.md](cards/low_brass.md) OQ2 and
  OQ4; [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ3, OQ4 and OQ18; [cards/horns.md](cards/horns.md)
  OQ11; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ3 and OQ19;
  [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ11;
  [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ15; [cards/flutes.md](cards/flutes.md)
  (`alto_flute`, naming trap); [cards/saxophones.md](cards/saxophones.md) OQ11.

### A22. Instrument controls and multi-zone keyboards

- **Question.** Several things the cards met are controls, not articulations, and have no place yet: the
  Rhodes and Wurlitzer tremolo ("vibrato" on the instruments), phaser, chorus, drive and wah; the Clavinet's
  tone tabs, pickup switches and mute slider; harp pedal states; organ registration, Hammond drawbars and
  Leslie speed; the vibraphone motor; accordion bellows. Pedals: the card format names sustain, una corda
  and sostenuto, but the Clavinet D6 has only a volume swell (`foot_swell`, CC11); the upright's soft pedal
  is a half-blow, not an una corda, though CC67 is "Soft Pedal"; the practice pedal and Fazioli's fourth
  pedal have no standard CC (VSL uses CC75). Zones: the Hammond B-3 uses three MIDI channels (two
  manuals and pedals), and an accordion's Stradella bass cannot be told from its chord buttons on one
  track.
- **Options.** Track or device controls (a control vocabulary beside the articulation ids), an open pedal
  list, and multi-channel or two-track input for organs and accordions.
- **Recommendation.** Controls are track or device controls, never articulation ids; keep the pedal list
  open; an accordion needs two tracks or a chord-button vocabulary.
- **Discussed in.** [cards/electric_keys.md](cards/electric_keys.md) (open questions table and FC);
  [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ9 and OQ16;
  [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ7; [articulation-vocabulary.md](articulation-vocabulary.md)
  3.3 (last rows).

### A23. Adopting the card values in `data.rs`

- **Question.** The cards propose changes to the 16 instruments in
  `crates/sonara-midi/src/orchestration/data.rs`: ranges (violin playable 108 → 105 and comfortable
  100 → 93, cello playable 93 → 88, double bass playable low 24 → 23, flute playable top 96 → 98, bassoon
  comfortable top 76 → 72, xylophone and glockenspiel ranges), comfortable ranges where none exist,
  Cited provenance in place of Estimates, and per-instrument articulation lists (`harmonics` for the
  flute, `con_sordino` for the double reeds, no `trill` on the contrabassoon). The shared `WIND` and
  `BRASS` constants cannot express per-instrument lists.
- **Options.** Adopt the card values, with per-instrument articulation lists; or keep data.rs and use the
  cards as reference only.
- **Recommendation.** Adopt them; the values that depend on A18 (flute top, violin top, horn comfortable)
  follow that decision. Each card's "Verification of data.rs" table lists the changes.
- **Discussed in.** the "Verification of data.rs" tables in [cards/flutes.md](cards/flutes.md),
  [cards/double_reeds.md](cards/double_reeds.md), [cards/clarinets.md](cards/clarinets.md),
  [cards/horns.md](cards/horns.md), [cards/high_brass.md](cards/high_brass.md),
  [cards/low_brass.md](cards/low_brass.md), [cards/strings.md](cards/strings.md),
  [cards/tuned_percussion.md](cards/tuned_percussion.md) and
  [cards/untuned_percussion.md](cards/untuned_percussion.md); [cards/flutes.md](cards/flutes.md) OQ11.

### A24. Small fixes outside the contracts

- **Question.** Accept the small corrections the research found, none of which touches a contract.
  - `crates/sonara-app/src/instruments/family.rs` lacks the keywords rhodes, wurlitzer, clavinet, clav
    (or epiano), chimes, tubular (VSCO's `TubularBells.sfz` reads as one word), crotales, carillon, vibes,
    glock and celeste; Sonatina's `Celeste.sfz` is inferred as percussion from its folder.
  - The Drum Kit description says toms on 41-50, but only 41, 43, 45, 47, 48 and 50 are toms
    (sfz-conformance C22).
  - The `load_sfz` doc says a region without a sample is malformed, but the code skips it (N3; QG-05).
  - `docs/orchestral-audio-engine-research.md` section 5 calls the FreePats harp CC BY-NC (only the page
    photo is; the sounds are CC0) and the VSCO 2 CE "bass" a section (it is a solo contrabass). **Done
    2026-10-30:** that document now carries both corrections.
- **Recommendation.** Make the code ones with a test; the documentation one is done.
- **Discussed in.** [cards/electric_keys.md](cards/electric_keys.md) (open questions table);
  [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ16; [sfz-conformance.md](sfz-conformance.md) C22
  and N3; [cards/free_libraries.md](cards/free_libraries.md) OQ2.

### A25. How coverage is counted

- **Question.** "About 48 of 85 ids" counts partly playing patches and substitutes, such as the mixed
  chorus for each choir part. Should a coverage metric count only patches that play as is or partly, with
  the right solo or section?
- **Recommendation.** None; the research reports both the loose count and its parts.
- **Discussed in.** [coverage.md](coverage.md) OQ10 and Summary.

## B. Human checks

| Id | What to check | Where |
|---|---|---|
| B1 | **Renders that confirm the code-reading verdicts.** Render and listen to: VSCO `ViolinEns-KS` with no switch note (Q1); Sonatina `Flutes KS` with each switch (Q11); Sonatina Timpani at velocity 60 and 100 (Q5, Q6); the Salamander remap (Q2, Q7); a VCSL grand with CC64 held across note-offs (N1); a VSCO patch with and without `ampeg_dynamic` (sfz-conformance C23); Sonatina `Horns Sustain`, `1st Violins Legato` and VCSL `Timpani 1 - Roll` (the strings, horns and timpani of the product sentence); Sonatina `Harpsichord KS` (C118); SSO harpsichord and organ at several velocities (Q8); SSO snare, triangle and bass drum rolls (`release_key`, velocity crossfades); the Sonatina legato double trigger; jRhodes3d; the free libraries' mallet and tubular-bell octave mappings. | [sound-mapping.md](sound-mapping.md) OQ2; [coverage.md](coverage.md) OQ1; [sfz-conformance.md](sfz-conformance.md) A, D; [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ14; [cards/electric_keys.md](cards/electric_keys.md); [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ8; [cards/high_brass.md](cards/high_brass.md) FC; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ11, FC; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ13 |
| B2 | **Listening tests for arranging defaults.** Whether an overlap crossfade on sustain samples is good enough for a legato melody (A9); the default pan table against an in-situ library such as Spitfire; the balance weights against library loudness at matched dynamics, and whether a string section's weight grows linearly with its players; a roughness model for the low-interval limits, checked by ear; Apple Studio Horns' "Expressive" and "Passionate" articulations before any mapping; Wikipedia's remark that some listeners hear tubular bells an octave below the written note. | [coverage.md](coverage.md) OQ7; [ensembles.md](ensembles.md) OQ4; [doubling-and-balance.md](doubling-and-balance.md) OQ2, OQ4, OQ8; [cards/apple.md](cards/apple.md) OQ8; [cards/tuned_percussion.md](cards/tuned_percussion.md) FC |
| B3 | **Identify and measure sample files.** The VSCO and VCSL sample names `HitFM` (triangle), `bassdrum_rub`, `gong_smack` and "SN"/"NS", and whether their "Gong" folders hold tam-tams; the pitch of the SSO celesta samples (keys 60-108 are assumed sounding); the key of the VSCO clarinet (B♭ or A) and of the VSCO and Sonatina trumpets (B♭ or C), from the pitch of the lowest sample. | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ15, FC; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) FC; [cards/clarinets.md](cards/clarinets.md) OQ15; [cards/high_brass.md](cards/high_brass.md) OQ13; [sound-mapping.md](sound-mapping.md) OQ9 |
| B4 | **Page scans of public-domain handbooks** (music examples lost in OCR). Rimsky-Korsakov: Table F at a higher resolution (1922 English or the Russian edition) for the soloists' upper comfortable limits (±1 diatonic step) and whether its unprinted accidentals mean B or B♭; the violin limit, which the images read as A6 = 93 and the transcription prints as "A7" (check the 1922 print); Table E (xylophone and keyboard glockenspiel); Table A and the Vol. II examples, to confirm the octave numbering that the doubling rules convert to MIDI; the meaning of the harp's "first to the fourth octave". Forsyth: p.152 (Wagner tubas) in a second scan, for example IMSLP; p.474, the harp's most sonorous part, and the 17-string harmonics window; the clef table for gong and castanets; the high-brass staff examples (shake notes, register limits); the low-brass and euphonium compasses; the saxophone compasses (with Widor's); the double-reed compasses, with Prout's contrabassoon "D1 and F3" (written or sounding?); the clarinet compasses, register limits, weak notes and impossible trills, with Widor's soft-dynamics top limit; Widor's register limits for the flute. Widor: the bassoon's low note that is hard to attack softly (OCR accidental unreadable: B♭1 or B1). Berlioz: whose "right" and "left" his seating means (the French original). | [cards/voices.md](cards/voices.md) OQ1, OQ2; [cards/strings.md](cards/strings.md) OQ3, FC; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ18; [doubling-and-balance.md](doubling-and-balance.md) OQ1; [cards/plucked.md](cards/plucked.md) OQ3, OQ6, FC; [cards/horns.md](cards/horns.md) OQ4, FC; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ21, FC; [cards/high_brass.md](cards/high_brass.md) OQ16; [cards/low_brass.md](cards/low_brass.md) OQ10, FC; [cards/saxophones.md](cards/saxophones.md) OQ4; [cards/double_reeds.md](cards/double_reeds.md) OQ5, OQ13, FC; [cards/clarinets.md](cards/clarinets.md) OQ9, OQ10; [cards/flutes.md](cards/flutes.md) FC; [ensembles.md](ensembles.md) OQ3 |
| B5 | **Printed scores.** Wagner tuba part numbering: tenors on parts 1 and 3 (a 2024 study) or 1 and 2 (Forsyth's example), in the printed _Ring_ and Bruckner's Seventh; the piccolo-trumpet tops of Bach's Brandenburg Concerto No. 2 and B minor Mass; whether celesta parts and Schiedmayer's compass names are written or sounding (a dated score, or a celesta player). | [cards/horns.md](cards/horns.md) OQ5, FC; [ensembles.md](ensembles.md) OQ11; [cards/high_brass.md](cards/high_brass.md) OQ6; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ2, FC |
| B6 | **Ask a player.** A harpist: the time a pedal change needs, in beats. A timpanist: a retuning rule for larger leaps than VSL's one step per quarter note (the cards' 1 beat per 2 semitones plus 1 bar is an Estimate). A percussionist: whether a tubular-bells part may assume F3-B3. A brass player: endurance and the time a mute change needs (the 2-bar rule is an Estimate). | [cards/plucked.md](cards/plucked.md) OQ7; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ4, OQ9; [cards/high_brass.md](cards/high_brass.md) OQ11; [doubling-and-balance.md](doubling-and-balance.md) OQ7 |
| B7 | **Inside Logic Pro and GarageBand.** The Bass Player's "B0" lowest note (naming, or a bass mapped an octave above sounding); the Library category and template names; Studio Strings players per section, and whether its second key-switch column means solo instruments; the release that introduced the Simple Pad style; whether a "Steinway Grand Piano" patch exists; the CC behind Studio Horns' "Dynamics via CC". | [cards/apple.md](cards/apple.md) OQ2, OQ5, OQ6, OQ7, OQ9; [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ12; [cards/saxophones.md](cards/saxophones.md) OQ8 |
| B8 | **Sources that could not be opened** (need access or a copy). VSL Academy pages that returned HTTP 502 or 404 (harp range, saxophones, alto and bass flute, clarinets) or only a summarised fetch (low brass, tuned percussion); Yamaha pages behind HTTP 403 (clarinets, English horn, flutes); the MIDI Association's GM1 table (a members-only PDF): every GM number now rests on Wikipedia, for some ids with the Finale manual or PG Music as a second table; a Roland document with explicit GS ORCHESTRA note numbers (the PDF read was encrypted); the Adams timpani range chart (unreadable); the EastWest Hollywood Choirs manual; the Spitfire Studio Woodwinds manual (text not extractable); the Dorico manual on score order; the copyrighted handbooks (Adler, Blatter, Piston, Del Mar) that would settle the modern tops of the English horn, oboe d'amore and bass clarinet and the saxophone altissimo. | [cards/plucked.md](cards/plucked.md) FC; [cards/saxophones.md](cards/saxophones.md) OQ1, OQ12; [cards/flutes.md](cards/flutes.md) FC; [cards/clarinets.md](cards/clarinets.md) OQ1, OQ12, FC; [cards/low_brass.md](cards/low_brass.md) FC; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ2, FC; [cards/double_reeds.md](cards/double_reeds.md) OQ1, OQ12; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) FC; [cards/voices.md](cards/voices.md) OQ10, OQ11; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ1; [ensembles.md](ensembles.md) OQ2 |

## C. Data disagreements

Each row names the values the sources give, the value the card adopted (as stored in
[model.md](model.md)) and the reason. "Playable" and "comfortable" are the card's `sounding_playable` and
`sounding_comfortable`.

### Woodwinds

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C1 | Flute top | C7 = 96 (Berlioz, Widor, Forsyth, Wikipedia's infobox; data.rs) vs D7 = 98 (VSL standard range, Wikipedia's text); MIS recorded to D♭7 | Playable 98, comfortable top 94: the modern sources; 96 is the conservative alternative (A18) | [cards/flutes.md](cards/flutes.md) OQ1, FC |
| C2 | Flute bottom | B3 = 59 with a B foot (VSL, Wikipedia) vs older handbooks that treat the lowest notes as rare | Playable 59, comfortable 60 | [cards/flutes.md](cards/flutes.md) OQ2 |
| C3 | Piccolo top C8 | never (Berlioz), fff only (Forsyth), ff (Widor, VSL) | Playable 108, comfortable top B♭7 = 106 (Forsyth) | [cards/flutes.md](cards/flutes.md) OQ3 |
| C4 | Piccolo bottom | D5 = 74 vs C5 = 72 on piccolos with a low C key (Verdi, Mahler; Sonatina's samples start there) | 74; C5 noted as an instrument option | [cards/flutes.md](cards/flutes.md) OQ4 |
| C5 | Piccolo comfortable bottom | 80 (VSL middle register) vs 86 (Berlioz's lower octave) vs 74 (range only) | 80; the low register is a balance problem, perhaps better a dynamics warning | [cards/flutes.md](cards/flutes.md) OQ5 |
| C6 | Alto flute top | altissimo to D♭7 = 97 (Wikipedia) vs the stated range to G6 = 91 | Playable 91; comfortable top 81 is an Estimate, because Forsyth's limit is an image | [cards/flutes.md](cards/flutes.md) OQ6, FC |
| C7 | Alto flute low register | strong (Forsyth, Wikipedia) vs easily covered (Forsyth's own example uses only muted strings; MIS pp scales stop early) | Its strongest, most characteristic register, with light accompaniment (Estimate) | [cards/flutes.md](cards/flutes.md) OQ7 |
| C8 | Bass flute bottom | C3 = 48 with a C foot (many models) vs B2 = 47 with a B foot | Playable 47 (the flute's B-foot convention), comfortable 48 | [cards/flutes.md](cards/flutes.md) OQ8 |
| C9 | Bass flute top | C6 = 84 (Wikipedia) vs D♭6 = 85 (MIS, ff) | 84; D♭6 recorded as an ff extreme | [cards/flutes.md](cards/flutes.md) OQ18 |
| C10 | Flute harmonics | close to normal soft notes (VSL) vs thin and flat (Forsyth) vs only the first 14 degrees (Widor) | Add `harmonics` to the flute, following VSL's modern practice | [cards/flutes.md](cards/flutes.md) OQ9 |
| C11 | Double tonguing | flute only (Rimsky-Korsakov) vs also trumpets and horns (Forsyth) | Not flute-only; Rimsky-Korsakov's remark is outdated | [cards/flutes.md](cards/flutes.md) OQ10 |
| C12 | English horn top | C6 = 84 (Wikipedia) vs A5 = 81 with B5 = 83 extreme (VSL), B5 (Prout), A5 (Forsyth) | Playable 84, comfortable 81; the 84 is a single modern source (B8) | [cards/double_reeds.md](cards/double_reeds.md) OQ1, FC |
| C13 | Oboe d'amore top | E6 = 88 (German Wikipedia) vs C♯6 = 85 (Forsyth) | Playable 88, comfortable 85; no maker's page was opened | [cards/double_reeds.md](cards/double_reeds.md) OQ2, FC |
| C14 | Bassoon top | A♭5 = 80 or about G5 (Wikipedia), F5 (VSL), B♭4 = 70 as the orchestral limit (Prout, Forsyth, Widor) | Playable 80, comfortable 72 (C5), because the modern source and Forsyth's footnote say the limit is often exceeded; the owner may prefer 70 | [cards/double_reeds.md](cards/double_reeds.md) OQ3 |
| C15 | Bassoon soft low notes | the low B♭ is velvety in piano (Forsyth) vs the lowest notes are hard to attack in piano (Widor) | Soft sustained low notes kept; an Estimate rule against exposed pp attacks on 34-38 | [cards/double_reeds.md](cards/double_reeds.md) OQ13, FC |
| C16 | Contrabassoon bottom | B0 in VSL's range line vs B♭0 in VSL's own text, Wikipedia and Heckel | B♭0 = 22; VSL's line has probably lost its flat | [cards/double_reeds.md](cards/double_reeds.md) OQ4 |
| C17 | Contrabassoon staccato | surprisingly agile (VSL) vs to be avoided (Widor, 1904, French instrument) | Staccato kept; `trill` removed (VSL: trills develop too slowly) | [cards/double_reeds.md](cards/double_reeds.md) OQ6 |
| C18 | Oboe endurance | 3-4 bars of rest per bar played (Forsyth) vs great sustaining power (Widor) | Both true (little air, much tension); the phrase-length rule is an Estimate | [cards/double_reeds.md](cards/double_reeds.md) OQ7 |
| C19 | B♭ clarinet comfortable top | written A6 = 91 (Widor) vs written B♭6 = 92 (Forsyth) | 91, the stricter; playable stays 94 | [cards/clarinets.md](cards/clarinets.md) OQ6 |
| C20 | E♭ clarinet top | written A6 = 96 (infobox) vs written G6 = 94 (orchestration books, Forsyth) vs C♯7 = 97 (Orchestral Tools samples, inferred octave) | Playable 96, comfortable 94 | [cards/clarinets.md](cards/clarinets.md) OQ8 |
| C21 | E♭ clarinet comfortable bottom | Forsyth's rarely used "bottom fifth" read as written E3-B3 (63) or E3-B♭3 (62) | 63 | [cards/clarinets.md](cards/clarinets.md) FC |
| C22 | Bass clarinet top | D6 = 86 (Wikipedia's infobox), B♭5 = 82, A5 = 81 (Czech Wikipedia), about F5 = 77 (Forsyth), D5 = 74 as the common solo top (Wikipedia's text) | Playable 86, comfortable 74; a modern handbook should settle the top (B8) | [cards/clarinets.md](cards/clarinets.md) OQ1, FC |
| C23 | Bass clarinet bottom | by model: written E3 = 38, E♭3 = 37, D = 36, C = 34 | Playable 34 (professional low-C models), comfortable from 37; an extension field is A19 | [cards/clarinets.md](cards/clarinets.md) OQ2 |
| C24 | Clarinet in A vs B♭ | warmer (Widor, Wikipedia) vs indistinguishable (Forsyth) | Same samples; the A's only extra note is C♯3 = 49, which B♭ sample sets lack | [cards/clarinets.md](cards/clarinets.md) OQ7 |
| C25 | Clarinet throat register | starts at written F4 (Czech Wikipedia) vs G4 or A4 (Wikipedia) | Not stored; the handbooks' weak-note lists are images (B4) | [cards/clarinets.md](cards/clarinets.md) OQ9 |
| C26 | Clarinet mutes | unnecessary (Rimsky-Korsakov) vs a leather bag in _Lélio_ (Berlioz) | `con_sordino` left out | [cards/clarinets.md](cards/clarinets.md) OQ13 |
| C27 | Saxophone playable top | keyed top written F6 or F♯6; no ceiling for altissimo (Wikipedia); four octaves (Rascher's method); written C7 (MuseScore's professional range) | Written C7, the only numeric ceiling read | [cards/saxophones.md](cards/saxophones.md) OQ1, FC |
| C28 | Saxophone comfortable top | written F6 (traditional keyed top) vs F♯6 (big-band writing) vs about C6 (Widor; the top palm keys unreliable in a section) | Written F6; F♯6 reports OutsideComfortable | [cards/saxophones.md](cards/saxophones.md) OQ2, FC |
| C29 | Baritone saxophone low A | on most models (Wikipedia) vs an extra key (Rogers); MuseScore uses both | Playable 36, comfortable 37 | [cards/saxophones.md](cards/saxophones.md) OQ3, FC |
| C30 | Saxophone ranges not used | Widor's and Forsyth's compasses; MuseScore's amateur top (written G6, the first altissimo note); Italian Wikipedia's soprano, which contradicts its own text | Not used; the modern keyed range applies to all four sizes | [cards/saxophones.md](cards/saxophones.md) OQ4-OQ6 |

### Brass

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C31 | B♭ trumpet top | 82 (Wikipedia standard, Forsyth), 85 (MuseScore professional), 87 (VSL, for the C trumpet), 91 (Samplemodeling, a top professional player) | Playable 91 so a lead part's written G6 or A6 passes, comfortable 82; an orchestral cap of 87 is A18 | [cards/high_brass.md](cards/high_brass.md) OQ1, FC |
| C32 | C trumpet top | written F6 = 89 (VSL, "and higher") vs the B♭ card's written A6 | 89 kept; aligning both to written A6 is suggested | [cards/high_brass.md](cards/high_brass.md) OQ2 |
| C33 | Trumpet bottom and writing range | Yamaha's concert F♯ vs sounding E3 (Wikipedia, Forsyth); Forsyth's writing rule written C4-G5 | E3 = 52; Forsyth's rule recorded, not used as comfortable | [cards/high_brass.md](cards/high_brass.md) OQ3, OQ4 |
| C34 | Piccolo trumpet bottom | D4 = 62 (VSL, which also names three valves although D4 needs a fourth; German Wikipedia), 59 (MuseScore), 56 (Samplemodeling, below the four-valve limit) | Playable 59, comfortable 62 | [cards/high_brass.md](cards/high_brass.md) OQ5 |
| C35 | Piccolo trumpet comfortable top | VSL's best register C5-C6 (to 84) vs MuseScore's amateur top 86 | 86, so ordinary Baroque high parts do not warn; repertoire tops unchecked (B5) | [cards/high_brass.md](cards/high_brass.md) OQ6, FC |
| C36 | Trumpet lip trill | impossible on the trumpet (VSL) vs a jazz shake between partials (Wikipedia, Samplemodeling) | `trill` for valve trills, a separate `shake` | [cards/high_brass.md](cards/high_brass.md) OQ7 |
| C37 | Trumpet and horn balance | one middle-register trumpet equals four horns (VSL) vs two horns balance one trumpet in forte (Rimsky-Korsakov) | 2:1 for forte tutti, the handbook value | [cards/high_brass.md](cards/high_brass.md) OQ8 |
| C38 | Horns in soft playing | all winds fairly equal in piano (Rimsky-Korsakov) vs trumpets and trombones at pp matching horns at p (VSL) | The weight table follows Rimsky-Korsakov; the other reading would halve the horn's piano weight | [doubling-and-balance.md](doubling-and-balance.md) OQ3 |
| C39 | Flugelhorn comfortable top | 82 vs a high register described as hard to reach, with no lower top stated | 82 kept; may be optimistic | [cards/high_brass.md](cards/high_brass.md) FC |
| C40 | Horn comfortable range | 48-65, comfortable for all four players (Forsyth) vs 41-72, combining his limits for the low and high pairs | 41-72 (Estimate); the meaning is A18 | [cards/horns.md](cards/horns.md) OQ6, FC |
| C41 | Horn range on Wikipedia | "C2 to C6" with no written or sounding marker, and an infobox alt text that disagrees | Not used; Forsyth and VSL give B1-F5 = 35-77 | [cards/horns.md](cards/horns.md) OQ7 |
| C42 | Wagner tuba top and comfortable range | tenor B♭5 = 82, bass F5 = 77 (VSL); horn-like F5 for both (Wikipedia); uncertain above concert D (wagner-tuba.com); Wagner's designed compass B♭2-F5 tenor, B♭1-G4 bass (Forsyth) | Playable from VSL; comfortable tenor 40-74 and bass 34-67 (Estimates; the bass after Forsyth). The tenor's comfortable top rests on an octave its source does not state | [cards/horns.md](cards/horns.md) OQ2, FC |
| C43 | Wagner tuba bottom | E♭1 = 27 (VSL) vs E1 = 28 (wagner-tuba.com) | 27; the gaps on some instruments are not modelled (A19) | [cards/horns.md](cards/horns.md) OQ3 |
| C44 | Tenor trombone comfortable top | D5 = 74 (Wikipedia's conservative practical top; data.rs), C5 = 72 (Czech textbook range), B♭4 = 70 (Forsyth) | 74; C5 is the most defensible stricter value | [cards/low_brass.md](cards/low_brass.md) OQ1 |
| C45 | Bass trombone top | F5 = 77 (VSL) vs "at least B♭4", with Kodály's B4 notable (Wikipedia) | 77; may be too generous for checking a part | [cards/low_brass.md](cards/low_brass.md) OQ5, FC |
| C46 | Tuba range | one id for four sizes: contrabass bottom A0 (VSL) with the F tuba's top G4; F tuba 26-67; B♭ contrabass 21-59; solo extremes 22-72 | Composite 21-67; size ids are A21 | [cards/low_brass.md](cards/low_brass.md) OQ4, FC |
| C47 | Euphonium bottom | B0 = 23, Wikipedia's full working range with pedals, vs B1 = 35 without pedals | 23 (changed from 34 in the fact-check); pedals in or out is A18 | [cards/low_brass.md](cards/low_brass.md) OQ10, FC |
| C48 | Trombone legato | no true legato across slide positions (Forsyth) vs seamless legato (VSL) | Libraries record legato transitions; probably soft-tongued legato versus a true slur | [cards/low_brass.md](cards/low_brass.md) OQ6 |
| C49 | Cimbasso key, valves, origin | F (Wikipedia), F or BB♭ (Wessex), F and E♭ (VSL); four to six valves; Pelitti's instrument dated 1881 (VSL) or by 1887 (Wikipedia) | F cimbasso, the only one with a stated range (VSL). The modern design came from Josef Meinl in 1985, with Thein adopting it; the two Pelitti dates are compatible | [cards/low_brass.md](cards/low_brass.md) OQ8 |
| C50 | Alto trombone after Beethoven's middle period | the tenor-trombone card says the alto was dropped from Beethoven's middle period on, but Symphony No. 5 scores alto, tenor and bass trombones, and is usually dated to that period (1808) [Estimate: the date was not checked in an opened source] | The Beethoven template follows the symphony; the card's wording needs a source or a correction | [ensembles.md](ensembles.md) OQ12; [cards/low_brass.md](cards/low_brass.md) |

### Strings

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C51 | Violin playable top | A7 = 105 stopped, D8 harmonic (VSL) vs C8 = 108 at the fingerboard end or by harmonics (Wikipedia; data.rs) | 105, VSL's stopped range for all four strings; harmonics need a separate check | [cards/strings.md](cards/strings.md) OQ1, FC |
| C52 | Violin comfortable top | A6 = 93 (Forsyth, Rimsky-Korsakov) vs E7 = 100 (Piston via Wikipedia) vs C♯7 = 97 (Spitfire) | 93; Rimsky-Korsakov's value depends on reading his image as A6 (B4) | [cards/strings.md](cards/strings.md) OQ2 |
| C53 | Viola comfortable top | A5 = 81 (Rimsky-Korsakov) vs G5 = 79 (VSL) vs about C6 = 84 ordinary and E6 = 88 maximum (Forsyth) vs F♯6 = 90 (Spitfire) | 81, the conservative handbook value between VSL and Forsyth | [cards/strings.md](cards/strings.md) OQ4, FC |
| C54 | Viola French name | "alto" (Forsyth, Wikipedia) vs "viola" (VSL) | "alto" | [cards/strings.md](cards/strings.md) OQ5 |
| C55 | Cello playable top | A5 = 81 (VSL), B♭5 = 82 (Spitfire), E6 = 88 (Widor's concerto), A6 = 93 (data.rs, Estimate) | 88, a solo-repertoire value | [cards/strings.md](cards/strings.md) OQ6, FC |
| C56 | Cello comfortable top | E5 = 76 (Widor, Forsyth) vs A4 = 69 (Rimsky-Korsakov) | 76, two sources | [cards/strings.md](cards/strings.md) OQ7 |
| C57 | Double bass bottom | E1 = 28 (four strings), C1 = 24 (C extension), B0 = 23 (five strings; VSL calls it indispensable) | 23 for the instrument type; a lowest-string setting is A19 | [cards/strings.md](cards/strings.md) OQ8, FC |
| C58 | Double bass top | G4 = 67 (VSL) vs D5 = 74 at the fingerboard end (Wikipedia) | 67; solo parts above it report AbovePlayable | [cards/strings.md](cards/strings.md) OQ9 |
| C59 | Double bass comfortable top | G3 = 55 (Wikipedia, Rimsky-Korsakov) vs D4 = 62, Forsyth's arco ensemble limit | 55 | [cards/strings.md](cards/strings.md) OQ10 |

### Harp and plucked strings

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C60 | Harp bottom | C♭1 = 23 (Rimsky-Korsakov, Wikipedia) vs the C1 string = 24 (Hugill, a Lyon & Healy dealer page) | 23, with a warning that the lowest string must be pre-tuned | [cards/plucked.md](cards/plucked.md) OQ1 |
| C61 | Harp top | F♯7 = 102 (Rimsky-Korsakov), G7 = 103 (Hugill), G♯7 = 104 by retuning or a mechanism (Forsyth, Wikipedia) | 103, the top string in its normal tuning; admitting 104 is A18 | [cards/plucked.md](cards/plucked.md) OQ2, FC |
| C62 | Harp harmonics range | vendor note names without an octave convention (Spitfire "G2 to C6"), unlabelled clips (Hugill) | Sounding 55-96 (Estimate) | [cards/plucked.md](cards/plucked.md) OQ3, FC |
| C63 | Harp chord span per hand | an octave, a tenth with a great stretch (Berlioz); a tenth or more is normal (Forsyth); an octave and a fourth at most (Berklee) | Comfortable within an octave, a tenth at most | [cards/plucked.md](cards/plucked.md) OQ4 |
| C64 | Harp harmonics per hand | at most three in a chord (Rimsky-Korsakov); two to four in the left hand (Forsyth); two or three (Berlioz); up to three in the left hand (Berklee) | Left hand at most 3 close notes, right hand 1 | [cards/plucked.md](cards/plucked.md) OQ5 |
| C65 | Mandolin top | E6 = 88 (Forsyth) vs about G6 = 91 (Berlioz); modern instruments probably higher | Playable 91, comfortable 88 (Estimates); no modern source read (D6) | [cards/plucked.md](cards/plucked.md) OQ8, FC |

### Keyboards

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C66 | Concert vs studio grand | concert grands from 2.2 m (English Wikipedia), 240 cm (German, Italian), 2-3 m with no standard (French) | Concert grand 2.4 m and longer, studio grand about 1.8-2.4 m | [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ2 |
| C67 | Bösendorfer Imperial width | 168 cm (an unlabelled number in the maker's diagram) vs 175 cm (Wikipedia) | Both recorded; prefer a maker statement if found. No Sonara data depends on it | [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ3 |
| C68 | Una corda | one of three strings left (Berlioz) vs two of three struck (modern sources) | The modern description; the volume claim is unmeasured | [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ5 |
| C69 | CC64 | on/off at 63/64 (MIDI 1.0) vs a half-pedal zone 26-102 (VSL); only better pedals send half values (Wikipedia) | Continuous CC64 with a library-defined curve | [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ6 |
| C70 | Rhodes range | 73 keys E1-E7 = 28-100 vs 88 keys A0-C8 = 21-108 | Playable 21-108, comfortable 28-100; the meaning is A18 | [cards/electric_keys.md](cards/electric_keys.md) |
| C71 | Wurlitzer tremolo rate | 5.75 Hz (service manual) vs 5.5 Hz (Apple) vs about 6.34 Hz (Arturia) | 5.75 Hz, the maker's manual; the rate is fixed, only depth is a control | [cards/electric_keys.md](cards/electric_keys.md) |
| C72 | Wurlitzer pickup voltage | 160-170 V DC (service manual) vs 150 V (Apple) vs 170 V (Wikipedia) | 160-170 V DC, the primary source | [cards/electric_keys.md](cards/electric_keys.md) |
| C73 | Wurlitzer GM program | 5 ("usually a Rhodes or Wurlitzer") vs 6 ("usually a DX7 preset"), both Wikipedia | 5 | [cards/electric_keys.md](cards/electric_keys.md) |
| C74 | GM program 8 name | "Clavi" (the spec's spelling) vs "Clavinet" (Wikipedia) | "Clavi" | [cards/electric_keys.md](cards/electric_keys.md) |
| C75 | Clavinet D6 tabs | the manual names AB, CD and four registers, yet says one of four tabs must be down | Four tone tabs plus two pickup tabs, as two other sources describe | [cards/electric_keys.md](cards/electric_keys.md) |
| C76 | Electric piano history | 88-key Rhodes 1971, 1972 or 1973; neoprene tips 1969 or 1971; Mk II 1978 or 1979; Wurlitzer 200A from 1972 or 1974; first Wurlitzer 1954 or the early 1960s; Apple's "metal reeds" vs tine and tone bar; Apple's tremolo as pitch or amplitude | Not stored; the card's table records each. Tine and tone bar (service manual); tremolo is amplitude modulation | [cards/electric_keys.md](cards/electric_keys.md) |
| C77 | Celesta top | 108 (four- and five-octave models) vs 113 (66-key Studio and Compact models) | Playable 113, the envelope of current models; comfortable 60-108, because the orchestra's instrument is usually unknown | [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ5 |
| C78 | Celesta history and mechanism | Mustel's first compass five octaves (Forsyth, Wikipedia) or four (Schiedmayer, Widor); tuning forks (Widor) vs steel plates (Forsyth, Schiedmayer, Wikipedia) | Plates; the compass matters for history text only | [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ3, OQ4 |
| C79 | Organ speech speed | slower than the piano (Berlioz) vs instant with a Barker lever (Widor) | Depends on the action and the building; no number for an agent | [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ6 |
| C80 | Largest harpsichords and clavichords | just over five octaves and up to six octaves, without endpoints, vs the stored 29-89 and 29-96 | Stored ranges kept, the largest compasses with named endpoints | [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ17 |

### Percussion

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C81 | Timpani compass | per drum a fifth (Wikipedia; hand-tuned drums per Rimsky-Korsakov and Forsyth), about a sixth (VSL), a minor sixth (Yamaha); top of the set C4 = 60 (VSL Timpani III) or C♯4 = 61 (VSL Academy, Synchron mapping); VSL's "about three octaves" contradicts its own table, and its Synchron page names a drum size it does not list | Set 36-61, comfortable 38-57 | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ1, OQ3, FC |
| C82 | Xylophone notation | +12 (VSL, modern sources) vs at pitch (Forsyth; some scores) | +12, with a per-part "at pitch" override (A20) | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ5 |
| C83 | Glockenspiel notation | +24 (standard) vs +12 (Wagner and some scores); the keyboard glockenspiel +12 | +24 | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ6 |
| C84 | Glockenspiel range | 2½ octaves G5-C8 vs 3 to 3⅓ octaves C5-C8, E8 or F5-F8; "normal written range" (Hugill) vs three octaves usual in the symphony orchestra (VSL) | Playable 72-108, comfortable 79-108 (the compass every listed model covers); tops above C8 left out | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ7 |
| C85 | Vibraphone range and bend | F3-F5 (Hugill) vs F3-F6 (all others); bend about a half step vs up to a major second | F3-F6 (Hugill's is a slip); plan on a semitone of bend | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ8 |
| C86 | Tubular bells | strike note C4-F5 or F3-F5 at pitch (orchestral sources; VSL gives both) vs C5-F6 (Yamaha's numbering) vs three different octaves in the free libraries | Transposition 0, playable 51-79, comfortable 60-77; F3-B3 needs a percussionist's view (B6) | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ9, FC |
| C87 | Crotales comfortable range | the full 84-108 vs sets commonly one octave | Kept equal to playable; which octave is common was not found | [cards/tuned_percussion.md](cards/tuned_percussion.md) FC |
| C88 | Gong or tam-tam | "gong" reserved for tuned bossed gongs, yet read as tam-tam by players (Wikipedia); Forsyth and Widor head the tam-tam chapter "The Gong" | `tam_tam` with the alias "gong"; a pitched `gong_tuned` later (A21) | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ3 |
| C89 | Tam-tam dynamics | little more than a single p or mp stroke (Forsyth) vs terrifying in forte (Widor); priming explains both (Wikipedia) | Full dynamic range with a slow bloom for an unprimed stroke | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ4 |
| C90 | Tabor | a single snare (Forsyth) vs without snares (Widor) | Recorded only; no id | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ7 |
| C91 | Tam-tam and castanets clefs | one line without clef (Forsyth's text, Widor) vs conventional bass or treble clef, or a single line (Forsyth's table, read from OCR) | Both listed in the json | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ21, FC |
| C92 | Clash cymbals in GM | GM has only kit crashes 49 and 57; GS ORCHESTRA puts Concert Cymbal 1 on 59, which GM and Sonara treat as ride | 49, 57 | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ12 |
| C93 | Temple blocks | usually five blocks tuned pentatonically, yet not commonly treated as pitched (Wikipedia) | Unpitched, with five ordered keys | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ19 |
| C94 | Percussion history | Gossec's first orchestral tam-tam 1790 or 1791 (Wikipedia, two passages); the snare drum never used in a symphony (Widor) vs Nielsen's Fifth and Ravel's _Boléro_ | Historical only; not stored | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ5, OQ6 |

### Voices

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C95 | Alto low end | G3 (Rimsky-Korsakov) vs F3 (Berlioz, Wikipedia) | Playable F3 = 53, comfortable G3 = 55 | [cards/voices.md](cards/voices.md) OQ4 |
| C96 | Choir bass extremes | C2 (Russian octavists) and top E4 (Rimsky-Korsakov) vs F2-E♭4 school range, E♭2 or D2 for second basses, top F♯4 (Berlioz, who also calls low G and F useless for many basses) | Playable C2-F♯4, comfortable G2-D4; a choir without octavists should treat C2-F2 as unreachable (A18) | [cards/voices.md](cards/voices.md) OQ5, FC |
| C97 | Tenor top | A4 (Rimsky-Korsakov) vs B♭4 chest limit (Berlioz), falsetto E♭5 and F5 | Playable B♭4 = 70; falsetto excluded | [cards/voices.md](cards/voices.md) OQ6 |
| C98 | Baritone | G2-G4 (Czech Wikipedia) vs A2-A4 (Rimsky-Korsakov, Wikipedia) | A2-A4 = 45-69 | [cards/voices.md](cards/voices.md) OQ8 |
| C99 | Solo voice ranges | Wikipedia's typical ranges (narrower; amateurs) vs Rimsky-Korsakov's outer limits with climax notes (professionals) vs the Fach page (ranges called written but sounding; no source named) | Rimsky-Korsakov; Fach values recorded only | [cards/voices.md](cards/voices.md) OQ7, OQ9 |
| C100 | Choir sizes | chamber 16-24 and symphonic 60-150, often quoted but found in no source read, vs Rimsky-Korsakov's 32, 16-20 or 8-10 per part, a chamber choir of 8-40 (Wikipedia), library choirs of 22 and 50 | The read values | [cards/voices.md](cards/voices.md) OQ12 |
| C101 | EastWest Hollywood Choirs legato | legato on Ah and Oh at two speeds vs an unconvincing polyphonic legato and no proper marcato (two reviews) | Not stored; the manual could not be read (B8) | [cards/voices.md](cards/voices.md) OQ11 |

### Names and conventions

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C102 | "Cor" | a French table abbreviates cornet "Cor", which means horn in French scores | Read "Cor" as horn in French scores | [cards/high_brass.md](cards/high_brass.md) OQ12 |
| C103 | "Clav." | used for both the clavichord and the clavinet | Resolve from context | [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ12 |
| C104 | "Bass flute" in old scores | Forsyth's and Rimsky-Korsakov's "bass flute" (and old British usage, for example Holst) is the alto flute in G | Map "bass flute in G" to `alto_flute` | [cards/flutes.md](cards/flutes.md) (`alto_flute`, naming trap) |
| C105 | "Trombone basso" | Verdi's and Puccini's "trombone basso" is the cimbasso, not the modern bass trombone | Map it to `cimbasso` | [cards/low_brass.md](cards/low_brass.md) OQ9 |
| C106 | Czech "zvonkohra" | linked to both glockenspiel and carillon (Czech Wikipedia), which uses "zvonohra" for a tower instrument | `glockenspiel` in an orchestral context; `carillon` for "zvonohra", "věžní zvonkohra" or "carillon"; ask when unclear | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ14 |
| C107 | Italian, German and Czech percussion names | snare: tamburo militare, tamburo piccolo, rullante; castanets: castagnette, nacchere; tambourine: Schellentrommel, Tamburin; Czech Wikipedia titles vs the score terms malý buben and velký buben | All variants listed; a Czech source is D9 | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ8 |
| C108 | Vendor note names | middle C is "C3" in Logic, Orchestral Tools (inferred) and VSCO's sample names, "C4" in SFZ opcodes, BBC SO and Spitfire (inferred) and VSL's Synchron page; VSL Academy, London Orchestral Percussion and BBC SO Discover's key list state none | Convert by MIDI number, never by note name; unstated conventions are D5 | [cards/strings.md](cards/strings.md) FC; [cards/clarinets.md](cards/clarinets.md) OQ11; [cards/flutes.md](cards/flutes.md) OQ13; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ14; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ20; [cards/low_brass.md](cards/low_brass.md) OQ13 |
| C109 | Harp-maker octave numbers | a dealer's "0th octave G to 7th octave C" is defined nowhere | Never parsed as scientific octaves | [cards/plucked.md](cards/plucked.md) OQ13 |
| C110 | GM notes 82-85 | Shaker, Jingle Bell, Belltree and Castanets appear in GM listings but lie outside GM1's 35-81 (Roland GS extensions) | Never stored as GM1 percussion notes | [cards/free_libraries.md](cards/free_libraries.md) OQ13 |
| C111 | Apple's horn in E♭ staff style | "E♭ (−3)" in Apple's table vs a horn in E♭ written a major sixth above sounding (+9 in Apple's sign) | Not imported; −3 fits an E♭ instrument sounding a minor third up | [cards/apple.md](cards/apple.md) OQ1, FC |

### Libraries and Apple

| Id | Topic | Values in the sources | Adopted, and why | Ref |
|---|---|---|---|---|
| C112 | VCSL contents | the README says it holds the bulk of VSCO 2 CE, but its tree has no bowed strings, brass or orchestral woodwinds | VSCO 2 CE for the orchestra, VCSL for keyboards and percussion | [cards/free_libraries.md](cards/free_libraries.md) OQ1 |
| C113 | VCSL's SFZ files | the keyboard and plucked cards found WAV files only (they read `master`) vs 183 `.sfz` in tag `v1.2.2-RC` | The tag holds the SFZ patches; the cards now say so (the harp, piano and harpsichord rows); the `Grand Piano, Steinway B` patch itself was not opened (D1) | [coverage.md](coverage.md) OQ4; [sfz-conformance.md](sfz-conformance.md) E; [cards/acoustic_pianos.md](cards/acoustic_pianos.md); [cards/plucked.md](cards/plucked.md) OQ14 |
| C114 | A free Wurlitzer | none found (electric-keys card) vs Greg Sullivan's Wurlitzer EP200 (free-libraries card, sound-mapping.md) | It exists and plays partly (SFZ v2 with ARIA extensions); its LICENSE and README say CC BY 3.0, while GitHub's licence detection says NOASSERTION; the electric keys card now records both | [coverage.md](coverage.md) OQ4; [cards/electric_keys.md](cards/electric_keys.md); [cards/free_libraries.md](cards/free_libraries.md) |
| C115 | Greg Sullivan E-Pianos licence | NOASSERTION in GitHub's metadata vs CC BY 3.0 in the `LICENSE` legal code and README | CC BY 3.0 | [sound-mapping.md](sound-mapping.md) OQ7 |
| C116 | jRhodes3d licence | CC BY-NC 4.0 (3d `LICENSE`), BY-NC-SA 4.0 linking the BY-NC deed (3c `LICENSE`), "CC-SA-BY" (3d `.sfz` header) | CC BY-NC 4.0, from the `LICENSE` files: user-installed only, never bundled | [cards/electric_keys.md](cards/electric_keys.md); [sound-mapping.md](sound-mapping.md) OQ7 |
| C117 | VSCO key-switch patches | VSCO 2 CE "mostly" plays as is (free-libraries card) vs `ViolinEns-KS`, `Flute-KS` and `Clarinet-KS` silent until a switch key (Q1) | Silent until a switch key: code reading, confirmed twice | [sfz-conformance.md](sfz-conformance.md) E; [sound-mapping.md](sound-mapping.md) Summary |
| C118 | Sonatina `Harpsichord KS` | plays (historical-keys card) vs articulations stacked (Q11) | Stacked (Q11); a render confirms it (B1) | [coverage.md](coverage.md) OQ4 |
| C119 | Salamander, Splendid and Sonatina Grand | partly playable (cards) vs no piano notes (Salamander, Q2) and load failure (Splendid, Sonatina Grand, Q3) | Do not play until Q2 and Q3 are fixed (A8) | [sfz-conformance.md](sfz-conformance.md) E; [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ14 |
| C120 | Keppy's Steinway licence | CC BY 4.0 for 5.1-6.27 and BY-SA 4.0 for 7.x (mirror README), BY-SA 4.0 (mirror `LICENSE.md`), CC0 (project page), by-sa (Musical Artifacts) | The strictest, BY-SA: user install only | [cards/free_libraries.md](cards/free_libraries.md) OQ3 |
| C121 | Kawai upright licence | GPL v3+ with an exception (MuseScore handbook) vs CC0 1.0 (current FreePats page) | The FreePats page, the current download's source; its relicensing date is not documented | [cards/free_libraries.md](cards/free_libraries.md) OQ4 |
| C122 | Maestro Concert Grand licence | "Custom" in the SFZ Instruments catalogue vs all rights reserved, no modifying or spreading (its readme) | Not a free licence | [cards/free_libraries.md](cards/free_libraries.md) OQ5 |
| C123 | VPO3 licence | copying and redistribution with credit (the site) vs CC BY-SA and Sampling Plus components and Philharmonia samples | Never bundled (A17) | [cards/free_libraries.md](cards/free_libraries.md) OQ8, FC |
| C124 | Sonara's own research doc | `docs/orchestral-audio-engine-research.md` calls the FreePats harp CC BY-NC and VSCO's "bass" a section | The harp sounds are CC0 (only the photo is BY-NC); VSCO's bass is a solo contrabass. The doc is corrected (A24) | [cards/free_libraries.md](cards/free_libraries.md) OQ2 |
| C125 | Download sizes | MuseScore_General 35.9 MB SF3 and 208 MB SF2 (handbook) vs 39.9 MB and 215.6 MB (server); BBC SO Discover 240 MB vs 360 MB on one page | Not used; a different file version is the likelier cause for MuseScore | [cards/free_libraries.md](cards/free_libraries.md) OQ6, OQ7 |
| C126 | Logic's Scripter setting | "Enable Complete Features" in the Advanced settings (Scripter page) vs no Advanced pane since 12.0 (What's New) | The newer 12.0 statement | [cards/apple.md](cards/apple.md) OQ3, FC |

## D. Research follow-ups

| Id | Follow-up | Where it is recorded |
|---|---|---|
| D1 | **Library files not inspected.** VPO3; every FreePats SFZ set (Salamander, Upright KW, Old Piano FB, nylon guitar, accordion, organ emulations, timpani, xylophone, tubular bells, clarinet, tenor sax, including whether its tenor sax adds +12 to VCSL's file names); Keppy's Steinway; the rest of the Sonatina and VCSL key-switch files; Sonatina's single organ stops, harpsichord registers, xylophone, glockenspiel, chimes, crotales, marimba and cymbals, and which keys of `All Unpitched Percussion` play castanets, tambourine, sleigh bells and shaker; VCSL's `Grand Piano, Steinway B`, Timpani 2, Renaissance Organ, the other harpsichords and the release SFZ; the Bigcat single programs and War Tuba's other programs; Spitfire LABS Choir; BBC SO Discover's articulations per instrument; NI's baritone saxophone; the release asset sizes of VSCO 2 CE and VCSL. | [sound-mapping.md](sound-mapping.md) OQ8; [coverage.md](coverage.md) OQ2, OQ3; [cards/free_libraries.md](cards/free_libraries.md) OQ11, OQ17, FC; [cards/saxophones.md](cards/saxophones.md) OQ9, FC; [cards/voices.md](cards/voices.md) OQ13; [sfz-conformance.md](sfz-conformance.md) E |
| D2 | **A wider search for sounds no free library covers.** Clavinet (a forum lead names bandshed.net; no licence seen); choirs with separate parts, and solo voices; carillon; recorded legato transitions; harp techniques beyond the pluck; guitar, lute and mandolin beyond the Terkelsen mandolin; clavichord, accordion, felt and prepared piano; Wagner tubas, cimbasso, euphonium, cornet, flugelhorn, piccolo trumpet, E♭ clarinet, oboe d'amore, bass flute; a tonewheel organ (setBfree exists, licence not checked against LIC-01). Only 10 of 76 sfzinstruments repositories were visible. | [cards/free_libraries.md](cards/free_libraries.md) OQ12; [coverage.md](coverage.md) OQ8, gap rank 15; [cards/electric_keys.md](cards/electric_keys.md); [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ15; [cards/plucked.md](cards/plucked.md) OQ14; [cards/voices.md](cards/voices.md) OQ13 |
| D3 | **Licences and provenance not confirmed.** The Terkelsen mandolin (no licence metadata); the Bigcat cello and D. Smolken bass (catalogue label and GitHub metadata only, no `LICENSE` file); Piano in 162 (no licence stated, and whether it is VCSL's Steinway B); Splendid Grand's public-domain basis (no AKAI document); Lars Palo's "NC20250613" organ package; the Sonus Paradisi and Piotr Grabowski organ sets (no stated licence; a GrandOrgue-to-SFZ conversion needs each set's permission); setBfree; the Kawai upright's relicensing date; the VPO web licence (read only as a paraphrase); MS Basic's sample identity; the stated provenance doubts of Sonatina and GeneralUser GS. | [sound-mapping.md](sound-mapping.md) OQ7; [cards/free_libraries.md](cards/free_libraries.md) OQ4, OQ9, OQ10, OQ15, OQ16, FC; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ15 |
| D4 | **GM and GS data not read.** The GM2 Orchestra Kit's note numbers (it has timpani); the GM2 variation banks (cornet, flugelhorn, piccolo trumpet; Carillon Bells as variation 2 of program 15); GS sets beyond the seven SC-55 kits (an SFX set may map note 85 differently); what GS STANDARD note 28 "Slap" is. | [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ15, OQ17; [cards/high_brass.md](cards/high_brass.md) OQ14; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ2, FC |
| D5 | **Conventions the makers do not state.** Middle-C naming of Orchestral Tools (C3 = 60 inferred), VSL Academy (C4 = 60 inferred; its harpsichord "F1-E6" and harmonium "C1-C6" cannot both fit one convention), Spitfire (C4 = 60 inferred), London Orchestral Percussion and BBC SO Discover; Sonatina's octave offset (the SFZ default c4 = 60 assumed); the octaves of Yamaha's timpani letters; MuseScore's pitch ranges read as sounding (an inference); the tuning of harpsichord libraries (A = 415 or 392 Hz). | [cards/clarinets.md](cards/clarinets.md) OQ11, FC; [cards/low_brass.md](cards/low_brass.md) OQ13, FC; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ13, OQ14, FC; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ20, FC; [cards/high_brass.md](cards/high_brass.md) OQ15, FC; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ2 |
| D6 | **Instrument facts with one source or none.** Keyboards: the honky-tonk detune (5-15 cents, Estimate), 85-key older uprights, the Fazioli F308's length and key count, double escapement in some uprights (search snippets only), the first undamped key per model, the Italian pedal abbreviations, the 54-key Rhodes and 44-key Wurlitzer 106P ranges, Leslie spin-up times, organ expression on CC11, accordion bellows on CC11 (a Roland MIDI implementation chart), comfortable ranges and clefs for harpsichord, clavichord and Hammond. Winds and brass: the B♭ clarinet above written C7; the flugelhorn's practical top; a second source for the bass trombone top and the euphonium top; the cimbasso in B♭ and C; euphonium tonguing and vibrato; whether VSL's horn valve-trill zone is written or sounding; sources for `tenuto` on the trumpets, `rip` and the flugelhorn's `tenuto` and `marcato`; a phrase length without a rest (the 8-bar rule is an Estimate; Widor gives a held-note figure for the saxophone and the clarinet only). Plucked: the mandolin's modern top; the lute's notation (−12 or 0) and the octave of its tuning letters. Percussion: pitches of tuned gongs; the crotales' common octave; carillon ranges, transpositions and dampers per tower (the json's undamped bells vs the GCNA page); Rimsky-Korsakov's "little bells" (sleigh bells or a glockenspiel-like set). | [cards/acoustic_pianos.md](cards/acoustic_pianos.md) OQ7-OQ10, OQ13, OQ16, OQ17; [cards/electric_keys.md](cards/electric_keys.md); [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ10, OQ15, FC; [cards/clarinets.md](cards/clarinets.md) OQ6; [cards/high_brass.md](cards/high_brass.md) OQ10; [cards/low_brass.md](cards/low_brass.md) OQ8, OQ10, FC; [cards/horns.md](cards/horns.md) OQ8; [doubling-and-balance.md](doubling-and-balance.md) OQ7; [cards/plucked.md](cards/plucked.md) OQ8, OQ9, FC; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ3, OQ16; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ15, FC |
| D7 | **Ensemble and score-practice sources.** Classical-era string counts (Mozart's letters, Haydn scholarship; the 8-6-4-3-2 default is an Estimate) and classical double-bass line-ups; the order of harp, celesta and piano, and of that block against soloists and chorus (a style guide); wind and brass quintet order; the film-orchestra roster (a handbook chapter on studio orchestras or a published session layout); which of the SFZ libraries Sonara can load are recorded in situ (VSCO 2 CE, Sonatina, VPO3), and whether any places its sections with the `pan` or `width` opcodes, which the sampler ignores (Spitfire states it, but its libraries and BBC SO Discover need plugin hosting); the number of percussionists per era. | [ensembles.md](ensembles.md) OQ1, OQ2, OQ5, OQ8, OQ9; [cards/strings.md](cards/strings.md) OQ14; [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ9 |
| D8 | **Doubling and balance sources.** A text source for the low-interval limits (the charts read were images or refused, and published charts disagree on the tritone and the fifth); weights for mezzo-forte, saxophones, Wagner tubas, euphonium, flugelhorn, the cornet (Rimsky-Korsakov says only that it is weaker than the trumpet), solo strings, harp, keyboards, tuned percussion and voices; Rimsky-Korsakov's Vol. II examples transcribed as a test oracle for the balance and voicing rules and their thresholds (TST-10); a source for "overlapping" as a fourth voicing method, if one exists (Rimsky-Korsakov's three are superposition, crossing and enclosure). | [doubling-and-balance.md](doubling-and-balance.md) OQ2, OQ4, OQ6, OQ9 |
| D9 | **Names and abbreviations.** Czech score terms (malý and velký buben; "zvony" and "trubicové zvony" for tubular bells), French names and Czech plurals of the voices, the Italian and German names marked Estimate (cornetta, tromba piccola, Kornett), and every score abbreviation marked Estimate (all except "S.D.", "B.D.", "Cym." and "sus. cym."). A Czech orchestration source and an engraving style guide would settle them. | [cards/untuned_percussion.md](cards/untuned_percussion.md) OQ8, FC; [cards/tuned_percussion.md](cards/tuned_percussion.md) OQ14; [cards/voices.md](cards/voices.md) FC; [cards/high_brass.md](cards/high_brass.md) OQ12; [cards/strings.md](cards/strings.md) OQ15; [cards/clarinets.md](cards/clarinets.md) FC; [cards/historical_keys_organs.md](cards/historical_keys_organs.md) OQ12; [cards/flutes.md](cards/flutes.md) OQ17 |
| D10 | **Claims resting on one reading.** Each fact-check log lists the sources it did not reopen; their claims stand as the first researcher read them. Among them: the Yamaha clarinet models; the double reeds' Czech names and the SSO README; several electric-keys sources read only through a fetch tool's quotes; Forsyth details for the flutes; the high-brass mute and naming claims (and a manual decoded with a home-made PDF decoder); horn claims from Horn Matters, a Spitfire article and the Philharmonia and Iowa terms; low-brass claims on the euphonium, the tenorbass, slide technique and bore classes (and Spitfire's Professional-only instruments, documented by a product page only); several saxophone sources; VSL's per-string colour adjectives; every VSL percussion page, read through a summarising fetch; the Apple pages not reopened, the release years of Logic 10.4 and 10.5 (from press coverage) and the guide's 12.3 against the release notes' 12.4. | the "Fact-check log" of every card in [cards/](cards/) |
| D11 | **Open points in the SFZ verification.** Whether project loading accepts a device whose `plugin_id` is not registered (sfz-conformance C3 covers the render only); sfizz read through a summarising fetch; the `opcodes/loprog` page not opened; sfizz's layer gating for switch keys (sfz-conformance C17) not read; the hits marked "not re-read" (Discord GM bank, Ixox Flute, Splendid, Bigcat cello, Organ Combinations, the VCSL harp, glockenspiel, marimba, Kawai and Knight, jRhodes, Sonatina Vibraphone and Celeste). | [sfz-conformance.md](sfz-conformance.md) Method, B, D |

## Review notes

Triage of 2026-10-30. What this file did with its inputs:

1. **Deduplicated.** The most repeated questions were merged into one item each: the sampler's opcode
   support (asked in 14 card files; now answered by [sfz-conformance.md](sfz-conformance.md), A8, with
   the audible result left to B1); the woodwind `fingered_tremolo` and the multiple-tonguing spelling
   (A12, A13); the GM fallback programs (seven cards, A11); the meaning of `comfortable` (five cards,
   A18); unreachable VSL, Yamaha and MIDI Association pages (B8); vendor octave naming (C108, D5).
2. **Updated to the latest state of the files.** The five applied vocabulary merges and the open
   `col_legno` decision follow the current [articulation-vocabulary.md](articulation-vocabulary.md); the
   jRhodes3d and Greg Sullivan licences follow [sound-mapping.md](sound-mapping.md) OQ7 (C115, C116).
3. **Left out as already resolved:** the release of Studio Strings and Studio Horns (Logic Pro X 10.4,
   [cards/apple.md](cards/apple.md) OQ4); Forsyth's Wagner tuba staves (read at pixel level,
   [cards/horns.md](cards/horns.md) OQ4; a second scan stays in B4); the octave of Forsyth's cello
   pizzicato limit (B♭4, [cards/strings.md](cards/strings.md) OQ11); the cimbasso's designer (Meinl,
   folded into C49); the VCSL licence (CC0 per its `LICENSE`, read by the free-libraries card, which
   settles [cards/flutes.md](cards/flutes.md) OQ16); the Czech "C tenor" saxophone (two instruments, not a
   contradiction).
4. **Left out as housekeeping:** tag counts, notes on temporary download folders, the card-template
   order of the fact-check log, and confirmations that were not corrections.
5. **Inconsistencies found and handled here, not in the source files:** [cards/plucked.md](cards/plucked.md) OQ5 attributes "two to four in the left hand" to
   Widor, but its source S2 is Forsyth in that card's Sources table; C64 names Forsyth. The
   [cards/electric_keys.md](cards/electric_keys.md) statement that no free Wurlitzer exists is
   contradicted by the free-libraries card (C114). The cards' GM programs and range conventions are not
   uniform; A11 and A18 list the differences. The tenor-trombone card's remark on the alto trombone in
   Beethoven's middle period, which [ensembles.md](ensembles.md) OQ12 questions, is C50. The cards and the
   synthesis files were not edited.
6. **Repository form.** No absolute paths, scratch locations or internal work labels; the files are
   linked relative to this directory. No Cited tag is carried over, because this file has no Sources
   table and the source numbers differ between cards; sources are named in words and the card is
   linked. No quotation from a non-public-domain source is longer than 15 words.
7. **Count.** 25 owner decisions (A1-A25), 8 human checks (B1-B8), 126 data disagreements (C1-C126) and
   11 research follow-ups (D1-D11): 170 items.
- C113 and C114 updated after the cards were corrected (VCSL SFZ in the tag; the Wurlitzer licence caveat).
- A24 and C124: the correction of `docs/orchestral-audio-engine-research.md` is done.
- Recorded the owner's decisions of 2026-10-30 (A1, A3, A4, A5, A6, A12, A14, A16, A17).
