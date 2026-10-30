# Performance layer: from notes to music

_Status: proposed (2026-10-30). Part of the orchestra design; decisions in [../../orchestra-design.md](../../orchestra-design.md)._

## Summary

This layer turns notes into a performance, so that Sonara can **compose** and not only write MIDI.
It refines spine decisions S4 (articulation and dynamics transport), S9 (ensembles) and S10 (agents)
and is milestone O4.

- **A pure module, `sonara_midi::performance`.** It infers articulations from note lengths, gaps and
  overlaps; shapes dynamics (phrase arcs, swells, timpani roll crescendos) as written velocities plus
  points for the sampler's `dynamics` parameter, as layer (b) defines it; humanizes per family, with
  attack compensation from the preset's facts; voices and doubles parts by family (string divisi,
  wood-wind interlocking, horn pairs, brass chorale); orchestrates a reduction (melody, harmony, bass)
  for an ensemble; and checks range, spacing, doubling and playability by the rule ids of
  [../doubling-and-balance.md](../doubling-and-balance.md) section 6. Everything is deterministic for
  a seed (TST-04). No new crate, and no dependency in `sonara-midi` beyond layer (a)'s `serde` and
  `unicode-normalization` edges.
- **The agent side.** `sonara_agent::orchestration` learns instrument and ensemble requests in
  English and Czech ("strings play a legato melody, horns and timpani underneath"; "smyčce hrají
  legato melodii, lesní rohy a tympány pod tím"). It resolves aliases, asks clarification questions,
  and emits a plan of **existing** tools only: `create_track`, `set_instrument`, `create_midi_clip`,
  `create_automation`, `configure_routing`, `set_track_pan`, `add_effect`. Two planner mechanics are
  new: step references (a later step uses an id an earlier step returns) and staged validation with
  rollback, which changes today's validation guarantee (declared under "Deviation from the spine").
  Names are matched by layer (a)'s `resolve()`, folding included; "did you mean" suggestions use
  `strsim`, already in the lock file (QG-13, QG-10).
- **The AI Composer** gets a generated orchestration reference in its system prompt (budget 24,000
  characters), rendered from the same statics, so it cannot drift. **`sonara_app::quality`** gets
  orchestration findings whose codes are the rule ids.
- **Honesty.** With VSCO 2 CE and VCSL, phase 1 delivers the right instruments in range and in
  score order, four solo horns voiced 1-3-2-4, timpani hits and a roll, a string line whose legato
  is an **overlap crossfade of sustain samples** (no recorded transitions) and dynamics as **gain
  within the velocity layer chosen at note-on** (timbre changes only between notes; a crescendo on
  one held note is bounded to +12 dB). Recorded transitions and timbral crescendos need M5 (plugin
  hosting) or richer libraries.

## Today (verified)

Code was read in a checkout of branch `research/orchestra-instruments`, whose `crates/` and `apps/`
equal `main` at `a212544` (`git diff a212544 HEAD -- crates apps` is empty). Sampler facts come from
[../sfz-conformance.md](../sfz-conformance.md), which wins over other research files.

| Fact | Evidence |
|---|---|
| `sonara-midi` depends only on `sonara-core`; `sonara-app` and `sonara-agent` may depend on it; real-time crates are `sonara-dsp`, `-rt`, `-io`, `-engine` | `crates/sonara-testkit/tests/architecture.rs:14-64`; `crates/sonara-midi/Cargo.toml` (deps `midly`, `rand_mt`, `sonara-core`, `thiserror`) |
| The metadata has 16 instruments (no `viola`, no `timpani`); `check_sounding` returns `Within`, `OutsideComfortable`, `BelowPlayable`, `AbovePlayable` and never clamps | `crates/sonara-midi/src/orchestration/data.rs:86-335`; `instrument.rs:92-104,290-308` |
| `transforms::humanize` (seeded `PyRandom`, timing σ in beats, velocity `randint`) and `transforms::legato(notes, overlap)` exist and are **golden-pinned** | `crates/sonara-midi/src/transforms.rs:81-140,242-263`; `crates/sonara-midi/tests/golden_generators.rs:247-268` |
| `PyRandom` is a bit-exact CPython `random.Random` over `rand_mt`; `PyRandom::new(seed: i64)` ignores the sign and seeds MT19937 through `Mt::new_with_key` | `crates/sonara-midi/src/pyrandom.rs:1-41` |
| Theory helpers exist: key parsing, chord parsing, Roman numerals, voice-leading `voice`, `bass_note`; `ChordSlot` places a chord in beats; the chord-quality table is a private const | `crates/sonara-midi/src/theory.rs:214,276,321,448,514,584`; `generators.rs:22-38` |
| A plan reaches a project as one undoable `Transaction` in pure code (precedent) | `crates/sonara-midi/src/form.rs:23,527-552`; `crates/sonara-core/src/commands/mod.rs:378` |
| A note's start must be ≥ 0 and its length > 0 | `crates/sonara-core/src/midi.rs:53-54` |
| The planner understands tempo, set-instrument-by-name and delete-track; `Step.args` is a fixed JSON value; `execute` dry-runs **every** step before the first commit; `StepResult` keeps no result payload | `crates/sonara-agent/src/orchestration.rs:7-27,60-70,108-116,179-238` |
| The planner splits a request at `and`, `then`, `,`, `;`, so "horns and timpani underneath" becomes two clauses | `orchestration.rs:309-327` |
| Name matching: lower-case, word boundaries by `char::is_alphanumeric` (Unicode-aware); no stemming, no diacritic folding | `orchestration.rs:351-360,423-483` |
| The planner has no caller outside its tests; no tool or GUI. Its documented guarantee: "A refusal changes nothing" | `docs/implementation-status.md:111`; only `crates/sonara-agent/tests/orchestration.rs` uses `Orchestrator`; `docs/orchestration.md:30-31`; `orchestration.rs:23-25` |
| `AgentService` exposes its `AppContext` and calls tools with the source `agent` | `crates/sonara-agent/src/service.rs:74,250` |
| `create_track` takes no `params`; a new instrument track defaults to the Poly Synth; its result holds `track_id` and `instrument_device_id` (nested under `result`) | `docs/agent-api.md:254-270`; `crates/sonara-agent/src/handlers/tracks.rs:49-82`; `golden/workflows/cinematic_orchestral/transcript.json` (step `horn`) |
| `set_instrument` mints a **new** device id, drops the automation lanes of the old device and returns `device_id` | `crates/sonara-agent/src/handlers/tracks.rs:176-192`; `crates/sonara-core/src/commands/device.rs:110-167` |
| `create_automation` targets `'<device_id>:<param>'` and refuses a parameter that is not automatable; points are `{at, value, curve: linear \| hold \| smooth}` | `docs/agent-api.md:205-217`; `crates/sonara-core/src/commands/automation.rs:69-74`; `crates/sonara-agent/src/contract/args.rs:227-237,1102-1129` |
| Device parameters are read once per block at the block's start beat; an enum is set by choice index `round(clamp(v, 0, n-1))`; mixer automation is per sample; blocks are fixed at 512 frames | `crates/sonara-engine/src/automation.rs:12-19`; `crates/sonara-engine/src/graph/process.rs:260,280,296-307`; `docs/audio-engine.md:48,187` |
| `list_plugins {plugin_id}` returns the descriptor with parameters, their `choices` and `automatable`; without `plugin_id` parameters are omitted | `crates/sonara-agent/src/handlers/devices.rs:12-31`; `crates/sonara-core/src/plugins/descriptor.rs:360-375,560-577` |
| The sampler has `volume` (dB, automatable: `spec` defaults to `automatable: true`), `pan`, `release_scale` and `instrument` (enum of patch paths; `ep` hard-codes `automatable: false`); no `articulation` or `dynamics` parameter yet | `crates/sonara-core/src/plugins/builtin.rs:49,55-62,84-91,767-798` |
| A voice's gain is the region's SFZ `volume` × (v/127)²; CC 7 and 11 are linear gain, CC 1 does nothing; `trigger=first/legato` become attacks; velocity crossfades are ignored; a note-on releases only the voices whose `off_by` equals its region's `group`; a note-off releases **every** voice of its key and channel | [../sfz-conformance.md](../sfz-conformance.md) Q5, Q6, Q8, C15; `crates/sonara-dsp/src/processors/sampler.rs:204-226,247-258,264-292,315-325,344-345` |
| At equal frames the engine sends note-offs before note-ons; notes are frame-accurate; a note-off is clamped to the clip end; controller events are dropped for looping clips | `crates/sonara-engine/src/schedule.rs:6-15,228-236` |
| VSCO `ViolinEnsSusVib.sfz` has 22 regions in two velocity layers (0-62, 63-127), `ampeg_attack=0.001`, `ampeg_release=0.8`, no `group`/`off_by`, no loop; `ViolinEns-KS.sfz` uses the same `susVib` samples and layers in its sustain group; the one sustain sample measured lasts 11.2 s | downloaded files and `VlnEns_susVib_A2_v1.wav` read for [../sfz-conformance.md](../sfz-conformance.md) (`afinfo`) |
| Quality findings exist (`OVERLAPPING_NOTES` only for the same pitch and channel; `UNIFORM_VELOCITY` for a clip of ≥ 8 notes all at one velocity; `code` is `&'static str`); no tool exposes them | `crates/sonara-app/src/quality.rs:1-13,25,51-58,223-310` |
| Consecutive undo steps with the same merge key within 1.5 s merge; a pan change keys on `mixer:<track>:pan` | `crates/sonara-core/src/undo.rs:7-9,61-70`; `crates/sonara-core/src/commands/track.rs:427-435` |
| The AI Composer's system prompt is 3,960 bytes, included at compile time and passed with `--append-system-prompt`; a test checks needles in it; each tool call is its own undo step | `crates/sonara-app/src/composer/invocation.rs:33,252-253,460-470`; `docs/ai-composer.md:70` |
| A committed generated doc is kept current by an example whose test fails on a difference | `crates/sonara-agent/examples/gen_agent_docs.rs:1-11` |
| `unicode-normalization` 0.1 is a workspace dependency (MIT/Apache-2.0); `strsim` 0.11 (MIT/Apache-2.0) is in the lock file through clap | `Cargo.toml:106`; `docs/dependencies.md:43,57`; `crates/sonara-media/src/names.rs` |
| In main since 0.1.12 (merged from `feature/ai-composer-finish` after this design was first written against `a212544`): `ProjectSession::begin_undo_group`/`end_undo_group`, wrapped by `sonara_app::edit` and used by the desktop composer, collapse the `agent` steps of a run into one undo step | main at 0.1.12 (`558f898`): `crates/sonara-core/src/session.rs:400-407`, `crates/sonara-app/src/edit.rs:670-676`, `apps/sonara-desktop/src-tauri/src/composer.rs:171-184` |

## Proposal

### 1. Placement and shape

`sonara_midi::performance`, a new module directory beside `sonara_midi::orchestration`:

| Module | Role |
|---|---|
| `performance::mod` | the types and `perform()`; re-exports |
| `performance::articulate` | per-note inference, passages, the phase-1 automation points |
| `performance::dynamics` | the level table, phrases, arcs, swells, roll crescendos, written velocity and lane ratios |
| `performance::humanize` | per-family deviations and attack compensation |
| `performance::voicing` | register fit, doubling and voicing helpers by family |
| `performance::check` | rule predicates keyed by rule id (data in `orchestration::rules`, S1) |
| `performance::reduction` | orchestrate a reduction for an ensemble |
| `performance::seed` | stable per-part random streams |

Why here: pure functions over `MidiNote`, `theory` and `orchestration` data, with no I/O.
`sonara-midi` may depend on `sonara-core` only, and `sonara-app` and `sonara-agent` may depend on it
(ARC-11). It is not a real-time crate and nothing on an audio thread calls it (RT-01); the
architecture test allows `sonara-engine` → `sonara-midi`, but no such edge exists or is added. No
new crate: the split brings no compilation or safety boundary (MOD-09). The golden-pinned
`transforms::humanize` and `transforms::legato` are reused unchanged or not at all (GLD-01).

What the chosen sound can do comes from the catalog (S7 profile plus the facts layer (b) derives from
the file), which lives in `sonara-app`. Because `sonara-midi` may not depend on `sonara-app`
(ARC-11), it is adapted into a plain data struct defined here (ARC-08: infrastructure adapts to the
domain; no trait, TRT-04):

```rust
// Sketch (not implemented): crates/sonara-midi/src/performance/mod.rs
pub struct SoundCaps {                    // filled by sonara-app from the catalog (PF-9)
    pub articulations: Vec<Articulation>, // what the preset can switch to (S4)
    pub legato: LegatoKind,               // None | Overlap { crossfade_ms } | Trigger
    pub layers: Vec<u8>,                  // upper velocity bound of each layer, e.g. [62, 127]
    pub dynamics: DynamicsKind,           // Velocity | Fixed(u8) (ratio, layer (b) 5.4) | Cc (absolute)
    pub attack_ms: Vec<(Articulation, f64)>, // layer (b) SS-35 or the profile override; empty if unknown
    pub max_hold_s: Option<f64>,          // longest unlooped sample (SS-35), if known
    pub sampled: Option<PitchRange>,      // keys that have attack regions
}
pub struct PartLine { pub part: PartId, pub instrument: &'static Instrument,
                      pub role: Role, pub notes: Vec<MidiNote>, pub caps: SoundCaps }
pub fn perform(line: &PartLine, score: &ScoreContext, seed: Seed)
    -> Result<Performed, PerformanceError>; // notes, passages, dynamics points, rule hits
```

`PartId` (`violins_1`, `horn_3`; the slug of layer (a)'s ensemble `Part`) and `Role` (melody, counter-melody, harmony, bass, pedal, rhythm,
colour; [../doubling-and-balance.md](../doubling-and-balance.md) section 1) are domain types
(TYP-01, TYP-02). `PerformanceError` is a `thiserror` enum (ERR-03); no `unwrap` in production
paths (QG-03).

### 2. Articulation inference

Input: one voice of a part, sounding pitches, the tempo map (thresholds are in seconds, so they hold
at any tempo), the instrument and its `SoundCaps`. Output: **one primary id per note** (S2) with a
reason, then passages. Inference never chooses effects, mutes or vibrato: those are states or
requests (S2), not something note lengths reveal. It emits only existing and preferred ids of S2's
vocabulary, never the invented ids that A13 decides.

**Duty** = length ÷ onset gap, where the onset gap is the time to the next onset of the voice; when
no onset follows within the note's length plus one beat (a rest, a phrase end), the duty is 1. The
rules are tried in order; the first match wins, so every note gets exactly one id:

| # | Signal (per note) | Inferred id | Notes |
|---|---|---|---|
| 1 | The next note of the voice starts before this one ends, at another pitch | `legato` | the library convention: overlap, monophonic ([../articulation-vocabulary.md](../articulation-vocabulary.md) `legato`) |
| 2 | ≥ 4 notes of one pitch with onsets ≤ 0.09 s apart | strings `tremolo`; timpani and drums `roll`; merged into one held note | the inverse of the emulation in [../sound-mapping.md](../sound-mapping.md) 4.2 |
| 3 | Length < 0.12 s | `staccatissimo` (winds, brass), `spiccato` (strings) | fallback `staccato` |
| 4 | Length < 0.25 s, or duty < 0.5 and length < 0.4 s | `staccato`; a string section with onsets ≤ 0.2 s apart: `spiccato` | |
| 5 | 0.5 ≤ duty < 0.85 | `tenuto` on winds and brass; `portato` (a preferred id) on strings | fallback `sustain` shortened ([../sound-mapping.md](../sound-mapping.md) 4.2) |
| 6 | Otherwise | ordinario long (no id or `sustain`, S2, A14) | |
| + | Velocity ≥ the local median + 20 on a note shorter than 0.4 s | `marcato` in place of 3-6 | only when the preset has it; else the velocity accent stays |

All thresholds are [Estimate: chosen to separate library long and short patches; to be checked by
listening, B2]. Explicit words in the request override inference. An id the preset does not have
follows the articulation substitution of [../sound-mapping.md](../sound-mapping.md) 4.2, including
emulation by rewriting notes (a written-out trill, a roll as repeated strokes), and the report says
so.

**Legato overlaps.** For a passage the request calls legato, the layer writes overlaps of
min(30 ms, 25 % of the shorter note), at least 10 ms [Estimate: a starting value for the overlap crossfade, to be tuned by the listening check B2]. Notes are frame-accurate
(`schedule.rs:6-15`), and layer (b)'s overlap legato fires on a note-on while another key is held
(or within `gap_ms` of a note-off) and crossfades over the profile's `crossfade_ms`, whatever the
overlap ([sound-source.md](sound-source.md) 6.3); the 10 ms floor only keeps the overlap positive
after rounding and humanization. **A repeated pitch is never overlapped:** the first note's note-off
would release the new voice too (`note_off` releases every voice of the key and channel,
`sampler.rs:264-292`), and `OVERLAPPING_NOTES` would flag it (`quality.rs:252-291`).

**Passages (phase 1).** The sampler holds one articulation state per track (S4). Per-note ids are
merged into passages of at least two notes or one beat. Each passage becomes one `hold` point on
`<device_id>:articulation` whose value is the **choice index read from the descriptor** returned by
`list_plugins {plugin_id: "sonara.sampler"}`, never a hard-coded number; an ordinario passage uses
the `ordinario` choice, not `auto`, which would leave the patch's default and any clip switch notes
in charge ([sound-source.md](sound-source.md) 5.1). The lane is read at each block's start beat (up
to 511 frames late), so the point sits two blocks (23 ms at 44.1 kHz, the slowest supported rate,
converted to beats) before the passage's first note-on, but never before the previous passage's last
note-on, which it would switch too; with less than one block between the two, the first note is
reported as possibly late. This is the one lead rule of the design: layer (b) 5.5 states only the
engine fact and layer (d) 7.2 draws this rule's marker. A switch affects only later note-ons. Notes that overlap but
need different ids are split into divisi tracks, because phase 1 cannot express them on one.

