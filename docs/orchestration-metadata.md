# Orchestration metadata

`sonara_midi::orchestration` holds machine-readable facts about orchestral instruments in
general: names and aliases in five languages, categories, pitch range, transposition, family, the
articulations they idiomatically play and how they appear in ensembles (SON-P06). It is plain
Rust data and pure functions. It depends on no audio engine, sample library or UI, adds no agent
tool and changes no contract (CTR-01).

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

## Names and aliases

Each record has display names in five languages, English, Czech, German, French and Italian, and
the abbreviation a score prints (`Instrument::names`; `Names::get(lang)`). A name is the first form
the sources give, in their spelling and capitalization; the alternates they add in parentheses
("Violine (Geige)", "Timpani (kettledrums)") are not part of the name. The German choir sections
are compounds with "Chor" ("Chortenor"), because "Tenor" alone is the solo voice. No two records
share a name in one language. `name()` is the English name.

Aliases (`Instrument::aliases`) are the forms a person may write to mean an instrument. They are
matching data, not display text:

- **Folded.** An alias is stored as `fold()` writes it: lower-case ASCII letters and digits,
  words separated by one space. The text is decomposed (Unicode NFKD) and its diacritics are
  dropped; `ß`, `œ`, `æ`, `ø`, `ł`, `♭` and `♯` are written `ss`, `oe`, `ae`, `o`, `l`, `b` and
  `is` (so "F♯" folds like "Fis"); every run of other characters is one space. "Lesní rohy" is
  `lesni rohy`, "Hörner" `horner`, "Clarinet in B♭" `clarinet in bb`.
