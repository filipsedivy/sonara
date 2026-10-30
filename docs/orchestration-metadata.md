# Orchestration metadata

`sonara_midi::orchestration` holds machine-readable facts about orchestral instruments in
general: names and aliases in five languages, categories, pitch range, transposition, family, the
articulations they idiomatically play, how they appear in ensembles, their General MIDI voice,
for the instruments played from a keyboard how the keyboard makes and stops the sound, and the
playability facts that rules read, such as how long a mute takes to put on (SON-P06). It also
holds ensemble templates: the parts of an orchestra or a smaller group in score order, with
their players, string sizes and stereo places, and a catalogue of orchestration rules with the
thresholds their checks compare with.
It is plain Rust data and pure functions. It depends on no audio engine, sample library or UI,
adds no agent tool and changes no contract (CTR-01).

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

Every range, every articulation list, every count of players and every keyboard mechanics record
is `Cited { source }` or `Estimate { reason }`; every General MIDI substitute is an
`Estimate { reason }`, since a source states only that GM1 has no voice for the instrument, not
which one to play instead. Cited means a source that was read states it. Each source the dataset
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

## General MIDI voice

Every record states its General MIDI Level 1 voice as a `GmVoice` (`Instrument::gm`). GM1 numbers
its programs 1-128 (`GmProgram`; `program_change()` gives the 0-127 value a program change
carries) and assigns the channel-10 keys 35-81 to percussion sounds (`GmNote`). A key outside that
map, such as the Roland GS castanets on 85, has no `GmNote`. An exact voice and a substitute are
separate variants, so a caller cannot mistake one for the other:

- `Program`: the GM1 program whose name names the instrument or its class at its register, such as
  41 Violin, 61 French Horn, or 72 Clarinet for the clarinets in B♭ and A. 43 records.
- `Percussion`: the GM1 channel-10 keys whose names name the instrument, the preferred key first:
  the bass drum 35 and 36, the snare drum 38, the triangle 81 and 80, the tambourine 54, the wood
  block 76 and 77 and the maracas 70.
- `Substitute { target, fit, provenance }`: GM1 has no voice for the instrument, and `target` is the
  nearest program (a pitched instrument) or channel-10 keys (an unpitched one). `fit` says how
  close it comes: `Approximate` for a near relative or another size, such as 74 Flute for the alto
  flute or 54 Voice Oohs for a solo voice, and `Weak` for a different instrument that shares only
  part of the sound, such as 61 French Horn for a Wagner tuba or 75 Claves for the castanets. The
  provenance is an estimate that cites the GM list read and says why the target is the nearest.
  33 records, among them every pitched instrument GM1 has no program for, the cornet, flugelhorn
  and piccolo trumpet (57 Trumpet), the E♭ and bass clarinets (72 Clarinet), the studio grand (2
  Bright Acoustic Piano) and the clash and suspended cymbals (49 Crash Cymbal 1, a drum-kit
  crash).
- `NoEquivalent`: GM1 has nothing that would not misrepresent the instrument: the tam-tam, the
  whip and the prepared piano.

Whether a substitute is ever played is the caller's decision.

`ORCHESTRAL_PERCUSSION_MAP` lists every GM1 key that names an orchestral percussion instrument,
with that instrument, in key order, and `percussion_instrument(key)` looks a key up. It is built
from the records' exact keys at compile time, and a compile-time check proves that no key names
two instruments; substitute keys have no entry. It sits beside the drum kit's map
(`sonara_midi::gm::GM_MAP`), which it does not change: the drum kit's sounds are recorded golden
data (GLD-01), and its map has, for example, no triangle (80, 81).

## Keyboard mechanics

The fifteen keyboards and the carillon, a percussion instrument played from a baton clavier, carry
`KeyboardMechanics` (`Instrument::keyboard`); every other record has none. A record holds:

- `action`: what a key moves. `Grand` (double escapement) and `Upright` (single escapement, spring
  return) for the acoustic pianos, `TineHammer` (Rhodes), `ReedHammer` (Wurlitzer), `PlateHammer`
  (celesta), `Plunger` (Clavinet), `Jack` (harpsichord), `Tangent` (clavichord), `PipeValve` (pipe
  organ), `Contacts` (Hammond), `ReedValve` (accordion) and `Baton` (carillon).
  `Action::is_touch_sensitive` says whether the force of the stroke sets the loudness: not for the
  harpsichord, the organs or the accordion, which change it by their choirs, stops, swell pedal or
  bellows.
