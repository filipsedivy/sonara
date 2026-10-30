# Instrument model (layer a)

_Status: proposed (2026-10-30). Part of the orchestra design; decisions in [../../orchestra-design.md](../../orchestra-design.md)._

## Summary

This layer extends `sonara_midi::orchestration` from 16 instruments with English names, ranges and a
15-id articulation list into the one source of instrument knowledge that the sound, performance and
UI layers read (spine S1, S2, S3, S9). It adds:

- a typed **instrument id** (`InstrumentType`, 85 variants in score order) and a record that carries
  names in five languages, **aliases** for a deterministic planner, a **category** with a default
  member, **clefs**, a **GM voice**, an **ensemble profile**, optional **keyboard mechanics** and
  **playability facts**, each value with its provenance;
- the **articulation vocabulary** of [../articulation-vocabulary.md](../articulation-vocabulary.md)
  (96 ids, 95 if A12 merges `col_legno_battuto`), each with a kind, and **mute** and **vibrato** as
  passage states beside one primary id per note;
- **ensemble templates** and the **doubling and balance rule tables** as typed, compile-checked data;
- one `Family` enum shared with the instrument catalog;
- for projects: nothing new in phase 1 (a sampler `instrument_type` parameter, owned by layer b), and a
  versioned **schema v2** proposal (track `part`, note `articulation`) for ADR 0011.

Everything stays pure Rust data and functions in sonara-midi: no I/O, no real-time code, no new crate,
no contract change before O6.

## Today (verified)

Read in the checkout of 2026-10-30 (`research/orchestra-instruments`, main at a212544).

- **Module.** `crates/sonara-midi/src/orchestration/mod.rs:8-11` declares `articulation`, `data`,
  `instrument`, `pitch`; `:13-19` re-export `INSTRUMENTS` and the types; `instrument(id)` is a linear
  search on the exact id (`mod.rs:21-25`). The test `lookup_is_by_stable_id_not_display_name` pins that
  `instrument("Violin")` is `None` and that `clarinet_bb` is named "Clarinet in Bb"
  (`crates/sonara-midi/tests/orchestration.rs:125-132`).
- **Record.** `Instrument { id, name, family, pitch, articulations, articulations_provenance }`, derived
  `Clone, Copy, Debug, PartialEq, Eq` (`instrument.rs:139-147`). `try_new` takes these six values and
  rejects an id that is empty or holds a byte other than `a-z`, `0-9`, `_`, a comfortable range outside
  the playable one, a transposition that leaves MIDI, and duplicate articulations; `new` is a `const fn`
  that panics, so bad data in the static is a compile error (`instrument.rs:155-224`, `:347-361`).
- **Pitch.** `WrittenPitch`/`SoundingPitch` are separate newtypes (`pitch.rs:15-20`);
  `Transposition(i8)` is sounding minus written (`pitch.rs:56-57`); ranges are sounding
  (`instrument.rs:72-81`). `check_sounding`/`check_written` return `Within`, `OutsideComfortable`,
  `BelowPlayable`, `AbovePlayable` and never clamp (`instrument.rs:286-323`).
- **Provenance.** `Cited { source }` or `Estimate { reason }` per range and per articulation list
  (`instrument.rs:48-61`). The dataset test requires a cited playable range to start with `https://`
  (`tests/orchestration.rs:78`).
- **Family.** `orchestration::Family` (8 variants, `slug()`, no serde; `instrument.rs:11-46`); its doc
  says the dataset uses neither `Choir` nor `Other` (`instrument.rs:8-10`), untrue once the voices land.
  `sonara_app::instruments::InstrumentFamily` has the same variants in the same order with `Serialize,
  Deserialize, PartialOrd, Ord`, `rename_all = "lowercase"` (`crates/sonara-app/src/instruments/family.rs:6-25`);
  the manifest reads a family string and parses it through serde (`manifest.rs:68,144-154`); `FamilyClaim`
  serializes as `{"source", "family"}` (`family.rs:29-38`); `infer_family` looks up alphanumeric runs
  of a path segment in `KEYWORDS` (`family.rs:53-156`). Users: `discovery.rs:86`, `manifest.rs:89`,
  `crates/sonara-app/tests/instrument_catalog.rs:10-11`; `crates/sonara-agent/tests/contracts.rs:19`
  imports `FamilyClaim`. The catalog's own `InstrumentId` is a preset id (`model.rs:87`).
- **Articulations.** 15 ids whose slugs match the manifest spelling (`articulation.rs:3-5`,
  `ALL: [Self; 15]` at `:44`); `from_id` is strict (`:86-90`, test `:123-132`);
  `articulation_availability` answers `Supported` only for a library-declared id (`instrument.rs:325-344`).
- **Data.** `pub static INSTRUMENTS: [Instrument; 16]` (`data.rs:86`), shared articulation lists
  `BOWED`, `WIND`, `BRASS`, `MALLETS`, `SNARE` (`data.rs:44-79`), all lists `ARTICULATION_ESTIMATE`
  (`data.rs:14-16`). Values: [../model.md](../model.md) "Proposed corrections to data.rs".
- **Dependencies.** sonara-midi depends on `midly`, `rand_mt`, `sonara-core`, `thiserror`
  (`crates/sonara-midi/Cargo.toml:11-15`); `serde` (derive) and `unicode-normalization` are workspace
  dependencies (`Cargo.toml:92,106`), the latter used by sonara-media (`docs/dependencies.md:43`).
  ARC-11 (`crates/sonara-testkit/tests/architecture.rs:14-60`): sonara-midi and sonara-project depend
  on sonara-core only, sonara-core on nothing; sonara-app, sonara-agent and the real-time sonara-engine
  may depend on sonara-midi. The first two do (`crates/sonara-app/src/catalog.rs:17`,
  `crates/sonara-agent/src/handlers/midi.rs:12-18`); sonara-engine does not (its `Cargo.toml:15-25`).
- **Consumers today.** No production code reads `orchestration`: the planner matches plugin and track
  names itself (`crates/sonara-agent/src/orchestration.rs:420-483`: whole phrases, a contained name
  loses to a longer matching one, "Which one do you mean?" on ambiguity). Tests do:
  `contract_orchestration_metadata` iterates `INSTRUMENTS` by value (`crates/sonara-agent/tests/contracts.rs:23,189-220`),
  and `crates/sonara-midi/tests/orchestration.rs` pins today's values and the six-argument `try_new`
  (section 12). Nothing in `golden/` or sonara-testkit reads `INSTRUMENTS`.
- **Project.** No track field for an instrument type or part; a note is `{id, pitch, start, length,
  velocity, channel}` with no `extra` map (`crates/sonara-core/src/midi.rs:12-25`), so unknown note keys
  are dropped while tracks keep theirs (`crates/sonara-core/src/model/track.rs:318`;
  `docs/project-format.md:124-126`); `schema_version` is `const: 1` (`project-v1.schema.json:9`); device
  `params` values are string, number or bool. `MIGRATIONS` is empty and runs before validation
  (`crates/sonara-project/src/migrations.rs:3-6,18-20`); `SCHEMA_VERSION = 1` (`serializer.rs:44`);
  `SUPPORTED_FEATURES` has 10 entries (`store.rs:74`); `required_features` follows content
  (`store.rs:289`). `decode` reports `migrated_from` whenever a migration ran (`store.rs:550-566`);
  `open_project` returns it as `migrated_from_schema` (`crates/sonara-agent/src/handlers/projects.rs:170`);
  `golden/coverage/transcript.json` step 164 records `null`, and the replay compares every result
  (`crates/sonara-agent/tests/golden_transcripts.rs:356`, `tests/replay/mod.rs:1-16`). The rich project's
  bytes are pinned (`crates/sonara-project/tests/serializer.rs:35-38,358`; `:350` asserts
  `schema_version == SCHEMA_VERSION`); `golden/workflows/*/final.project.json` record version 1.
- **Sampler.** `sampler_descriptor(patches)` in sonara-core builds `volume`, `pan`, `release_scale` and
  the enum `instrument` from app-supplied paths (`crates/sonara-core/src/plugins/builtin.rs:755-798`;
  `ep` hard-codes `automatable: false`, `:84-91`); the app registers it beside the 12 golden
  descriptors (ADR 0010 decision 4).

## Proposal

### 1. Placement and module layout