- **Written out, never derived.** English has singular, plural and common synonyms ("French
  horn", "kettledrums", "chimes"); German, French and Italian singular and plural; Czech every
  case but the vocative, singular and plural ("lesní roh", "lesních rohů", "lesními rohy",
  "lesních rozích"). Each alias has a language (`Lang`) and a grammatical number (`Number`):
  `Singular`, `Plural`, or `Unmarked` for a form that is both, such as "housle", a Czech form
  shared by a singular and a plural case, or an abbreviation.
- **Three letters or more.** One- and two-letter abbreviations (`Vn.`, `Hn.`, `S.`) collide with
  words such as the Czech "a" and are never aliases; longer score abbreviations are (`picc`,
  `timp`, `glock`). Forms that are also common words are left out: the bare Czech "roh" (also a
  corner), "Ten." and "Bar." (English words), "Vib." (vibrato) and the like.
- **Voices and sections.** A solo voice has the singular forms and a choir section the plural:
  "tenor" names the solo tenor, "tenors" and the Czech "tenory" the section.
- **Ambiguity is data.** One text may name several targets: `bass` the double bass and the bass
  voice, `alto` the French viola and the English voice, and folding joins some Czech forms
  (`tenoru` is "tenoru", of the tenor, and "tenorů", of the tenors). Every target is kept, and the
  crate's tests list each such text, so a new collision is a deliberate change.

`all_aliases()` lists every alias with its `Target` (an instrument type, a category or a
family) in data order: the instruments in score order, then the categories, then the families.
`Family::aliases` holds the words for a whole family ("strings", "smyčce", "Blechbläser",
"cuivres", "legni"). The checks run when the data is built: an alias that is not folded, has fewer
than three letters, or appears twice in one language of one record is an error
(`InstrumentError::AliasNotFolded`, `AliasTooShort`, `DuplicateAlias`). Every display name folds
to an alias of its own record in its own language.

`resolve(text, languages)` finds the instruments, categories and families a text names. It folds
the text and reads it word by word; at each word the longest alias that starts there wins ("bass
clarinet" over "bass", "anglický roh" over a shorter form), and reading continues after it. Each
find is a `Mention`: the positions of its words among the words of the folded text, the alias
text, and every target with the alias that names it, in the order of `all_aliases()`, so an
ambiguous word lists all its targets and the caller asks. Only aliases in `languages` count
(`Lang::ALL` for all of them); a word that starts no alias is skipped. Nothing is guessed: there is
no stemming and no edit distance. "Smyčce hrají legato melodii, lesní rohy a tympány pod tím"
names the strings, the horns and the timpani. The alias index is built on first use; neither
function is for real-time code.

## Categories

A category is a group of instrument types that one word names (`CategoryId`, with a stable
`slug()`). A record names its category (`Instrument::category`), and `category(id).members()` are
the records that name it, in score order. The category's `selection()` says what the bare word
means, with the reason as its provenance:

| Category | Members | Selection |
|---|---|---|
| `clarinet` | E♭, B♭, A and bass clarinet | the clarinet in B♭ |
| `saxophone` | soprano, alto, tenor, baritone | the alto saxophone |
| `wagner_tuba` | tenor and bass | all |
| `trumpet` | piccolo, C and B♭ trumpet | the trumpet in B♭ |
| `trombone` | tenor and bass trombone | the tenor trombone |
| `cymbals` | clash and suspended cymbal | the clash cymbals |
| `bells` | glockenspiel, tubular bells, crotales, carillon | ask |
| `piano` | the six acoustic pianos | the concert grand |
| `electric_piano` | Rhodes and Wurlitzer | the Rhodes |
| `organ` | pipe and Hammond organ | the pipe organ |
| `solo_voice` | the six solo voices | ask |
| `choir` | the four choir sections | all |

`Ask` means no member is the usual meaning, so the one who asked chooses. A category's aliases are
the words for the whole group ("clarinet", "klarinety", "sbor"); a member's own words ("bass
clarinet") are aliases of its record, and a word that is a member's display name stays with that
member (the Czech "varhany" is the pipe organ).

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
instruments (the drums, cymbals and the other untuned percussion) have no range and the check
returns `CheckError::Unpitched`; a written pitch that converts outside MIDI returns
`ConversionOutOfRange`. An unknown instrument id is `instrument(id) == None`; no generic range is
substituted. A comfortable range is unknown only for the carillon, whose range belongs to each
tower instrument.

## Provenance

Every range, every articulation list and every count of players is `Cited { source }` or
`Estimate { reason }`. Cited means a source that was read states it. Each source the dataset
cites is written out once, in the dataset's source list
(`crates/sonara-midi/src/orchestration/data/sources.rs`): its author or publisher, its title (with
the edition for a book) and where it was read, a URL or, for a printed book without one, a page.
A `Cited` value gives each source it rests on in full, followed by what that source states, so
its text always names a URL or a page; the dataset test checks this. An `Estimate` whose reason
rests on a source names it in full the same way.

The sources are orchestration handbooks (Berlioz, Rimsky-Korsakov, Forsyth, Widor, Prout), the
documentation of instrument makers and sample-library makers, specifications, and articles of
the English, Czech, German and French Wikipedia, read in October 2026. Wikipedia is a tertiary
source under CC BY-SA, used for facts only. A `Cited` value may end with a remark that names the
part of it that is estimated, such as a low end taken from the usual extension. The ranges that
are estimates are those no source states as such: the playable range of the guitar, lute,
mandolin, carillon and honky-tonk piano, and the comfortable range of the alto flute, the horn in
F, the Wagner tubas, the bass trombone, the cimbasso, the harp, the guitar, lute and mandolin and
of most keyboards (equal to the playable range where every key is equally reachable). The
articulation lists of the wood block, temple blocks, sleigh bells, lute, felt and honky-tonk
pianos and harpsichord are estimates from general practice. A count of players that no source
states is an estimate of the instrument's usual scoring.

## Articulations

`Articulation` is a vocabulary of 77 ids (`sustain`, `pizzicato`, `trill_half`, `mute_cup`, …),
spelled like the catalog manifest's; `from_id` accepts only an exact id. Each id has a one-sentence
`description()` and a `kind()`, one of ten: long note, short note, ornament, transition, effect,
timbre modifier, mute, dynamic shape, keyboard control and vocal.

- **Order.** `Articulation::ALL` is append-only, because a choice list built from it is automated
  by index: an id is never removed, renamed or moved, and a new id goes at the end. The 15 original
  ids come first (`is_stable()`), then 47 established technique terms and 15 newer terms that at
  least one sample library records, each group alphabetical. Other terms are added later, at the
  end.
- **Ordinario.** No id means the instrument's ordinary sound; there is no `ordinario` id, and
  `sustain` is the long, held note.
- **Passage states.** The eight mute ids (`con_sordino`, `mute_cup`, `mute_harmon`,
  `mute_plunger`, `mute_bucket`, `stopped`, `palm_mute`, `xylophonic`) and the two vibrato ids
  (`vibrato_on`, `non_vibrato`) name a `Mute` or a `Vibrato` that holds over a passage (`axis()`),
  not a note's primary articulation. `Mute::Open` and `Vibrato::Ordinary` are the defaults and have
  no id.
- **Refinements.** `parent()` names the general id that a specific one refines (`trill_half` →
  `trill`, `mute_cup` → `con_sordino`, `col_legno_battuto` → `col_legno`), so a library without the
  refinement can play the parent. The original ids refine nothing.
- **Rolls.** A mallet instrument's roll is `tremolo`; the timpani, drums, cymbals and other
  unpitched percussion roll with `roll`.

`Instrument::articulations` lists what the instrument idiomatically plays, each record its own
list. The ordinary sound has no id there either: a mallet instrument's ordinary stroke is not
`staccato`, and the freely decaying note of a piano, harpsichord, celesta or plucked instrument
is not `sustain`. A technique the vocabulary does not have yet (such as `detache` or `subtone`)
joins the records when it is appended to the vocabulary.
`articulation_availability(id, library_declared)` returns:

- `Supported` only when the library declared the id (`library_declared`, passed in);
- `IdiomaticUnverified` when the instrument plays it but nothing confirms the library does;
- `NotIdiomatic`; `UnknownArticulation` for an id outside the vocabulary.

A `Request` states what a passage asks for: one primary articulation (`None` for the ordinary
sound), a `Mute` and a `Vibrato`. `Request::new` rejects a mute or vibrato id as the primary
(`RequestError::NotPrimary`), so those always travel as states; `Request::default()` is the
ordinary sound, open, with the ordinary vibrato. `request_availability(request, library_declared)`
answers each of the three parts on its own, as `articulation_availability` answers for the part's
id; a part at its default has no id and is `Supported`. A declared `con_sordino` does not answer
for a requested cup mute: playing the general mute instead is a substitution, not availability.
Whether a library recorded the parts together, such as a muted staccato, is not modelled here.

How a library produces an articulation (patch, key switch, controller, note length) is not
modelled.

## Ensemble profile

Every record carries an `EnsembleProfile` (`Instrument::ensemble`; the type allows `None`): its
`Forms` (`Solo`, `Section` or `Both`) and, for each `Era` (`Classical`, `Romantic`, `Film`,
`Chamber`), the typical players as an `EraPlayers` value. Its `players` is a `PlayerRange`
(`min <= max`; `PlayerRange::NOT_USED`, `0..=0`, means the instrument is not used there) or
`None` when the source states no number; `note` keeps the qualifier that came with the number,
and `provenance` says where it comes from. The count is the players of the instrument in such a
score, all parts together: a Romantic orchestra's violins, 16 first and 14 second (16 and 16 in
Die Walküre), count as 30 to 32, while a choir section counts the singers of its one part. The
section sizes of a particular orchestra are ensemble data, not part of the profile.

## Validation

A record is an `InstrumentSpec` checked by `Instrument::try_new(instrument_type, spec)`. Records
are written as literals; one that leaves fields out ends in `..InstrumentSpec::EMPTY`, which
gives them their "not known" value. `EMPTY` itself is not a valid record: its name and its
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
Compile-time checks also hold the identity together: every type id and every articulation id is
unique lower-case `snake_case`, `InstrumentType::ALL` and `Articulation::ALL` list every variant
once in declaration order, every articulation has a description, each family is one block of the
score order, and `INSTRUMENTS` holds one record per type, each at its type's place in score
order.
`crates/sonara-midi/tests/orchestration.rs` also checks the type ids against an independently
written list, provenance, the score-order conventions above, that every record rebuilds from its
spec, and that a fixture breaking each check is rejected. It also compares every record's ranges,
transposition, written range, clefs, forms and articulations with tables written out
independently of the data.

## Coverage

All 85 types have a record, listed in score order by `INSTRUMENTS` and `instruments()`. The
records live in one file per family under `crates/sonara-midi/src/orchestration/data/`
(woodwinds, brass, percussion, plucked, keyboards, voices, strings). A display name is the first
form of the English name, with a flat spelled `♭` ("Clarinet in B♭"); an alternate in parentheses
("Timpani (kettledrums)") is not part of it. Range extensions and gaps, alternative tunings and
techniques outside the vocabulary are not modelled.