### 3. Dynamics shaping

One level table serves the generator, the velocity choice and the quality layer's reading of
velocity as a dynamic ([../doubling-and-balance.md](../doubling-and-balance.md) 0.3), so the three
cannot disagree (DES-01):

| Marking | Velocity band (derivation) | Level velocity | `dynamics` value in `cc` mode |
|---|---|---|---|
| pp | 1-39 | 32 | 0.15 |
| p | 40-54 | 47 | 0.30 |
| mp | 55-69 | 62 | 0.45 |
| mf | 70-84 | 76 | 0.60 |
| f | 85-104 | 95 | 0.75 |
| ff | 105-127 | 115 | 0.90 |

All values are [Estimate: evenly spaced; calibrate against library loudness, B2].

**What `dynamics` means.** Layer (b) defines the parameter ([sound-source.md](sound-source.md) 5.1,
5.4): 0.0-1.0, default 1.0 = no change. In `velocity` mode (the VSCO and VCSL profiles) a note-on
plays the effective velocity `max(1, round(v · d))`, which picks the layer and the gain, and a held
note follows `(d / d₀)²` from its note-on value `d₀`, capped at 4 (+12 dB). In `cc` mode `d · 127`
drives the profile's controller crossfade, an absolute level.

- **Velocity mode: the written velocity is the level a note reaches.** Within a phrase, notes are
  written at the level velocity of the phrase's peak (then humanized, section 4), and the lane holds
  `level velocity ÷ written velocity` (≤ 1). The lane follows the arc without jumps between notes,
  and each note-on lands on its level's velocity, so the layer chosen at note-on is the level's
  layer (`SoundCaps.layers`; humanization is clamped inside it): for VSCO's two-layer violin (0-62,
  63-127), pp to mp sound the soft layer and mf to ff the loud one. A held note's crescendo can at most double `d` (+12 dB); a longer
  one (a roll from pp to ff) becomes a re-attack at the higher velocity.