All new code is in sonara-midi, a non-real-time crate with no I/O (RT-01, ARC-11; MOD-09: no new
crate, because no safety or compilation boundary is gained). No function here may be called from a
real-time module (RT-02, RT-03): the alias index is a `LazyLock` (lazy init counts as a lock) and
`resolve` allocates. The crate graph does not enforce this, since ARC-11 allows the real-time
sonara-engine to depend on sonara-midi (`architecture.rs:22-31`); the engine gains no such edge for
this design, and no RT module names `orchestration`. sonara-dsp cannot depend on sonara-midi (ARC-11),
so the sampler receives plain indices prepared by sonara-app off the audio thread (layer b).

```text
crates/sonara-midi/src/orchestration/
  mod.rs            re-exports; instrument(), instruments()
  id.rs             InstrumentType (85, score order), CategoryId, Category
  instrument.rs     Instrument, InstrumentSpec, Family, Provenance, RangeCheck (extended)
  pitch.rs          unchanged + Clef
  names.rs          Names, Alias, Lang, fold(), resolve(), the alias index
  articulation.rs   Articulation (96), Kind, Axis, Mute, Vibrato, Request, availability
  general_midi.rs   GmVoice, GmProgram, GmNote, SubstituteFit   (not `gm`: sonara_midi::gm exists)
  keyboard.rs       KeyboardMechanics, Action, Pedal, Resonance, Noise, Decay, TimbreState
  playability.rs    Fact, FactKind, Span
  ensembles.rs      Ensemble, Part, Players, Doubler, StringSize, Seating
  rules.rs          RuleSpec, RuleId, Severity, NamedDoubling, WeightRow, RegisterFactor, limits
  fallback.rs       Neighbour, ArticulationSubstitute, Emulation   (the chain's musical data, section 11)
  reference.rs      the AI Composer's orchestration reference (layer c's PF-14, O4)
  data/             mod.rs (INSTRUMENTS in score order), sources.rs, woodwinds.rs, brass.rs,
                    percussion.rs, keyboards.rs, plucked.rs, voices.rs, strings.rs,
                    categories.rs, ensembles.rs, rules.rs, fallback.rs
```

Two new dependency **edges**, no new crate (QG-10): `serde` with `derive` (section 13) and
`unicode-normalization` (section 3), both already in `Cargo.lock` with allowed licences; their rows in
`docs/dependencies.md` gain sonara-midi as a user (DOC-01).

### 2. Identity: instrument type, family, category

`InstrumentType` is an enum of the 85 ids in score order (woodwinds, brass with horns first,
percussion with timpani first, harp and keyboards, voices, strings; [../ensembles.md](../ensembles.md)
1.1). It is named for the sampler parameter `instrument_type` and to stay apart from the catalog's
preset `InstrumentId` (TYP-07). Ensembles and rule tables reference instruments by variant, so a
misspelt id does not compile (PRI-04). The string id stays the external form (TYP-06).

```rust
// Sketch (not implemented)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum InstrumentType { Piccolo, Flute, /* … 85, score order … */ DoubleBass }
impl InstrumentType {
    pub const ALL: [Self; 85] = [/* … */];
    pub const fn id(self) -> &'static str;              // "horn_f"
    pub fn from_id(id: &str) -> Option<Self>;          // strict, like Articulation::from_id
    pub const fn get(self) -> &'static Instrument;     // INSTRUMENTS[self as usize]
}
```

A `const` block in `data/mod.rs` asserts `INSTRUMENTS[i].instrument_type() as u8 == ALL[i] as u8` for
every `i`, so the index in `get` cannot fail (QG-03 reason in the code); a minimal example of both
patterns compiles on the pinned 1.99.1. Every new field is `Copy`, so `Instrument` keeps `Clone, Copy,
PartialEq, Eq`, which tests that iterate the static by value need (`contracts.rs:194`).

**Categories** cover words that name a group of ids rather than one (spine S1). Each record carries
`category: Option<CategoryId>`; the category owns the selection rule:

| Category | Members | Selection |
|---|---|---|
| `piano` | the six acoustic pianos | default `piano_concert_grand` (spine) |
| `electric_piano` | `electric_piano_rhodes`, `electric_piano_wurlitzer` | default Rhodes [Estimate] |
| `organ` | `organ_pipe`, `organ_hammond` | default `organ_pipe` [Estimate] |
| `clarinet` | `clarinet_bb`, `clarinet_a`, `clarinet_eb`, `bass_clarinet` | default `clarinet_bb` (spine) |
| `saxophone` | the four saxophones | default `saxophone_alto` [Estimate] |
| `trumpet` | `trumpet_bb`, `trumpet_c`, `trumpet_piccolo` | default `trumpet_bb` [Estimate] |
| `trombone` | `trombone_tenor`, `trombone_bass` | default `trombone_tenor` [Estimate] |
| `wagner_tuba` | tenor, bass | `All` (they are written in pairs) |
| `cymbals` | `clash_cymbals`, `suspended_cymbal` | default `clash_cymbals` [Estimate] |
| `choir` | the four `choir_*` sections | `All` (SATB) |
| `solo_voice` | the six `voice_*` | `Ask` |
| `bells` | `tubular_bells`, `glockenspiel`, `crotales`, `carillon` | `Ask` |

`Selection` is `Default(InstrumentType) | All | Ask`; `Ask` hands the choice to the planner's
clarification path. Family words (strings, brass, woodwinds, percussion) resolve to a `Family`, and the
ensemble in use supplies its parts (section 11).

### 3. Names and aliases

**Names** (display, never identifiers): `Names { en, cs, de, fr, it, abbreviation }` from the card json
blocks (all 85 have the five languages and `score_abbreviation`); the Italian name is the score name;
one `names_provenance` per record. Card strings carry alternates in parentheses ("Violine (Geige)",
"Timpani (kettledrums)") and `♭` ("Clarinet in B♭"; today "Clarinet in Bb", pinned at
`tests/orchestration.rs:130`): data entry keeps the first form as the name, moves alternates into
aliases, and updates that test with any changed spelling.

**Aliases** are matching data, stored already folded:

```rust
// Sketch (not implemented)
pub struct Alias { pub text: &'static str, pub lang: Lang, pub number: Number }
pub enum Lang { En, Cs, De, Fr, It }
pub enum Number { Singular, Plural, Unmarked }       // "housle" is both
pub enum Target { Instrument(InstrumentType), Category(CategoryId), Family(Family), Ensemble(EnsembleId) }
pub struct Mention { pub words: Range<usize>, pub alias: &'static Alias, pub targets: Vec<Target> }
pub fn fold(text: &str) -> String;
pub fn resolve(text: &str, languages: &[Lang]) -> Vec<Mention>;
```

**Folding** (input side; the stored text is checked to be folded already): Unicode NFKD, drop
combining marks, lower-case, map `ß`→`ss`, `œ`→`oe`, `æ`→`ae`, `ø`→`o`, `ł`→`l`, `♭`→`b` and
`♯`→`is` (German spelling, so "F♯" folds like "Fis"), then every run of characters other than ASCII
letters and digits to one space, trimmed; the output is therefore `[a-z0-9 ]` only. "Lesní rohy" and
"lesni  rohy" both become `lesni rohy`; "Hörner" becomes `horner`; "Clarinet in B♭" becomes
`clarinet in bb`. QG-13: `unicode-normalization` does NFKD and the mark test
(`unicode_normalization::char::is_combining_mark`); the seven characters above are a const table
because NFKD does not decompose them.

**Forms stored** (explicit, written by a person, never derived by code):

| Language | Forms per name |
|---|---|
| en | singular, plural, common synonyms ("french horn", "kettledrums", "chimes") |
| cs | nominative singular and plural, accusative singular, genitive plural, instrumental plural ("lesní roh", "lesní rohy", "lesních rohů", "lesními rohy") |
| de, fr, it | singular and plural ("Horn"/"Hörner", "cor"/"cors", "corno"/"corni") |
| all | score abbreviations whose folded form has three or more letters ("picc", "fag", "timp") |

Every alias has at least three folded letters (section 12), so one- and two-letter abbreviations
(`S.`, `A.`, `T.`, `B.`, also `Fl.`, `Ob.`, `Hn.`, `Vn.`, `Va.`, `Vc.`, `Cb.`) are never aliases: they
collide with words such as Czech `a` or German `ob`. A reviewed two-letter list stays open (Risks).

