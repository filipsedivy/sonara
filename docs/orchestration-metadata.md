# Orchestration metadata

`sonara_midi::orchestration` holds machine-readable facts about orchestral instruments in
general: pitch range, transposition, family, the articulations they idiomatically play and how
they appear in ensembles (SON-P06). It is plain Rust data and pure functions. It depends on no audio engine, sample
library or UI, adds no agent tool and changes no contract (CTR-01).

## Identity

Ids name the **acoustic instrument type** (`violin`, `clarinet_bb`, `horn_f`), not a sample
preset. They are independent of the preset ids of the instrument catalog
(`<library>/<patch path>`); the catalog may map presets to these ids later. Display names are not
identifiers. Family slugs equal the catalog's (`strings`, `woodwinds`, `brass`, `percussion`,
`keyboards`, `plucked`, `choir`, `other`).

`InstrumentType` is the typed id: one variant for each of the 85 instrument types the metadata
knows, from the piccolo to the double bass. `id()` gives the string id, which is the external form
and is never renamed; `from_id` accepts only an exact id. Each type has one family, which its
record shares: voices, solo and choral, are `choir`, and no type is `other`.

The variants are in **score order**, the order of the staves in a full score: woodwinds, brass,
percussion, harp and the other plucked strings, keyboards, voices, strings. Within a family the
order follows the usual conventions (auxiliary instruments beside their family, saxophones after
the clarinets, horns at the top of the brass, timpani first in the percussion, then untuned and
tuned instruments, solo voices before the choir); the order inside the untuned percussion, the
keyboards and the low brass is an estimate. `InstrumentType::ALL` lists the types in that order,
and each family is one block of it. The order is not stored anywhere, so a later type may be
inserted anywhere in it.

## Pitch conventions

- Pitches are MIDI note numbers, middle C = 60 = C4, the convention of `theory`. The model is in
  semitones: enharmonic spelling is out of scope.
- `WrittenPitch` and `SoundingPitch` are separate types. Metadata stores **sounding** ranges;
  written ranges are derived.
- `Transposition` is `sounding − written` in semitones: Bb clarinet −2, horn in F −7, English
  horn −7, piccolo +12, xylophone +12, glockenspiel +24, double bass −12, contrabassoon −12.
  Octave transposition is the same field. Conversions return `None` outside MIDI 0–127.
- Range endpoints are inclusive.

## Clefs

Each record lists the clefs its parts are written in, in order of use (`Instrument::clefs`; the
bassoon's are bass, tenor and treble). `Clef` has seven values with stable slugs: `treble`,
`treble_8vb`, `bass`, `alto`, `tenor`, `percussion` and `single_line`. The percussion clef and the
one-line staff show no pitch.

A clef can carry an octave of its own: `Clef::octave` is sounding minus written in octaves, −1 for
`treble_8vb` (the notes sound an octave lower than written) and 0 for every other clef. That
octave belongs to the staff, not to the instrument, so the octave is stated once: in the clef or in
the transposition, never in both. A tenor voice is at transposition 0. In `treble_8vb` its
sounding C3 (48) is written C4 (60); on a bass-clef staff shared with the basses the same note is
written at pitch. Instruments that are written whole octaves away from their sound in every clef
(the piccolo, the double bass, the contrabassoon, the xylophone and, two octaves, the
glockenspiel) keep those octaves in their transposition.

`Instrument::written_in(clef, sounding)` gives the pitch to write in a clef: the transposition,
then the clef's octave. The clef need not be one the record lists. It returns `None` for an
unpitched instrument, for a clef that shows no pitch, and outside MIDI 0–127. `written_playable`
and `check_written` stay clef-free: they apply the transposition alone.

## Ranges

| Range | Meaning |
|---|---|
| `playable` | the outer limits; outside it a note is `BelowPlayable` or `AbovePlayable` |
| `comfortable` | ordinary orchestral writing, inside `playable`; `None` when unknown |

`check_sounding` and `check_written` return `Within`, `OutsideComfortable` (a warning: still
playable), `BelowPlayable` or `AbovePlayable`. Nothing is clamped or changed. Unpitched
instruments (the snare drum) have no range and the check returns `CheckError::Unpitched`; a
written pitch that converts outside MIDI returns `ConversionOutOfRange`. An unknown instrument id,
or a type without a record, is `instrument(id) == None` (and `InstrumentType::get` is `None`); no
generic range is substituted.

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

## Ensemble profile

A record may carry an `EnsembleProfile` (`Instrument::ensemble`, `None` when not recorded): its
`Forms` (`Solo`, `Section` or `Both`) and, for each `Era` (`Classical`, `Romantic`, `Film`,
`Chamber`), the typical players as an `EraPlayers` value. Its `players` is a `PlayerRange`
(`min <= max`; `PlayerRange::NOT_USED`, `0..=0`, means the instrument is not used there) or
`None` when the source states no number; `note` keeps the qualifier that came with the number,
and `provenance` says where it comes from. The section sizes of a particular orchestra are
ensemble data, not part of the profile. No record of the dataset carries a profile yet.

## Validation

A record is an `InstrumentSpec` checked by `Instrument::try_new(instrument_type, spec)`. Records
are written as literals ending in `..InstrumentSpec::EMPTY`, so a field added later gets its
default without editing every record. `EMPTY` itself is not a valid record: its name and its
articulation provenance are empty, so a record that forgets either does not build.

`PitchRange::try_new`, `PlayerRange::try_new` and `Instrument::try_new` reject:

- inverted or non-MIDI ranges, a comfortable range outside the playable one, and a transposition
  that moves the range outside MIDI;
- an inverted player range;
- an empty or blank display name;
- an unpitched instrument outside the percussion family (every other family plays definite
  pitches);
- an articulation listed twice;
- a clef listed twice; for a pitched instrument, a clef that shows no pitch, a clef with an octave
  of its own on a transposing instrument (the octave would count twice), and a clef in which the
  written playable range leaves MIDI;
- an empty or blank provenance anywhere in the record: a range, the articulation list or an era
  of the ensemble profile.

A record is keyed by its `InstrumentType`, so its id cannot be malformed. The shipped data is
built with the panicking `new` in a `static`, so invalid data is a compile error.
Compile-time checks also hold the identity together: every type id is unique lower-case
`snake_case`, `InstrumentType::ALL` lists every type once in declaration order, each family is one
block of the score order, and `INSTRUMENTS` lists at most one record per type, in score order.
`crates/sonara-midi/tests/orchestration.rs` also checks the type ids against an independently
written list, provenance, the score-order conventions above, that every record rebuilds from its
spec, and that a fixture breaking each check is rejected.

## Coverage

16 of the 85 types have a record, listed in score order by `INSTRUMENTS` and `instruments()`:
piccolo, flute, oboe, English horn, Bb clarinet, bassoon, contrabassoon, horn in F, Bb trumpet,
tenor trombone, snare drum, glockenspiel, xylophone, violin, cello, double bass. The records live
in one file per family under `crates/sonara-midi/src/orchestration/data/`. The other types
(among them the viola, harp, timpani, tuba, bass clarinet, the other clarinets and trumpets, the
voices and the keyboards) have no record yet; alternative tunings and extended techniques are not
modelled. Absence of a record means "no metadata", not "unrestricted".