- **`fixed` mode** (organs): the sampler plays the profile's velocity, not the written one, so the
  lane alone carries the level, as a ratio to that velocity (layer (b) 5.4).
- **`cc` mode** writes the table's last column and keeps each note at its level velocity.

Shapes, all in markings and converted by the rules above:

- **Phrases.** A boundary is a rest of at least one beat or 0.6 s, a section marker, or a note at least
  twice the median length followed by a gap; at most 8 bars.
- **Phrase arc.** Start at the passage level L, peak one step higher at the phrase's highest note (the
  later one on ties, 60 % of the phrase when the contour is flat), end half a step below L. Three or
  four points per phrase, `smooth` curves.
- **Swell** (`crescendo_swell`, requested or on a held harmony note of at least two beats when the
  request says "swell"): L, then one step up at 60 % of the note, then L.
- **Timpani roll crescendo.** From the start level to the target, back-loaded: a point at 70 % of
  the roll at one third of the rise, then `smooth` to the target. The timpani card, after Forsyth,
  places the audible change of a roll within about eight bars of moderato at its end
  ([../cards/tuned_percussion.md](../cards/tuned_percussion.md)); the curve shape is an Estimate.
- **Accompaniment one step below the melody.** Harmony, bass and pedal parts start one marking below
  the melody. That realizes the "one degree" correction of
  [../doubling-and-balance.md](../doubling-and-balance.md) 3.3 and keeps BAL-03 satisfied.
- **Orchestral crescendo across families** adds strings, then wood-wind, then brass, and removes them
  in reverse (TEX-05).
- **Output.** Points `{at, value, curve}` for `<device_id>:dynamics` (S4); a value that must hold at
  a note-on sits two blocks before it, as articulation points do, and layer (b) smooths the per-block
  value. On a device without `dynamics` (a part whose chain ends at the Poly Synth or the Drum Kit,
  degradation step 6), the same shape, converted to dB as `40 · log10(d)` (the sampler's square law;
  an Estimate for the synth), drives the mixer `volume` lane, which is evaluated per sample.

### 4. Humanization and attack compensation

New code in `performance::humanize`, never a change to `transforms::humanize` (GLD-01). Deviations
are drawn in milliseconds and converted at the local tempo.

| Family | Timing σ | Velocity ± | Length ± | Ensemble behaviour |
|---|---|---|---|---|
| String section | 8 ms | 4 | 5 % | notes struck together in one part share one offset ±2 ms |
| Solo string, wood-wind | 6 ms | 5 | 4 % | as above |
| Brass | 8 ms | 5 | 4 % | as above |
| Timpani, percussion | 3 ms | 6 | 0 | |
| Keyboards, harp | 4 ms | 6 | 3 % | chords keep their voicing order |

All [Estimate: chosen to stay below audible flamming; B2]. Invariants (TST-07): a humanized note
never starts before 0 in its clip, never crosses the next note's start in a non-legato voice, never
removes or shrinks a legato overlap below 10 ms, never creates a same-pitch overlap, and never leaves
velocity 1-127.

**Attack compensation.** A slow onset sounds late. `SoundCaps.attack_ms` per articulation is the SFZ
`ampeg_attack` (supported, `sfz.rs:154-187`) plus the sample's onset, measured when the catalog's
deep `verify` decodes the patch, off the real-time thread (layer (b) SS-35); a profile's `attack_ms`
overrides it (layer (b) 2.2). Long notes are
advanced by it, shorts are not; a legato note after the first fades in over `crossfade_ms` past its
sample's attack (layer (b) 6.3), so it is advanced by half that [Estimate: half the measured attack keeps the perceived onset near the beat; design judgement, to be tuned by listening]. A note at clip start
cannot move earlier (`midi.rs:53`), so generated clips start one beat before the material when there
is room; otherwise the first note is reported as uncompensated.

### 5. Register, voicing and doubling helpers by family

Each helper is a pure function over a chord or line plus the ensemble's parts, and returns notes per
part. Sources are the tables of [../doubling-and-balance.md](../doubling-and-balance.md).

| Helper | Behaviour | Rules applied while writing |
|---|---|---|
| `melody_doublings` | picks carriers from the function table (section 1) for the melody's band, then named doublings by density: thin (one colour), medium (unison or octave), full (the three-group unisons) | RNG-04, DBL-02, DBL-03, DBL-04, DBL-08, DBL-09 |
| `strings_divisi` | divides every upper section the same way, prefers Vns II with violas, at most three ways at forte, never octave divisi in violas, cellos or basses, at most two sustained notes per part | VOI-10, VOI-11, DBL-05, DBL-06, VOI-12 |
| `woodwind_pairs` | superposition by default; interlocking Ob 1, Cl 1, Ob 2, Cl 2 for background; enclosure only between different colours; concords in one colour, discords in two | VOI-01 to VOI-06 |
| `horns` | four-part close harmony, order from the top 1, 3, 2, 4; each player keeps a tessitura; the bass doubled by a bassoon when one is present | VOI-07, PLY-13 |
| `brass_chorale` | close order; trumpets on top; tenor trombones with tuba in octaves as the bass, or two horns in unison on a mixed part | SPC-01, BAL-02 |
| `timpani` | two drums on the tonic and dominant of the passage; hits on strong beats of chords that contain a tuned pitch; rolls on held tonic or dominant pedals | PLY-11, PLY-12 |
| `fit_register` | moves a harmony voice by octaves into the comfortable range; never moves a melody silently (a question instead) | RNG-01, RNG-02 |

### 6. Range and playability checks, by rule id