**Matching** is leftmost-longest over folded words: at each word position try the longest alias
first (its length in words is a `const`), emit a mention, skip past it, else advance one word.
"anglický roh" wins over "roh", "bass clarinet" and "bass drum" over "bass", the same intent as the
planner's `found_longer` (`orchestration.rs:455-471`). The index is a `LazyLock<HashMap<&'static str,
Vec<(Target, &'static Alias)>>>` built once in data order; results come back in data order, so no hash
order leaks (TST-04). No stemming (no maintained crate covers Czech; stems over-match) and no edit
distance (`viola`/`violin` differ by one letter).

**Ambiguity is data, not an error.** One folded text may name several targets in one language
(`bass`: `double_bass`, `voice_bass`) or across languages (`alto`: French viola, English voice); the
mention lists every target and the planner asks. `languages` filters by `Lang`, and `Number` separates
voices from sections (`tenor` → `voice_tenor`; `tenors`, `tenory` → `choir_tenor`). An explicit
`KNOWN_AMBIGUOUS` list makes an accidental collision fail a test.

| Input | Folded | Result |
|---|---|---|
| "smyčce hrají legato melodii, lesní rohy a tympány pod tím" | `smycce … lesni rohy … tympany …` | `Family(Strings)` plural; `horn_f` plural; `timpani` plural |
| "the strings play a legato melody, horns and timpani underneath" | | same three targets |
| "corni", "Hörner", "cors" | `corni`, `horner`, `cors` | `horn_f`, plural |
| "klavír" | `klavir` | `Category(piano)` → `piano_concert_grand` |
| "alto" | `alto` | `viola` (fr), `voice_alto` (en): ask |

### 4. Pitch, clefs and transposition

Ranges, `Transposition`, `check_sounding` and `check_written` stay as they are; the record keeps
`playable` and `comfortable` with their provenance (A18 decides their meaning). New:

- `Clef` enum with the seven slugs the cards use: `treble`, `treble_8vb`, `bass`, `alto`, `tenor`,
  `percussion`, `single_line`; `clefs: &'static [Clef]` in order of use (bassoon: bass, tenor, treble).
- **Tenor voices** (spine S2 owner point 3, A20). The cards store `voice_tenor` and `choir_tenor` as
  −12 with clef `treble_8vb`, but a tenor part on a shared bass-clef staff is written at pitch, so a
  fixed −12 mis-transposes it. Recommendation: transposition 0 for both, and
  `Clef::Treble8vb.octave() == -1`, so the written view shifts by the clef in use. Octave-transposing
  instruments in plain clefs (piccolo, double bass, guitar, contrabassoon, xylophone, glockenspiel,
  celesta) keep their transposition. `check_written` stays clef-free, so tenor parts in `treble_8vb`
  go through one method, `Instrument::written_in(clef, sounding) -> Option<WrittenPitch>` (DES-01).
- **Notation overrides** (bass clarinet in bass clef, Wagner tuba in F, xylophone at pitch; A20) are
  per part, not per id: the phase-2 `part.notation` (section 14). Phase 1 keeps the default.
- **A19** extensions (low extensions, gaps, tessitura, registration footage) are not modelled now
  (DES-03). `InstrumentSpec` uses struct-update defaults, so an optional field can be added later
  without editing 85 records.

### 5. General MIDI voice

```rust
// Sketch (not implemented)
pub enum GmVoice {
    Program(GmProgram),                    // GM1 1..=128 names this instrument or its class
    Percussion(&'static [GmNote]),         // GM1 channel-10 keys 35..=81, preferred first
    Substitute { target: GmTarget, fit: SubstituteFit, provenance: Provenance },
    NoEquivalent,                          // the spine's `None`
}
pub enum GmTarget { Program(GmProgram), Notes(&'static [GmNote]) }
pub enum SubstituteFit { Approximate, Weak }
```

Policy (A11; recommended: exact and substitute values are kept apart by the type):

- `Program` where the GM1 program names the instrument or its class at its register: violin 41,
  horn 61, clarinet_bb and clarinet_a 72, trombone_bass 58, rhodes and wurlitzer 5 (GM 5 covers both).
- `Substitute` where a card stored or named an approximation, or [../sound-mapping.md](../sound-mapping.md)
  section 5 flags one: the six solo voices (54, approximate), cornet and piccolo trumpet (57, stored
  substitutes per A11), flugelhorn (57, weak), studio grand (2, the card's choice), clarinet_eb and
  bass_clarinet (72, approximate), and 15 of the 16 pitched ids with no GM1 program (alto flute 74 …
  carillon 15; all but `piano_prepared`), with the card as `Cited` where the card names the fallback
  and `Estimate` otherwise. Unpitched, as `GmTarget::Notes` of channel-10 keys: suspended cymbal (49),
  castanets (75 Claves; 85 is GS), temple blocks (76/77), sleigh bells (54), shaker (70).
- `Percussion` for exact GM1 keys: snare 38 (the card's 40 "Electric Snare" is a substitute and is
  dropped, A11), bass drum 35/36, clash cymbals 49/57, triangle 81/80,
  tambourine 54, wood block 76/77, maracas 70. `wood_block` keeps only its channel-10 keys: an
  unpitched id has one representation (TYP-02); program 116 stays in sound-mapping's table.
- `NoEquivalent` where sound-mapping section 5 says so: `whip`, `tam_tam`, `piano_prepared`.

Whether a substitute is ever played is the degradation chain's choice (spine S8, layer b). The Drum
Kit's golden note map (`crates/sonara-midi/src/gm.rs:5`, 34 notes) is not changed (GLD-01); GM keys
here are GM1 facts, not Drum Kit capabilities.

### 6. Ensemble profile

```rust
// Sketch (not implemented)
pub struct EnsembleProfile { pub forms: Forms, pub typical: [EraPlayers; 4] }   // by Era
pub enum Forms { Solo, Section, Both }                     // cards: 61 solo, 6 section, 18 both
pub enum Era { Classical, Romantic, Film, Chamber }        // the four keys of every card
pub struct EraPlayers { pub players: Option<PlayerRange>, pub note: &'static str, pub provenance: Provenance }
```

The cards give players as prose with embedded citations ("4 (6-8 in large scores; Wagner's Ring 8)").
Data entry turns each into `PlayerRange { min, max }` (`0..=0` for "not used"), keeps the qualifier in
`note`, and keeps the card's tag as provenance; `None` where no number is stated. String-section sizes
per orchestra are ensemble data (section 11), not per-instrument data.

### 7. Keyboard mechanics and the piano family

Optional on any family (carillon is percussion with a baton clavier; A21):

```rust
// Sketch (not implemented)
pub struct KeyboardMechanics {
    pub action: Action, pub keys: u8, pub pedals: &'static [Pedal],
    pub resonance: Resonance, pub noises: &'static [Noise], pub decay: Decay,
    pub timbre: Option<TimbreState>, pub provenance: Provenance,
}
pub struct Pedal { pub kind: PedalKind, pub cc: Option<u8> }   // practice_muffler: no CC
pub enum PedalKind { Sustain, Sostenuto, UnaCorda, Soft, PracticeMuffler, FootSwell, Expression, Crescendo }
pub enum Resonance { None, Weak, DampersRaised, Undamped }
pub enum Decay { TwoStage, Long, WhileHeld, HeldThenMuted, Free }
pub enum TimbreState { Felt, Prepared { pitch_reliable: bool }, DetunedUnisons }
```

`Noise` lists what a library may sample separately (key release, damper return, pedal thump, hammer
knock, key click, wind or blower, bellows, clapper). Pedals stay an open list (A22). The piano family
differs on three axes, action, pedal set and timbre state, from [../model.md](../model.md)
"Keyboard mechanics":

| id | Action | Pedals (CC) | Resonance | Timbre state, decay | GM |
|---|---|---|---|---|---|
| `piano_concert_grand` | grand, double escapement | sustain 64, una corda 67, sostenuto 66 | dampers raised; duplex rings | –, two-stage, long bass | Program 1 |
| `piano_studio_grand` | grand, double escapement | as concert grand | weaker low resonance | –, shorter bass | Substitute 2 |
| `piano_upright` | upright, single escapement | sustain 64, soft 67 (half-blow), practice muffler (no CC) | dampers raised, smaller | –, shorter | Substitute 1 |
| `piano_felt` | upright with felt rail | as upright | as upright | Felt, darker and shorter | Substitute 1, weak |
| `piano_prepared` | grand | as concert grand | partly suppressed | Prepared, pitch unreliable | NoEquivalent |
| `piano_honky_tonk` | old upright | sustain 64, soft 67 | as upright | DetunedUnisons, wavering | Program 4 |
| `electric_piano_rhodes` | tine hammer | sustain 64 | weak [Estimate] | –, long | Program 5 |
| `electric_piano_wurlitzer` | reed hammer, 64 keys | sustain 64 | weak [Estimate] | –, shorter | Program 5 |

Controls that are not articulations (A22: Rhodes tremolo, Clavinet pickups, drawbars, Leslie, harp
pedal state, vibraphone motor) are not modelled here; they are track or device controls.

### 8. Playability facts

Facts that rules read, each with its own provenance (`&'static [Fact]`, empty for most ids):

| `FactKind` | Value | Ids | Rules ([../doubling-and-balance.md](../doubling-and-balance.md) 6.2) |
|---|---|---|---|
| `Breath` | longest phrase without a rest, held note | winds, brass, voices | PLY-01 |
| `MultiTongue` | can double-tongue | flute, trumpets, cornet | PLY-14 |
| `MuteChange` | time to put on / take off | strings, brass | PLY-04 |
| `ArcoPizzSwitch` | time needed | bowed strings | PLY-05 |
| `SustainedStops` | notes a multiple stop can hold (2) | bowed strings | VOI-12 |
| `Retune` | time per whole tone; drums in use | timpani | PLY-11 |
| `HarpPedals` | 7 pedals; left D C B, right E F G A; changes per beat | harp | PLY-07, PLY-08 |
| `NotesPerHand` | 4 | harp | PLY-09 |

Durations are `Span::Millis(u32)` or `Span::ModerateBeats(u16)` (beats of a moderate 4/4) as the
sources state them: PLY-05's half second is `Millis(500)`, PLY-04's "slightly under two bars" is
`ModerateBeats(7)`; whole seconds or whole bars could not hold either. One `MODERATO_BPM` constant (an
Estimate) converts beats, and rules use the project tempo when it is known.

### 9. Provenance

`Provenance` keeps its shape. New values carry it per range, per articulation list, per GM
substitute, per era count, per keyboard record and per fact. The cards cite card-local numbers
("Cited S1, S5"); data entry replaces each with a full source string from `data/sources.rs`
(URL, or book, edition and page). Some sources are books without a URL, so the dataset test at
`tests/orchestration.rs:78` changes from "starts with https://" to "non-empty and names a URL or a
page". Aliases carry no provenance: they are matching data, reviewed through tests.

### 10. Articulation vocabulary (spine S2)

```rust
// Sketch (not implemented)
pub enum Kind { LongNote, ShortNote, Ornament, Transition, Effect, TimbreModifier, Mute, DynamicShape, KeyboardControl, Vocal }
pub enum Axis { Primary, Mute(Mute), Vibrato(Vibrato) }
pub enum Mute { Open, Generic, Cup, Harmon, Plunger, Bucket, Stopped, Palm, Xylophonic }
pub enum Vibrato { Ordinary, On, Off }
pub struct Request { primary: Option<Articulation>, mute: Mute, vibrato: Vibrato }  // None = ordinario
impl Request {   // rejects a primary whose axis() is not Primary (con_sordino, vibrato_on, …)
    pub const fn new(primary: Option<Articulation>, mute: Mute, vibrato: Vibrato) -> Result<Self, RequestError>;
}
impl Articulation {
    pub const fn kind(self) -> Kind;
    pub const fn axis(self) -> Axis;            // con_sordino -> Mute(Generic), vibrato_on -> Vibrato(On)
    pub const fn parent(self) -> Option<Self>;  // trill_half -> trill (3.5 table)
    pub const fn is_stable(self) -> bool;       // the 15 ids of today
}
```

- **Ids.** The 15 existing ids keep spelling and meaning (stable); the 81 proposed ids of
  [../articulation-vocabulary.md](../articulation-vocabulary.md) are added, as approved, with their kind
  and one-line definition (`description()`, used by the composer reference and tooltips). `ALL` is **append-only**:
  phase-1 enum parameters are automated by choice index (`crates/sonara-engine/src/automation.rs:13-14`),
  and layer b's sampler `articulation` choices are one frozen list, `auto`, `ordinario` (this layer's
  `primary: None`), then `ALL` order ([sound-source.md](sound-source.md) 5.1). Ids are never removed
  or reordered, so an id awaiting A12 or A13 enters `ALL` only after that decision. `from_id` stays strict.
- **States.** The eight mute-kind ids map one-to-one onto `Mute` values and back; `vibrato_on` and
  `non_vibrato` map onto `Vibrato`. The ids stay in the vocabulary because library maps and manifests
  name them as strings; in a request they are states, never the primary id. `Open` and `Ordinary`
  are the defaults and have no id.
- **Ordinario** (A14, spine S2 owner point 2). Recommended: `primary: None` is the instrument's
  ordinary sound (Logic's "no ID"); `sustain` stays "the long patch". The card usages of section 1.5
  (the xylophone's `staccato` and the pianos' `sustain` as the plain note) are then corrected in the
  data; if the owner keeps them, the data stays as the cards have it.
- **Refinement parents** (3.5 table) are vocabulary data; the substitution itself is layer b's chain.
- **Per-instrument lists** replace the shared `BOWED`/`WIND`/`BRASS`/`MALLETS`/`SNARE` constants with
  each card's union of existing and proposed ids.
- **Availability.** `articulation_availability(id, declared)` keeps its four answers. New:
  `request_availability(Request, declared) -> RequestAvailability { primary, mute, vibrato }`, each an
  `ArticulationAvailability` of the state's id (a default state is `Supported`). Whether a library
  recorded the combination is the preset map's question (layer b).

Open, not decided here: A12 (`col_legno_battuto`), A13 (34 invented ids, `loure`/`choke` merges),
A15 (mallet `roll`), A16 (shape: the spine chose B).

### 11. Ensembles and rule tables as data (spine S9, S10)

```rust
// Sketch (not implemented)
pub struct Ensemble { pub id: EnsembleId, pub name: &'static str, pub seating: Seatings,
    pub string_sizes: &'static [StringSize], pub parts: &'static [Part],
    pub doublers: &'static [Doubler], pub provenance: &'static [Provenance] }
pub struct Part { pub slug: &'static str, pub name: &'static str, pub short: &'static str,
    pub instrument: InstrumentType, pub alternatives: &'static [InstrumentType],
    pub players: Players, pub required: bool, pub player: Option<&'static str>,
    pub pan: &'static [(Seating, f32)], pub divisi: Option<Divisi> }
pub enum Players { Fixed(u8), Doubling, FromStringSize(usize) }   // index into StringSize.counts
```

- Content: the templates of [../ensembles.md](../ensembles.md) section 3 (classical, Beethoven,
  romantic, late-romantic, film, chamber orchestra, string orchestra, big band, choir and orchestra,
  small ensembles). Folders are per `Family`, sorted in score order, which comes from `InstrumentType`
  order (a `Family::score_rank()`), not from `Family`'s derived `Ord`: that keeps the catalog's order,
  `Strings` first (section 13). Buses, effects and plugin presets are not here: they name plugins,
  which the metadata must not know (`mod.rs:4-6`); sonara-app's ensemble service adds them (S9).
- The golden `orchestral` template (`crates/sonara-app/src/templates.rs:52`) is not an ensemble and is
  never changed (GLD-01).
- Rules: `RuleSpec { id: RuleId, title, inputs: &[RuleInput], severity: Severity, provenance }` for
  the 67 rules (RNG 4, SPC 8, DBL 9, BAL 9, MSK 4, VOI 12, PLY 15, TEX 6), `RuleId::code()` gives
  `"RNG-01"`; `Severity` is `Error | Warning | Hint | ByDynamic { soft, loud }`. Tables:
  `NAMED_DOUBLINGS` (2.1-2.7), `DO_NOT_DOUBLE` (2.8), `WEIGHTS` with `Weight::Units(f32) | Overpowers`
  per dynamic band (3.3), `REGISTER_FACTORS` with a `PitchRange` (3.4), `LOW_INTERVAL_LIMITS` (4.1),
  and named thresholds. Selectors are `Ids(&[InstrumentType]) | Family(Family) | Played { ids,
  articulation } | SoloString`. f32 values derive `PartialEq` only (TYP-10).
- The rule predicates are code in `sonara_midi::performance::check` and the project findings in
  `sonara_app::quality::orchestration` (S10, layer c). This layer owns the types and data; the
  composer reference renders them as text.
- **Fallback data for the degradation chain** (spine S8; `fallback.rs`, IM-20): per id, the ordered
  family neighbours of [../sound-mapping.md](../sound-mapping.md) 4.3 (step 4), and per articulation
  the nearest substitutes and the emulation kind of its 4.2 table (step 2), all Estimates with that
  provenance. The step-1 presets and the step-6 built-in (Poly Synth preset, Drum Kit note) name
  libraries and plugins, which this module must not know (`mod.rs:4-6`), so they stay in layer b's
  `sonara_app::instruments::chain`.

### 12. The 85-id dataset

- **Files.** One file per family in `data/`, each record a named `const` built from an
  `InstrumentSpec` literal that ends in `..InstrumentSpec::EMPTY` (a `const` item: struct update in a
  `const` cannot call `Default::default()`); `data/mod.rs` lists the 85 in score order in
  `pub static INSTRUMENTS: [Instrument; 85]`; `instruments()` returns it as a slice so a later count
  does not change the API again.
- **Compile-time validation** (`Instrument::new(spec)`, const, panics): today's checks, plus folded
  aliases of at least three letters, no duplicate alias or articulation in a record, a mute or vibrato
  id never listed twice through its state, keyboard pedal CCs 0-127 and unique, GM program 1-128 and
  GM keys 35-81, era counts `min <= max`, cited sources non-empty. Cross-record checks (alias
  collisions, categories, ensembles) are tests, because quadratic const evaluation over about 1,300
  aliases risks the `long_running_const_eval` lint.
- **Corrections to today's 16** (A23; values from [../model.md](../model.md), "Proposed corrections";
  14 of 16 ids change: 20 values marked "changed" there, plus one provenance change, `horn_f` playable
  Estimate to Cited; `trombone_tenor` and `snare_drum` keep their ranges):

| id | Playable today → proposed | Comfortable today → proposed |
|---|---|---|
| `violin` | 55-108 → 55-105 | 55-100 → 55-93 |
| `cello` | 36-93 Estimate → 36-88 Cited | none → 36-76 |
| `double_bass` | 24-67 Estimate → 23-67 Cited | 28-55 (kept) |
| `flute` | 59-96 → 59-98 | none → 60-94 |
| `piccolo`, `oboe`, `english_horn`, `clarinet_bb`, `contrabassoon` | kept | none → 80-106, 60-87, 52-81, 50-91, 22-57 |
| `bassoon` | kept | 34-76 → 34-72 |
| `horn_f` | 35-77 Estimate → Cited | none → 41-72 Estimate |
| `trumpet_bb` | 52-82 → 52-91 | none → 52-82 |
| `glockenspiel` | 79-108 Estimate → 72-108 Cited | none → 79-108 |
| `xylophone` | 65-108 Estimate → 60-108 Cited | none → 65-108 |

  The flute top, violin top and horn comfortable range follow A18. The `Instrument::name()` accessor
  returns `names.en`, so existing callers keep working. The crate tests that pin today's values
  (`tests/orchestration.rs:136-149`, `:166-191`: trumpet written top 84 becomes 93, violin 100/101 and
  flute 97 change class) and the six-argument `try_new` calls (`:34-45,58-66,101-122`) are rewritten
  in the same change; they are not golden data (GLD-01 untouched).

### 13. One `Family` enum

`orchestration::Family` becomes the only family enum. It gains
`#[derive(Serialize, Deserialize, PartialOrd, Ord)]` and `#[serde(rename_all = "lowercase")]`
(sonara-midi takes `serde.workspace = true`; the workspace enables `derive`), plus `from_slug`. Variant
order is already the catalog's, so `Ord` is unchanged. sonara-app deletes its copy and re-exports
`pub use sonara_midi::orchestration::Family as InstrumentFamily;`, so `discovery.rs`, `manifest.rs`,
the catalog tests and `crates/sonara-agent/tests/contracts.rs:19` compile unchanged. The wire form
stays the lowercase slug, so manifests and `FamilyClaim` JSON do not change; the catalog is not a
CTR-01 surface (no tool, CLI or GUI reads it). Effects: the doc at `instrument.rs:8-10` is corrected
(voices use `Choir`); in O3, `infer_family` can resolve patch-path words through `resolve()` and report
an inferred instrument type, which also closes the keyword gaps of A24 (rhodes, wurlitzer, clavinet,
chimes, tubular, crotales, carillon, vibes, glock, celeste). Two limits: `fold` does not split camel
case, so sonara-app splits `TubularBells.sfz` (one word today, A24) before calling `resolve()`; and
path matching passes a narrow language list (at first `&[Lang::En]`), so a folder named "Alto" or
"Bass" does not pick up the French viola or a voice.

### 14. What a project stores

**Phase 1 (no contract change; spine S3).** Nothing new in `project-v1.schema.json`. Identity is
derived (preset → library profile → instrument type, layer b). The sampler gains a non-automatable
enum `instrument_type` (`auto` plus the 85 ids), stored in `device.params` as a string, which the
schema already allows. sonara-core may not depend on sonara-midi (ARC-11), so sonara-app passes the
ids (in `InstrumentType::ALL` order) as it passes `patches` today (`builtin.rs:767`), inside layer b's
`SamplerChoices { presets, instrument_types, articulations }` ([sound-source.md](sound-source.md)
5.1). Not automatable and stored as a string, its choice order may follow score order; ids are never
renamed or removed; an unknown stored choice falls back to `auto` with a warning (ADR 0010 decision 5;
layer b 5.1 and SS-14 make the graph load path soft). Part names live in track names; a match on a
Poly Synth or Drum Kit track name is shown as *inferred*.

**Phase 2 (owner, CTR-01, versioned; for ADR 0011).** Schema version 2 adds an optional track `part`
and an optional note `articulation`:

```json
"part": {"type": "object", "required": ["instrument"], "properties": {
  "instrument": {"type": "string", "pattern": "^[a-z][a-z0-9_]*$"}, "name": {"type": "string", "minLength": 1},
  "players": {"type": "integer", "minimum": 1, "maximum": 128}, "solo": {"type": "boolean"},
  "player": {"type": "string", "minLength": 1},
  "notation": {"type": "object", "properties": {"transposition": {"type": "integer", "minimum": -36, "maximum": 36},
    "clef": {"enum": ["treble", "treble_8vb", "bass", "alto", "tenor", "percussion", "single_line"]}}}}}
```

and in `$defs.note.properties`: `"articulation": {"type": "string", "pattern": "^[a-z][a-z0-9_]*$"}`
(absent = ordinario), in a new `project-v2.schema.json` beside v1 (v1 pins `schema_version` to
`const: 1`). `instrument` and `articulation` are patterns, not JSON enums, so adding an id is not a
schema change. sonara-project checks only the pattern: it may depend on sonara-core alone
(`architecture.rs:16`), so it cannot see the dataset (ARC-11). sonara-app checks the values against
the dataset after load and reports an unknown one as a warning, keeping it. `part` is allowed on every
track kind that holds an instrument, hosted plugins included. Mute and vibrato stay passage states; in
phase 1 they travel through layer b's `articulation` choices (the mute-kind ids, `vibrato_on`,
`non_vibrato`; [sound-source.md](sound-source.md) 5.7), and a schema field for them waits for A16.