- `keys`: the sounding pitches of the lowest and the highest key, every key between them present
  (`key_count()`); 88 keys from A0 to C8 on the pianos and the Rhodes. An instrument with registers
  is given at its unison (8-foot) register, the accordion by its right-hand keyboard, and the
  carillon by a representative 49-key clavier.
- `pedals`: each `Pedal` has a `PedalKind` (`Sustain`, `Sostenuto`, `UnaCorda`, `Soft`,
  `PracticeMuffler`, `FootSwell`, `Expression`, `Crescendo`) and the MIDI controller it sends, or
  none where MIDI 1.0 defines none (the practice muffler, the crescendo pedal). `controller(kind)`
  looks one up. A `Controller` is 0–119: MIDI 1.0 reserves 120–127 for channel mode messages. A
  grand sends 64 (sustain), 67 (una corda) and 66 (sostenuto); an upright's half-blow pedal uses the
  soft-pedal controller, 67; the Clavinet's foot swell and the organs' swell pedals send 11. The
  list of pedal kinds is open. Pedals are controls, never articulations, and other controls (a
  Rhodes tremolo, drawbars, a Leslie, registration) are not modelled here.
- `resonance`: `Absent`, `Weak`, `DampersRaised` (the pianos' strings ring along while the sustain
  pedal holds the dampers up) or `Undamped` (the carillon's bells).
- `noises`: the sounds of the mechanism a library may record on its own (`KeyRelease`,
  `DamperReturn`, `Pedal`, `Hammer`, `KeyClick`, `Wind`, `Bellows`, `Clapper`, `Rattle`).
- `decay`: `TwoStage` (the pianos' prompt sound and aftersound), `Decaying` (one stage while the
  key is held), `WhileHeld` (no decay while wind or air is supplied: the organs and the accordion),
  `Free` (the carillon's bells ring on whatever the key does) or `PerKey` (the prepared piano).
- `timbre`: a lasting change to the ordinary sound, `Felt`, `Prepared` (the pitch of a prepared
  key is unreliable) or `DetunedUnisons` (the honky-tonk piano), or none.
- `note`: what the values leave out, such as the Hammond's tonewheel leakage or the room an organ
  sounds in; `provenance` for the record.

## Playability facts

`Instrument::facts` lists what rules need to know about playing an instrument beyond its ranges
and articulations: a `Fact` is a `FactKind` with its values and a provenance, each kind at most
once per record. Most records have none. The kinds:

- `Breath { held_note }`: the longest one note can be held in the medium register at piano, from
  Widor's experiments: about 40 seconds on the clarinets in B♭ and A (p. 33), at most 10 bars of
  moderate 4/4 on the saxophones (p. 127) and 12 bars at crotchet = 120 on the trumpets in C and B♭
  (p. 70; half as long forte). Other winds and the voices have none. Widor's horn figures (p. 61)
  come from an experiment he calls a dangerous one in the orchestra, his bassoon and contrabassoon
  figures (pp. 45, 48) are for the low and upper registers, and no other source that was read
  gives the figure.
- `MuteChange { on, off }`: the time to put a mute on and take it off. The strings take a couple
  of bars of moderate time on and a little less off (Forsyth, p. 374); every brass instrument with
  a mute takes an estimated two bars each way. The flugelhorn, which lists no mute, has none.
- `ArcoPizzSwitch { to_arco }`: about half a second from pizzicato back to arco (Forsyth, p. 367,
  for the violin; the other bowed strings take the same, an estimate).
- `SustainedStops { notes }`: a string player holds the two upper notes of a chord; chords of
  three or four notes are played quickly (Rimsky-Korsakov, p. 69).
- `Retune { whole_tone, drums }`: the timpani. A pedal drum changes a step of the scale every
  crotchet up to crotchet = 150, so a whole tone takes 400 ms; a standard set has four drums. Drums
  tuned by screws needed two or three bars of 4/4 moderato (Forsyth, p. 46).
- `HarpPedals { left, right, change }`: the harp's seven pedals as `LetterName`s, D C B under the
  left foot and E F G A under the right (Forsyth, p. 464), and an estimated beat of moderate time
  for one foot to change one pedal.
- `NotesPerHand { notes }`: four notes at most in one hand on the harp (Rimsky-Korsakov, p. 28).

A duration is a `Span`: `Millis`, or `ModerateBeats`, beats of a moderate 4/4 as the sources count
them. `Span::millis` converts beats at `MODERATO_BPM`, 120 crotchets per minute (an estimate:
Wikipedia's tempo list gives moderato as 108–120, and Widor times the trumpet's breath "in
moderate tempo" at 120). A span states the time a player needs, so it converts at that tempo
whatever the tempo of the piece; a rule measures the time a piece leaves with the piece's own
tempo. Whether an instrument double-tongues is not a fact here: its articulation list says so
(`multi_tongue`).

## Ensembles

`ENSEMBLES` (also `ensembles()` and `ensemble(id)`) holds thirteen ensemble templates, keyed by
`EnsembleId`: the Classical orchestra (Haydn, Mozart), the Beethoven orchestra, the Romantic
orchestra (Brahms, Tchaikovsky), the late-Romantic orchestra (Wagner, Mahler, Strauss), a film
orchestra, a chamber orchestra, a string orchestra, a big band, chorus and orchestra (Brahms's Ein
deutsches Requiem), a string quartet, a wind quintet, a brass quintet and a piano trio. A template
is a recipe, not project data: it names instrument types only, never a library, plugin or
preset, and applying one (sounds, tracks, folders, buses) is the application's work. The
`orchestral` project template is not an ensemble and does not change. A template holds:

- `parts`: one `Part` per track, in the order its score prints them. A part has a `slug`
  (`horn_3`), a display `name` ("Horn 3", "Piccolo (Fl. 3)") and a `short` name ("Hn. 3"), the
  `instrument` it plays, the `alternatives` a work may use instead (the clarinet in A for the
  clarinet in B♭), whether it is `required` or used only by some works, its `pan` and a `note`
  ("finale only").
- Each part's `players`: `Own` with a `PlayerRange` (one for a wind part, 16 to 20 singers for a
  choir section); `Section` for a string section, whose players the chosen string size gives; or
  `Doubling { by }`, played by the player of part `by`, who changes instrument, so the two never
  sound at once: the third flute's piccolo, the fifth to eighth horns' Wagner tubas, a
  percussionist's second instrument. That part has one player of its own and another instrument.
- `string_sizes`, the default first: Rimsky-Korsakov's full (16-14-12-10-8), medium (12-10-8-6-4)
  and small (8-6-4-3-2) orchestras, Die Walküre's 16-16-12-12-8, Gurre-Lieder's 20-20-16-16-12
  and the chamber string orchestras of 21 and 12 players.
- `seatings`, the default first: `American` (the violins together on the left) and `Antiphonal`
  (first and second violins on opposite sides); the Classical, Beethoven and late-Romantic
  orchestras default to antiphonal, the repertoire's own layout. A part's `Pan` holds a track pan
  (−1 to +1, seen from the audience) for each seating, read off the seating plans and kept within
  0.6 of the centre; every pan is an estimate. The big band and the chamber groups offer no
  seating: the big band's parts sit where their section puts them, the chamber groups stay
  centred. A source recorded with the players in their places carries the stage image, so
  applying the pan is the caller's decision.
- `order`: `FullScore` for the orchestras and the string quartet, the order of `InstrumentType`,
  except that a doubled instrument may sit directly below its player's part (a doubling placed
  anywhere else keeps the order of its type); `Edition` where the
  group's own scores print another order: the big band (rhythm section at the bottom), the wind
  quintet (horn between clarinet and bassoon), the brass quintet (horn below the trumpets) and the
  piano trio (piano at the bottom).
- `provenance`: the scorings and handbooks the roster follows, and what is estimated.

Parts no instrument type covers are left out rather than invented, and the template's provenance
names them: the late-Romantic orchestra's bass trumpet, contrabass trombone, cowbells and hammer
(and the Rute, a beater on the bass drum), the film orchestra's synthesizer layer, and the big
band's guitar and drum kit, so the big band lists 15 of its 17 players. Where a near type stands
in, such as the horn in F for the Classical orchestra's natural horn or the tenor trombone for
Beethoven's alto trombone, the part plays it and its note says so.

## Rules

`orchestration::rules` holds orchestration rules as data. The predicates that check a project are
code elsewhere; this module holds what they read, and nothing in it runs on a real-time thread.

`RULES` holds 67 rules keyed by `RuleId`, whose stable `code()` is the external form: `RNG-01` to
`RNG-04` (ranges), `SPC-01` to `SPC-08` (spacing and low intervals), `DBL-01` to `DBL-09`
(doubling), `BAL-01` to `BAL-09` (balance), `MSK-01` to `MSK-04` (masking), `VOI-01` to `VOI-12`
(voicing), `PLY-01` to `PLY-15` (playability) and `TEX-01` to `TEX-06` (texture). `from_code`
accepts only an exact code. A rule holds:

- a `title` and a `check`: what the check looks for, in words, with the numbers it compares;
- its `inputs` (`RuleInput`): the instrument, notes, dynamic, tempo, metre, part, role, harmony,
  articulation, weights, register factors, harp pedals or timpani tuning it needs;
- its `severity`: a `Level` (`hint`, `warning`, `error`) that is `Fixed`, `ByDynamic` (one level
  from pp to mf and a stronger one at f and ff) or `ByCase` (a stronger level in a case the check
  tells apart, such as a harp chord of more than eight notes). Which level a rule deserves is an
  estimate throughout: a judgement of how firmly its source states it;
- its `provenance`: the handbook passage it comes from (Rimsky-Korsakov, Forsyth, Widor, and an
  arranging course for the low-bass dissonance), with a remark when a number in it is estimated.
  The two range rules rest on the records' own ranges and say so.

The doubling tables hold what the doubling and masking rules read:

- `NAMED_DOUBLINGS`: 78 doublings the handbooks name, each with its `members` (a `Selector` and,
  where the source counts them, how many play, such as two oboes), the `spacings` the source names
  (`Unison`, `Octaves(n)` with the members listed from the top, `Chord`), the colour or effect in
  words, which members prevail (`Members`, `Blend`, `Apart` for strings and brass that stay
  separate, `ByRegister` for woodwind pairs whose leader changes with the register, or
  `Unstated`), whether the doubling builds the bass, and its provenance. They cover strings with
  woodwind (including the woodwind absorbing one string section), strings with brass, woodwind
  with brass, the colour doublings of glockenspiel, xylophone, celesta and harp, woodwind unisons
  and octaves of one branch, strings with strings, mixed brass in octaves, the three-group unisons
  of a tutti and the ways of building the bass. A unison pair the table does not list is what
  `DBL-01` reports.
- A `Selector` covers lines by instrument (`Ids`), by `Family`, by instrument and articulation
  (`Played`, such as pizzicato strings or stopped horns), or by forces (`Solo`, `Divided`);
  `matches` takes a `Playing`: the instrument, its articulation and its `Forces` (`Usual`, `Solo`,
  `Divided`).
- `DO_NOT_DOUBLE`: twelve warnings (what not to do and why), each with the rules that check it.
- `WOODWIND_ORDER`: the natural order of the woodwinds from the top, flutes, oboes, clarinets,
  bassoons, with the auxiliary instruments in their branches; `rank` gives an instrument's branch
  and `None` for the saxophones, which the order does not place.

`DynamicBand` groups the notated dynamics as the sources compare them: `Soft` (pp, p), `Medium`
(mp, mf) and `Loud` (f, ff); where mp falls is an estimate.

The numbers a check compares with are named thresholds, `Threshold<T>`, each with the rules that
use it and its own provenance: `SUSTAINED_HIGH_BRASS` (two bars of moderate 4/4),
`UNDOUBLED_HIGH_MELODY` and `UNDOUBLED_LOW_MELODY` (MIDI 90 and 42), `LOW_DISSONANCE` (D3, MIDI
50), `BASS_SPACING` and `MIDDLE_GAP` (an octave), `WEAK_CHORD_MEMBER` (half the strongest part),
`MELODY_MARGIN` (1.5 times the accompaniment), `SOLO_STRING_ACCOMPANIMENT` (2 units),
`FEWEST_FIRST_VIOLINS` (8), `LOUD_DIVISI` (four ways), `WIND_PHRASE` (eight bars of moderate
4/4), `SOFT_SLUR` and `MANY_NOTE_SLUR` (four and two bars of moderate time) and
`CHARACTERISTIC_COLOUR_SHARE` (more than half the bars). Most of them turn a source's words into
a number and are estimates; a rule that compares with an estimated threshold is never stronger
than a warning. Durations are `Span`s and convert at `MODERATO_BPM`, as the playability facts do.

## Validation

A record is an `InstrumentSpec` checked by `Instrument::try_new(instrument_type, spec)`. Records
are written as literals; one that leaves fields out ends in `..InstrumentSpec::EMPTY`, which
gives them their "not known" value. `EMPTY` itself is not a valid record: its name and its
articulation provenance are empty and its GM voice is a percussion voice with no key, so a record
that forgets any of them does not build.

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
- an empty or blank provenance anywhere in the record: a range, the articulation list, an era of
  the ensemble profile, the keyboard mechanics, a playability fact or a GM substitute;
- a GM voice of channel-10 keys that lists none or one key twice, and a GM voice that does not fit
  the pitch: a pitched instrument takes a program and an unpitched one channel-10 keys;
- keyboard mechanics whose keys do not lie inside the playable range (an unpitched instrument has
  none), a pedal kind listed twice, two pedals on one controller, and a noise listed twice;
- a kind of playability fact listed twice, a fact that describes another sort of instrument
  (`FactKind::fits`: a breath belongs to a wind, brass or voice part, a mute change to strings or
  brass, a change back to arco and held chord notes to strings, a retuning to the timpani, pedals
  to the harp, notes per hand to a plucked or keyboard instrument), a zero duration or count, harp
  pedals that do not name each letter name once, and a mute change or a change back to arco on a
  record that lists no mute it puts on (`con_sordino` or a named mute) or no `pizzicato`.

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
independently of the data, every record's GM voice and the orchestral percussion map with
tables written from the GM1 lists, the keyboard mechanics of the sixteen instruments played
from a keyboard with a table written from the research, and the playability facts of every
record with a table written from the sources.

An ensemble template is an `EnsembleSpec` checked by `Ensemble::try_new(id, spec)`, which
rejects:

- a blank name or description, and a missing or blank provenance of the template or a string
  size;
- a string size whose slug is not `snake_case` or repeats another's, whose name is blank or which
  gives a section no players; string sizes without a string section; a seating listed twice;
- a part whose slug is not `snake_case`, whose name or short name is blank, or whose slug or name
  another part has; an alternative that is the part's own instrument, of another family or listed
  twice; own players with none at the low end; a string section on another instrument than its
  own, listed twice or without string sizes; a doubling that names no part, a part without exactly
  one player of its own or a part of the same instrument, or that is required while its player's
  part is optional; a pan outside −1 to +1; a pan that depends on a seating the template does not
  offer;
- a template without a required part, and a full-score template out of score order.

The shipped templates are built with the panicking `Ensemble::new` in a `static`, so an invalid
template is a compile error. `crates/sonara-midi/tests/ensembles.rs` compares every template's
parts, string sizes, seatings and pans with tables written from the cited scorings and seating
plans, checks the track and player counts the sources state, and rejects a fixture breaking each
check.

A rule is a `RuleSpec` checked by `Rule::try_new(id, spec)`, which rejects a blank title or
check, no inputs or an input listed twice, a graded severity that does not rise from its usual
level or names no case, and a blank provenance. The catalogue is built with the panicking
`Rule::new` in a `static`, and a compile-time check places each rule at its id's place, so an
invalid rule does not compile. The rows of the tables have a `check` that rejects a selector
naming no instrument, one twice or the `Other` family, or playing no articulation or one twice; a
doubling with fewer than two members, a member counted with no players, no spacing, a spacing
twice or of no octaves, or a prevailing member it does not have; a woodwind order with an empty
branch, an instrument of another family or one twice; and blank texts or provenance. A
compile-time check runs it on every shipped row. `crates/sonara-midi/tests/rules.rs` compares the
codes, grades and thresholds with tables written from the sources, checks that every rule is
traceable and that an estimated threshold backs no error, compares the named unisons with the
pairs and leaders Rimsky-Korsakov gives, checks that octave doublings list their members from the
top in woodwind order and that a played articulation is one its instruments list, and rejects a
fixture breaking each check.

## Coverage

All 85 types have a record, listed in score order by `INSTRUMENTS` and `instruments()`. The
records live in one file per family under `crates/sonara-midi/src/orchestration/data/`
(woodwinds, brass, percussion, plucked, keyboards, voices, strings). A display name is the first
form of the English name, with a flat spelled `♭` ("Clarinet in B♭"); an alternate in parentheses
("Timpani (kettledrums)") is not part of it. Range extensions and gaps, alternative tunings and
techniques outside the vocabulary are not modelled. The thirteen ensemble templates live in
`data/ensembles.rs`; divisi tracks are not part of a template.