`performance::check` implements predicates for the rule ids of
[../doubling-and-balance.md](../doubling-and-balance.md) 6.2; the rule table (id, severity,
provenance) is data in `orchestration::rules` (S1). Rules are used in two modes: **generative** (the
layer applies a rule while writing parts, because it knows each part's role, part name and level)
and **diagnostic** (`sonara_app::quality` checks a project, which in phase 1 stores none of these,
A1). The first wave is chosen by what its inputs need ([../doubling-and-balance.md](../doubling-and-balance.md)
6.1); the three rows cover all 67 rules:

| Wave | Rules | Inputs | Why |
|---|---|---|---|
| 1, diagnostic and generative (O4) | RNG-01, RNG-02, RNG-03, SPC-01, SPC-02, DBL-02, DBL-03, DBL-04, DBL-09, VOI-12, PLY-01, PLY-02, PLY-04, PLY-05, PLY-06, PLY-12, PLY-14, PLY-15, TEX-01, TEX-02 | id, notes, tempo; the level from velocity bands and the `dynamics` lane; the articulation from the `articulation` lane | every input exists on a phase-1 track whose id is known; RNG rules rest on `check_sounding` and Cited ranges; the errors (RNG-01) are unambiguous |
| 1, generative only (O4) | RNG-04, SPC-03, SPC-04, SPC-07, DBL-05, DBL-06, DBL-08, VOI-01 to VOI-07, VOI-10, VOI-11, BAL-02, BAL-03, PLY-11, PLY-13, TEX-05 | plus role, part, harmony and dynamic, which the layer itself chose | the layer created the parts, so these inputs are facts inside the plan; the helpers of section 5 and the walkthrough need them |
| 2 (after A1 or O6) | BAL-01, BAL-04 to BAL-09, MSK-01 to MSK-04, SPC-05, SPC-06, SPC-08, DBL-01, DBL-07, VOI-08, VOI-09, PLY-03, PLY-07 to PLY-10, TEX-03, TEX-04, TEX-06, and the generative-only rules in diagnostic mode | part, role, harmony analysis, harp pedals, timpani tuning, slur groups | the project has no place for them in phase 1 (A1); a chord analyser is an Estimate (6.1); several thresholds are Estimates without an oracle (open question 9 there) |

PLY-04 (mute change) and PLY-05 (arco and pizzicato) are wave 1 although
[../doubling-and-balance.md](../doubling-and-balance.md) 6.2 lists `part` as PLY-04's input: phase 1
carries the mute state as a mute-kind choice and the pizzicato as a choice on the `articulation`
lane (layer (b) 5.7), so both are read from that lane, and their times are layer (a)'s `MuteChange`
and `ArcoPizzSwitch` facts; layer (d)'s mute control shows PLY-04.

**Derived inputs (O4, PF-6).** `check::harp_pedals(notes, tempo)` derives a pedal setting per passage
(the fewest changes that cover the notes; the enharmonic choice is an Estimate) and
`check::timpani_tunings(notes, drums)` the drum tunings per passage, both returning the passages they
cannot cover. They are pure, feed the generative helpers of section 5 and layer (d)'s harp and timpani
views (its UI-20). PLY-07, PLY-08 and PLY-11 in diagnostic mode stay wave 2: a derived setting is a
guess where the rules assume the player's stored one (A1, O6).

Diagnostic identity needs the track's orchestration id, in the order and with the words of layer (b)
3.3: *part* (O6), *named* (`instrument_type` not `auto`), *declared* (library manifest), *profile*
(shipped profile; S3, S7, which land in O3), *inferred*. A track whose id is only **inferred** (an
alias match, or a Poly Synth or Drum Kit track name, S3) has every finding lowered one severity step
and labelled inferred. A rule whose threshold is an Estimate is never stronger than a warning.

### 7. Determinism

- One request seed (default 0, as `generate_midi`'s `seed`). Each part draws from its own
  `PyRandom`, seeded through MT19937's key array `[seed low word, seed high word, the slug's byte
  length, the part slug's UTF-8 bytes packed into u32 words]` (injective by construction) by a new constructor `PyRandom::with_key` beside `new`
  (`pyrandom.rs:29-41` already calls `Mt::new_with_key`). A part's slug (`horn_3`) does not depend on
  the order of parts, so adding a part never changes another part's humanization; the seed's two's
  complement words keep negative seeds distinct (`new` drops the sign).
- No hash function of our own (QG-13: `rand_mt`'s key seeding does the mixing; QG-10: no new
  dependency). `PyRandom::new` and its golden streams are unchanged (GLD-01).
- No clock, no environment, no `HashMap` iteration in outputs (`BTreeMap` or sorted vectors),
  single-threaded. Identical project state and request give identical plans (TST-04).

### 8. Orchestrate from a reduction

Input: a `Reduction { melody, harmony: Vec<ChordSlot>, bass: Option<Vec<MidiNote>> }`, an ensemble
(S9 template or the parts the request names) and options (density thin, medium or full; base
marking; seed). A reduction comes from a sketch track: the top note of each onset is the melody
(skyline), the rest groups by onset into chords matched against `theory`'s chord-quality table
(`theory.rs:276`, made `pub(crate)`, MOD-04), the lowest note of each chord is the bass. Chord
recognition and key finding are [Estimate: a pitch-class match; the planner asks when two keys or
chords fit equally].

1. **Analyse.** Phrases (section 3), the melody's band per phrase (`sub` to `top`, 0.1 there), the
   harmonic rhythm, a bass from `theory::bass_note` when none is given.
2. **Assign functions.** Explicit assignments in the request win ("strings play the melody"). Other
   functions take carriers from the function table in preference order, among the ensemble's parts.
3. **Double.** `melody_doublings` by density; the bass by [../doubling-and-balance.md](../doubling-and-balance.md)
   2.7 (cellos and basses in octaves by default).
4. **Voice.** `theory::voice` gives smooth voice leading of the chord tones; the family helpers of
   section 5 distribute them to parts; `fit_register` corrects harmony voices.
5. **Timpani and percussion.** Tuning on tonic and dominant; hits and rolls as in section 5.
6. **Perform.** Articulation inference, dynamics shaping, humanization per part.
7. **Check.** Wave-1 rules. Deterministic fixes are applied (octave moves of harmony voices, a
   staggered breath for PLY-01); everything else is reported.
8. **Emit.** An `Orchestration` value: parts (id, part name, players, role), performed notes,
   passages, dynamics points, findings and the questions that blocked anything.

The function is O(n log n) in notes per part, and its output is plain data that the agent planner, a
GUI service or a test can consume.

### 9. Agent side: the planner grammar

`sonara_agent::orchestration` becomes a module directory: `mod.rs` (today's planner, unchanged for
its three request kinds), `grammar.rs`, `refs.rs` (section 10) and `ensemble.rs` (an `Orchestration`
to steps). The orchestration grammar runs **before** `split_clauses`, because that function splits
at "and".

```text
Sketch (not implemented): orchestration grammar
request   := clause ((";" | "," | "then" | "potom") clause)*
clause    := tempo | delete | use_instrument | orch            (the first three as today)
orch      := groups verb? role modifier*
groups    := group (("and" | "a" | "i" | "+" | ",") group)*      (a list only if every member is a group)
group     := count? alias ("section" | "solo" | "sekce" | "sólo")? ("with" | "s" | "se") alias?
role      := melody | countermelody | harmony | bass | pedal | rhythm | "underneath" | "pod tím"
modifier  := articulation | dynamic | range | key | density
```

- **Aliases** are layer (a)'s: the grammar calls `sonara_midi::orchestration::resolve()` (its
  section 3: folding, leftmost-longest matching, every target of an ambiguous word) and works on the
  returned `Mention`s; it does no folding of its own (DES-01). Czech inflection is listed forms in
  that data (`lesní roh`, `lesní rohy`, `lesních rohů`, `lesními rohy`; `tympány`, `tympánů`,
  `kotle`; `smyčce`, `smyčců`), not a stemmer, so "tympany" matches "tympány" through `fold()`.
  The forms this grammar needs are added to layer (a)'s IM-8 data.
- **Catalog access.** No tool reaches presets, profiles or the S8 chain ([../sfz-conformance.md](../sfz-conformance.md)
  C6), and a new one is a CTR-01 change. The runner seam gains a read-only catalog query, implemented
  for `AgentService` through `AgentService::app()` (`service.rs:250`) and stubbed in tests (TRT-04).
  All writes still go through tools.
- **Categories and defaults.** "strings" means the ensemble's string parts (Vns I, Vns II, Va, Vc,
  Cb); "horns" means `horn_f`, four parts in a romantic ensemble and two in a classical one; "with"
  joins a doubling ("violins with flute" is a unison doubling, reported with its colour from 2.1).
- **Roles.** "melody", "tune", "theme", "melodii", "téma"; "underneath", "under", "accompany",
  "doprovod", "pod tím" mean harmony for sustaining instruments and bass or pedal for timpani;
  "chords", "akordy"; "bass line", "basovou linku".
