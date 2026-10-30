# Doubling and Balance: Orchestration Rules for Software

_Synthesized 2026-10-30 from the research cards in [cards/](cards/) and the generated instrument table
[model.md](model.md). Status: research; nothing here is implemented._

## Sources

| # | Source | URL | Kind | Opened |
|---|---|---|---|---|
| S1 | N. Rimsky-Korsakov, _Principles of Orchestration_ (ed. M. Steinberg, tr. E. Agate, 1922), Project Gutenberg eBook 33900, plain text (Vol. I text). Cited by chapter, section title and page; a section's page is the one in the book's own table of contents, and the pages of quoted sentences were checked against the page anchors of the HTML edition. | https://www.gutenberg.org/cache/epub/33900/pg33900.txt (HTML edition https://www.gutenberg.org/files/33900/33900-h/33900-h.htm, searched for the octave-numbering note and used for the page anchors) | handbook (public domain) | yes: Ch. I (string section sizes, wind character, brass remarks, harp, kettle-drums, piano and celesta, percussion, "Comparison of resonance"), Ch. II (all), Ch. III (all), Ch. IV (all sections listed in its contents), Ch. V ("Transparence of accompaniment") |
| S2 | C. Forsyth, _Orchestration_ (Macmillan / Stainer & Bell, 1914), Internet Archive item `orchestration00fors`, OCR text. Cited by printed page, checked against the page numbers in the OCR text. | https://archive.org/details/orchestration00fors (text: https://archive.org/download/orchestration00fors/orchestration00fors_djvu.txt) | handbook (public domain) | yes: pp. 45-46 (kettle-drum intervals and retuning), 122-123 (horn pairs, dove-tailing), 227 (overlapped chord colours), 262 (oboe and clarinet interlock), 462-464 and 469-470 (harp pedal mechanism and pedal notation) |
| S3 | Ch.-M. Widor, _The Technique of the Modern Orchestra_ (tr. E. Suddard, 1906), Internet Archive item `cu31924022381556`, OCR text. Cited by printed page. | https://archive.org/download/cu31924022381556/cu31924022381556_djvu.txt | handbook (public domain) | yes: p. 31 (clarinet as a blending stand-in, Egmont balance), p. 127 (saxophone and clarinet held notes), p. 190 (the double-bass mute passage), pp. 197-198 (four closing principles) |
| S4 | E. Rogers, "11 \| Voicings (Part 2)", big-band arranging course | https://www.evanrogersmusic.com/blog-contents/big-band-arranging/voicings-part2 | other (arranging teacher's website) | yes (text only) |
| S5 | R. Hoffmann, "Low Interval Limits" | https://www.robin-hoffmann.com/dfsb/low-interval-limits | other (teacher's website) | yes (text; the chart itself is an image and was not read) |
| S6 | Sleepfreaks, "Voicing, Arpeggios, and Low Interval Limit" | https://sleepfreaks-dtm.com/en/music-theory-beginner/voicing-arpeggio/ | other (tutorial site) | yes (text; the tables are images and were not read) |
| S7 | Sweetwater inSync, "Low Interval Limit" (glossary entry) | https://www.sweetwater.com/insync/low-interval-limit/ | other (retailer glossary) | **no** (HTTP 403). Not cited for any value. |
| S8 | Sonara repository, orchestration metadata: `docs/decisions/0010-sample-based-orchestral-instruments.md`, `crates/sonara-midi/src/orchestration/instrument.rs` (`RangeCheck`, `check_sounding`, `Provenance`), `crates/sonara-midi/src/orchestration/data.rs` (`INSTRUMENTS: [Instrument; 16]`), `docs/orchestration-metadata.md` (C4 = 60) | repository paths | other | yes |
| S9 | Sonara repository, project format and agent contract: `docs/project-format.md` (the `extra` map), `crates/sonara-project/schemas/project-v1.schema.json` (track and note fields, `tempo_map`, `time_signatures`), `docs/agent-api.md` (the 57 tools, `create_automation` targets), `crates/sonara-agent/src/contract/args.rs` (`NoteInput`) and `crates/sonara-agent/src/contract/inputs.rs` (the `create_project` template literal), `crates/sonara-app/src/instruments/` (the instrument catalog), `crates/sonara-core/src/plugins/builtin.rs` (the sampler's parameters), `crates/sonara-app/src/composer/system_prompt.md` (the AI Composer) | repository paths | other | yes |
| S10 | [sfz-conformance.md](sfz-conformance.md): the verification of Sonara's SFZ parser and sampler (`crates/sonara-media/src/sfz.rs`, `crates/sonara-dsp/src/processors/sampler.rs`) against the code, cited by its claim ids (Q1-Q11, C1-C24) | [sfz-conformance.md](sfz-conformance.md) | other (this research) | yes |

Card tags point into [cards/](cards/); the card's own Sources table gives the source behind each
card statement. Three copyrighted handbooks often used for this topic (Adler, Blatter, Piston)
were not read, and nothing here cites them.

## 0. Conventions used in this file

This file turns the doubling, balance, spacing and voicing teaching of the public-domain handbooks
into data and checks that a rule engine or an AI agent can use when it writes orchestral parts in
Sonara. Sections 1 to 5 are the knowledge. Section 6 is the list of checkable rules. Every factual
statement carries a tag: `[Cited Sn]` for a row of the Sources table, `[Card cards/<file>.md#<id>]`
for a research instrument card, or `[Estimate: reason]`. Instrument ids are those of
[model.md](model.md).

### 0.1 Pitch, registers and Rimsky-Korsakov's octave numbers

- Pitches are MIDI note numbers, middle C = C4 = 60, the convention of Sonara's `theory` module
  [Cited S8: docs/orchestration-metadata.md].
- Rimsky-Korsakov's prose numbers octaves in his own way. The eBook's "C4 = middle C" note applies
  only to the single-note music examples that the transcriber rendered as text [Cited S1,
  transcriber's notes]. His prose octave _n_ (n ≥ 1) is read here as scientific octave _n_ + 1
  (octave 1 = C2-B2, MIDI 36-47; octave 3 = C4-B4, MIDI 60-71; octave 5 = C6-B6, MIDI 84-95) and
  his octave -1 as C1-B1 (MIDI 24-35) [Estimate: inferred from three anchors in S1, each consistent
  with this reading and not with the C4 reading: (a) Ch. I, Table C (brass group), footnote B: "The
  b♮ of the octave -1 does not exist on the trombones"; under this reading that note is B1 (35), the
  bottom of the straight tenor trombone's gap B1-E♭2 between its pedal B♭1 (34) and its
  seventh-position E2 (40), the one note that even the F attachment cannot fill without pulling its
  slide to E [Card cards/low_brass.md#trombone_tenor]; (b) Ch. II "Combination in unison" (f)-(g),
  p. 48: the low flute and high bassoon overlap "practically restricted to the limits of the third
  octave", which is C4-B4, where the flute's lowest note is B3 (59) and the bassoon's comfortable top
  is C5 (72) [model.md](model.md); (c) Ch. V, p. 121: sustained harmony "of the second octave to the
  middle of the third" leaves women's voices free, which fits C3 to about F♯4].
- Register bands used by the rules below [Estimate: Rimsky's octaves under the reading above, and
  his register names for the strings in Ch. II "Melody in stringed instruments", pp. 36-38]:

| Band | MIDI | Rimsky octave | Rimsky's register words |
|---|---|---|---|
| `sub` | below 36 | -1 and below | "basso profondo + a still lower compass" (double bass) |
| `bass` | 36-47 | 1 | bass |
| `tenor` | 48-59 | 2 | tenor |
| `alto` | 60-71 | 3 | alto |
| `soprano` | 72-83 | 4 | soprano |
| `high` | 84-95 | 5 | "extra-high compass" |
| `top` | 96 and above | 6 | piccolo, glockenspiel territory |

### 0.2 Handbook names and Sonara ids

| Handbook name (S1, S2, S3) | Sonara id | Note |
|---|---|---|
| Violins I, Violins II (Vns I, Vns II) | `violin` | two **parts** of one id; Sonara has no part field yet [Cited S9: project-v1.schema.json] |
| Violas, 'Cellos, Double basses | `viola`, `cello`, `double_bass` | |
| Flute, Piccolo | `flute`, `piccolo` | |
| "Bass flute" (Rimsky) | `alto_flute` | Rimsky's "bass flute" is the alto flute in G: the flute card infers it from his Table B gloss "Alto Fl. F, G" [Card cards/flutes.md#alto_flute] |
| Oboe, Eng. horn | `oboe`, `english_horn` | |
| Clarinet, small clarinet (E♭/D), bass clarinet | `clarinet_bb` (or `clarinet_a`), `clarinet_eb`, `bass_clarinet` | |
| Bassoon (Fag.), double bassoon (C-fag.) | `bassoon`, `contrabassoon` | |
| Horns 1-4 | `horn_f` | four parts of one id |
| Trumpets, cornet | `trumpet_bb` / `trumpet_c`, `cornet` | |
| Trombones, bass trombone, tuba | `trombone_tenor`, `trombone_bass`, `tuba` | |
| Kettle-drums | `timpani` | |
| Side drum, bass drum, cymbals, gong | `snare_drum`, `bass_drum`, `clash_cymbals` (struck with sticks: `suspended_cymbal`), `tam_tam` | |
| Triangle, tambourine, castanets, little bells | `triangle`, `tambourine`, `castanets`, `sleigh_bells` | "little bells" = `sleigh_bells` [Estimate: closest id; S1 does not describe the instrument] |
| Glockenspiel, celesta, xylophone, bells | `glockenspiel`, `celesta`, `xylophone`, `tubular_bells` | |
| Harp, piano | `harp`, `piano_upright` / `piano_concert_grand` | S1 prefers an upright in the orchestra (section 5.7) |

### 0.3 What "dynamic" means here

The handbooks reason in notated dynamics (pp to ff). Sonara notes carry a velocity, and no agent
tool writes MIDI CC into a clip [Cited S9: docs/agent-api.md, project-v1.schema.json]. The rules
below take a **dynamic level per part** as input; until Sonara stores one, it is derived from
velocity bands [Estimate: the only per-note loudness that exists today].

## 1. Functions and who carries them

Rimsky-Korsakov's frame: the three sustaining groups (strings, wood-wind, brass) carry "melody,
harmony and rhythm"; instruments of little sustaining power serve "chiefly ... for ornament and
colour"; indefinite-pitch percussion is "purely rhythmical"; strings rank first for expression and
endurance, and wind, plucked and percussion colours tire the ear sooner [Cited S1 Ch. I
"Comparison of resonance", p. 35].

The table lists, for each function and register band (section 0.1), the usual carriers by family.
"Other" covers percussion, keyboards, plucked instruments and voices.

| Function | Band | Strings | Woodwinds | Brass | Other | Sources |
|---|---|---|---|---|---|---|
| Melody | `high`, `top` | `violin` (Vns I); above the middle of Rimsky's 5th octave doubled an octave below, a few desks taking the top octave | `piccolo` (extends the flute upward), `flute`, `clarinet_eb` (more penetrating than the B♭ in its top) | `trumpet_c` / `trumpet_bb` only for fanfare-like or "unmodulated diatonic" phrases | `glockenspiel`, `xylophone`, `celesta` accent or double the top notes | [Cited S1 Ch. II pp. 36, 40-41, 44; Ch. I wood-wind remarks, p. 20; Ch. II "Melody in the brass", pp. 53-54]; [Card cards/tuned_percussion.md#xylophone], [Card cards/tuned_percussion.md#glockenspiel] |
| Melody | `soprano`, `alto` | Vns I, sometimes Vns II or both in unison ("fuller resonance without impairing quality") | `flute`, `oboe`, `clarinet_bb`, `english_horn` as solo colours | `horn_f` (its "genial and poetic tone" in piano), `trumpet_bb` | solo voices | [Cited S1 Ch. II pp. 36, 46, 54] |
| Melody | `tenor` | `cello` on its A string ("tense passionate cantabile"); `viola` for short characteristic phrases, "generally doubled by other strings or by the wood-wind" | `bassoon` (upper register), `bass_clarinet`, `english_horn`, low `clarinet_bb` | `horn_f`, `trombone_tenor` (diatonic, "energetic power"), `euphonium` | | [Cited S1 Ch. II pp. 37-38, 53-55]; [Card cards/low_brass.md#euphonium] |
| Melody | `bass`, `sub` | `cello` + `double_bass` in unison or octaves, "occasionally" for low phrases; the double bass never alone in Rimsky's own practice | `bassoon`, `contrabassoon` doubling the basses | `tuba`, `trombone_bass` (dark, massive) | | [Cited S1 Ch. II p. 38 (d), p. 40 (f)] |
| Counter-melody | any, one band away from the melody | `cello` (tenor cantilena), `viola` + `clarinet_bb`; crossing (cellos above violas and violins) brings an inner line forward | `bassoon` above the clarinets, `clarinet_bb` or `oboe` above the flutes (crossing); `english_horn` | `horn_f`; `cornet` (Berlioz's added counter-melody) | | [Cited S1 Ch. II intro, p. 36]; [Card cards/strings.md#cello]; [Card cards/high_brass.md#cornet]. Placement one band away is [Estimate: S1 gives the rule only for a melodic design against a harmonic basis, section 4.4] |
| Sustained harmony | `soprano`, `high` | `violin` divisi; string harmonics | `flute`, `oboe`, `clarinet_bb` in pairs or threes; "Fl./Ob. (Cl.) + Vns div." | trumpets in close harmony at the top | `celesta`, `vibraphone` (sustains) for colour | [Cited S1 Ch. III pp. 71-81, p. 94]; [Card cards/tuned_percussion.md#vibraphone] |
| Sustained harmony | `alto`, `tenor` | Vns II + `viola` tremolando "middle parts"; `viola` / `cello` divisi | `clarinet_bb` + `bassoon`; Ob 1 / Cl 1 / Ob 2 / Cl 2 interlocked as a "background" | four `horn_f` ("perfectly balanced in tone"); `bassoon` + `horn_f` in four parts, soft | `harp` arpeggios, `organ_pipe`; `marimba` (mostly harmonic material in its middle and low registers) | [Cited S1 Ch. II p. 37; Ch. III pp. 82, 90]; [Cited S2 p. 262]; [Card cards/plucked.md#harp]; [Card cards/historical_keys_organs.md#organ_pipe]; [Card cards/tuned_percussion.md#marimba] |
| Sustained harmony | `bass` | `cello` (lowest chord note with the basses) | `bass_clarinet`, `bassoon` | `trombone_tenor` ×2-3 with `tuba`; or two horns + `tuba` as the bass under three trombones | | [Cited S1 Ch. III pp. 69, 82-83] |
| Bass | `bass`, `sub` | `cello` / `double_bass` in octaves ("The bass is usually constructed in this manner") | `bassoon`; `contrabassoon`; `bass_clarinet` | `tuba` an octave below the group's bass; `trombone_bass` + `tuba` in octaves; in a horn chord the bass is doubled by a bassoon, not by trombone or tuba | `timpani` (tonic and dominant, pedal basses); `harp` bass octaves in quiet passages | [Cited S1 Ch. II p. 43; Ch. III pp. 82-83]; [Card cards/low_brass.md#tuba], [Card cards/low_brass.md#trombone_bass], [Card cards/tuned_percussion.md#timpani], [Card cards/plucked.md#harp] |
| Rhythm, ostinato | `tenor` to `soprano` | repeated notes and tremolando in Vns II and violas; `pizzicato` strings (with `harp`) | `oboe` and `bassoon` for "distinct and penetrating staccato"; `flute` (double tonguing) for fast repeated notes | `horn_f` repeated-note accompaniment, almost always combined with wood-wind; `trumpet_bb` / `cornet` double tonguing | indefinite-pitch percussion; `timpani` persistent rhythms | [Cited S1 Ch. I wood-wind remarks, p. 18; brass remarks, p. 25; "Pizzicato", p. 27]; [Card cards/horns.md#horn_f]; [Card cards/tuned_percussion.md#timpani] |
| Rhythm, ostinato | `bass` | `cello` / `double_bass` pizzicato under wood-wind tutti, which "renders the sustained notes in the wood-wind more distinct" | `bassoon` staccato | `tuba`, `trombone_bass` | `bass_drum`, `timpani` | [Cited S1 Ch. IV "Tutti in the wind", p. 103] |
| Colour | `high`, `top` | string harmonics (link to flute and piccolo) | `piccolo`, `flute` | | `glockenspiel`, `celesta`, `crotales`, `xylophone`, `triangle` | [Cited S1 Ch. I "Comparison of resonance", p. 34; Ch. IV "Economy", p. 118]; [Card cards/tuned_percussion.md#crotales] |
| Colour | `alto`, `tenor` | muted strings; `sul_ponticello` | `english_horn`, `alto_flute`, low `clarinet_bb` | stopped or muted `horn_f` / `trumpet_bb` with `oboe` or `english_horn` | `harp`, `vibraphone` | [Cited S1 Ch. I p. 34; Ch. III p. 89]; [Card cards/strings.md#violin] |
| Colour | `bass`, `sub` | | `contrabassoon`, `bass_clarinet` | | `tam_tam`, `bass_drum` roll, low `harp` | [Cited S1 Ch. I percussion; Ch. IV "Use of percussion", p. 117] |
| Pedal point | any; upper and inner pedals included | `double_bass` (its main independent role); tremolando strings | `flute`, `clarinet_bb`; `contrabassoon` deep pedals in slow tempo | two `trumpet_bb` in octaves, or two or four `horn_f` in octaves or double octaves; trombones "rarely" | `timpani` (pedal basses, roll), `organ_pipe` pedal | [Cited S1 Ch. I strings note, p. 9; Ch. III pp. 66, 86]; [Card cards/double_reeds.md#contrabassoon]; [Card cards/tuned_percussion.md#timpani]; organ pedal [Estimate: the organ card gives 16′ pedal doubling of contrabass lines only as an Estimate] |
| Tutti | all bands | the whole string body | full wood-wind (piccolo needed in the extreme top, flutes unnecessary in the low register) | full brass; brass alone "may attain an extraordinary volume" | percussion and harp are additions, not part of the definition | [Cited S1 Ch. IV "Full Tutti", p. 101; "Tutti in the wind", p. 103] |

Further facts about the functions:

- **Melody in tutti.** The most natural three-group unisons are violins + oboe (or flute or
  clarinet) + trumpet; violas (or cellos) + clarinet (or English horn) + horn; cellos and double
  basses + 2 bassoons + 3 trombones + tuba [Cited S1 Ch. II "Combination of the three groups", p. 61].
- **Full and partial tutti.** "Full tutti" means strings, wind and brass together; a "partial
  tutti" uses part of the brass. Tutti is "used essentially in forte and fortissimo, rarely in
  pianissimo and piano" [Cited S1 Ch. IV "Full Tutti", pp. 101-102].
- **Crescendo and diminuendo by orchestration.** A long crescendo adds groups in the order
  strings, wood-wind, brass; a diminuendo removes them in reverse. After the strings, the brass is
  the group most capable of dynamic shading, and clarinets excel at the diminuendo [Cited S1 Ch. IV
  "Crescendo and diminuendo", p. 112].
- **Hand-over between instruments.** When a passage passes from one instrument to another, the
  last note of one part coincides with the first note of the next [Cited S1 Ch. IV "Transference of
  passages and phrases", pp. 107-108]. Imitation follows the "law of register": an answer in the
  upper register goes to a higher instrument [Cited S1 Ch. IV "Repetition of phrases, imitation,
  echo", p. 110].
- **Echo.** The echoing instrument is weaker than the first but related to it (muted trumpets
  after oboes, flutes after clarinets or oboes); a wood-wind cannot echo the strings or the reverse
  [Cited S1 Ch. IV p. 110].

## 2. Doubling rules

General principle: a unison of two different timbres gains resonance, sweetness and power but
restricts "the variety of colour and expression"; "Phrases or melodies demanding diversity of
expression alone should be entrusted to solo instruments of simple timbres"; two instruments of
the same kind in unison gain power and lose expressive capacity [Cited S1 Ch. II "Combination in
unison", p. 48]. Doubling and mixed timbres belong more to loud passages and to "broad" rather than
intimate expression [Cited S1 Ch. II p. 49]. Constant compound timbres produce "a dull, neutral
texture" [Cited S1 Ch. I p. 35].

### 2.1 Named doublings and the colour they produce

| Doubling (ids) | Interval | Colour or effect | Who dominates | Sources |
|---|---|---|---|---|
| `violin` + `flute` | unison | among the "best and most natural" string-wind unisons; the flute adds resonance, the strings soften it | strings, if the two are "of equal power" | [Cited S1 Ch. II "Combination of strings and wind", p. 58] |
| solo `violin` + `flute` | unison | "great purity and abundance of tone" without losing the solo timbre, in the high register | the solo violin's timbre is kept | [Cited S1 Ch. IV "Soli in the strings", p. 106] |
| `violin` + `oboe` | unison | natural pair; "Vns I + Vns II + 1 Ob." only thickens the strings | strings ("the wood-wind timbre being lost") | [Cited S1 Ch. I p. 34; Ch. II p. 58] |
| `violin` + `clarinet_bb` (or `clarinet_eb`) | unison | natural pair | strings | [Cited S1 Ch. II p. 58] |
| `viola` + `clarinet_bb` | unison | natural pair; viola timbre is "vaguely compared" to the clarinet's lowest compass, a point of contact between the groups | strings when equal; "2 Ob. + 2 Cl. + Violas" absorbs the violas | [Cited S1 Ch. I p. 34; Ch. II p. 58] |
| `viola` + `oboe` / `english_horn`; `viola` + `bassoon` | unison | natural pairs | strings | [Cited S1 Ch. II p. 58] |
| `cello` + `bassoon` | unison | natural pair; "'Cellos + D. basses + 1 Fag." only thickens the strings | strings | [Cited S1 Ch. I p. 34; Ch. II p. 58]; cello melody doubled by bassoons [Card cards/strings.md#cello] |
| `cello` + `clarinet_bb` / `bass_clarinet` | unison | natural pair | strings | [Cited S1 Ch. II p. 58] |
| `double_bass` + `bass_clarinet` / `bassoon` / `contrabassoon` | unison | natural pairs | strings | [Cited S1 Ch. II p. 58] |
| `horn_f` + `cello` | unison | "a beautifully blended, soft quality of tone" | blended | [Cited S1 Ch. II "Combination of strings and brass", p. 61] |
| `horn_f` + divided `viola` or `cello` | unison, chords | "the splendid effect of horns doubled by divided violas or 'cellos" | blended | [Cited S1 Ch. III p. 95] |
| `violin` + `trumpet_bb`; `viola` + `horn_f`; `cello` / `double_bass` + `trombone_tenor` / `tuba` | unison | best string-brass pairs, but "each can be heard separately"; the low pair is for "heavy massive effects" | neither blends | [Cited S1 Ch. II p. 61] |
| `trumpet_bb` + `clarinet_bb` / `oboe` / `flute` (or all three) | unison | brass tone dominates; the wood-wind "softens and rarefies it"; a finer legato than the brass alone | brass | [Cited S1 Ch. II "Combination of wind and brass in unison", pp. 56-57] |
| `horn_f` + `clarinet_bb` / `bassoon`; `trombone_tenor` + `bassoon`; `tuba` + `bassoon` | unison | same character as above | brass | [Cited S1 Ch. II p. 56] |
| stopped or muted `horn_f` / `trumpet_bb` + `oboe` / `english_horn` | unison or chord | "a magnificent tone"; stopped notes resemble oboe and English horn | blended | [Cited S1 Ch. III p. 89] |
| stopped `horn_f` (middle) + low `clarinet_bb` | chord | "A beautiful dark tone"; with bassoons instead "the effect loses part of its character" | blended | [Cited S1 Ch. III p. 89] |
| `oboe` + `clarinet_bb` | unison | closest wood-wind substitute for a trumpet phrase that lies too high | | [Card cards/clarinets.md#clarinet_bb] |
| `piccolo` over `flute`, `oboe` or `clarinet_bb` | octave | the normal way to support the piccolo | | [Cited S1 Ch. II "Combination in octaves", p. 50] |
| `piccolo` two octaves above a wood-wind line | two octaves (Forsyth: "at the 16th") | used with the middle octave left out; flute top notes are often better doubled by the piccolo in unison | | [Card cards/flutes.md#piccolo] |
| `piccolo` / `flute` / `oboe` (+`clarinet_bb`) / `bassoon` | 3 to 4 octaves | melody spread over the wood-wind in natural order, e.g. "Picc./2 Fl./2 Ob. + Cl./Fag." | | [Cited S1 Ch. II "Doubling in two, three and four octaves", p. 51] |
| `glockenspiel` + `flute` / `piccolo` / `celesta` / `harp` | unison or 1-3 octaves above | usual colour doublings of a melody | glockenspiel [Estimate: the card does not say; S1 p. 33 says the glockenspiel "will easily prevail"] | [Card cards/tuned_percussion.md#glockenspiel] |
| `xylophone` + `celesta` / `glockenspiel`; + `violin` / `viola` pizzicato | unison or octave above | sharpens pizzicato; accents the top notes of a melody | xylophone | [Card cards/tuned_percussion.md#xylophone] |
| `celesta` + `flute` / `harp` / piano | unison | duplicates a theme | | [Card cards/historical_keys_organs.md#celesta] |
| `harp` + pizzicato strings | unison | one plucked group | | [Card cards/plucked.md#harp]; [Cited S1 Ch. IV "Tutti pizzicato", p. 103] |
| `harp` harmonics + `flute` / `clarinet_bb` | unison | magical, in the middle register | | [Card cards/plucked.md#harp] |

### 2.2 Wood-wind with wood-wind

Unison mixtures [Cited S1 Ch. II "Combination in unison", pp. 47-48]:

| Unison | Colour | Predominates low | Predominates high |
|---|---|---|---|
| `flute` + `oboe` | "fuller than that of the flute, sweeter than that of the oboe" | flute (played softly) | oboe |
| `flute` + `clarinet_bb` | fuller than the flute, duller than the clarinet | flute | clarinet |
| `oboe` + `clarinet_bb` | fuller than either | oboe ("dark, nasal") | clarinet ("bright, 'chest' quality") |
| `flute` + `oboe` + `clarinet_bb` | "Very full in quality" | flute low, oboe middle | clarinet |
| `bassoon` + `clarinet_bb` | "Very full quality" | clarinet ("gloomy") | bassoon ("sickly") |
| `bassoon` + `oboe`, `bassoon` + `flute` (and with a clarinet added) | "very seldom found except in certain orchestral tutti"; more resonance, no new colour | low flute notes in the lower third of Rimsky's 3rd octave | high bassoon notes in its middle third; the clarinet, "weak in the middle compass", does not stand out |
| all four | rich, "difficult to define"; rare | each separates as above | |

Octaves and parallel intervals [Cited S1 Ch. II pp. 49-53]:

| Rule | Detail |
|---|---|
| Natural octave pairs (upper / lower) | Fl/Ob, Fl/Cl, Fl/Fag, Ob/Cl, Ob/Fag, Cl/Fag; Fl/Fag is "rare on account of the widely separated registers" |
| Same branch, always good | `bassoon` / `contrabassoon`, `clarinet_bb` / `bass_clarinet`, `oboe` / `english_horn`, `clarinet_eb` / `clarinet_bb`, `flute` / `alto_flute`, `piccolo` / `flute` |
| Extremes | a melody in the extreme top is doubled an octave lower, in the extreme bottom an octave higher: piccolo by flute, oboe or clarinet below; contrabassoon by bassoon, clarinet or bass clarinet above |
| Same colour in octaves (2 Fl, 2 Cl, 2 Fag) | "certainly not to be recommended", unless strings (arco or pizz) double both, best in the middle compass, for repeated or sustained notes |
| Order | departing from the order Fl, Ob, Cl, Fag (e.g. bassoon above clarinet) gives "a far-fetched, unnatural tone", for special effects only |
| Three and four octaves | keep the natural order: (Fl, Ob, Cl), (Ob, Cl, Fag), (Fl, Cl, Fag), (Fl, Ob, Fag); four octaves Fl, Ob, Cl, Fag; five octaves need the strings |
| Thirds and sixths | two of one colour, or two colours in normal order (Fl over Ob, Fl over Cl, Ob over Cl, Cl over Fag, Ob over Fag); inverted order sounds "strained and forced"; same-kind pairs are best for thirds, different kinds for sixths |

### 2.3 Strings with strings

| Combination | Interval | Result | Sources |
|---|---|---|---|
| Vns I + Vns II | unison | no change of colour; more power; usually with some wood-wind doubling, which cannot predominate | [Cited S1 Ch. II "Grouping in unison", p. 39] |
| `violin` + `viola` | unison | violins predominate, "rich and full" | [Cited S1 p. 39] |
| `viola` + `cello` | unison | "rich full resonance, the 'cello quality predominating" | [Cited S1 p. 39] |
| `violin` + `cello` | unison | cello tone prevails, fuller | [Cited S1 p. 40] |
| Vns I + II + `viola` + `cello` | unison | only possible in the alto-tenor register; "very tense and powerful in forte", "extremely full and rich in piano" | [Cited S1 p. 40] |
| `cello` + `double_bass` | unison | "rich full resonance", occasionally for very low phrases | [Cited S1 p. 40] |
| Vns I / Vns II | octave | "very common", especially very high; secures "expression, fulness of tone and firmness of timbre"; very high lines otherwise "too isolated" | [Cited S1 Ch. II "Stringed instruments doubling in octaves", pp. 40-41] |
| Vns I divisi / Vns II divisi in octaves | octave | halves the players per line, loses resonance, worst in small orchestras; acceptable when wood-wind doubles and the line is high | [Cited S1 p. 41] |
| `violin` / `viola` | octave | common, especially when the lower octave goes below the violin's G string; "Vns I + II / Violas" for brilliance on top, "Vns I / Vns II + Violas" for a fuller lower part | [Cited S1 p. 42] |
| `viola` / `cello` | octave | when the violins are busy | [Cited S1 p. 42] |
| `violin` / `cello` | octave | expressive passages with the cellos on A or D strings; more resonant than violas/cellos | [Cited S1 p. 42] |
| `cello` / `double_bass` | octave | the normal bass; the bass part may be simplified | [Cited S1 p. 43] |
| `viola` / `double_bass` | octave | seldom, when the cellos are busy | [Cited S1 p. 43] |
| Vns I + II / `viola` + `cello` | octave, each doubled in unison | middle-range melodies; "a beautiful quality of tone, somewhat severe" | [Cited S1 p. 43] |
| Vns I / Vns II / Violas, or Vns I / Vns II / Cellos | double octave | very tense cantabile, preferably forte | [Cited S1 "Melody in double octaves", p. 44] |
| strings over 3-4 octaves | 3-4 octaves | "very seldom found", usually with wind support | [Cited S1 p. 45] |
| thirds | | thirds go to Vns I and Vns II (also violas/cellos), sounding even despite unequal numbers; for sixths different timbres may be used | [Cited S1 "Melody in thirds and sixths", p. 45] |

### 2.4 Strings with wood-wind

- "All combinations of strings and wood-wind are good"; the wind adds resonance and the strings
  soften the wind; the strings predominate when the two are of equal power; "If several wind
  instruments play in unison with one group of strings, the latter will be overpowered"; the
  wood-wind loses more character than the strings [Cited S1 Ch. II "Combination of strings and
  wind", p. 58].
- Absorption in both directions [Cited S1 Ch. I "Comparison of resonance", p. 34]:
  - several winds absorb one string department: "2 Fl. + 2 Ob. + Vns I", "2 Ob. + 2 Cl. + Violas",
    "2 Cl. + 2 Fag. + 'Cellos" (a "sweet coherent quality, the wood-wind timbre still
    predominating");
  - one wind added to strings only thickens them: "Vns I + Vns II + 1 Ob.", "Violas + 'Cellos +
    1 Cl.", "'Cellos + D. basses + 1 Fag.".
- Muted strings "do not combine so well with wood-wind", the two qualities staying separate
  [Cited S1 Ch. I p. 34].
- When only one octave of a soprano melody is doubled by strings, let the wood-wind run in octaves
  and give the strings the lower part: "Picc./Fl. + Vns] 8", "Fl./Ob. (Cl.) + Vns] 8"
  [Cited S1 Ch. II p. 60].
- A soft low melody: "'Cellos + Fag./D. basses] 8", the basses undoubled [Cited S1 Ch. II p. 60].
- Octaves across the groups (flute above violins, oboe above cellos) were a classical balancing
  device that the editor calls "not to be recommended" because the colours differ so much
  [Cited S1 Ch. II footnote 14, editor's note].

### 2.5 Wood-wind with brass

- In octaves, wood-wind often replaces a trumpet above a horn: one horn below "2 Ob. or 2 Cl. or
  2 Fl." (or 1 Ob. + 1 Cl.); two horns below need three or four wind instruments above, "especially
  in forte" [Cited S1 Ch. II "Combination of wind and brass in octaves", p. 57].
- "To double a trumpet in the upper octave three or four wind instruments are required, but in the
  top register two flutes will suffice" [Cited S1 p. 57].
- "Wood-wind instruments should not be used to double a trombone in the octave above; trumpets are
  more suitable" [Cited S1 p. 57].
- Melody over three or four octaves across wind and brass rarely balances perfectly [Cited S1
  p. 58].
- A chord for full brass doubled by the same chord for full wood-wind in pairs gives "a magnificent
  and uniform tone"; trumpet + flute / oboe / clarinet (2+2 or 3+3) and horn + bassoon / clarinet
  are the usual unisons; three trombones + three bassoons or clarinets are "very rare"
  [Cited S1 Ch. III "Harmony in combined groups", p. 88].

### 2.6 Brass with brass

- Three trombones or four horns in unison: "extreme power and resonance of tone" [Cited S1 Ch. II
  "Brass in unison, in octaves, thirds and sixths", p. 55].
- Octaves, thirds and sixths in one brass kind "invariably" work; mixed kinds work in normal
  register order (upper / lower): Trumpet / 2 Horns, Trumpet / Trombone, Trombone / Tuba,
  2 Trombones / Trombone + Tuba, 2 Trumpets / 2 Trombones, 2 Horns / Tuba. Horns above trombones
  are "not so reliable" and only in octaves: 2 Horns / 1 Trombone, 4 Horns / 2 Trombones
  [Cited S1 pp. 55-56].
- Sustained notes in two or three octaves: the tenuto goes to two trumpets, or to two or four horns
  in octaves; trumpets and horns can form the octave together; "The trombone with its ponderous
  tone rarely takes part" [Cited S1 Ch. III "Duplication in the brass", pp. 85-86].
- A horn chord placed beside the same chord in trumpets or trombones softens their "penetrating
  timbre"; trumpets beside trombones is "not so common, as this unites the two most powerful agents"
  [Cited S1 pp. 85-86].

### 2.7 Doubling the bass

| Bass doubling | When | Sources |
|---|---|---|
| `cello` + `double_bass` in octaves | the default orchestral bass; the bass part may be simplified | [Cited S1 Ch. II p. 43] |
| `double_bass` doubling `cello` in unison, or doubling `bassoon` | the double bass rarely plays an independent part except pedal notes | [Cited S1 Ch. I p. 9] |
| `double_bass` phrase always supported | "no phrase of any importance given to the double bass without the support of 'cellos or bassoons" (Rimsky's own practice) | [Cited S1 Ch. II p. 38] |
| `cello` + `bassoon` over undoubled `double_bass` | soft, sweet low melody; also when no contrabassoon is available | [Cited S1 Ch. II p. 60] |
| `double_bass` + `contrabassoon` | depth; contrabassoon doubles double basses | [Cited S1 Ch. II p. 38]; [Card cards/double_reeds.md#contrabassoon] |
| `viola` two octaves above `double_bass` | reinforces the bass's partials | [Card cards/strings.md#double_bass] |
| `tuba` an octave below the group's bass; `trombone_bass` + `tuba` in octaves | brass bass | [Card cards/low_brass.md#tuba], [Card cards/low_brass.md#trombone_bass] |
| third `trombone_tenor` + `tuba` in octaves | bass of a trombone chord | [Cited S1 Ch. III "Harmony in the brass", p. 83] |
| two `horn_f` + `tuba` in octaves under three trombones | "beautiful full resonance" | [Cited S1 p. 83] |
| `bassoon` doubling the bass of a four-horn chord an octave below | the "too resonant trombone and tuba are seldom used" for this | [Cited S1 p. 83, p. 90] |
| `cello` / `double_bass` + 2 `bassoon` + 3 trombones + `tuba` | the heavy three-group bass unison | [Cited S1 Ch. II p. 61] |
| Bass doubled only downward | in harmony the bass is doubled "in the lower octave only"; in wide spacing "doubling the bass part creates an effect of heaviness" and the bass "should never mix with the others" | [Cited S1 Ch. III "Number of harmonic parts", p. 64] |

### 2.8 What not to double

| Do not | Why | Sources |
|---|---|---|
| Double an expressive solo line | doubling reduces "capacity for expression" | [Cited S1 Ch. II p. 48] |
| Duplicate the whole wood-wind only to balance an oversized string body | Rimsky's stated dislike; prefer a medium string body | [Cited S1 Ch. II p. 49; Ch. I p. 6] |
| Divide Vns I and Vns II in octaves in a small orchestra | halves the players on each line | [Cited S1 Ch. II p. 41] |
| Put divided halves of one viola, cello or bass section in octaves | the parts lie on non-corresponding strings and "unity of tone is impaired" (violins excepted) | [Cited S1 Ch. II p. 44, Note II] |
| Put two same-colour winds in octaves without string support | registers do not correspond | [Cited S1 Ch. II pp. 49-50] |
| Invert the wood-wind order (bassoon over clarinet, flute under oboe) in doublings | "far-fetched, unnatural tone" | [Cited S1 Ch. II p. 47] |
| Double a trombone an octave above with wood-wind | use trumpets | [Cited S1 Ch. II p. 57] |
| Double the tenor part in wide spacing; double the bass upward; double the bass of an inverted dominant seventh (or other seventh chords) in an upper part; write consecutive octaves between upper parts | harmony rules carried into orchestration | [Cited S1 Ch. III pp. 64-65] |
| Double strings with wood-wind and brass, or brass with wood-wind, under a voice; give one harmonic part to 2 clarinets, 2 oboes or 2 horns in unison under a voice | "liable to drown the singer" | [Cited S1 Ch. V "Transparence of accompaniment", pp. 120-121] |
| Replace a missing celesta by the glockenspiel | use an upright piano instead | [Cited S1 Ch. I "Piano and Celesta", p. 32] |
| Use compound timbres all the time | "dull, neutral texture" | [Cited S1 Ch. I p. 35] |
| Pair strings and brass in unison expecting a blend | "each can be heard separately" | [Cited S1 Ch. II p. 61] |

## 3. Balance

### 3.1 Rimsky-Korsakov's balance of tone

All rows are from S1 Ch. I "Comparison of resonance in orchestral groups", pp. 33-34, unless
another source is named.

| Comparison | Forte | Piano |
|---|---|---|
| Trumpet, trombone, tuba | "1 Trumpet = 1 Trombone = 1 Tuba" | "all wind-instruments, wood or brass are of fairly equal balance" |
| Horn against the other brass | "= 2 Horns" (horns "only one-half as strong") | fairly equal |
| Wood-wind against horn | "1 Horn = 2 Clarinets = 2 Oboes = 2 Flutes = 2 Bassoons" | fairly equal |
| One string department (all Vns I, or all Vns II, ...) in a medium orchestra against wind | "Violins I = 2 Flutes = 1 Oboe + 1 Clarinet" | "Violins I = 1 Flute" |
| Strings pizz. or col legno, piano played softly, celesta | easily overpowered by the sustaining groups | same |
| Glockenspiel, bells, xylophone; kettle-drums | "will easily prevail" | same |
| Harp | "At least three, if not four harps in unison are necessary" against a full orchestra forte [Cited S1 Ch. I "Harp", p. 29] | "never a very powerful instrument" |
| Bass drum, cymbals, gong and a side-drum tremolo, ff | "sufficient to overpower any orchestral tutti" [Cited S1 Ch. IV "Use of percussion", p. 117] | |

Rimsky-Korsakov's brass remarks say the same of the brass and add the cornet: trumpets, trombones
and tubas "are about equal in strength", cornets "have not quite the same force", and horns in
forte are about half as strong [Cited S1 Ch. I brass remarks, pp. 22-23]. The horn card reports
the same from Rimsky-Korsakov: horns are about half as strong as the other brass in forte, about
equal in piano, and should be marked one degree stronger [Card cards/horns.md#horn_f]. One card
source disagrees for soft playing: the Vienna Symphonic Library's horn page has trumpets or
trombones at pp roughly matching horns at p [Card cards/horns.md#horn_f]. That would make horns
weaker than the other brass even in piano, which Rimsky's "fairly equal" does not say (Open
question 3).

String section sizes behind the "medium orchestra" ratio [Cited S1 Ch. I "Stringed instruments",
p. 6]:

| Orchestra | Vns I | Vns II | Violas | Cellos | Double basses |
|---|---|---|---|---|---|
| Full | 16 | 14 | 12 | 10 | 8-10 |
| Medium | 12 | 10 | 8 | 6 | 4-6 |
| Small | 8 | 6 | 4 | 3 | 2-3 |

Fewer than eight first violins "is a mistake, as the balance between strings and wind is completely
destroyed"; 20-24 first violins overpower the customary wood-wind; "it is advisable to rely on a
medium-sized body of strings" [Cited S1 Ch. I p. 6].

### 3.2 Balancing a chord across families

- **Each family complete on its own.** "Write your orchestral music so that each group can be heard
  without the others": each group alone should give "complete harmony, with it[s] true bass"
  [Cited S3 p. 197].
- **Keep instruments in their best register** and avoid sustained extreme high notes; brass lips
  tire under the pressure of high notes [Cited S3 p. 198].
- **Horns count half in forte.** In brass chords "the number of horns should be doubled" when they
  are mixed with trumpets or trombones; with only two trumpets and no horn pairs, mark the horns
  "one degree louder than the other instruments, to secure balance of tone" [Cited S1 Ch. III
  "Harmony in the brass", pp. 84-85].
- **Wood-wind against brass.** When a chord mixes them, the wood-wind goes above the trumpets
  ("flutes, oboes and clarinets are better used to form the harmonic part above the trumpets");
  stationary parts to the brass, moving parts to the wood-wind; in difficult cases mark "the
  wood-wind one degree louder than the brass" [Cited S1 Ch. III "Harmony in combined groups",
  pp. 90-94].
- **Four horns with wood-wind on top.** A piano chord of four horns can be completed above by
  clarinets, oboes or flutes, with a bassoon doubling the bass an octave below; in forte the horns
  are stronger, so the upper wood-wind parts are doubled [Cited S1 Ch. III p. 90].
- **Two bassoons and two horns** give a balanced four-part chord in piano; in forte "the horns
  overwhelm the bassoons" and four horns alone are better [Cited S1 Ch. III p. 90].
- **Wood-wind in pairs.** In a classical four-part chord for paired wood-wind "the two extreme parts
  are the thinnest and weakest in tone, the intermediate parts the fullest and strongest"; mix the
  doubled timbres as much as possible [Cited S1 Ch. III "Duplication of timbres", pp. 77-78].
- **Self-correcting octaves.** Parts in octaves "strengthen one another", which offsets some
  inequality [Cited S1 Ch. III p. 94].
- **Divided strings.** Six- or seven-part harmony with every upper string part divided the same way
  balances; when the upper parts are divided and the cellos and basses are not, the basses are "a
  trifle heavy": mark them down or reduce the players [Cited S1 Ch. III "String harmony", p. 70].
- **Muted strings.** When every other string section is muted, the bass can be balanced by one or
  two double basses marked solo [Cited S3 p. 190].
- **Orchestration over dynamics.** Rimsky prefers a harmonic basis that is weaker in dynamic power
  and an octave removed from the melodic design (section 4.4), and warns against "artificial
  reduction of tone" just to let a voice through [Cited S1 Ch. IV pp. 114-115; Ch. V p. 120].

### 3.3 Proposed numeric weight table (for software)

**Unit and meaning.** 1.0 = one `flute`, `oboe`, `clarinet_bb` or `bassoon` in its middle register
at the same notated dynamic. A weight compares parts that sound **at the same marking**. It is a
relative count of "how many wood-wind players this equals", in Rimsky's own additive arithmetic,
not a decibel value [Estimate: derived from the ratios in 3.1; no source gives absolute levels].

**Derivation** [Estimate: derived from S1 Ch. I p. 33]: forte: horn = 2 wood-winds, trumpet = trombone =
tuba = 2 horns = 4; one medium-orchestra string department = 2. Piano: every wind = 1 and one string
department = 1. Mezzo-forte is not given by the source; it takes the geometric midpoint (horn 1.4,
trumpet 2, string department 1.4).

| Instrument (id), as played | pp-p | mf | f-ff | Provenance |
|---|---|---|---|---|
| `flute`, `oboe`, `clarinet_bb`, `clarinet_a`, `bassoon` (one player) | 1 | 1 | 1 | unit [Cited S1 p. 33] |
| `english_horn`, `oboe_damore`, `clarinet_eb`, `bass_clarinet`, `alto_flute`, `bass_flute`, `piccolo`, `contrabassoon` (one player) | 1 | 1 | 1 | [Estimate: same group as the unit, S1 gives no separate figure]; apply the register factors in 3.4 |
| `saxophone_soprano`, `saxophone_alto`, `saxophone_tenor`, `saxophone_baritone` | 1 | 1.4 | 2 | [Estimate: no opened source gives a ratio; placed with the horn as a provisional value] |
| `horn_f` (one player, open) | 1 | 1.4 | 2 | [Cited S1 p. 33]; mf [Estimate: midpoint] |
| `wagner_tuba_tenor`, `wagner_tuba_bass`, `euphonium`, `flugelhorn` | 1 | 1.4 | 2 | [Estimate: conical, horn-like brass; no opened ratio] |
| `trumpet_bb`, `trumpet_c`, `trumpet_piccolo` | 1 | 2 | 4 | [Cited S1 p. 33] (piccolo trumpet [Estimate: same as trumpet]); mf [Estimate: geometric midpoint of the pp-p and f-ff values, as in the derivation above] |
| `cornet` | 1 | 1.7 | 3 | [Cited S1 Ch. I brass remarks, pp. 23-24: cornets "have not quite the same force" as trumpets and are "softer and weaker"]; [Card cards/high_brass.md#cornet]; numbers [Estimate: placed between the horn (2) and the trumpet (4) in forte; mf the geometric midpoint; no source gives a ratio] |
| `trombone_tenor`, `trombone_bass`, `tuba`, `cimbasso` | 1 | 2 | 4 | [Cited S1 p. 33] (cimbasso [Estimate: same as tuba]); mf [Estimate: geometric midpoint of the pp-p and f-ff values, as in the derivation above] |
| One string department, arco (`violin` Vns I or Vns II, `viola`, `cello`, `double_bass`), medium orchestra | 1 | 1.4 | 2 | [Cited S1 p. 33] for Vns I; other sections [Estimate: S1 states the ratio for "the whole of one department", taken to hold for each]; mf [Estimate: geometric midpoint of the pp-p and f-ff values, as in the derivation above] |
| String department of _n_ players | × _n_ / medium count | same | same | [Estimate: linear in players, matching Rimsky's additive count of winds; medium counts from S1 p. 6] |
| One solo string player | 0.5 | 0.5 | 0.5 | [Estimate: S1 Ch. IV "Soli in the strings" p. 105 says the solo tone is "comparatively weak" and needs "light, transparent accompaniment"; no number] |
| Pizzicato or col legno (any string department) | 0.5 | 0.5 | 0.5 | [Estimate: "easily overpowers" in S1 pp. 33-34; no number] |
| `harp` (one) | 0.5 | 0.5 | 0.5 | [Estimate: three or four harps needed against a forte tutti, S1 p. 29] |
| `celesta`; piano played softly | 0.5 | 0.5 | - | [Estimate: "easily overpowers" in S1 pp. 33-34] |
| `glockenspiel`, `xylophone`, `tubular_bells`, `crotales` | 2 | 3 | 4 | [Estimate: "will easily prevail", S1 pp. 33-34] |
| `timpani` | 1 | 2 | 4 | [Estimate: "ringing, resounding quality" that prevails, S1 pp. 33-34; set equal to the trumpet] |
| `bass_drum`, `clash_cymbals`, `tam_tam`, `snare_drum` tremolo, ff together | - | - | dominates | [Cited S1 p. 117]; model as a flag "overpowers tutti", not a number |
| Choir section, solo voice (`choir_*`, `voice_*`) | 1 | 1 | 1.5 | [Estimate: no opened ratio; voices are handled by the transparency order in 4.4] |

**Dynamic offset** [Estimate: derived from Rimsky's two "one degree louder" corrections in 3.2]:
for a part marked one dynamic step above the prevailing level, multiply its weight by 2; one step
below, by 0.5. Rimsky replaces the doubling of the horns (factor 2) by marking them one degree
louder, so one step is worth a factor of about 2 in his arithmetic.

**Worked check.** A forte four-part brass chord, two trumpets on the upper parts (4 + 4) and two
tenor trombones below (4 + 4), is level. Replace the trombones by single horns (2 + 2): the lower
parts fall to half, which is why S1 asks for two horns per part, or one horn per part marked one
degree louder (2 × 2 = 4). A full wood-wind chord in pairs adds only 2 per part, half a trumpet
part, so in forte a wood-wind part needs about four players to match one trumpet part's 4 when the
trumpets are not doubling it [Estimate: arithmetic on the table, consistent with S1's "three or four
wind instruments" above two horns in forte and to double a trumpet an octave higher, p. 57].

### 3.4 Register factors

Multiply the weight by the factor when the note lies in the named register. The factors are
[Estimate: numbers chosen to encode the cited qualitative statements; no source gives numbers].
The MIDI spans in the Register column are [Estimate: arithmetic on the ranges in
[model.md](model.md) and the cards' register boundaries].

| Instrument | Register | Factor | Statement it encodes |
|---|---|---|---|
| `flute` | lowest notes, up to about G4 (B3-G4, 59-67) | 0.5 | low flute is "easily covered", double basses blur it; in combination it "only begins to tell" from about G or A upward [Card cards/flutes.md#flute] |
| `alto_flute` | lowest sixth | 1 (no reduction) | Forsyth calls the bottom sixth "remarkably strong", and the card's other source gives it more dynamic presence in the bottom octave and a half; the card judges it still light in absolute terms, so it is not raised above 1 [Card cards/flutes.md#alto_flute] |
| `flute` | top notes | 1.5 | "bright, telling" [Card cards/flutes.md#flute] |
| `piccolo`, `clarinet_eb` | low and middle | 0.5 | "the tone is so much weaker that it is of little service in those regions" [Cited S1 Ch. I wood-wind remarks, p. 20] |
| `piccolo` | highest compass | 2 | "extraordinarily powerful" [Cited S1 Ch. I, p. 20] |
| `clarinet_bb`, `clarinet_a` | middle (throat: sounding about E♭4-A♭4, 63-68, on `clarinet_bb`; D4-G4, 62-67, on `clarinet_a`) | 0.5 | "weak in the middle compass" [Cited S1 Ch. II p. 48]; clarinets are effective at top and bottom, not as middle-register instruments except for a neutral background [Card cards/clarinets.md#clarinet_bb]; throat spans [Card cards/clarinets.md#clarinet_bb], [Card cards/clarinets.md#clarinet_a] |
| `horn_f` | bottom fifth of the low horn's compass (about B1-F♯2, 35-42) | 0.5 | the 4th horn tends to "drop out of sight" there [Card cards/horns.md#horn_f] |
| `contrabassoon` | very deep notes | 1.5 at pp-p | "very powerful in piano passages" [Cited S1 Ch. I, p. 20] |
| `oboe` | low | 1.25 | the oboe's "dark, nasal tone" prevails in unison with the clarinet low [Cited S1 Ch. II p. 47] |
| `bassoon` | high | 1.25 | the bassoon prevails over the clarinet high [Cited S1 Ch. II p. 48] |
| `harp` | low and middle, fast figures | 0.5 | rapid figures are clear only in the upper register, where strings are shorter [Cited S1 Ch. I "Harp", pp. 27-28] |

## 4. Register conflicts and masking

### 4.1 Spacing and the low-interval limits

What the opened sources state:

- **The natural harmonic series is the guide.** Wide intervals belong low and close ones high; a
  chord of wide range in several parts takes "wide intervals (octaves and sixths), in the bass part,
  lesser intervals (fifths and fourths) in the middle, and close intervals (3rds or 2nds) in the
  upper register" [Cited S1 Ch. III "Distribution of notes in chords", p. 67; "Remarks" 2, p. 78].
- **Bass to tenor.** "The bass should rarely lie at a greater distance than an octave from the part
  directly above it" [Cited S1 Ch. III p. 67].
- **The lowest octaves.** Music centred "in the lowest range (octaves 1 and -1)" suffers because
  "the proximity of harmonic intervals creates a bad effect" [Cited S1 Ch. IV "Limits of orchestral
  range", p. 106]; under the reading in 0.1 that is below MIDI 48 [Estimate: octave mapping].
- **Below the middle line of the bass staff** (D3 = MIDI 50), keep intervals consonant, ideally a
  tenth, fifth or octave; octaves work anywhere, tenths fairly low, fourths sound bad when low. The
  author gives this tip in place of the detailed charts, and gives as the reason that big-band horns
  are timbrally similar and cannot clear up the texture themselves [Cited S4].
- **Limits are guidelines.** Some intervals (a perfect fifth) can sit lower than others (a minor
  second); muddiness depends on dynamic and on the sound's upper harmonics, so soft low strings
  forgive more than loud brass; check a voicing as if its root were present even when it is not
  [Cited S5]. Limits vary by instrument, and some music uses mud on purpose [Cited S6].
- **Timpani pairs.** Two drums a third apart "only seem to get in each other's way" through "the
  confusion of their upper partials"; fifths give the heavy drum effect, octaves are good but less
  "drummy" [Cited S2 p. 45].

Proposed limit table for a rule engine. Lowest acceptable **lower** note for each interval
[Estimate: the commonly taught arranging guideline as remembered, cross-checked only against
Rimsky's "below MIDI 48 is risky" and S4's "below MIDI 50 consonant only"; the numeric chart
(S7) could not be opened and the S5 and S6 charts are images. Published charts disagree on
several values, notably the tritone and the fifth]:

| Interval | Lowest lower note (MIDI) | Note name |
|---|---|---|
| minor 2nd | 52 | E3 |
| major 2nd | 51 | E♭3 |
| minor 3rd | 48 | C3 |
| major 3rd | 46 | B♭2 |
| perfect 4th | 45 | A2 |
| tritone | 46 | B♭2 |
| perfect 5th | 34 | B♭1 |
| minor 6th, major 6th | 41 | F2 |
| minor 7th, major 7th | 41 | F2 |
| octave | none | |
| minor 9th | 40 | E2 |
| major 9th | 39 | E♭2 |

Severity scales with dynamic and family: a hint at pp-p in strings, a warning at f-ff or in brass
[Estimate: from S5's dependence on dynamic and upper harmonics].

The table is looser than S4's tip: below D3 (MIDI 50) it still admits a fourth, a tritone, sevenths
and ninths, which S4 rejects. The two checks are kept apart: SPC-01 applies the table, and SPC-02
applies S4's stricter rule, most strictly where the parts share a family, which is the case S4
gives its reason for. A seventh that clears the table can still be flagged by SPC-02 [Estimate:
reconciliation of S4 with the table].

### 4.2 Crowding of the middle register

- A melody in the middle of the orchestral range "is not so prominent" and needs help from
  dynamics, contrasting timbre, doubling or crossing; melody on top stands out "from the very fact
  of position alone", and less so at the bottom [Cited S1 Ch. II intro, p. 36].
- Close writing in the middle must have no voids: "Modern orchestrators do not allow any void in
  the intermediate parts"; wide empty intervals between upper and lower parts are worst in forte
  [Cited S1 Ch. III pp. 68, 78].
- When three upper wood-wind parts stand over a bass from another group, they should be complete in
  themselves to avoid open fourths and fifths; when wind only fills the two middle parts, use
  seconds, sevenths, thirds or sixths between them [Cited S1 Ch. III "Remarks" 4, p. 79].
- The middle needs the most care when the outer parts diverge: new parts fill the widening space,
  and they drop out as the parts converge [Cited S1 Ch. III p. 68; Ch. IV "Diverging and converging
  progressions", p. 113].

### 4.3 Which instruments mask which

| Masker | Masked | Condition | Sources |
|---|---|---|---|
| any sustaining group | `harp`, pizzicato and col legno strings, soft piano, `celesta` | together | [Cited S1 pp. 33-34] |
| `glockenspiel`, `xylophone`, bells, `timpani` | everything near them | together | [Cited S1 pp. 33-34] |
| `bass_drum` + `clash_cymbals` + `tam_tam` + `snare_drum` tremolo | the whole tutti | ff | [Cited S1 p. 117] |
| several winds | one string department | unison | [Cited S1 p. 34; Ch. II p. 58] |
| a string department | one wind | unison | [Cited S1 p. 34] |
| trumpets, trombones, tuba | horns | forte, one horn per part | [Cited S1 p. 33, p. 85] |
| horns | bassoons | forte, mixed chord | [Cited S1 Ch. III p. 90] |
| horns | wood-wind on top | forte | [Cited S1 Ch. III p. 90] |
| `double_bass` | low `flute` | together | [Card cards/flutes.md#flute] |
| `cello` | `violin` | violins and cellos in octaves | [Card cards/strings.md#cello] |
| powerful, piercing register of one instrument | weak register of another in the same chord (e.g. second flute low under oboes) | wood-wind chords | [Cited S1 Ch. III p. 72 rule 3, pp. 72-73] |
| sustained harmony in the voice's own register | women's voices especially | under a voice | [Cited S1 Ch. V p. 121] |
| long sustained double-bass notes | voices ("a peculiar throbbing effect") | under a voice | [Cited S1 Ch. V p. 121] |
| timpani and percussion tremolo | voices | under a voice | [Cited S1 Ch. V p. 120] |

### 4.4 Keeping the melody audible

- **Means.** Accentuated dynamics, contrast of timbre, doubling and tripling, and crossing of parts
  [Cited S1 Ch. II intro, p. 36].
- **Harmonic basis against a melodic design.** "The greater the dissimilarity in timbre between the
  harmonic basis on the one hand and the melodic design on the other, the less discordant the notes
  extraneous to the harmony will sound." The greatest contrast is voice against orchestra, then
  strings, wood-wind, plucked strings and percussion against each other; wood-wind and brass differ
  less, so a wind basis "generally remains an octave removed from the melodic design, and should be
  of inferior dynamic power" [Cited S1 Ch. IV "Tone quality as a harmonic force", pp. 114-115].
- **Transparency order under a solo or voice.** Strings are "the most transparent medium"; then
  wood-wind, then brass in the order horns, trombones, trumpets; pizzicato strings with harp suit a
  voice well; long sustained notes overpower more than short detached ones [Cited S1 Ch. V
  "Transparence of accompaniment", p. 120].
- **Fewer parts, more pedals.** Under a voice, avoid "incessant four-part harmony"; reduce the
  number of parts, sustain pedal notes, keep the harmony within one octave or duplicate it higher;
  save orchestral outbursts for the singer's rests [Cited S1 Ch. V pp. 119-121].
- **Solo strings.** A solo string instrument needs "light, transparent accompaniment"; a solo
  violin or a flute doubling can carry a melody in the top register [Cited S1 Ch. IV "Soli in the
  strings", pp. 105-106].
- **Instruments know their role.** Write so that each player can tell whether the part is
  foreground, secondary or background [Cited S3 p. 198].

## 5. Voicing per family

### 5.1 Terms

| Term | Meaning | Sources |
|---|---|---|
| Superposition (overlaying) | the pairs stacked in normal register order, e.g. 2 Fl over 2 Ob | [Cited S1 Ch. III "Wood-wind harmony", p. 72; "Overlaying (superposition), crossing, enclosure of parts", p. 90] |
| Crossing (interlocking, dove-tailing) | two pairs alternate: 1st of A, 1st of B, 2nd of A, 2nd of B | [Cited S1 p. 72]; Forsyth's "interlock" and "dove-tailed" [Cited S2 pp. 123, 262] |
| Enclosure | one pair placed inside another: A1, B1, B2, A2 | [Cited S1 p. 72] |
| Overlapping (hand-over) | the colour of one chord is "just overlapped on to the tone-colour of the next, so as to prevent any sort of 'gap'"; for lines, the last note of one instrument coincides with the first of the next | [Cited S2 p. 227]; [Cited S1 Ch. IV pp. 107-108] |

Rimsky's three methods are superposition, crossing and enclosure; "overlapping" is not one of his
terms (Open question 6).

### 5.2 Strings

- Close four-part harmony with the bass in octaves is "uniformly resonant"; fill an empty middle
  with double stops on violins or violas rather than by dividing only some parts [Cited S1 Ch. III
  "String harmony", p. 70].
- Divide every upper part the same way for six or seven parts (Vns I div., Vns II div., Violas
  div.); then mark cellos and basses down or thin them [Cited S1 p. 70].
- Short chords (three or four notes) are played quickly, sound well only forte (sf), best with wind
  support, and go to violins and violas; the cello usually takes the lowest note with the basses
  [Cited S1 p. 69].
- Divisi by desk (desks 1, 3, 5 upper) or by stand side; à 3 is harder to balance; four or more
  parts are for piano, as they "greatly reduce volume of tone"; mark the division in the score
  [Cited S1 Ch. I p. 7].
- Prefer dividing Vns II with violas over Vns I with violas: similar numbers and they sit together
  [Cited S1 Ch. I p. 7].
- Double stops above the third position are practically unknown in the orchestra
  [Card cards/strings.md#violin].
- Product reference: Logic's Studio Strings has an Auto Voice Split that deals chord notes to the
  sections, with Allow Unison and Split Voicing modes (8va, Drop 2, Drop 2+4)
  [Card cards/apple.md#2.2].

### 5.3 Wood-wind

Rimsky's four rules for wood-wind chords [Cited S1 Ch. III "Wood-wind harmony", pp. 71-72]:

1. Keep the doubling consistent through a passage: doubled or undoubled throughout, unless one part
   is to be prominent.
2. Follow the normal register order except for deliberate crossing or enclosure.
3. Make corresponding registers coincide (no second flute low under oboes: "The second flute will
   sound too weak and the oboes too piercing").
4. Give concords (octaves, thirds, sixths) to instruments of one colour and discords (seconds,
   fourths, fifths, sevenths) to different colours, especially with the oboe.

Distribution [Cited S1 Ch. III pp. 72-78]:

- **Pairs (2 Fl, 2 Ob, 2 Cl, 2 Fag).** Choose superposition, crossing or enclosure by register (do
  not set a weak register against a piercing one) and by motion ("one tone quality should be
  devoted to the stationary and another to the moving parts"). An enclosed colour must sit between
  two _different_ colours. Four different colours in close four-part harmony "is to be avoided";
  in wide four-part harmony it is acceptable and better the higher it lies.
- **Three parts.** Two instruments of one colour and one of another, "but never ... three
  different timbres"; superposition is the best course.
- **Threes (3 Fl, 2 Ob + English horn, 3 Cl, 2 Fag + contrabassoon).** Close three-part chords in
  one colour always sound well; for close four-part use three of one colour plus one other; three of
  one colour in wide spacing is inferior unless the third is a low instrument (alto flute, English
  horn, bass clarinet, contrabassoon).
- **More parts.** Extra parts are octave duplications of the four real parts; pick instruments
  that make the best octaves (2.2).
- **Background interlock.** For supporting harmony in the treble register: 1st oboe, 1st
  clarinet, 2nd oboe, 2nd clarinet, "in the same way that the two pairs of Horns are interlocked";
  the 1st oboe may then follow the melodic outline [Cited S2 p. 262].
- **Clarinet as stand-in.** The clarinet's neutral middle register can replace a second flute, a
  second horn or a bassoon without attracting notice [Cited S3 p. 31].

### 5.4 Brass

- Close order, no empty spaces [Cited S1 Ch. III "Harmony in the brass", p. 82].
- **Four horns** give perfectly balanced four-part harmony without doubling the bass; double the
  bass with a bassoon if needed [Cited S1 pp. 82-83].
- **Horn pairs.** "Nos. 1 and 3 play the upper parts and Nos. 2 and 4 the lower", with the notes
  "dove-tailed"; Forsyth points to the Eroica, "where the 3rd Horn is pitched throughout above
  the 2nd", so the order from the top is 1, 3, 2, 4; low-note specialists (2nd, 4th) should not be
  sent to the top, and a sudden complete change of tessitura defeats the player [Cited S2
  pp. 122-123].
- **Trombones and tuba.** Rarely in close four parts. Usual layouts: third trombone + tuba as the
  bass in octaves, the upper three parts on two trombones plus a trumpet or two horns in unison; or
  two horns + tuba as the bass in octaves with the three trombones above [Cited S1 pp. 82-83].
- **Trumpets on top.** Two trumpets on the upper parts, completed by two trombones or four horns in
  pairs; with three trumpets the fourth part goes to one trombone or two horns in unison [Cited S1
  p. 83].
- **Three-part brass.** Best in three trombones, three horns or three trumpets; when mixed, double
  the horns [Cited S1 p. 84].
- **Discords** of the seventh or second go to instruments of different colour [Cited S1 p. 84].
- **Wide chorale layout.** In widely divided chords across registers the horn register need not be
  doubled, like a chorale for double or triple choir [Cited S1 p. 85].
- **Endurance.** The high-note lip pressure of horn and trumpet cannot be borne for long [Cited S3
  p. 198]; an assistant may double the first horn to rest the principal [Card cards/horns.md#horn_f].

### 5.5 Mixed families

- Bassoons and horns bridge wood-wind and brass; in four parts, crossing gives the concords to the
  horns and the discords to the bassoons; bassoons may sit inside the horns, "but the inverse
  process is not to be recommended"; clarinets "should rarely be set inside the horns"
  [Cited S1 Ch. III p. 90].
- Wood-wind goes above trumpets and trombones in a combined chord [Cited S1 p. 92].
- Flute thirds low, sometimes with clarinets, between trumpets in octaves give "a beautiful
  mysterious effect" in soft passages [Cited S1 p. 90].
- Strings and wood-wind: "Fl./Ob. (Cl.) + Vns div.; Clar./Fag. + 'Cellos + Violas div."; rhythmic
  motion in the wind over sustained strings, or the reverse [Cited S1 Ch. III p. 94].
- Strings and brass are rarely juxtaposed, crossed or enclosed; exceptions are tremolo strings under
  sustained brass chords, short sforzando string chords, and horns doubled by divided violas or
  cellos [Cited S1 Ch. III p. 95].
- Chords alternated between groups keep regular voice leading as if no octave leap occurred, and a
  chord can be passed between groups in the same position [Cited S1 Ch. IV "Chords of different tone
  quality used alternately", pp. 108-109].

### 5.6 Percussion

- When any part of the orchestra plays a rhythmic figure, percussion should join it; light rhythms
  suit `triangle`, `tambourine`, `castanets`, `snare_drum`; vigorous ones suit `bass_drum`,
  `clash_cymbals`, `tam_tam`; strokes fall on strong beats, accented syncopations or detached
  sforzandi [Cited S1 Ch. IV "Use of percussion instruments", p. 117].
- Register pairing: triangle, side drum and tambourine with harmony in the upper register;
  cymbals, bass drum and gong with the lower [Cited S1 p. 117]. In Ch. I, triangle, castanets and
  little bells count as high, tambourine, rod, side drum and cymbals as medium, bass drum and gong
  as deep [Cited S1 Ch. I "Percussion instruments producing indefinite sounds", p. 33].
- Standard pairings: triangle or tambourine tremolo with wood-wind and violin trills; side-drum or
  stick-struck cymbal tremolo with sustained trumpet and horn chords; bass-drum or gong tremolo
  with trombone chords or low cello and bass notes [Cited S1 p. 117].
- Brass and wood-wind combine best with percussion; wind clarifies pizzicato, harp, timpani and
  percussion; plucked strings and percussion "blend perfectly" [Cited S1 Ch. I p. 34].
- Economy: percussion is "seldom employed, and practically never all together, but in single
  instruments or in two's and three's"; a silent group gains interest when it returns [Cited S1
  Ch. IV "Economy in orchestral colour", p. 118].
- Timpani: two drums on the main harmonic notes of the passage [Cited S2 pp. 45-46]; with three
  screw drums a timpanist can retune at least one "during a pause of some length" [Cited S1 Ch. I
  "Kettle-drums", p. 30].

### 5.7 Harp, keyboards and celesta in the texture

- **Harp.** Almost entirely harmonic; chords close, at most four notes per hand, not too far
  between hands, arpeggiated unless marked otherwise; in quick modulations the lingering strings
  mix chords; fast figures are clear only in the upper register; Rimsky uses only his octaves 1 to
  4; it is "essentially a diatonic instrument" and modulates slowly; glissandi sound best high and
  soft [Cited S1 Ch. I "Harp", pp. 27-29]. Under the reading in 0.1, octaves 1 to 4 are C2-B5
  (MIDI 36-83); the harp card's comfortable range C2-C7 (36-96) shares the bottom but is an
  Estimate built from other sources, and S1 allows the extreme notes for special effects and octave
  doubling [Card cards/plucked.md#harp]. It is "an instrument of harmonic formulae": once set, it
  is bound to the formula, and a pedal acts on every octave of its note name [Cited S2 pp.
  463-464]. Two harps can share the pedalling of chromatic music [Cited S1 p. 28].
- **Celesta.** "very similar to the glockenspiel" but gentler; only in full orchestras; replace it
  with an upright piano, not the glockenspiel [Cited S1 Ch. I "Piano and Celesta", p. 32]. It is
  among the instruments "too characteristic in timbre to be employed over frequently" [Cited S1
  Ch. IV p. 118].
- **Piano.** In the orchestra (outside concertos) an upright is preferable; with the harp it
  imitates the guzli or soft bells [Cited S1 p. 32]. Played softly it is overpowered by sustaining
  groups [Cited S1 pp. 33-34].
- **Glockenspiel and xylophone.** The glockenspiel is "more brilliant and penetrating" than the
  celesta; the xylophone is "both powerful and piercing"; col legno strings sound similar to the
  xylophone [Cited S1 Ch. I p. 32].
- **Organ.** Views differ: Berlioz held that organ and orchestra rarely combine without one
  dominating; Widor answered that swell boxes let it balance the orchestra
  [Card cards/historical_keys_organs.md#organ_pipe].

## 6. Checkable rules for Sonara's arranging layer and the AI Composer

### 6.1 Inputs the rules assume

| Input | Meaning | Exists in Sonara today? |
|---|---|---|
| `id` | the orchestration id that a track plays | no: no field maps a track, preset or library patch to an orchestration id [Cited S9: project-v1.schema.json (track fields), crates/sonara-app/src/instruments/] |
| notes | sounding pitch, start, length, velocity per note | yes [Cited S9: project-v1.schema.json (note: id, pitch, start, length, velocity, channel)] |
| `range` | `check_sounding` → `Within`, `OutsideComfortable`, `BelowPlayable`, `AbovePlayable` | yes, for the 16 ids in `INSTRUMENTS` [Cited S8: instrument.rs, data.rs]; [model.md](model.md) gives researched ranges for all 85 ids and proposes corrections to 14 of the 16, which are not in the code yet |
| `dyn` | notated dynamic per note or passage (pp to ff) | no; velocity is the only per-note loudness, and no agent tool writes MIDI CC [Cited S9: project-v1.schema.json, docs/agent-api.md] |
| `part` | Vns I / Vns II, Horn 1-4, solo or section, number of players, divisi, mute | no part, role or ensemble field [Cited S9: project-v1.schema.json] |
| `role` | melody, counter-melody, harmony, bass, rhythm, colour, pedal | no [Cited S9: project-v1.schema.json] |
| `harmony` | the chord per beat, and which part holds which chord member | derivable from the notes [Estimate: needs a chord analyser; `generate_midi` writes chords but stores no analysis] |
| `tempo`, `meter` | for durations in bars and seconds | yes: `tempo_map` and `time_signatures` in the project, set by `set_tempo` and `set_time_signature` [Cited S9: project-v1.schema.json, docs/agent-api.md] |
| `pedals`, `tuning` | harp pedal setting; timpani tuning per drum | no |

Severity: **error** = unplayable or out of range; **warning** = playable but a standard handbook
says it fails (balance, masking, endurance); **hint** = style or colour advice that a composer may
override on purpose. The severity choices are [Estimate: this file's judgement of how hard each
source states the rule].

### 6.2 Rule catalogue

| ID | Rule | Inputs | Check | Severity | Source |
|---|---|---|---|---|---|
| RNG-01 | Note outside the playable range | `id`, notes | `check_sounding` is `BelowPlayable` or `AbovePlayable` | error | [Cited S8]; ranges per [model.md](model.md) (the `data.rs` values until its proposed corrections land) |
| RNG-02 | Note outside the comfortable range | `id`, notes | `check_sounding` is `OutsideComfortable` | warning | [Cited S8]; ranges as for RNG-01 |
| RNG-03 | Sustained extreme high brass | `id` in brass, notes, `tempo` | notes above the comfortable range held or repeated for more than about 2 bars | warning | [Cited S3 p. 198]; bar count [Estimate: provisional threshold; S3 gives no bar count] |
| RNG-04 | Extreme-register melody undoubled | `role`=melody, notes | a violin or wood-wind melody above about MIDI 90 without a doubling an octave below, or a melody below about MIDI 42 without one an octave above | hint | [Cited S1 Ch. II pp. 44, 50]; MIDI values [Estimate: octave mapping in 0.1] |
| SPC-01 | Low-interval limit | notes sounding together | any interval whose lower note is under the 4.1 table value | hint at pp-p in strings; warning at f-ff or in brass | [Cited S1 Ch. IV p. 106]; [Cited S4]; [Cited S5]; table [Estimate: the 4.1 limit table, a remembered guideline; see Open question 2] |
| SPC-02 | Dissonance in the low bass | `id`, notes sounding together | a dissonant interval (second, fourth, tritone, seventh, ninth) between two parts with its lower note under MIDI 50; stricter than SPC-01, whose table admits some of these lower (4.1) | warning when both parts are of one family; hint otherwise | [Cited S4]; family scope [Estimate: from S4's stated reason, timbrally similar horns] |
| SPC-03 | Bass too far below the harmony | `harmony`, notes | bass more than an octave below the next part up, sustained | hint | [Cited S1 Ch. III p. 67] |
| SPC-04 | Hole in the middle | `harmony`, notes, `dyn` | adjacent upper parts more than an octave apart in a sustained chord | warning at f-ff, hint at pp-p | [Cited S1 Ch. III pp. 68, 78] |
| SPC-05 | Spacing against the harmonic series | `harmony`, notes | an interval between adjacent parts larger than the interval below it, near the bottom of the chord | hint | [Cited S1 Ch. III pp. 67, 78] |
| SPC-06 | Bass doubled upward or mixed into the upper parts | `harmony`, notes | the bass's pitch class doubled in an upper part in wide spacing, or the bass part crossing above the tenor | hint | [Cited S1 Ch. III p. 64] |
| SPC-07 | Consecutive octaves between upper parts | `harmony`, notes | two independent upper parts move in parallel octaves (not a whole-line doubling) | warning | [Cited S1 Ch. III p. 65] |
| SPC-08 | Bass of an inverted seventh chord doubled | `harmony`, notes | the bass note of an inverted dominant or other seventh chord doubled in an upper part | hint | [Cited S1 Ch. III p. 65] |
| DBL-01 | Unknown unison pairing | `id` pairs in unison | the pair is not in the tables of 2.1-2.6 | hint (report the expected colour when it is in the tables) | [Cited S1 Ch. II pp. 47, 58, 61] |
| DBL-02 | Wood-wind order inverted in a doubling | `id`, notes | the lower-family instrument above the higher one in an octave, third or sixth doubling (e.g. `bassoon` over `clarinet_bb`) | hint | [Cited S1 Ch. II pp. 47, 49, 52] |
| DBL-03 | Same-colour winds in octaves without strings | `id`, notes | two players of one wood-wind id in octaves with no string doubling | hint | [Cited S1 Ch. II pp. 49-50] |
| DBL-04 | Wood-wind octave above a trombone | `id`, notes | a wood-wind doubles a trombone an octave higher and no trumpet does | warning | [Cited S1 Ch. II p. 57] |
| DBL-05 | Divided low strings in octaves | `part`, notes | two halves of one `viola`, `cello` or `double_bass` section move in octaves | warning | [Cited S1 Ch. II p. 44] |
| DBL-06 | Violins divided in octaves, too few players | `part`, notes | Vns I and II both divided in octaves, string count below the medium orchestra, no wind doubling | warning | [Cited S1 Ch. II p. 41; Ch. I p. 6] |
| DBL-07 | Expressive solo doubled | `role`, `dyn`, notes | a part marked solo and expressive, at p, doubled in unison by another instrument | hint | [Cited S1 Ch. II p. 48] |
| DBL-08 | Double bass phrase unsupported | `id`, `role`, notes | a `double_bass` melodic phrase with no `cello` or `bassoon` in unison or octave | warning | [Cited S1 Ch. II p. 38] |
| DBL-09 | Strings and brass in unison expected to blend | `id`, notes | a string section and a brass instrument share a melody in unison with no wood-wind | hint | [Cited S1 Ch. II p. 61] |
| BAL-01 | Unequal chord members | weights (3.3, 3.4), `dyn`, `harmony` | in a sustained chord, a harmonic part's summed weight under half the strongest non-melody part | warning at f-ff, hint at pp-p | [Cited S1 Ch. III p. 69: "the resonance of different harmonic parts must be equally balanced"]; threshold [Estimate: provisional threshold; S1 gives no number] |
| BAL-02 | Single horns against trumpets or trombones | `id`, `part`, `dyn` | forte brass chord, one horn per part, horns not marked a step louder | warning | [Cited S1 Ch. III pp. 84-85] |
| BAL-03 | Melody not on top of the weight | weights, `role`, notes | melody weight not above the accompaniment weight in its own band and the bands next to it | warning | [Cited S1 Ch. II p. 36; Ch. IV pp. 114-115]; margin [Estimate: require at least 1.5 times] |
| BAL-04 | Wood-wind under-weighted against brass | weights, `dyn` | forte chord where wood-wind parts weigh less than brass parts | hint: mark the wood-wind a step louder or double them | [Cited S1 Ch. III p. 94] |
| BAL-05 | Non-sustaining melody against a sustained forte | `id`, `role`, `dyn` | `harp`, `celesta`, piano or pizzicato carries the melody while sustaining groups play f | warning | [Cited S1 Ch. I pp. 29, 33-34] |
| BAL-06 | Heavy percussion ff | `id`, `dyn` | `bass_drum`, `clash_cymbals`, `tam_tam` and a `snare_drum` roll together at ff over a melody | hint (it overpowers any tutti) | [Cited S1 Ch. IV p. 117] |
| BAL-07 | A family without its own bass | `id`, `harmony` | a family's sustained chord has no member on the chord's bass note | hint | [Cited S3 p. 197] |
| BAL-08 | Solo string over heavy accompaniment | `part`, weights | a solo string carries the melody while accompaniment weight exceeds about 2 units | warning | [Cited S1 Ch. IV p. 105]; threshold [Estimate: provisional threshold on the 3.3 weights; S1 gives no number] |
| BAL-09 | String body too small for the winds | `part` | fewer than 8 first violins with wind in pairs or more | hint | [Cited S1 Ch. I p. 6] |
| MSK-01 | Accompaniment in the voice's register | `id`, `role`, notes | sustained accompaniment in the same band as a voice or solo melody | warning | [Cited S1 Ch. V p. 121] |
| MSK-02 | Heavy doublings under a voice | `id`, `role` | under a voice: strings doubled by wood-wind and brass; brass doubled by wood-wind; one harmonic part on 2 clarinets, 2 oboes or 2 horns in unison; timpani or percussion tremolo; long sustained double-bass notes | warning | [Cited S1 Ch. V pp. 120-121] |
| MSK-03 | Weak register exposed | `id`, notes, weights | a melody in a register with factor 0.5 (3.4) under accompaniment of weight 1 or more in the same band | warning | [Cited S1 Ch. I p. 20; Ch. II p. 48]; [Card cards/flutes.md#flute]; [Card cards/horns.md#horn_f] |
| MSK-04 | Harmonic basis too close to the melody | `role`, notes, `id` | a wood-wind or brass harmonic basis within an octave of a wood-wind or brass melody, at equal or higher dynamic | hint | [Cited S1 Ch. IV pp. 114-115] |
| VOI-01 | Three-part wood-wind chord in three colours | `id`, `harmony` | three wood-wind ids, one per chord member | hint | [Cited S1 Ch. III p. 74] |
| VOI-02 | Close four-part chord in four colours | `id`, `harmony` | four wood-wind ids in close position | hint | [Cited S1 Ch. III p. 74] |
| VOI-03 | Enclosure inside one colour | `id`, `harmony` | an enclosed pair sits between two instruments of the same id | hint | [Cited S1 Ch. III p. 74] |
| VOI-04 | Doubling changes within a passage | `id`, `harmony` | some chord members doubled and others not, changing chord to chord without melodic reason | hint | [Cited S1 Ch. III p. 72] |
| VOI-05 | Discord in one colour | `id`, `harmony` | a second, fourth, fifth or seventh between two players of one wood-wind id (stricter for `oboe`) | hint | [Cited S1 Ch. III p. 72] |
| VOI-06 | Non-corresponding registers | `id`, notes, register factors | a member in a weak register (factor 0.5) next to a member in a strong one (factor ≥ 1.25) in one chord | hint | [Cited S1 Ch. III pp. 72-73] |
| VOI-07 | Horn pairs broken | `part` (Horn 1-4), notes | Horn 3 below Horn 2, or Horn 2 or 4 given the top of the chord, in filling-up parts | hint; warning when Horn 4 is sent above the comfortable range | [Cited S2 pp. 122-123] |
| VOI-08 | Clarinets inside horns; horns inside bassoons | `id`, `harmony` | the named enclosure | hint | [Cited S1 Ch. III p. 90] |
| VOI-09 | Wood-wind below trumpets in a combined chord | `id`, `harmony` | wood-wind chord members below the trumpets while trumpets hold the top | hint | [Cited S1 Ch. III p. 92] |
| VOI-10 | Uneven string divisi | `part`, `harmony` | some upper string parts divided and others not in a sustained chord; or cellos and basses undivided under fully divided upper strings at equal dynamic | hint | [Cited S1 Ch. III p. 70] |
| VOI-11 | Divisi in four or more at forte | `part`, `dyn` | one section divided four or more ways at f-ff | warning | [Cited S1 Ch. I p. 7] |
| VOI-12 | Sustained multi-stop string chords | `id`, notes | three or four simultaneous notes in one string part longer than a short chord, or at p | warning (only the upper two notes can be held) | [Cited S1 Ch. III p. 69] |
| PLY-01 | Wind breath | `id`, notes, `tempo` | a wind part plays without a rest for longer than a provisional 8 bars of moderate 4/4; one held medium-register note lasts longer than about 10 bars of moderate 4/4 on a saxophone, or longer than about 40 s (a soloist's maximum at piano) on a saxophone or clarinet; the flute's lowest register runs more than a few notes without a breath | warning | rule [Cited S1 Ch. I p. 19, p. 25]; held note [Cited S3 p. 127], [Card cards/saxophones.md#saxophone_alto]; flute low [Card cards/flutes.md#flute]; 8 bars [Estimate: provisional; no opened source gives a phrase length without a rest] |
| PLY-02 | Brass endurance | `id`, notes | long loud or high passages with no rest; repeated notes outside the middle register | warning | [Cited S3 p. 198]; [Card cards/high_brass.md#trumpet_bb] |
| PLY-03 | Bow length | `id`, notes, `dyn`, `tempo` | one slurred pp group longer than about 4 bars of moderate time (about half that for many-note slurs); long forte slurs | hint | [Card cards/strings.md#violin] |
| PLY-04 | Mute change time | `part`, `tempo` | mute on with less than about two bars of moderate time free; off with less than slightly under that | warning | [Card cards/strings.md#violin] |
| PLY-05 | Pizzicato and arco switch | `id`, articulation, `tempo` | a switch from pizz to arco with less than about half a second free | warning | [Card cards/strings.md#violin] |
| PLY-06 | Pizzicato speed | `id`, articulation, notes | pizzicato faster than the same passage could be bowed; double-bass pizzicato much slower than violin | hint | [Cited S1 Ch. I "Pizzicato", p. 27] |
| PLY-07 | Harp pedal setting | `pedals`, notes | a note outside the current setting with no pedal change; two spellings of one letter at once; a double sharp or flat | error | [Cited S1 Ch. I p. 28]; [Cited S2 pp. 463-464]; [Card cards/plucked.md#harp] |
| PLY-08 | Harp pedal changes | `pedals`, notes, `tempo` | more than one change per foot (left D C B, right E F G A) per beat at moderate tempo, or a change on a string that must sound | warning | [Card cards/plucked.md#harp] (the per-beat limit is the card's [Estimate: synthesis of the card's pedal rows; no source states it as a rule]) |
| PLY-09 | Harp chord shape | `id`, notes | more than four notes for one hand, or hands too far apart | error for more than 8 notes, warning otherwise | [Cited S1 Ch. I "Harp", p. 28] |
| PLY-10 | Harp fast figures low | `id`, notes, `tempo` | rapid figures in the low and middle register | hint | [Cited S1 Ch. I p. 28] |
| PLY-11 | Timpani retuning | `tuning`, notes, `tempo`, `meter` | a pitch not on a tuned drum; a retune of a whole tone with less than 2-3 bars of 4/4 moderato rest, proportionally more for larger changes; more distinct pitches in a passage than drums | error when no drum and no time; warning when time is short | [Cited S2 p. 46]; [Cited S1 Ch. I "Kettle-drums", p. 30] |
| PLY-12 | Timpani thirds | `id`, notes | two drums struck together a third apart | hint | [Cited S2 p. 45] |
| PLY-13 | Horn tessitura jumps | `part`, notes | a sudden complete change from high to low tessitura in one horn part | hint | [Cited S2 p. 123] |
| PLY-14 | Fast repeated notes and tonguing | `id`, notes, `tempo` | fast repeated notes needing double tonguing on `oboe`, `clarinet_bb` or `bassoon` (only the flute double-tongues among wood-wind), or on low brass (only trumpets and cornets among brass) | hint | [Cited S1 Ch. I p. 18, p. 25] |
| PLY-15 | Clarinet octave leaps | `id`, notes | rapid leaps between octaves on `clarinet_bb` | hint | [Cited S1 Ch. I p. 18] |
| TEX-01 | Characteristic colours overused | `id`, notes | `celesta`, `glockenspiel`, `xylophone`, `castanets` or piano sounding in most bars of a piece | hint | [Cited S1 Ch. IV p. 118] |
| TEX-02 | Percussion all together, or never resting | `id` | all percussion at once, or brass and percussion with no tacet | hint | [Cited S1 Ch. IV p. 118] |
| TEX-03 | Tutti at pp | `id`, `dyn` | full tutti marked pp-p | hint | [Cited S1 Ch. IV p. 102] |
| TEX-04 | Echo or imitation against register | `id`, notes | an echo given to a different family (wood-wind echoing strings), or an imitation in a higher register given to a lower instrument | hint | [Cited S1 Ch. IV p. 110] |
| TEX-05 | Crescendo order | `id`, `dyn` | a long crescendo that brings in brass before wood-wind and strings, or a diminuendo that drops strings first | hint | [Cited S1 Ch. IV p. 112] |
| TEX-06 | Hand-over gap | `id`, notes | a line passed between instruments with no common note at the join | hint | [Cited S1 Ch. IV pp. 107-108]; [Cited S2 p. 227] |

### 6.3 How the rules reach the arranging layer and the AI Composer

- **Data first.** The weight table (3.3), register factors (3.4), doubling tables (2.1-2.7) and the
  limit table (4.1) are static data of the same kind as `INSTRUMENTS`, which already carries
  `Provenance::Cited` or `Estimate` per value [Cited S8: instrument.rs]. Keeping every row's tag lets
  the checker report its source, and an Estimate can be shown as softer advice than a Cited rule
  [Estimate: design proposal].
- **Contracts.** Library code and data in a crate are not CTR-01 changes; a new agent tool that
  runs the checks, a new template id (the `create_project` template is a literal in the tool
  schema), or new track fields in `.sonara` are [Cited S9: crates/sonara-agent/src/contract/inputs.rs,
  project-v1.schema.json]. Unknown keys on tracks survive a round trip in the `extra` map, so `part`
  and `role` could be stored there before a versioned schema decision [Cited S9:
  docs/project-format.md]; whether that is acceptable is an owner decision (Open question 5).
- **Real time.** The checks read notes and metadata and never run on the audio thread; they belong
  outside the real-time crates (RT-01) [Estimate: design proposal; `sonara-midi` is not one of the
  four real-time crates that RT-01 names].
- **AI Composer.** Its system prompt carries no orchestration data (ranges, articulations,
  doublings); it only allows the composer to look up musical reference, instrument ranges among it,
  with WebSearch and WebFetch [Cited S9: crates/sonara-app/src/composer/system_prompt.md]. The
  cheapest first step is to give it the function table (section 1), the doubling and "do not"
  tables (2.1, 2.8) and the balance ratios (3.1) as text; the rule catalogue needs a tool or a CLI
  check before an agent can run it [Estimate: design proposal].
- **Realization limits.** A weight correction can act today through three levers: note velocity
  (the sampler's gain is volume × (v/127)², the SFZ default curve, and velocity also picks a
  patch's `lovel`/`hivel` layer); the track's volume (`set_track_volume`, or `create_automation`
  with target `volume`); and the sampler's automatable `volume` parameter (`create_automation` with
  target `<device_id>:volume`) [Cited S9: docs/agent-api.md, crates/sonara-core/src/plugins/builtin.rs];
  [Cited S10: Q6, Q8]. None of them changes tone colour the way a played dynamic does: the sampler
  plays no CC 1 dynamics crossfade and ignores `xfin_*`/`xfout_*` velocity crossfades, so layers
  switch abruptly at their velocity boundaries; CC 7 and CC 11 act only as gain and reach a clip only
  through `import_midi` or live input, because no agent tool writes MIDI CC [Cited S10: Q6, C15];
  [Cited S9: docs/agent-api.md]. A "one step louder" correction (factor 2 in 3.3) therefore moves
  velocity or volume, not the instrument's timbre.

## Open questions

1. **Rimsky-Korsakov's octave numbers.** The mapping in 0.1 is inferred from three anchors in the
   text. It should be confirmed against the Vol. II examples or Table A (images in the HTML
   edition) before any rule uses its MIDI thresholds (RNG-04, the bands, the "below MIDI 48" limit,
   the harp's octaves 1 to 4 in 5.7).
2. **Low-interval limits.** The numeric table in 4.1 is an Estimate. No numeric chart could be read
   as text (S7 refused the request; S5 and S6 are images), and published charts disagree, notably
   on the tritone and the perfect fifth. S4 is stricter than the table below MIDI 50 (SPC-02). A
   text source, or a roughness model checked against listening, is needed before SPC-01 can be
   stricter than a hint.
3. **Horns in soft playing.** Rimsky-Korsakov makes all winds "fairly equal" in piano; the Vienna
   Symphonic Library's horn page, quoted in the horn card, makes trumpets and trombones at pp match
   horns at p. The weight table follows Rimsky-Korsakov; the other reading would halve the horn's
   piano weight.
4. **Weights without sources.** Mezzo-forte values, saxophones, Wagner tubas, euphonium,
   flugelhorn, the cornet's number (S1 says only that it is weaker than the trumpet), solo strings,
   harp, keyboards, tuned percussion and voices are Estimates. Two ways to calibrate: compare the
   cited handbook examples (Rimsky-Korsakov's Vol. II scores) with the weights, or measure the
   loudness of real library samples at matched dynamics. Sample libraries may normalise their
   recordings, so a library's own balance may differ from the hall balance the handbooks describe
   [Estimate: general sample-library practice; no opened source states it].
5. **Where parts, roles and dynamics live.** Vns I / Vns II, Horn 1-4, solo or section, players,
   divisi, mute, role and notated dynamic have no place in Sonara's model. Options: the `extra` map
   on tracks, a versioned `.sonara` schema change (CTR-01), or only in-memory metadata derived by
   the arranging layer. This needs an owner decision.
6. **"Overlapping".** Rimsky-Korsakov's three voicing methods are superposition (overlaying),
   crossing and enclosure; "overlapping" is used here for Forsyth's hand-over of chord colours and
   Rimsky-Korsakov's common-note transfer. If a fourth voicing method was intended, the source for
   it is still missing.
7. **Numbers in seconds.** Harp pedal changes and pedal-timpani retuning have no figure in seconds
   in any opened source; Forsyth's timpani figure is in bars of moderato 4/4 and dates from hand-
   tuned drums. Breath limits are numeric only for one held note on the saxophone and the clarinet
   (Widor, S3 p. 127); no opened source gives a phrase length without a rest, so PLY-01's 8 bars is
   an Estimate.
8. **String section scaling.** The weight of a string section is assumed linear in its players,
   which matches Rimsky-Korsakov's additive counting of winds but not necessarily the ear. A larger
   string body needs a different rule if listening says otherwise.
9. **Thresholds.** Every numeric threshold in 6.2 (bars, margins, the half-weight rule) is an
   Estimate and needs test cases. Rimsky-Korsakov's own examples are a natural oracle for the
   balance and voicing rules (TST-10), if they can be transcribed.

## Review notes

Changes made when this file was prepared for the repository. The 67 rule ids of 6.2 (RNG-01 to
TEX-06) are unchanged; no rule was added, removed or renumbered.

- **Repository form.** Removed local paths and internal labels; S8 now names repository paths only;
  the former repository-notes source was replaced by S9, the repository files themselves (project
  schema, project-format doc, agent API, contract inputs, instrument catalog, plugin descriptors,
  AI Composer prompt), each re-checked in the code; added S10 for the verified sampler behaviour in
  [sfz-conformance.md](sfz-conformance.md). Card tags now name `cards/<file>.md`. The "Opened"
  column no longer refers to a session.
- **Rimsky-Korsakov pages (S1), checked against the HTML edition's page anchors.** Wind and brass
  unison: the text says the wood-wind "softens and rarefies" the brass (the file had a misquote,
  "rarefied"). Page fixes: band words pp. 36-38; "middle parts" p. 37; Vns I / Vns II octaves
  pp. 40-41; same-colour winds in octaves pp. 49-50 (2.8, DBL-03); trumpets beside trombones
  pp. 85-86; bassoon doubling a horn chord's bass p. 83 (was 82) and four horns pp. 82-83;
  "liable to drown the singer" pp. 120-121; hand-over pp. 107-108 (section 1, 5.1, TEX-06);
  harmonic basis pp. 114-115 (3.2, 4.4, BAL-03, MSK-04). All other S1 quotes were found on the
  cited pages.
- **Widor (S3).** The double-bass mute passage is p. 190; the saxophone and clarinet held-note figure
  is p. 127, now cited in PLY-01 and Open question 7. pp. 31 and 197-198 confirmed. Forsyth (S2)
  pages 45-46, 122-123, 227, 262 and 463-464 confirmed.
- **Contradictions with the cards, fixed.** `alto_flute` was in the "lowest notes easily covered"
  register factor; its card calls the bottom sixth "remarkably strong", so it now has its own row
  (factor 1). `cornet` had the trumpet's weight; S1 and the high-brass card make it weaker, so it now
  has its own row (Estimate 1 / 1.7 / 3). `marimba` was a rhythm carrier; its card gives it mostly
  harmonic material in the middle and low registers, so it moved to sustained harmony. "Glockenspiel
  dominates" is now marked as an Estimate from S1, because the card does not say it. The piccolo
  doubling uses the card's "two octaves (Forsyth: at the 16th)". Anchor (a) in 0.1 now matches the
  low-brass card (B1 is the bottom of the tenor trombone's gap B1-E♭2). The organ-pedal Estimate now
  says what the card states. The horn card's balance figures are noted as Rimsky-Korsakov's own.
- **Contradictions inside the file, fixed.** SPC-02 (S4: below MIDI 50 consonances only) and the 4.1
  table (which admits sevenths and ninths lower) conflicted; 4.1 now explains the relationship, and
  SPC-02 is a warning when both parts are of one family (S4's stated case) and a hint otherwise.
  PLY-01 applied the saxophone's single held-note figure to a whole phrase; it now separates the
  held note (cited) from the phrase length (Estimate). PLY-05 follows the card's direction (pizz to
  arco).
- **Code claims, fixed against the repository and sfz-conformance.md.** Meter exists
  (`time_signatures`, `set_time_signature`), so it is no longer an Estimate. The AI Composer prompt
  does mention instrument ranges as a web-lookup topic, though it carries no range data. A balance
  correction can move velocity, the track volume or the sampler's automatable `volume` parameter,
  not only velocity; CC 7 and CC 11 act as gain; CC 1 and velocity crossfades do nothing.
- **Added for the rule engine.** MIDI spans for the flute's weak bottom, the clarinet throat and the
  horn's bottom fifth (3.4), the harp's octaves 1 to 4 against its card (5.7), and a pointer from
  RNG-01/RNG-02 to [model.md](model.md) and its proposed `data.rs` corrections.
- Gave every bare Estimate tag its reason (10 tags), and put the comma-form tag of the 3.3 derivation into the colon form.