- **Migration v1 → v2** (`MIGRATIONS[0]`, a pure function on the document): set `schema_version` to 2;
  a stray v1 `part` key (kept in `extra`) stays if it validates against v2, else moves to
  `extra.part_v1`. It lifts nothing from `params.instrument_type` (one fact stored twice, DES-01, and a
  rewrite on open); identity precedence instead, the one order all four layers use: `part.instrument`
  (*part*), a non-`auto` `instrument_type` (*named*), the library manifest (*declared*), a shipped
  profile (*profile*), an alias match (*inferred*), else *none* (layer b 3.3). Nothing is inferred
  from presets or names (a migration must be pure).
- **No reported migration for an unchanged v1 document.** `decode` reports `migrated_from` whenever a
  migration ran (`store.rs:550-566`), so with `SCHEMA_VERSION = 2` every v1 open would answer
  `migrated_from_schema: 1`, not the `null` of `golden/coverage/transcript.json` step 164 (GLD-01; the
  `open_project` result is CTR-01). `decode` reports it only when a migration changed more than
  `schema_version`; a test pins the golden step.
- **Features.** `required_features` adds `parts` when a track has `part` and `note-articulations` when
  a note has `articulation`; both join `SUPPORTED_FEATURES`. An older build already refuses
  `schema_version: 2` (`migrations.rs:29`); the features make its `UNSUPPORTED` error name what is
  missing, and they stop a build that reads v2 but lacks a feature from dropping note keys silently
  (notes keep no unknown keys).