- **Modifiers.** Articulation words resolve through the vocabulary (`legato`, `staccato`, `pizz`,
  "držené"); dynamics ("softly", "potichu", pp to ff, "crescendo", "zesiluje", "swell"); ranges ("in
  bars 5-12", "v taktech 5–12"); a key ("in D minor", "v d moll"); density ("full", "plně").
- **Clarification questions** (nothing is changed while any is open, as today):
  1. An unknown word in an instrument position: "I don't know 'saxhorn'. Did you mean …?"
     (aliases within edit distance 2, computed with `strsim`, MIT/Apache-2.0, already in the lock
     file through clap; a new direct dependency edge of `sonara-agent`, recorded under QG-10; for
     suggestions only, never for matching, layer (a) section 3).
  2. "horns" where nothing else is orchestral: "French horns, or a brass and sax section?"
     ([../cards/apple.md](../cards/apple.md) 2.3 shows the second meaning).
  3. No material: "Which clip holds the melody? Or ask the AI Composer to write one."
  4. A melody outside the playable range of the requested instrument (RNG-01): "Bars 6-7 reach C7,
     above the violas. Give the line to the violins, or move it down an octave?"
  5. No sound at all for an id (degradation step 7): asked before a silent track is created.
  6. More timpani pitches than drums with no time to retune (PLY-11 error).

### 10. Agent side: references and staged validation

The planner cannot express `create_track` followed by `create_midi_clip` on the new track: `Step.args`
is fixed at plan time and every step is dry-run before the first commit. Proposal:

```rust
// Sketch (not implemented): crates/sonara-agent/src/orchestration/refs.rs
pub struct StepRef { pub arg: &'static str, pub step: usize, pub pointer: &'static str }
// e.g. StepRef { arg: "track_id", step: 4, pointer: "/result/track_id" }  // JSON pointer into the reply
//      StepRef { arg: "target", step: 5, pointer: "/result/device_id" }   // formats "<id>:dynamics"
// Step gains `refs: Vec<StepRef>`; StepResult gains `reply: Option<Value>`.
```

1. **Stage 0, before any commit.** Every step deserializes into its contract argument struct
   (`deny_unknown_fields`; a reference is a placeholder string); every note passes `MidiNote::new`;
   every `instrument` value and every articulation index exists in the descriptor from
   `list_plugins {plugin_id}`; every automation target is automatable; steps without references are
   dry-run as today. A failure here is `Rejected`, and nothing changed (the existing guarantee).
2. **Stage 1, commit loop.** A step with references is resolved from earlier replies, dry-run, then
   committed.
3. **Rollback.** If a stage-1 dry-run or commit fails, the planner calls `undo` once per committed
   step, newest first; no two plan steps share a merge key (the keys name the track or device,
   `undo.rs:61-70`), so each is one undo entry. Before each `undo`, `get_history` must show the
   label its own step produced; the tool returns labels and the audit tail but no source per step
   (`crates/sonara-agent/src/handlers/history.rs:28-50`), so a user edit with another label stops the
   rollback and is reported, while one with the same label, or a user pan change merged within 1.5 s,
   cannot be told apart (Risks). The status is `Failed` with the rolled-back steps listed. When the
   GUI wraps `execute` in `sonara_app::edit::begin_undo_group`/`end_undo_group`
   (in main since 0.1.12), a plan becomes one step "Orchestrate"; inside the group `undo`
   still undoes one step, so rollback works there too.

The guarantee in `docs/orchestration.md:30-31` ("A refusal changes nothing") and
`orchestration.rs:23-25` is rewritten (DOC-01). One request is 67 calls (section 12), each through
permissions and the audit log: the price of using no new tool (CTR-01).

### 11. The AI Composer's orchestration reference and quality findings

**Reference.** `sonara_midi::orchestration::reference::markdown()` renders, from the S1 statics,
the instrument table (id, English and Czech name, family, sounding playable and comfortable range in
MIDI numbers and names, transposition, solo or section, typical roles), the ensembles and string
sizes, the condensed function table, the named doublings and the "do not" table (2.1, 2.8), the
voicing rules for strings, wood-wind and brass (5.2-5.4), the weights (3.3) and the wave-1 rules (one
line each). `sonara_app` adds the shipped profile map (which VSCO 2 CE and VCSL preset plays which
id, and the instruction to confirm it in `list_plugins` choices) and a "how to perform in Sonara"
block: one track per part, `set_instrument` before automation, the articulation choice index from
`list_plugins {plugin_id}`, points two blocks early and after the previous passage's last note,
legato as overlaps of 10-30 ms never on a repeated pitch, notes written at the phrase's peak velocity
and shaped down with `dynamics` (≤ 1).

- **Size.** Budget 24,000 characters (about six times today's prompt), enforced by a test that
  fails the build; if exceeded, the profile map and then the comfortable ranges are shortened
  first. The host's `ARG_MAX` is 1,048,576 bytes (`getconf ARG_MAX`), so the argument fits; the real
  cost is that every turn carries it.
- **No drift.** The reference is rendered at run start from the same statics the planner uses, and
  appended to `SYSTEM_PROMPT`. A checked-in copy `docs/orchestra/composer-reference.md` is
  regenerated by an example whose test fails while the committed file differs (the pattern of
  `crates/sonara-agent/examples/gen_agent_docs.rs:1-11`), so a data change that is not regenerated
  fails CI (DOC-01).

**Quality.** `crates/sonara-app/src/quality.rs` becomes `quality/mod.rs` plus
`quality/orchestration.rs` with `check_orchestration(project, identity)`; `identity` maps a track to
its orchestration id, ensemble and provenance (*named*, *declared*, *profile*, *inferred*; layer (b)
3.3) through layer (b)'s resolver. A finding's `code` is the rule id (a `&'static str` from the rule table; DES-01); error,
warning, hint map to `Error`, `Warning`, `Recommendation`; the explanation names the source and any
Estimate. Existing codes are unchanged. Consumers: the GUI (O5) and tests; no tool or CLI (CTR-01).

### 12. End-to-end walkthrough: "strings play a legato melody, horns and timpani underneath"

**Setting.** Phase 1 means O1 to O5 have landed (no contract change). The open project is at 72
BPM, 4/4, with a track "Sketch" holding an 8-bar piano clip "Theme" in D minor: a right-hand melody
between A4 and A5 (69-81), chords Dm, B♭, Gm, A, Dm, Gm, A7, Dm. VSCO 2 CE (branch `SFZ`) and VCSL
(tag `v1.2.2-RC`) are installed with their shipped profiles (S7). The user types the sentence into the
Orchestrate field (O5), which calls the planner in-process. Preset ids follow layer (b)'s sketch
profile.

**1. Parse.** Clause 1: group `strings` (category), role melody, modifier `legato`. Clause 2: group
list `horn_f`, `timpani` ("and" joins two groups), role "underneath". No questions.

**2. Read** (read tools only): `get_project_state {detail: "full", include_notes: true}` (tempo,
tracks, the Sketch notes), `list_tracks {}`, `list_plugins {plugin_id: "sonara.sampler"}` (the
`instrument`, `articulation` and `dynamics` parameters with their choices); the catalog through the
seam of section 9.

**3. Reduce and orchestrate** (`performance::reduction`, seed 0):

| Part | Material | Why |
|---|---|---|
| Violins I, Violins II | the melody in unison, A4-A5 | Vns I + II in unison: fuller, no change of colour (2.3) |
| Violas, Cellos | the melody an octave lower, A3-A4 (57-69) | Vns I + II over Va + Vc in octaves suits middle-range melodies (2.3); inside both comfortable ranges ([../model.md](../model.md)) |
| Double basses | tacet, reported | no bass was asked of the strings; Horn 4 carries the bass (four horns need no doubled bass, 5.4) |
| Horns 1-4 | the chords in close four parts, top to bottom 1, 3, 2, 4; Dm = F4, D4, A3, D3 (65, 62, 57, 50); Gm = B♭4, G4, D4, G3 (70, 67, 62, 55) | VOI-07; wide intervals below, close above (4.1); all inside the comfortable F2-C5 (41-72); Horns 1 and 2 breathe at the end of bar 4, Horns 3 and 4 at the end of bar 5, so no part nears PLY-01's 8-bar limit |
| Timpani | drums tuned D2 and A2 (38, 45): hits on beat 1 of bars 1 (D), 4 (A), 5 (D), 7 (A); a roll on D2 through bar 8 | PLY-11 (two drums, tonic and dominant), inside the comfortable D2-A3 (38-57) |

**4. Perform.** Strings: one `legato` passage, overlaps of 30 ms, none between repeated pitches; the
sampler crossfades over the profile's `crossfade_ms`. Two phrase arcs from mf: notes written at f
(95), the arc's peak on the A5 of bar 7, with the lane from 76/95 = 0.80 up to 1.0 and down to
69/95 = 0.73 (half a step below mf), so every note-on lands at 69-95, in VSCO's loud layer. Horns:
ordinario long at mp (62), one step below the melody. BAL-03 holds: the four string departments
carrying the melody weigh 4 × 1.4 = 5.6 against the four horns at 4 × 1.4 × 0.5 = 2.8, a ratio of 2.0
over the 1.5 margin ([../doubling-and-balance.md](../doubling-and-balance.md) 3.3, Estimate
arithmetic). The bar-7 swell writes the horns' A7 at mf (76) with the lane 0.82 → 1.0 → 0.82 (+3.5 dB
at its peak). Timpani: every note written at f (95) with the lane held at 0.65 (effective 62, mp)
through the hits, so no lane step dips a ringing hit; it rises to 1.0 only during the roll,
back-loaded: +7.4 dB, inside the +12 dB bound. Humanization per section 4.

**5. Choose sounds** (degradation chain, S8). Violins I and II: the VSCO violin-section preset that
the profile marks `legato: overlap`, `vsco2ce/ViolinEns-KS` in layer (b)'s sketch (it plays once O2
fixes Q1; its sustain group uses the same `susVib` samples and 0-62/63-127 layers as
`ViolinEnsSusVib`); the `legato` choice resolves to that sustain with overlap legato (layer (b) 5.2,
6.3). Violas and cellos: the matching `ViolaEns` and `CelloEns` presets (the cello's switch keys
84-87 lie above this part, [../sound-mapping.md](../sound-mapping.md) 2.1). Horns:
`vsco2ce/@horn-solo` per part, layer (b)'s set of `FHornSus`, `FHornStac` and `FHornMute` (step 3:
"No horn section is installed; each horn part is one solo horn"; the Sonatina section is not
profiled, A17). Timpani: both libraries match, but if VCSL's `Timpani 1 - Hit` and `- Roll` are
sampled only from key 41, as its key-switch file is ([../sound-mapping.md](../sound-mapping.md) 2.3),
D2 cannot sound there, so the sampled keys select the set `vsco2ce/@timpani` (`Timpani` keys 36-60,
[../cards/tuned_percussion.md](../cards/tuned_percussion.md)), whose `TimpaniRolls` member is the
`roll` articulation (S6.4). Every substitution is reported.

**6. Check.** Wave 1 passes with at most a hint: RNG-01 and RNG-02 for every part; SPC-01 and SPC-02
among the horns for the voicings shown (a fifth, a fourth and a minor third whose lower notes, D3 or
G3 and up, clear the 4.1 table and are not below MIDI 50; the other six chords are checked the same
way); SPC-02 may give a **hint** (two families) for the timpani's A2 under a horn G in the A7 of bar
7, which the report lists; SPC-03 (Horn 4 never lies more than an octave under Horn 2); PLY-01;
PLY-02 (no loud or high brass); PLY-12 (no thirds on the drums). `max_hold_s` is respected: the
longest string note, a half note at 72 BPM, lasts 1.7 s against the 11.2 s of the VSCO sustain sample
measured, and the 3.3 s roll is checked against the longest `TimpaniRolls` sample.

**7. Tool calls** (`$Vn1.track`, `$Vn1.dev` are references to earlier replies; 3 reads, 64 writes).
Folders, bus, part names, order and pan come from the ensemble plan service (PF-20), which layer
(d)'s Add Ensemble… applies as one `Transaction` and this planner turns into tool steps (DES-01):

| # | Tool | Arguments (abridged) |
|---|---|---|
| 1-3 | `create_track` ×3 | `{name: "Strings" \| "Brass" \| "Percussion", kind: "folder"}` |
| 4 | `create_track` | `{name: "Hall", kind: "bus"}` (only when the project has no reverb bus) |
| 5 | `add_effect` | `{track_id: $Hall.track, plugin_id: "sonara.reverb", params: {mix: 1.0, decay_s: 2.4}}` |
| per part, ×9 | `create_track` | `{name: "Violins I", kind: "instrument", parent_id: $Strings.track}`: no `instrument`, so the cheap Poly Synth placeholder is replaced next instead of loading a first patch |
| | `set_instrument` | `{track_id: $Vn1.track, plugin_id: "sonara.sampler", params: {instrument: "vsco2ce/ViolinEns-KS"}}` (S5 preset id; a path before S5) → `device_id` |
| | `create_midi_clip` | `{track_id, start: 0, length: 33, name: "Theme", notes: [...]}`: the length covers the humanized end of the last notes, because note-offs are clamped to the clip end (`schedule.rs:6-8`); release tails sound past it anyway |
| | `create_automation` | `{track_id, target: "$Vn1.dev:dynamics", points: [{at: 0, value: 0.8, curve: "smooth"}, ...]}` |
| | `set_track_pan` | Vns I −0.55, Vns II −0.30, Va +0.10, Vc +0.35, Horns −0.35, Timp −0.10 ([../ensembles.md](../ensembles.md) 2.5, Estimates; skipped for an in-situ profile) |
| | `configure_routing` | `{track_id, add_sends: [{target: $Hall.track, level_db: -12, pre_fader: false}]}` |
| strings ×4, timpani | `create_automation` | `{target: "$Vn1.dev:articulation", points: [{at: 0, value: <index of legato>, curve: "hold"}]}`; timpani: `ordinario` at 0, `roll` two blocks before beat 28 |

