# Orchestration metadata

`sonara_midi::orchestration` holds machine-readable facts about orchestral instruments in
general: pitch range, transposition, family and the articulations they idiomatically play
(SON-P06). It is plain Rust data and pure functions. It depends on no audio engine, sample
library or UI, adds no agent tool and changes no contract (CTR-01).

## Identity

Ids name the **acoustic instrument type** (`violin`, `clarinet_bb`, `horn_f`), not a sample
preset. They are independent of the preset ids of the instrument catalog
(`<library>/<patch path>`); the catalog may map presets to these ids later. Display names are not
identifiers. Family slugs equal the catalog's (`strings`, `woodwinds`, `brass`, `percussion`,
`keyboards`, `plucked`, `choir`, `other`).

## Pitch conventions

- Pitches are MIDI note numbers, middle C = 60 = C4, the convention of `theory`. The model is in
  semitones: enharmonic spelling is out of scope.
- `WrittenPitch` and `SoundingPitch` are separate types. Metadata stores **sounding** ranges;
  written ranges are derived.
- `Transposition` is `sounding − written` in semitones: Bb clarinet −2, horn in F −7, English
  horn −7, piccolo +12, xylophone +12, glockenspiel +24, double bass −12, contrabassoon −12.
  Octave transposition is the same field. Conversions return `None` outside MIDI 0–127.
- Range endpoints are inclusive.

## Ranges

| Range | Meaning |
|---|---|
| `playable` | the outer limits; outside it a note is `BelowPlayable` or `AbovePlayable` |
| `comfortable` | ordinary orchestral writing, inside `playable`; `None` when unknown |

`check_sounding` and `check_written` return `Within`, `OutsideComfortable` (a warning: still
playable), `BelowPlayable` or `AbovePlayable`. Nothing is clamped or changed. Unpitched
instruments (the snare drum) have no range and the check returns `CheckError::Unpitched`; a
written pitch that converts outside MIDI returns `ConversionOutOfRange`. An unknown instrument id
is `instrument(id) == None`; no generic range is substituted.

## Provenance

Every range and every articulation list is `Cited { source }` or `Estimate { reason }`. Cited
means a source that was read states it. Range sources were read on 2026-10-28
(English Wikipedia articles, used for facts only; each `Cited` record names its URL and what it
states). Wikipedia is a tertiary source under CC BY-SA; only the numbers are taken. Verify
against orchestration handbooks before relying on a value for production.
Estimates: cello and double bass upper limits, horn in F, glockenspiel and xylophone ranges, and
**all** articulation lists (general orchestration practice).

## Articulations

`Articulation` is a vocabulary of 15 stable ids (`sustain`, `pizzicato`, `col_legno`, …), spelled
like the catalog manifest's. `Instrument::articulations` lists what the instrument idiomatically
plays. `articulation_availability(id, library_declared)` returns:

- `Supported` only when the library declared the id (`library_declared`, passed in);
- `IdiomaticUnverified` when the instrument plays it but nothing confirms the library does;
- `NotIdiomatic`; `UnknownArticulation` for an id outside the vocabulary.

How a library produces an articulation (patch, key switch, controller, note length) is not
modelled.

## Validation

`PitchRange::try_new` and `Instrument::try_new` reject inverted or non-MIDI ranges, a comfortable
range outside the playable one, a transposition that moves the range outside MIDI, a malformed id
and duplicate articulations. The shipped data is built with the panicking `new` in a `static`,
so invalid data is a compile error; `crates/sonara-midi/tests/orchestration.rs` also checks
unique ids, provenance and that bad fixtures are rejected.

## Coverage

16 instruments: violin, cello, double bass, flute, piccolo, oboe, English horn, Bb clarinet,
bassoon, contrabassoon, horn in F, Bb trumpet, tenor trombone, glockenspiel, xylophone, snare
drum. Not covered: viola, harp, timpani, tuba, bass clarinet, other transposing clarinets and
trumpets, voices, keyboards, alternative tunings and extended techniques. Absence of an
instrument means "no metadata", not "unrestricted".