- **Lowest sufficient version.** The writer writes `schema_version: 1` and no new key when a document
  uses no v2 field. The rich project's bytes are pinned (`serializer.rs:35-38,358`) and every golden
  workflow records version 1, so an unconditional v2 writer would break GLD-01; the assertion at
  `serializer.rs:350` becomes "version 1 for a v1-only document".
- **Tools** (`NoteInput.articulation`, a `part` argument on `create_track`) are a separate CTR-01 change
  owned by the performance layer, against `golden/contracts/tools.json`.

### 15. Public API

```rust
// Sketch (not implemented): sonara_midi::orchestration
pub fn instrument(id: &str) -> Option<&'static Instrument>;     // unchanged, strict id
pub fn instruments() -> &'static [Instrument];                  // score order
pub fn resolve(text: &str, languages: &[Lang]) -> Vec<Mention>; // aliases, section 3
pub fn category(id: CategoryId) -> &'static Category;
impl Instrument {   // kept: id, name, family, pitch, articulations, check_sounding, check_written, written_playable
    pub const fn instrument_type(&self) -> InstrumentType;  pub const fn names(&self) -> &Names;
    pub const fn category(&self) -> Option<CategoryId>;  pub const fn clefs(&self) -> &'static [Clef];
    pub const fn gm(&self) -> GmVoice;  pub const fn ensemble(&self) -> &EnsembleProfile;
    pub const fn keyboard(&self) -> Option<&KeyboardMechanics>;  pub const fn facts(&self) -> &'static [Fact];
    pub fn written_comfortable(&self) -> Option<(WrittenPitch, WrittenPitch)>;
    pub fn written_in(&self, clef: Clef, p: SoundingPitch) -> Option<WrittenPitch>;  // section 4
    pub fn request_availability(&self, r: Request, declared: &[&str]) -> RequestAvailability;
}
pub mod ensembles { pub static ENSEMBLES: [Ensemble; N]; pub fn ensemble(id: &str) -> Option<&'static Ensemble>; }
pub mod rules { pub static RULES: [RuleSpec; 67]; /* NAMED_DOUBLINGS, WEIGHTS, REGISTER_FACTORS, … */ }
pub mod fallback { pub fn neighbours(t: InstrumentType) -> &'static [Neighbour];
                   pub fn substitutes(a: Articulation) -> &'static [ArticulationSubstitute]; }
```