`set_instrument` must precede `create_automation`, because it creates a new device and drops lanes of
the old one. The report ends: "Created 9 tracks in 3 folders and a Hall bus. Horn section → four solo
horns. Timpani from VSCO (VCSL's are sampled from F2). Basses tacet. Legato is an overlap crossfade of
sustain samples."

**What the user hears (phase 1).** A string line in the right register, all sections in the same
bow, with gentle phrase swells. Each new note crossfades out of the previous one, starting past its
own sample's attack: bow changes are softened, not hidden, with no portamento or recorded transition
(A9, B2). The timbre changes only between notes whose effective velocities cross VSCO's 62/63
boundary. Four solo horns give no a2 section weight, and the bar-7 swell is a level change. Timpani
hits are pitched D and A; the roll grows from mp to f in level only (a stepped re-attack would
brighten it with a seam, an option behind B2). Panning places the sections; the Hall bus adds space.

**Parts without the new parameters.** A Poly Synth or Drum Kit part (degradation step 6), or a
sampler preset without overlap legato (VSCO's `SusVib` patches set no `group`), would sound overlaps
polyphonically, each with its full attack: the planner writes none, says "legato is not available
with this sound", and drives dynamics with the mixer `volume` lane. Hits and rolls from unrelated
patches need two tracks, because `instrument` is not automatable (sfz-conformance C2); a set avoids it.

**AI Composer variant.** The composer writes its own melody and chords (it has no planner tool) and
follows the reference: the same parts, ranges, voicing, tool order and automation. Its result is not
deterministic. The GUI's findings (layer (d)'s UI-12, O5) check the project it leaves like any other;
the composer itself cannot read them until a `check_orchestration` tool exists (O6).

### 13. What needs phase 2 (O6), and why phase 1 lives without it

| Change (CTR-01, versioned; owner) | What it adds | Phase-1 substitute |
|---|---|---|
| Per-note `articulation` in schema v2 and `NoteInput` (S4; IM-19, PF-18) | mixed ids inside a passage on one track (a legato phrase ending staccato), divisi with different articulations on one track, no block granularity, articulations that travel with copied clips | passages on the sampler's `articulation` lane; divisi split into tracks |
| A tool that writes MIDI CC (`controls`) into clips (A4; PF-21) | dynamics that move with the clip; CC 1 and CC 11 for hosted plugins (M5), which have no Sonara `dynamics` parameter; it also needs controller events in looping clips, which the schedule drops today (`schedule.rs:8`) | the `dynamics` lane of the sampler |
| `orchestrate` and `check_orchestration` tools (S10; PF-19) | MCP agents and the AI Composer get the deterministic planner and the checks instead of re-implementing them | the GUI field; the reference in the composer's prompt |
| Track `part` in schema v2 (S3, A1; IM-19) | part, role and players survive reopening, so wave-2 rules run on any project | the layer knows them while generating; names show "inferred" |
| An ensemble tool or wider `create_project` enum (S9, A7; PF-22) | one call instead of ~60 | the planner's tool sequence |

Phase 1 lives without these: the sampler is not a golden built-in, so new automatable parameters
change no contract (ADR 0010 decision 4, GLD-01); `create_automation` reaches any automatable device
parameter; one track per part (A2) gives every part its own articulation and dynamics state.

**With M5** (plugin hosting), commercial libraries add recorded legato triggered by the same
overlaps, CC 1 crossfades and real horn sections; the profile maps `dynamics` and articulations to CC
and key switches at output ([../cards/apple.md](../cards/apple.md) 3.1), through the CC tool or a host-side mapping.

## Contracts and rules

| Rule id | How the proposal holds it |
|---|---|
| CTR-01 | No tool, schema, IPC or CLI change in phase 1: only existing tools, library APIs and a GUI field; the catalog is read as a library, not through a new tool; the five contract changes are listed as O6 owner decisions (section 13) |
| GLD-01 | `transforms::humanize` and `legato` stay unchanged; `PyRandom::new` is untouched (only `with_key` is added); the `orchestral` template and Ascension are untouched; the sampler is not in `builtin_registry()`; existing quality codes stay; no golden file is re-recorded |
| LIC-01 | Profiles are our own data; test fixtures are hand-written SFZ and notes, copying no library text or samples (ADR 0010: nothing bundled); `unicode-normalization` and `strsim` (both MIT/Apache-2.0) are on the allow-list |
| HW-01 | Listening checks (B2) are local and recorded in the docs, never CI gates |
| DOC-01 | `docs/orchestration.md` (grammar, references, rollback, the rewritten guarantee), `docs/ai-composer.md` (reference), a new `docs/performance.md`, `docs/dependencies.md` (`sonara-agent` as a user of `strsim`), `CHANGELOG.md`, `docs/implementation-status.md` |
| ARC-11 | `performance` in `sonara-midi` (depends on `sonara-core` only); adapters and the ensemble plan service (`sonara_app::ensembles`, PF-20) in `sonara-app`; planner in `sonara-agent` (may use `sonara-app`, `-midi`, `-core`); the desktop calls `sonara_app::edit`; no new edge in the architecture test |
| RT-01 to RT-05 | No code of this layer runs on an audio thread; it requires of the sampler only the automatable parameters of layer (b), implemented there under RT-02 to RT-05; attack onsets are measured at `verify` (SS-35), off the real-time thread. The one exception is O6's PF-21: keeping controller events in looping clips changes `sonara-engine::schedule`, a real-time module whose `NoteSchedule::build` is setup (`schedule.rs:1-2`), so the events are placed at build and the per-block lookup stays allocation-free (RT-02, RT-05) |
| RT-10 | No `unsafe`; `sonara-midi` and `sonara-agent` keep `#![forbid(unsafe_code)]` |
| MOD-09 | No new crate |
| QG-03, QG-05 | No `unwrap` in production paths; every public item documented with its errors |
| QG-09, QG-13, QG-10 | Tests per section "Tests"; `rand_mt` key seeding instead of own hash code; folding is layer (a)'s `fold()` over `unicode-normalization`; `strsim` for suggestions (a new edge to a crate already in the lock file, under the one reading of layer (a)'s Deviation 5); a stemmer was considered and not needed (listed inflections) |
| TRT-04, TYP-01, TYP-02, ERR-03 | `SoundCaps` is data, not a trait; the catalog seam is a test seam; `PartId`, `Role`, `Seed` are domain types; `PerformanceError` is a typed enum |
| TST-04 | Streams keyed by part slug; no clock, no hash-order output |
| QG-11, PERF-10 | No real-time cost; the GUI field and lanes fall under layer (d)'s ADR 0009 measurements; the plan's wall time (64 commits, nine patch loads) is measured before PF-15 ships |

## Alternatives considered

- **A live MIDI "performer" in the engine** (like a MIDI effect before the instrument). Rejected:
  it runs on the render thread (RT-02 to RT-05), its output is not editable notes, and the Apple
  card's lesson is that generated parts must convert to plain MIDI ([../cards/apple.md](../cards/apple.md) 7).
- **The layer in `sonara-app`** (rejected: pure logic, ARC-07) or **a new crate `sonara-orchestra`**
  (rejected: no compilation, distribution or safety boundary, MOD-09).
- **Changing `transforms::humanize` to work in milliseconds.** Rejected: GLD-01 pins its output.
- **Key-switch notes written into clips** (A3 option a). Kept only for imported material: the
  spine's `articulation` parameter (S4) avoids switch notes inside the user's MIDI and survives a
  seek (sfz-conformance C11).
