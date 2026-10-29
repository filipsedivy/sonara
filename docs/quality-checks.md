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

## Limits

The checks know nothing about key, harmony or the orchestral ranges of instruments; the
theory validators (`sonara_midi::theory::validate`) and the orchestration metadata are not
connected yet. A project is checked as stored, not as rendered.

## Tests

`crates/sonara-app/tests/quality.rs` covers each code with a project built to have it, a clean
project with none, the ordering, the explanation and place of every finding, that checking
changes nothing, and a clipped square wave for the audio mapping.
