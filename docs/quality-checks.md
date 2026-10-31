# Quality checks (SON-P18)

`sonara_app::quality` returns explained findings instead of a score. A `Finding` has:

- a **severity**: `Error` (the result is broken), `Warning` (probably not what was meant) or
  `Recommendation` (advice; nothing is wrong);
- a stable **code** and a **message** that names the thing;
- an **explanation** of why it matters and what to do;
- the **place**: `track_id`, `clip_id` and the `note_ids` concerned.

A `QualityReport` lists the findings with errors first. `is_sound()` is true without errors, and
`to_json()` adds the counts. It is a library API; it adds no agent tool, CLI command or schema
field (CTR-01). `check_project` only reads the project.

## Project checks

| Code | Severity | Case |
|---|---|---|
| `INSTRUMENT_MISSING` | error | an instrument track without an instrument |
| `DUPLICATE_NOTE` | error | notes of one pitch and channel with the same start |
| `NOTE_AFTER_CLIP_END` | warning | notes starting after the end of a plain (unlooped, unoffset) clip |
| `OVERLAPPING_NOTES` | warning | a note starting inside an earlier note of the same pitch and channel |
| `FADER_HOT` | warning | a fader above +6 dB |
| `EMPTY_CLIP` | recommendation | an unmuted MIDI clip without notes |
| `UNIFORM_VELOCITY` | recommendation | 8 or more notes in a clip with one velocity |
| `SILENCED_BY_SOLO` | recommendation | a track with content that a solo silences |

Muted clips are skipped. The notes' pitch, length and velocity ranges are enforced by the model
and are not findings.

## Audio checks

`audio_findings(&AnalysisReport)` maps each analyzer issue (`NONFINITE`, `OVER_FULL_SCALE`,
`CLIPPING`, `TRUE_PEAK`, `VERY_LOUD`, `QUIET`, `SILENT`, `PHASE`, `DC_OFFSET`, `LOW_END_HEAVY`)
to a finding with the analyzer's message and an explanation. The analyzer's `info` is a
recommendation. The thresholds are the analyzer's and are unchanged (GLD-01).

## Orchestration checks

`quality::orchestration::check_orchestration(&ProjectData)` checks the tracks that play a part
(project schema version 2) against the orchestration rules that a project's notes can be checked
against. It returns a `QualityReport` of its own; `check_project` is unchanged.

- **Which tracks.** A track whose part names an instrument type of the orchestration metadata. A
  track without a part is not checked, whatever its name or sound. A part that names an unknown
  instrument is reported as a warning when the project opens and is not checked.
- **Which notes.** The notes of the track's unmuted MIDI clips where the timeline plays them: a
  looped clip repeats its content, a note crossing the clip's end is cut there, and a note outside
  the clip is not played, as a MIDI file export writes them. So the rules that compare parts see
  the notes that sound together. The project's tempo and time signatures measure times and bars.
- **What a note says.** Its sounding pitch, its velocity as its dynamic (soft, medium or loud, by
  `sonara_midi::performance::check::velocity_band`) and its own articulation, such as
  `pizzicato` or `con_sordino`.
- **The finding.** The code is the rule's id; the rule's error, warning and hint are an `Error`,
  a `Warning` and a `Recommendation`. The message gives the rule's title, the tracks concerned,
  the bars and what was found, for example `Note outside the playable range on 'Violins', bar 2:
  1 note below the playable range of the Violin, G3 (55) to A7 (105).` The explanation says what
  the rule looks for and the source it rests on, or that it is an estimate. The finding is placed
  on the first track concerned, with every note concerned, and with the clip when the notes all
  lie in one.
- **Order.** Most serious first, then by rule in the order below, then by where the finding
  starts. The same project always gives the same report.
- **Errors.** `CheckError::InvalidNote` names a note the model refuses (a velocity of 0, for
  example) that was written into a note's fields directly; a project the model validated never
  has one.

| Code | Rule |
|---|---|
| `RNG-01` | Note outside the playable range |
| `RNG-02` | Note outside the comfortable range |
| `RNG-03` | Sustained extreme high brass |
| `SPC-01` | Low-interval limit |
| `SPC-02` | Dissonance in the low bass |
| `DBL-02` | Woodwind order inverted in a doubling |
| `DBL-03` | Same-colour winds in octaves without strings |
| `DBL-04` | Woodwind octave above a trombone |
| `DBL-09` | Strings and brass in unison expected to blend |
| `VOI-12` | Sustained multi-stop string chords |
| `PLY-01` | Wind breath |
| `PLY-02` | Brass endurance |
| `PLY-04` | Mute change time |
| `PLY-05` | Pizzicato and arco switch |
| `PLY-06` | Pizzicato speed |
| `PLY-12` | Timpani thirds |
| `PLY-14` | Fast repeated notes and tonguing |
| `PLY-15` | Clarinet octave leaps |
| `TEX-01` | Characteristic colours overused |
| `TEX-02` | Percussion all together, or never resting |

What each rule compares, its thresholds and their sources are data in
`sonara_midi::orchestration::rules`; the predicates are `sonara_midi::performance::check`.

## Limits

The project checks know nothing about key or harmony; the theory validators
(`sonara_midi::theory::validate`) are not connected yet. The orchestration checks read no
role, harmony or notated dynamic, which a project does not store, so the rules that need them are
not run on a project. They do not read the sampler's `articulation` and `dynamics` parameters or
their automation: those choose a sound, and the written part is the notes. A project is checked
as stored, not as rendered.

## Tests

`crates/sonara-app/tests/quality.rs` covers each code with a project built to have it, a clean
project with none, the ordering, the explanation and place of every finding, that checking
changes nothing, and a clipped square wave for the audio mapping.

`crates/sonara-app/tests/quality_orchestration.rs` gives each orchestration rule a project that
breaks it and one that keeps it, with the passages of the predicates' own contract tests, and
covers a project without parts, the timeline (clips at different starts, a loop, a muted clip, a
note cut at its clip's end), a finding across tracks, the message and explanation, the order, and
an invalid note.