- **Velocity-only dynamics.** Kept as the level at note-on, but it cannot shape a held note.
- **`dynamics` as an absolute level in velocity mode.** Rejected: layer (b) defines a ratio with 1.0
  as "no change" (today's renders stay identical); an absolute reading scales velocities twice.
- **Dry-run everything by simulating new ids.** Rejected: the session mints ids; a guess is no check.
- **A language model inside the planner.** Rejected: the planner stays deterministic; the AI Composer
  is the language-model path.
- **A Czech stemmer instead of inflected aliases.** Rejected for now: listed forms are exact and
  reviewable. **Own FNV and splitmix hashes for part seeds:** rejected under QG-13 (`rand_mt`'s key
  seeding does it).

## Implementation plan

| Work item | Milestone | Crate/module | Size | Depends on |
|---|---|---|---|---|
| PF-1 Types, `perform()`, per-part streams (`PyRandom::with_key`) | O4 | `sonara-midi` `performance::{mod, seed}`, `pyrandom` | S | IM-1, IM-6 (85 ids, vocabulary) |
| PF-2 Articulation inference and passages, choice-index points | O4 | `performance::articulate` | M | PF-1; SS-14, SS-16 (`articulation` parameter) |
| PF-3 Level table, phrases, arcs, swells, roll crescendos, written velocities and lane ratios | O4 | `performance::dynamics` | M | PF-1; SS-14, SS-21 (`dynamics`, layer (b) 5.4) |
| PF-4 Humanization per family, attack compensation | O4 | `performance::humanize` | S | PF-1, PF-9 |
| PF-5 Voicing and doubling helpers by family | O4 | `performance::voicing` | L | IM-13, IM-14 (ensembles, `orchestration::rules`) |
| PF-6 Wave-1 rule predicates; harp-pedal and timpani-tuning derivations (section 6) | O4 | `performance::check` | M | IM-12, IM-14 |
| PF-7 Orchestrate from a reduction (skyline, chord match, key) | O4 | `performance::reduction`; `theory` (`CHORD_QUALITIES` to `pub(crate)`) | L | PF-2-PF-6 |
| PF-8 (Moved to layer (b)'s SS-35, which measures onsets and the longest unlooped sample at `verify`; kept for traceability) | O3 | – | – | – |
| PF-9 Catalog → `SoundCaps` adapter and the planner's catalog seam | O3 | `sonara-app` `instruments`; `sonara-agent` `orchestration` | S | SS-09, SS-13, SS-35 |
| PF-10 Orchestration findings | O4 | `sonara-app` `quality::orchestration` | M | PF-6, PF-9 |
| PF-11 Grammar over layer (a)'s `resolve()` mentions, suggestions (`strsim`), questions | O4 | `sonara-agent` `orchestration::grammar` | L | IM-8, IM-9 |
| PF-12 Step references, staged validation, rollback, report | O4 | `sonara-agent` `orchestration::refs`, `mod` | M | — |
| PF-13 `Orchestration` → steps (tracks, instrument, clips, lanes, routing, pan) | O4 | `sonara-agent` `orchestration::ensemble` | M | PF-7, PF-9, PF-12, PF-20, SS-26 |
| PF-14 Composer reference renderer, profile block, size and drift tests, generator example | O4 | `sonara-midi` `orchestration::reference`; `sonara-app` `composer::invocation` | M | IM-1..IM-14, SS-12 |
| PF-15 Orchestrate field, undo-group wrapping, measured plan wall time (QG-11) | O5 | `apps/sonara-desktop` (`src-tauri`, Svelte) through `sonara_app::edit` | S | PF-13; the undo groups of 0.1.12 |
| PF-16 Docs (DOC-01) | O4 | `docs/` | S | PF-1-PF-14, PF-20 |
| PF-17 Listening checks of thresholds, overlap and crossfade times (B2), recorded locally | O4 | `docs/performance.md` | S | SS-24, PF-2-PF-4 |
| PF-18 Per-note articulation: `NoteInput.articulation` and a `part` argument on `create_track` (golden `tools.json`), per-note inference output | O6 | `performance::articulate`; `sonara-agent` contract | M | IM-19; owner (A1, A3 b, A16) |
| PF-19 `orchestrate` and `check_orchestration` tools, golden contract update | O6 | `sonara-agent` contract | L | owner (S10) |
| PF-20 Ensemble plan service (spine S9): parts in score order, family folders, sources through the chain, pan unless in situ, family buses; one plan for the GUI's `Transaction` and the planner's steps | O4 | `sonara-app` `ensembles` | M | IM-13, SS-26 |
| PF-21 CC-writing tool and CC output of `perform()`; controller events in looping clips | O6 | `sonara-agent` contract; `performance::dynamics`; `sonara-engine` `schedule` | M | owner (A4 a) |
| PF-22 Ensemble tool or a wider `create_project` enum | O6 | `sonara-agent` contract | M | PF-20; owner (A7 c) |

## Tests

- **TST-01** (`performance` units): `inference_rules_are_exhaustive` (a length × duty grid; oracle:
  section 2's list), `harp_pedals_and_timpani_tunings_cover_a_known_passage` (hand-written passage
  and expected settings), `ensemble_plan_orders_parts_by_score_order` (PF-20; oracle:
  [../ensembles.md](../ensembles.md) 1.1), `overlap_at_another_pitch_is_legato`, `level_table_round_trips`,
  `ratio_lane_puts_note_on_at_level_velocity`, `horns_voice_dm_as_f4_d4_a3_d3` (oracle: section 12),
  `with_key_differs_per_slug_and_keeps_new_unchanged` (oracle: the golden `PyRandom::new` streams).
- **TST-02** `walkthrough_strings_legato_horns_timpani` (`crates/sonara-agent/tests/orchestration.rs`):
  the planner against `AgentService` with a hand-written stub library (fixture SFZ, LIC-01) and a
  profile; oracle: 13 new tracks (3 folders, a bus, 9 instrument tracks in score order), 9 clips, the
  lanes of section 12 step 7 in order, and the sampler `instrument` values.
- **TST-03** `each_clarification_question_changes_nothing` and `stage_zero_rejection_changes_nothing`
  (oracle: `get_history` and the project version unchanged); `stage_one_failure_rolls_back` (oracle:
  `get_project_state` equals the state before the run); `user_edit_with_another_label_stops_rollback`;
  per wave-1 rule a `<rule>_fires` and a `<rule>_stays_quiet` fixture in `sonara-app`'s quality tests.
- **TST-04** `same_request_project_and_seed_give_the_same_plan` (serialized plans equal);
  `adding_a_part_keeps_other_parts_notes`.
- **TST-05** `golden/contracts/tools.json` and the golden generator tests stay green;
  `composer_reference_fits_the_budget` (≤ 24,000 characters) and the generator example's test
  (the committed copy equals the rendering).
- **TST-06** `horns_and_timpani_underneath_is_one_clause` (the `split_clauses` case).
- **TST-07** Proptest invariants: `legato_overlap_is_never_on_a_repeated_pitch`; overlaps ≥ 10 ms;
  no note before 0 or after its clip; velocities 1-127; `dynamics` in 0-1 and
  `dynamics_lane_keeps_note_on_in_intended_layer` (`round(v · d)` in the level's layer); held-note
  ratio change ≤ 2; `switch_point_lies_after_previous_passage_last_note_on` and one block before its
  passage unless reported; no melody note moved by `fit_register`.
- **TST-09** Proptest: inference on a line equals inference on the line split at a rest; humanized
  deviations stay within their bounds; voiced parts stay inside playable ranges.
- **TST-10** Oracles: the horn order 1, 3, 2, 4 of Forsyth's Eroica remark (5.4), Rimsky-Korsakov's
  string octave doublings (2.3), and a 4-bar chorale reduction with hand-written expected parts.

## Risks

- **Legato quality.** An overlap crossfade on sustain samples may sound like soft re-attacks (A9).
  Mitigation: B2 listening before the demo; per-profile crossfade time; honest report text.
- **Dynamics are gain within one layer.** Swells and roll crescendos do not change timbre until CC
  crossfades exist (S6.2) or M5, and a held-note crescendo is bounded to +12 dB. Mitigation: the
  written velocity picks the layer at note-on; the stepped re-attack option.
- **Solo samples and blocks.** Four solo horns are a quartet, not a section a4; VSCO's contrabass is
  solo (coverage gap 6). Switches and dynamics land per 512-frame block; points are placed early.
- **Plan size and time.** About 67 calls per request through permissions and audit; without the undo
  group, as many undo steps. Each commit edits the project, and layer (b) 5.6 states that a live
  graph rebuild recreates samplers today; nine `set_instrument` calls load patches. The wall time is
  measured (QG-11) before the GUI field ships.
- **Shared undo stack.** Rollback through `undo` could hit a user's edit; the `get_history` label
  check stops it only when the labels differ, and a user pan change on a planned track within 1.5 s
  merges into the planner's step. In the GUI path, the undo group (PF-15) lets one `undo` take back
  the whole plan instead.
- **Cross-layer coupling.** The overlap rule, the ratio lane and the switch timing rest on layer (b)'s
  definitions (5.4, 5.5, 6.3); a change there changes this layer's output. The TST-07 invariants
  pin the contract between the two.
- **Estimates.** Thresholds, levels, humanization and weights are Estimates; findings label them, and
  none is stronger than a warning. Listed Czech inflections can miss forms (unknown words become
  questions). The composer reference costs 24,000 characters on every turn.

## Owner decisions

- [A3](../open-questions.md#a3-how-articulations-reach-a-library) (option c, chosen by spine S4),
  [A4](../open-questions.md#a4-dynamics-an-automatable-sampler-parameter-or-a-cc-writing-tool) (option b now, a CC tool
  in O6, PF-21) and [A16](../open-questions.md#a16-articulation-vocabulary-flat-list-or-separate-axes-mutes-beaters-vowels-shapes)
  (the shape a per-note field takes, PF-18): the phase-1 transport (one articulation state per track,
  a `dynamics` lane).
- [A9](../open-questions.md#a9-sampler-features-legato-crossfades-velocity-and-pitch-bend) with
  [B2](../open-questions.md#b-human-checks): whether an overlap-crossfade legato is good enough to demonstrate.
- [A14](../open-questions.md#a14-articulation-vocabulary-the-ordinary-note-and-ids-used-in-two-meanings): the ordinario
  rule (no id or `sustain`) that inference rule 6 emits;
  [A13](../open-questions.md#a13-articulation-vocabulary-the-34-invented-ids-and-the-candidate-merges) decides the
  invented ids, which inference never emits.
- [A18](../open-questions.md#a18-what-playable-and-comfortable-mean): the meaning of `comfortable`, which sets RNG-02
  and the voicing targets.
- [A1](../open-questions.md#a1-where-an-instruments-identity-and-part-live-in-a-project): where part and role live
  (wave-2 diagnostics, PF-18); [A2](../open-questions.md#a2-a-doubled-instrument-its-own-track-or-a-switch): a doubled
  instrument on its own track, so one track per part;
  [A7](../open-questions.md#a7-ensemble-templates-gui-only-or-the-create_project-enum): ensembles through the planner
  and PF-20 now, a tool later (PF-22).
- [A17](../open-questions.md#a17-licences-recommending-and-supporting-sonatina--iowa--or-philharmonia-derived-libraries):
  no Sonatina profile means solo horns for "horns".
- **P1 (new, product):** the planner's rollback through `undo` and its rewritten guarantee (see
  "Deviation from the spine"). Recommended.
- **P2 (new, product):** humanization on by default for orchestration. Recommended.
- **P3 (new, product):** the 24,000-character composer reference carried on every turn. Recommended.
- S10 (CTR-01): `orchestrate` and `check_orchestration` (PF-19), a CC writer (PF-21) and an ensemble
  tool (PF-22), for O6.

## Deviation from the spine

One deviation:

- **S10's "dry-run validation".** The spine keeps the planner's existing guarantee that every step
  is validated before the first commit, so a refused plan changes nothing (`orchestration.rs:23-25`,
  `docs/orchestration.md:30-31`). A plan whose later steps use ids that earlier steps mint cannot be
  dry-run before those steps commit, so this layer validates in two stages and rolls back with `undo`
  on a stage-1 failure (section 10). Reason and rules: CTR-01 rules out a new all-or-nothing plan
  tool in phase 1; ERR-06 and ERR-08 rule out a silent partial result, so the report lists every
  rolled-back step; stage 0 keeps "nothing changed" for every refusal it can detect before the first
  commit; TST-03 tests both paths. The owner decides the rewritten guarantee (Owner decisions, P1).

Refinements that stay inside the spine:

- S4's per-preset `articulation` choices: a descriptor belongs to the plugin (`builtin.rs:767-798`),
  so layer (b) declares one frozen list (its own deviation); this layer reads indices from the
  descriptor and uses `ordinario`, never `auto`, for ordinary passages.
- S4's `dynamics` as layer (b) defines it: a ratio to the written velocity, absolute in `cc` mode.
- S7 gains one optional field (an `attack_ms` override, now in layer (b)'s 2.2 table); the other
  facts are derived from the file by layer (b)'s `inspect` and `verify` (SS-09, SS-35).
- S9's "app service" is PF-20 in `sonara-app`: one ensemble plan that layer (d)'s New Project and
  Add Ensemble… apply as one `Transaction` and this planner emits as tool steps.

## Review

Adversarial review of this file (2026-10-30). Changes, one per line:

- Today: the local checkout path removed; line ranges tightened; `pyrandom.rs` shows key seeding; the private `CHORD_QUALITIES` named; `OVERLAPPING_NOTES` is same pitch and channel; `UNIFORM_VELOCITY` made exact; device lanes are read at the block's start beat.
- Today: the gain row cites `sampler.rs:344-345` and the region's SFZ `volume`, and adds that a note-off releases every voice of its key and channel (`sampler.rs:264-292`).
- Today: added `AgentService::app()` and the `agent` source, the undo merge window, the `gen_agent_docs` precedent, `unicode-normalization` and `strsim`, and the `sonara_app::edit` undo-group wrappers and desktop caller on the feature branch.
- Today: the planner guarantee is quoted from `docs/orchestration.md:30-31`; the VSCO row adds that `ViolinEns-KS` shares the `susVib` samples and layers, and that 11.2 s is one measured sample.
- Section 2: the inference table was not exhaustive (duty ≥ 0.85 with 0.25-0.4 s matched nothing; long notes before rests became staccato); it is now an ordered first-match list with a defined duty.
- Section 2: "at least 2 blocks so the engine sees the old key held" was wrong (notes are frame-accurate); overlaps are 10-30 ms, and the crossfade time is layer (b)'s `crossfade_ms`, not "60 ms (profile value)".
- Section 2: repeated pitches cite the engine reason (the note-off releases the new voice); "portato if preferred ids are approved" replaced (preferred ids are in S2; A13 covers invented ids).
- Section 2: switch points lie after the previous passage's last note-on; timing uses 44.1 kHz and layer (b) 5.5; `ordinario`, not `auto`, marks ordinary passages.
- Section 3: `dynamics` was an absolute level (mf = 0.60), contradicting layer (b) 5.4 (ratio, 1.0 = no change): velocity 76 with 0.6 would have played 46 in VSCO's soft layer. Rewritten with peak-level velocities, a ratio lane, humanization clamped inside the layer, `cc` mode absolute, and the +12 dB held-note bound.
- Sections 3 and 12: "before O3" was impossible (O4 depends on O3); it now covers parts that fall back to the Poly Synth or the Drum Kit.
- Section 4: legato notes are compensated by half the crossfade; attack facts come from `verify`, not a profile field.
- Section 6: DBL-08 needs `role` and moved to generative-only; VOI-01 to VOI-06, applied by `woodwind_pairs`, moved to generative wave 1; all 67 rules still covered.
- Section 7: own FNV/splitmix hashing (QG-13) and the `i64` sign collision replaced by `rand_mt` key seeding (`PyRandom::with_key`).
- Section 9: folding reuses `unicode-normalization`, edit distance uses `strsim`; a catalog seam added, because no tool exposes the catalog (C6).
- Section 10: references are JSON pointers (`create_track` nests ids under `result`); merge keys, the 1.5 s merge risk and rollback inside the undo group stated.
- Section 11: drift-test precedent is `gen_agent_docs.rs`, not the needle test at `invocation.rs:460-470`; the "how to perform" block follows the new rules.
- Section 12: the stale `-KS` "silent until a switch key" reason (Q1 is fixed in O2) removed; presets follow layer (b)'s sketch (`ViolinEns-KS`, `@horn-solo`, `@timpani`); "hit" became `ordinario`.
- Section 12: timpani notes all written at f with the lane at 0.65 through the hits, so the roll's lane rise never dips a ringing hit; BAL-03 counts all four horns; SPC claims limited to the voicings shown, with a possible SPC-02 timpani hint; "room for releases" corrected; dynamics numbers redone.
- Contracts, Alternatives, Plan (PF-1, PF-3, PF-7, PF-8 now `sonara-media` plus `sonara-app` size M, PF-9, PF-11, PF-13-PF-15, PF-17), Risks (wall time, undo merge, cross-layer coupling) and Owner decisions (A13/A14 roles, A2) updated.
- Tests: named, with oracles; TST-02 counts 13 new tracks, not 9.
- Deviation: staged validation is a declared deviation from S10 with reason and rule ids; the S7 refinement is one optional field.

Not fixed here (outside this file):

- Layer (b)'s sketch gives overlap legato only to `ViolinEns-KS` and `@horn-solo` (if the shipped profile differs, step 5's violin preset changes).
- Not measured: whether a commit while stopped rebuilds the graph and loads the patch (QG-11, PF-15). Not voiced here: the A7 chord, so the SPC-02 timpani hint awaits TST-02.

Cross-layer review of 2026-10-30 against layers a, b and d (one line per change):

- Work items renamed `PF-n` (layer d also used `W1`…; layer b cited "layer (c)'s W8").
- Summary, section 9, PF-11, contracts: the grammar folded names itself (NFD, no special-character table) beside layer (a)'s `fold()`/`resolve()` (NFKD plus table, IM-9); it now consumes `resolve()`, and sonara-agent gains only `strsim` (suggestions only), under layer (a)'s one dependency-edge reading.
- Section 1: `SoundCaps.dynamics` gains `Fixed`, the mode layer (b) 5.4 defines; `attack_ms` and `max_hold_s` come from layer (b)'s SS-35; `PartId` is layer (a)'s `Part` slug.
- Section 2: stated as the one lead rule (layer (b) 5.5 had a second, different one, now removed there).
- Section 3: dynamics points sat "one block" early "as articulation points do", which sit two blocks early; now two; `fixed` mode added.
- Section 4: the onset measurement and the override are layer (b)'s (SS-35, 2.2).
- Section 6: PLY-04 and PLY-05 move to wave 1 (their inputs ride on the `articulation` lane, layer (b) 5.7) because layer (d)'s mute control shows PLY-04; harp-pedal and timpani-tuning derivations added to PF-6 for layer (d)'s UI-20, while PLY-07, PLY-08 and PLY-11 stay wave 2 in diagnostic mode; all 67 rules still covered.
- Sections 6 and 11: identity words (*part*, *named*, *declared*, *profile*, *inferred*) aligned with layers a 14, b 3.3 and d principle 4 ("profiled"/"certainty" replaced).
- Section 12: the ensemble parts, folders, bus and pan come from PF-20, shared with layer (d)'s `Edit::ApplyEnsemble` (DES-01); the AI Composer variant now says the GUI's findings (UI-12, O5) do check its result.
- Plan: PF-8 moved to layer (b)'s SS-35; PF-20 (ensemble plan service, O4), PF-21 (CC tool, O6) and PF-22 (ensemble tool, O6) added, because no layer owned the spine's O6 CC and ensemble tools or S9's app service; PF-18 now names `NoteInput.articulation` and `create_track` `part` (layer (a) assigned them here); dependencies name SS-, IM- and PF- ids.
- Owner decisions: A-id links get anchors; the three product decisions are P1-P3; A2 restated as the doubled-instrument question.
- "Not fixed here": layer (b)'s 11.6 ms is fixed there; the item now lists both overlap-legato presets of its sketch.
- Section 6: the PLY-04 wave-1 note now says why `part` (its input in doubling-and-balance 6.2) is read from the mute-kind choice on the `articulation` lane, so the move does not read as a contradiction of the rule catalogue.