Consumers (ARC-11): `sonara_midi::performance` (layer c, same crate), sonara-app (catalog family,
ensemble service, quality findings, composer reference, sampler choices, the chain's fallbacks),
sonara-agent (planner mentions through `resolve()`), the desktop only through sonara-app and
sonara-agent (`architecture.rs:60`).

## Contracts and rules

| Rule id | How the proposal holds it |
|---|---|
| CTR-01 | O1-O5 change no tool, schema, IPC or CLI. The sampler parameter is on a non-golden descriptor. Schema v2 and the tool arguments are a versioned O6 decision; even then `open_project` keeps answering `migrated_from_schema: null` for an unchanged v1 project. |
| GLD-01 | No golden file is read or re-recorded; the `orchestral` template, the 12 built-in descriptors and the Drum Kit map stay; the v2 writer writes version 1 when no v2 field is present, and a v1 open reports no migration (`golden/coverage/transcript.json` step 164); no tolerance changes. Only non-golden crate tests that pin today's values change (section 12). |
| LIC-01, ADR 0010 | No new crate; `serde` and `unicode-normalization` are MIT/Apache-2.0 and already recorded. Names and facts are our own data with sources; no third-party text is copied beyond names. Nothing is bundled: the dataset holds no samples and names no library. |
| ADR 0009, PERF-10 | No GUI or render-path code; the alias index is built once, on first use, off the audio thread. Layer d measures any view that renders this data. |
| HW-01 | No hardware figures. |
| DOC-01 | `docs/orchestration-metadata.md`, `docs/instrument-catalog.md`, `docs/dependencies.md`, `CHANGELOG.md` and `docs/implementation-status.md` change with O1; `docs/project-format.md` with O6. |
| ARC-11 | Data in sonara-midi (depends on sonara-core only); users in sonara-app and sonara-agent; sonara-core gets the id list as an argument; sonara-project checks only id patterns and sonara-app checks ids against the dataset; the desktop never depends on sonara-midi. |
| RT-01..RT-05, RT-10 | Nothing runs on the audio thread; no `unsafe`; sonara-dsp never sees these types; sonara-engine, which ARC-11 would allow to depend on sonara-midi, gains no such edge, and no RT module calls `orchestration` (its `LazyLock` and `Vec`s would break RT-02, RT-03). |
| MOD-09 | No new crate; new modules inside `orchestration`. |
| QG-10, QG-13 | Crates considered: `unicode-normalization` (used), `deunicode` (transliterates the seven special characters too, but is a new crate in `Cargo.lock`), `rust-stemmers` (no Czech), `strsim` (edit distance confuses viola and violin), `aho-corasick` (in tree via regex, but word boundaries need a post-filter; a word-level map is simpler), `bitflags` (not needed, slices of enums suffice). |
| QG-05 | Every public type documents its errors and panics; `new` documents its compile-time panic. |
| TYP-01, TYP-02, TYP-07 | `InstrumentType`, `CategoryId`, `GmProgram`, `GmNote`, `Mute`, `Vibrato` are distinct types; no magic `-1` or empty string. |
| TYP-06 | Strings become types at the boundary: `from_id`, `fold`/`resolve`, the project loader. |
| TYP-10 | Weight and pan rows hold `f32` and derive `PartialEq` only. |
| ERR-01, ERR-03 | No `unwrap` in data access; `InstrumentError` grows variants for the new checks. |
| DES-01 | One family enum, one alias table and `fold()` (the planner calls it), one vocabulary, one rule table, one fallback table for app, agent and GUI. |
| PRI-04 | Typed references in ensembles and rules; `Request` has private fields and `Request::new` rejects a mute or vibrato id as the primary. |

## Alternatives considered

- **JSON data loaded at start-up** instead of statics: rejected by the earlier research and the spine
  (S1); it loses compile-time checks and typed references.
- **String ids everywhere** instead of `InstrumentType`: fewer types, but a misspelt id in an ensemble
  compiles. Kept as the boundary form only.
- **Stemmers or fuzzy matching** for plurals and cases: not deterministic enough and not available for
  Czech (QG-13 above). Explicit forms cost about 15 strings per id.
- **Folding without `unicode-normalization`**, a const table of about 30 letters: no new edge, but it
  rewrites what a maintained crate already does in the tree.
- **Family without serde in sonara-midi**: the manifest parses with `Family::from_slug` (it reads the
  family as a string already, `manifest.rs:68,150`) and `FamilyClaim` gets a hand-written `Serialize`
  through `slug()` (simpler than `#[serde(remote)]`; the orphan rule forbids a plain impl). Chosen only
  if the owner reads "no new dependency" as "no new edge".
- **Lifting `instrument_type` into `part` in the v1 → v2 migration**: rejected (section 14): one fact
  stored twice, and a v1 document would change on open.
- **Schema features without a version bump** (the new optional keys in a revised v1 schema, gated by
  `required_features`, the mechanism `migrations.rs:5-6` names): no migration question, but the spine
  (S3) asks for a versioned v2; kept as the owner's alternative under A1.
- **GM substitutes only in sound-mapping's table** (A11 option c): keeps the model exact, but the
  degradation chain and the composer then read two sources; the `Substitute` variant keeps exactness
  explicit in one place.
- **Mute and vibrato as primary ids** (shape A): every muted short needs its own id or is lost; the
  spine chose shape B.
- **Tenor voices at −12** (the cards): wrong for a shared bass-clef staff; kept as the owner's
  alternative under A20.
- **Schema v2 with JSON enums** for `instrument` and `articulation`: strict, but every new id becomes a
  schema version.

## Implementation plan

| Work item | Milestone | Crate/module | Size | Depends on |
|---|---|---|---|---|
| IM-1 `InstrumentType`, `instruments()`, per-family `data/` files, score-order static with const index check | O1 | sonara-midi `orchestration::{id, data}` | M | – |
| IM-2 `InstrumentSpec` (with `const EMPTY`) and the extended record; new `InstrumentError` variants and const checks; rewrite the six-argument `try_new` test calls | O1 | `orchestration::instrument`; `tests/orchestration.rs` | M | IM-1 |
| IM-3 `Clef` with octave; tenor case per A20 | O1 | `orchestration::pitch` | S | IM-2, A20 |
| IM-4 `data/sources.rs`; card S-numbers to full sources; relax `tests/orchestration.rs:78` | O1 | `orchestration::data` | M | IM-1 |
| IM-5 Transcribe the 85 card json blocks (names split from their parentheticals); apply the corrections; per-instrument articulation lists; update the value-pinning tests | O1 | `orchestration::data::*`; `tests/orchestration.rs` | L | IM-2, IM-4, IM-6, A18, A23 |
| IM-6 Vocabulary with `Kind`, `Axis`, `Mute`, `Vibrato`, `parent`, descriptions: the 15 stable and the approved ids; ids awaiting A12/A13 follow their decision (append-only) | O1 | `orchestration::articulation` | M | A12, A13 |
| IM-7 `Request` and `request_availability` | O1 | `orchestration::{articulation, instrument}` | S | IM-6 |
| IM-8 `Names`, categories, alias data in five languages (the Czech inflected forms the planner needs, PF-11) | O1 | `orchestration::{names, data::categories}` | L | IM-1, IM-5 |
| IM-9 `fold`, alias index, `resolve`; `unicode-normalization` edge (the only folding in the workspace; the planner calls it) | O1 | `orchestration::names` | M | IM-8 |
| IM-10 `GmVoice` and data per A11 | O1 | `orchestration::general_midi` | S | IM-5, A11 |
| IM-11 `KeyboardMechanics` and the 16 keyboard records | O1 | `orchestration::keyboard` | M | IM-5 |
| IM-12 Playability facts | O1 | `orchestration::playability` | M | IM-5 |
| IM-13 Ensemble templates and their validation | O1 | `orchestration::{ensembles, data::ensembles}` | L | IM-1 |
| IM-14 Rule catalogue, doubling, weight, register and limit tables | O1 | `orchestration::{rules, data::rules}` | L | IM-1, IM-6 |
| IM-15 One `Family`: serde derive in sonara-midi; re-export in the catalog | O1 | sonara-midi; `sonara_app::instruments::family` | S | – |
| IM-16 Docs and dependency rows (DOC-01) | O1 | `docs/` | S | IM-1..IM-15, IM-20 |
| IM-17 (Merged into layer b's SS-14, which builds `SamplerChoices` from `InstrumentType::ALL`; no separate work, kept for traceability) | O3 | – | – | IM-1 |
| IM-18 Catalog inference through `resolve()` (A24 keywords; camel-case split, `Lang::En`): the `Inferred` claim of layer b's SS-13 | O3 | `sonara_app::instruments::family` | S | IM-9, IM-15 |
| IM-19 Schema v2 `part` and note `articulation`: `project-v2.schema.json`, relabel migration, `migrated_from` only for content changes, features, lowest-version writer; dataset check in the app | O6 | sonara-project `schemas`, `migrations`, `serializer`, `store`; sonara-core `model`, `midi`; sonara-app load path | L | A1, A16, CTR-01 |
| IM-20 Fallback data for the degradation chain (section 11): family neighbours per id, articulation substitutions and emulation kinds | O1 | `orchestration::{fallback, data::fallback}` | M | IM-1, IM-6 |

## Tests

- **TST-07 invariants** (`crates/sonara-midi/tests/orchestration.rs`, extending `dataset_invariants_hold`):
  ids unique and in `InstrumentType::ALL` order; every record rebuilt through `Instrument::try_new`;
  comfortable inside playable, written range inside MIDI; every display name folds to one of its own
  aliases; `KNOWN_AMBIGUOUS` equals the computed set of folded aliases with several targets; category
  defaults are members and members point back; mute-kind and vibrato ids map to their states and back;
  every `parent` chain is acyclic and ends at an id without a parent (3.5 ends at non-stable ids, e.g.
  `loure` → `portato`), and stable ids have none; GM programs 1-128, keys 35-81; doublers exist for
  every `player`, `Doubling` parts have one, string-size indices are in bounds, pans in -1..1;
  register-factor ranges lie inside the playable range; every rule id appears once; every fallback
  neighbour is another id of the same family and every substitute an id of the vocabulary (IM-20).
- **TST-09 properties** (proptest): `fold(fold(s)) == fold(s)`; `fold` output is `[a-z0-9 ]` for any
  input; every stored alias still resolves after random case changes, cs/de/fr diacritics and extra
  punctuation; written → sounding → written is the identity inside each playable range;
  `check_written(w) == check_sounding(t.to_sounding(w))` for clefs with octave 0.
- **TST-03 failure paths**: unknown text gives no mention; an ambiguous alias returns all targets in
  data order; `instrument("Violin")` and `InstrumentType::from_id("Violin")` are `None`; an unknown
  articulation is `UnknownArticulation`; `Request::new` with `con_sordino` or `vibrato_on` as the
  primary is a `RequestError`; an unpitched check is `CheckError::Unpitched`; each new
  `InstrumentError` variant has a rejected fixture.
- **TST-06 regressions** (`corrected_ranges_match_model_md`): the corrected values against
  [../model.md](../model.md); it replaces the cases of `derived_written_ranges_match_the_sources` and
  `range_checks_do_not_clamp_and_distinguish_comfortable_from_playable` that pin today's values
  (trumpet written 54-93; violin 100, 101 and flute 97 become `OutsideComfortable`).
- **TST-10 oracle** (`cards_fixture_matches_statics`): a fixture generated once from the card json
  blocks (`crates/sonara-midi/tests/fixtures/orchestration-cards.json`) compared field by field with the
  statics (ranges, transposition, clefs, GM, forms; names after the documented split).
- **TST-05 contracts**: `contract_orchestration_metadata` (`contracts.rs:192`) passes unchanged over 85
  records; `FamilyClaim` and manifest JSON unchanged for every slug; `product_sentence_resolves`: the
  sentence in Czech and English gives `Family(Strings)`, `horn_f` plural and `timpani`.
- **Schema v2 (O6)**: the pinned documents keep their bytes (`serializer.rs:358,439,465`) and golden
  saved projects still match (`golden_transcripts.rs:203-219`); `coverage_batch_matches_the_prototype`
  passes, so a v1 open answers `migrated_from_schema: null`; migration cases (unchanged v1, a sampler
  with `instrument_type`, which stays unlifted, a stray `part` that validates and one that does not);
  a v2 document with `part` and `articulation` round-trips; a build without the features rejects it
  with `UNSUPPORTED`; an unknown `instrument` value loads with an app warning.

## Risks

- **Transcription errors** across 85 records and about 1,300 aliases: the card fixture oracle and a
  review table generated from the statics.
- **False alias hits** (Czech "roh" also means corner; "bas", "alt"): multi-word forms where a single
  word is a common noun, the three-letter minimum, the planner's dry run and confirmation gate.
- **Short abbreviations**: without two-letter aliases, "vc" or "cb" finds nothing and the planner asks;
  a reviewed exception list is the remedy if users write them.
- **Choice-index drift**: reordering `Articulation::ALL` would change saved automation; `ALL` is
  append-only and a test pins the first 15 positions.
- **Range changes** turn some existing notes `OutsideComfortable` or `AbovePlayable`; checks advise and
  never edit (`RangeCheck` does not clamp).
- **Const-evaluation cost** grows with the data; cross-record checks stay in tests.
- **Schema v2 and golden data**: an unconditional bump breaks pinned bytes, and a reported relabel turns
  the golden `migrated_from_schema: null` into `1`; the lowest-version writer and the unreported relabel
  are requirements.

## Owner decisions

| Item | What this layer needs or recommends |
|---|---|
| [A1](../open-questions.md#a1-where-an-instruments-identity-and-part-live-in-a-project) | phase-2 track `part` (section 14); recommended for ADR 0011, versioned |
| [A7](../open-questions.md#a7-ensemble-templates-gui-only-or-the-create_project-enum) | context only: how agents reach the ensemble data defined here (spine S9), an O6 contract gate |
| [A11](../open-questions.md#a11-gm-numbers-for-ids-without-a-gm1-program-and-the-drum-kit-map) | GM policy; recommended: exact and substitute values separated by the `GmVoice` type |
| [A12](../open-questions.md#a12-articulation-vocabulary-the-merges-and-col_legno) | the five merges and `col_legno_battuto` |
| [A13](../open-questions.md#a13-articulation-vocabulary-the-34-invented-ids-and-the-candidate-merges) | the 34 invented ids, `loure` and `choke` |
| [A14](../open-questions.md#a14-articulation-vocabulary-the-ordinary-note-and-ids-used-in-two-meanings) | ordinario; recommended `None` = ordinary sound (spine S2 point 2) |
| [A15](../open-questions.md#a15-articulation-vocabulary-mallet-rolls-as-tremolo-or-roll) | mallet rolls |
| [A16](../open-questions.md#a16-articulation-vocabulary-flat-list-or-separate-axes-mutes-beaters-vowels-shapes) | confirm shape B (primary id plus mute and vibrato) |
| [A18](../open-questions.md#a18-what-playable-and-comfortable-mean) | the meaning of the two ranges; fixes the flute top, violin top and horn comfortable values |
| [A19](../open-questions.md#a19-range-model-extensions-extensions-gaps-variants-and-a-third-tier) | recommended: defer optional range fields |
| [A20](../open-questions.md#a20-notation-one-transposition-per-instrument-and-clefs) | per-clef tenor octave (spine S2 point 3) and per-part notation overrides |
| [A21](../open-questions.md#a21-instrument-ids-what-to-add-or-split-and-families) | ids to add or split; accordion and carillon families as the cards propose |
| [A22](../open-questions.md#a22-instrument-controls-and-multi-zone-keyboards) | controls stay out of the vocabulary; the pedal list stays open |
| [A23](../open-questions.md#a23-adopting-the-card-values-in-datars) | adopt the card values (the spine recommends it) |
| [A24](../open-questions.md#a24-small-fixes-outside-the-contracts) | the catalog keyword gaps, closed in O3 through `resolve()` (section 13) |
| not an A item | whether "no new dependency" (S1) also forbids new edges to crates already in `Cargo.lock`: here `serde` and `unicode-normalization` (sonara-midi), and in the other layers `strsim` (sonara-agent, PF-11) and `proptest` as a dev edge (sonara-app, UI tests); one reading for all four layers, recommended: no new crate (see "Alternatives considered") |

## Deviation from the spine

No decision of the spine is contradicted. Refinements and one correction:

1. **Correction of a count.** S1 says "the 13 corrections"; the table in [../model.md](../model.md)
   changes 14 of the 16 ids (20 values marked "changed" plus one provenance change), as
   [../doubling-and-balance.md](../doubling-and-balance.md) 6.1 also counts 14 ids. This design
   applies the table, whatever the number.
2. **Ten kinds, not nine.** S2 lists nine kinds but takes the vocabulary of
   [../articulation-vocabulary.md](../articulation-vocabulary.md), which has ten; `KeyboardControl`
   (`let_ring`, `damped`, `cluster`) is kept.
3. **Naming.** The GM variant `None` is `NoEquivalent`, so it cannot be confused with `Option::None`
   in a pattern (NAM-01); `GmVoice::Substitute` also carries a target that may be channel-10 keys.
4. **Categories without a default.** S1 gives a category a default member; `choir` (all four
   sections), `solo_voice` and `bells` (ask) need `Selection::All` and `Selection::Ask`.
5. **Dependency reading.** S1's "no new dependency" is read as no new crate in `Cargo.lock` (QG-10);
   two new edges are named in section 1 (layers c and d add `strsim` and a `proptest` dev edge under
   the same reading).

## Review

Adversarial review of 2026-10-30 against the checkout, the spine, the SFZ conformance record and layer b;
every "Today" reference was reopened. Changes, one per line:

1. Today: line ranges corrected (`mod.rs:8-11,13-19,21-25`, tests `:125-132`, `:123-132`); derives and the exact id check stated; `contracts.rs:19` imports `FamilyClaim`.
2. Today: ARC-11 also allows the real-time sonara-engine → sonara-midi (unused); "Consumers: none" was wrong (`contract_orchestration_metadata`, value-pinning tests).
3. Today: dropped note keys cited to `MidiNote` (no `extra`); `ep` cited; the `migrated_from` path and the golden step recording `null` added.
4. Section 1 and RT row: the graph allows engine → midi, so the rule is no RT caller and no new engine edge (`LazyLock`, RT-03).
5. Section 2: the const index check compiled as a minimal example on 1.99.1; `Instrument` must stay `Copy, Eq`.
6. Section 3: card names carry parentheticals and `♭`; data entry splits them; the pinned "Clarinet in Bb" test moves with them.
7. Section 3: folding to ASCII (the `[a-z0-9 ]` property was false), seven characters not six; `vc`/`cb` contradicted the three-letter minimum; `found_longer` is "same intent", not "same rule".
8. Sections 4 and 5: `written_in(clef, …)` for clef-free `check_written`; unpitched substitutes marked as channel-10 keys (75 Claves).
9. Section 8: `Seconds(u16)`/`ModerateBars(u8)` could not hold PLY-05's 0.5 s or PLY-04's under-two-bars; now `Millis`/`ModerateBeats`.
10. Section 10: the per-preset choice subset contradicted layer b's one frozen list; ids pending A12/A13 enter `ALL` only after the decision.
11. Sections 11 and 12: score order is not `Family`'s derived `Ord`; `InstrumentSpec::EMPTY` for const struct update; 20 values plus one provenance change, not 21 values; pinned tests rewritten.
12. Section 13: `resolve()` on patch paths needs a camel-case split and a narrow language list.
13. Section 14: the descriptor argument aligned with layer b's `SamplerChoices`, the parameter left to layer b (plan item IM-17); phase-1 mute and vibrato carriage aligned with its 5.7.
14. Section 14: the dataset check moved from sonara-project (cannot see sonara-midi, ARC-11) to sonara-app; `project-v2.schema.json` named.
15. Section 14 (GLD-01, CTR-01): a reported v1 → v2 migration would turn the golden `migrated_from_schema: null` into `1`; now unreported.
16. Section 14: the `instrument_type` → `part` lift removed (one fact twice, DES-01); identity precedence and `serializer.rs:350` stated; feature role restated.
17. Contracts: CTR-01, GLD-01, ARC-11, RT rows extended; ADR 0010 and ADR 0009/PERF-10 rows added; `deunicode` added to QG-13.
18. Alternatives and plan: hand-written `Serialize`, the rejected lift, a features-only schema; items IM-2, IM-5, IM-6, IM-17, IM-18, IM-19 updated.
19. Tests: names added; parent-chain invariant fixed (3.5 ends at non-stable ids); the nonexistent "song fixture" replaced by the pinned documents and the golden replay.
20. Risks, owner decisions (a table, A24 added) and Deviation 1 (count wording) updated.

Not fixed, left to the owner or other layers:

- Whether "no new dependency" (S1) allows new edges to workspace crates is still a reading (Deviation 5).
- A20 (tenor transposition) and A14 (ordinario) remain recommendations.
- Card data (ranges, GM numbers, names) was checked against model.md and sound-mapping.md, not against the cards' sources.

Cross-layer review of 2026-10-30 against layers b, c and d (one line per change):

21. Plan: rows renamed `IM-1`…`IM-20` so ids differ from layer b's `SS-`, layer c's `PF-` and layer d's `UI-` items.
22. Plan: IM-17 duplicated layer b's SS-14 (both built `SamplerChoices`); merged into SS-14 and kept as a traceability row.
23. Section 11, plan IM-20, module layout and API: layer b's chain read family-neighbour and articulation-substitution tables "in `sonara_midi::orchestration`" that no layer defined; added `fallback.rs` (O1), with presets and built-ins left in sonara-app because the module names no plugin (`mod.rs:4-6`).
24. Section 11: rule predicates sit in `sonara_midi::performance::check` (layer c), not in `sonara_app::quality`, which holds the findings.
25. Module layout: `reference.rs` added for layer c's composer reference (PF-14), which writes into this module.
26. Plan IM-8, IM-9 and section 15: the planner (layer c, PF-11) consumes `resolve()`; layer c no longer folds names itself, so `unicode-normalization` stays one edge, on sonara-midi.
27. Section 14: identity precedence and its words (*part*, *named*, *declared*, *profile*, *inferred*, *none*) aligned with layers b 3.3, c 6 and d principle 4.
28. Section 14 and "Not fixed": the unknown-`instrument_type` fallback is now specified by layer b (5.1, SS-14); the open note removed.
29. Plan IM-18: tied to layer b's SS-13 as the source of its `Inferred` claim.
30. Owner decisions and Deviation 5: the dependency-edge reading now covers layer c's `strsim` and layer d's `proptest` dev edge, so the ADR records one reading.
31. Tests: TST-07 invariant for the fallback data.

