# Harp and plucked strings: instrument cards

_Researched 2026-10-30. MIDI note numbers, C4 = 60 = middle C. Ranges are sounding unless marked written.
Transposition = sounding minus written, in semitones._

## Sources

| # | Source | URL | Kind (handbook, manufacturer, library maker, spec, encyclopedia, other) | Opened in this session |
|---|---|---|---|---|
| S1 | N. Rimsky-Korsakov, *Principles of Orchestration*, Project Gutenberg ebook 33900, plain text. The transcriber's note states that middle C is written C4. | https://www.gutenberg.org/cache/epub/33900/pg33900.txt | handbook | Yes: "Plucked strings" and "Harp" sections, read in full |
| S2 | Cecil Forsyth, *Orchestration* (Macmillan, 1914), Cornell copy, OCR text | https://archive.org/details/cu31924022381440 (text: https://archive.org/download/cu31924022381440/cu31924022381440_djvu.txt) | handbook | Yes: No. 52 Harp, No. 53 Chromatic Harp, No. 54 Mandoline, No. 55 Guitar, pp. 291-293 (lute family). Music examples are images that the OCR lost; no pitch is taken from them |
| S3 | H. Berlioz, *A Treatise on Modern Instrumentation and Orchestration*, tr. Mary Cowden Clarke, rev. J. Bennett (Novello, 1882), OCR text | https://archive.org/details/treatiseonmodern00berl | handbook | Yes: "The Harp", "The Guitar", "The Mandolin". Music examples are images and were not readable |
| S4 | Andrew Hugill, *The Orchestra: A User's Manual* (with the Philharmonia), harp pages: harps, harp_range, harp_techniques, harp_glissandos, harp_harmonics, harp_extended, harp_player, harp_construction | https://andrewhugill.com/OrchestraManual/harps.html | handbook | Yes: page text only (an online handbook). Audio and video were not played; several headings lead only to videos |
| S5 | Andrew Hugill, same manual, fretted-strings pages: frets, frets_guitar, frets_mandolin, frets_plucking, frets_extended, frets_player | https://andrewhugill.com/OrchestraManual/frets.html | handbook | Yes: page text only (an online handbook). Most technique headings lead only to videos |
| S6 | Berklee College of Music, "Writing for the Harp" | https://berklee.edu/node/2392666 | other | Yes (an educational article) |
| S7 | "Pitch Properties of the Pedal Harp, with an Interactive Guide", *Music Theory Online* | https://www.mtosmt.org/ojs/index.php/mto/article/view/155/128 | other | Yes (a peer-reviewed journal article) |
| S8 | Wikipedia, "Pedal harp" | https://en.wikipedia.org/wiki/Pedal_harp | encyclopedia | Yes |
| S9 | Wikipedia, "Harp" | https://en.wikipedia.org/wiki/Harp | encyclopedia | Yes (both parts of the page) |
| S10 | Harp Connection (dealer), Lyon & Healy Style 23 specification page | https://www.harpconnection.com/harpstore/details-23-harp.html | manufacturer | Yes (a dealer page for a Lyon & Healy model) |
| S11 | Vienna Symphonic Library, *Synchron Harp Player* manual, "GUI overview" | https://www.vsl.co.at/manuals/synchron-harp-player/gui-overview | library maker | Yes |
| S12 | Spitfire Audio, *Skaila Kanga - Harp Redux* user manual (PDF) | https://www.spitfireaudio.com/cdn/shop/files/SkailaKangaHarp_UserManual.pdf | library maker | Yes: the text was extracted locally and read |
| S13 | VSCO 2 Community Edition, `SFZ` branch: `Harp.sfz`, the file tree and `LICENSE` | https://github.com/sgossner/VSCO-2-CE/tree/SFZ | library maker | Yes |
| S14 | VCSL (Versilian Community Sample Library): the file tree and `LICENSE` | https://github.com/sgossner/VCSL | library maker | Yes |
| S15 | Sonatina Symphonic Orchestra: `Concert Harp.sfz`, `README.md`, `LICENSE` | https://github.com/peastman/sso | library maker | Yes |
| S16 | sfzformat.com, `lokey` opcode page | https://sfzformat.com/opcodes/lokey/ | spec | Yes |
| S17 | Wikipedia, "General MIDI" (the GM1 program table) | https://en.wikipedia.org/wiki/General_MIDI | encyclopedia | Yes |
| S18 | Wikipedia, "General MIDI Level 2" (the variation table) | https://en.wikipedia.org/wiki/General_MIDI_Level_2 | encyclopedia | Yes |
| S19 | The MIDI Association, General MIDI Level 1 pages | https://midi.org/general-midi-level-1 and http://www.midi.org/techspecs/gm1sound.php | spec | Yes, but neither page carries the program table, and the RP-003 PDF needs membership. Never cited for a number |
| S20 | Wikipedia, "Classical guitar" | https://en.wikipedia.org/wiki/Classical_guitar | encyclopedia | Yes (first 100,000 characters) |
| S21 | Wikipedia, "Mandolin" | https://en.wikipedia.org/wiki/Mandolin | encyclopedia | Yes (both parts of the page) |
| S22 | Wikipedia, "Lute" | https://en.wikipedia.org/wiki/Lute | encyclopedia | Yes (first 100,000 characters) |
| S23 | Wikipedia, "Symphony No. 7 (Mahler)" | https://en.wikipedia.org/wiki/Symphony_No._7_(Mahler) | encyclopedia | Yes |
| S24 | Wikipedia interlanguage links (MediaWiki API) for Harp, Pedal harp, Classical guitar, Lute and Mandolin, in cs/it/de/fr | https://en.wikipedia.org/w/api.php?action=query&prop=langlinks&lllimit=500&titles=Harp | encyclopedia | Yes |
| S25 | Sonara repository: `docs/sample-libraries.md`, `docs/orchestral-audio-engine-research.md` §5, `docs/orchestration-metadata.md`, `crates/sonara-midi/src/orchestration/{data.rs, articulation.rs, instrument.rs}`, `crates/sonara-midi/src/gm.rs` | (read-only checkout of the Sonara repository; paths relative to its root) | other | Yes |
| S26 | VSL Academy harp pages (`/en/Harp/Range`, `/en/Harp/Playing_Techniques`, `/en/Harp/Synchron_Harp`) | https://www.vsl.co.at/en/Harp/Range | library maker | No: HTTP 502. Not cited |
| S27 | VSL Synchron Harp product page | https://www.vsl.co.at/Synchron_Harp | library maker | No: HTTP 404. Not cited |
| S28 | Isabelle Perrin, harp notation PDF | https://www.isabelle-perrin.eu/wp-content/uploads/2019/04/andresNotations2018.pdf | other | No: HTTP 403. Not cited |
| S29 | Danielle Kuntz, "Harp writing tips" | https://daniellekuntz.substack.com/p/harp-writing-tips | other | No: paywalled. Not cited |
| S30 | composingforharp.com technique index | https://composingforharp.com/?p=7 | other | Opened: menu only, no technique text. Not cited |
| S31 | Dartmouth, introduction to Renaissance lute tablature | https://www.cs.dartmouth.edu/wbc/lute/tab-intro.html | other | No: TLS certificate error. Not cited |

## harp: Harp

```json
{"id": "harp", "family": "plucked", "names": {"en": "Harp", "cs": "harfa", "it": "arpa", "de": "Harfe", "fr": "harpe", "score_abbreviation": "Hp."}, "pitched": true, "transposition": 0, "clefs": ["treble", "bass"], "sounding_playable": [23, 103], "sounding_comfortable": [36, 96], "range_provenance": {"playable": "Cited S1, S8, S4: low Cb1 = 23 is the bottom of the range in S1 (C1 flat, C4 = middle C) and S8 (C flat 1, octave designations); it needs the lowest C string, which has no pedal (S8), to be tuned to C flat before the performance. Top G7 = 103 (S4: range C1-G7; S8: the top G string has no pedal and is normally G natural). G#7 = 104 is also obtainable, by retuning the unpedalled top G string before the piece (S8) or with a top-G mechanism (S2 footnote, S8 note), but no opened source gives G#7 as the end of the range, so it is left out (see Open question 2)", "comfortable": "Estimate: no opened source gives endpoints. C2-C7 is built from S3 (chords in the lowest part are confused and fit only for doubling a bass an octave below; the top octave sounds hard when struck forcefully) and S1 (the extreme notes are for special circumstances; rapid figures are clear only in the upper register)"}, "written_playable": [23, 103], "gm_program": 47, "gm_program_name": "Orchestral Harp", "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["sustain", "harmonics", "tremolo", "trill"], "articulations_proposed_new": ["glissando", "bisbigliando", "pres_de_la_table", "let_ring", "damped", "portamento", "xylophonic"], "articulations_provenance": "Cited S2, S3, S4, S11, S12 (harmonics, tremolo, trill, glissando, bisbigliando, pres de la table, let ring / laissez vibrer, damped / sons etouffes, pedal glissando = portamento); xylophonic is an Estimate (the only source found, S28, could not be opened)", "solo_or_section": "both", "typical_players": {"classical": "0-1 (rare before the Romantic era)", "romantic": "1-2 normal; 2-4 in large scores", "film": "1-2", "chamber": "1"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | harp, pedal harp, concert harp | [Cited S8, S9] |
| Czech | harfa | [Cited S24] |
| Italian | arpa | [Cited S2, S24] |
| German | Harfe (Pedalharfe for the pedal harp) | [Cited S2, S24] |
| French | harpe | [Cited S2, S24] |
| Score | "Hp." or "Hrp."; "Harp I / Harp II" when there are two | [Estimate: common publishing practice; no opened source prints the abbreviation] |

- Family slug `plucked`; `Family::Plucked` already exists in `instrument.rs` [Cited S25].
- The orchestral harp is the double-action pedal harp: 47 strings and 7 pedals [Cited S2, S4, S8]. A concert
  grand such as the Lyon & Healy Style 23 has 47 strings and stands about 74 in (188 cm) high [Cited S10].
- Of the ancient plucked instruments it is the only one in everyday orchestral use [Cited S2].

### 2. Sound production

- The player plucks with the fingertips of the thumb and first three fingers. The little finger is not used
  [Cited S2, S8].
- With every pedal up, the strings sound a C-flat major scale [Cited S2, S8]. Each of the 7 pedals moves every
  string of one pitch class: up = flat, middle = natural, down = sharp [Cited S6, S8, S12]. Each pedal has two notches; each notch raises the strings one
  semitone [Cited S2].
- Pedal layout, left to right: D, C, B (left foot) | E, F, G, A (right foot) [Cited S2, S6, S7, S8, S12].
- Strings: the lowest are copper- or steel-wound nylon, the lower middle gut, the middle to top nylon; some harps
  use more gut [Cited S8]. Forsyth's harp had the 11 longest strings covered [Cited S2]. C strings are red; F strings
  black or blue [Cited S8] (Forsyth: F blue [Cited S2]).
- The lowest C and D and the highest G have no pedal mechanism. They are normally tuned C♮, D♮, G♮ and can be
  retuned before a performance; a few modern harps have a pedal for the high G [Cited S8].
- The string must be plucked well out of the vertical to sound well. In the lower and middle registers the
  vibration is visible, and each pluck on the same string damps the previous note [Cited S2].
- The lower and middle strings ring on. Players damp them by hand at changes of harmony; in quick modulations they
  cannot, and the chords blur [Cited S1].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

**Octave conventions in this file.** S1 states that middle C is written C4 [Cited S1]. S8 anchors its "octave
designations" to middle C: the range starts at "the C♭ three octaves below middle C", which it writes C♭1, so S8
uses scientific pitch and C♭1 = 23 [Cited S8]. S4 states no convention; its
C1–G7 agrees with S8, so it is read the same way [Estimate: S4 gives no frequencies]. S11 and S12 state no middle-C
convention. VSL and Kontakt products often call middle C "C3" [Estimate: vendor convention, not stated in S11 or
S12], so their note names are reported as printed and converted only as Estimates. In this file, `[Cited Sx, Sy]`
means that each listed source was opened and states the fact; arithmetic derived from cited facts is an Estimate.

| Item | Value | Provenance |
|---|---|---|
| Strings | 47 diatonic strings, C1 to G7 | 47 strings [Cited S2, S4, S8, S10]; the span C1–G7 [Cited S4] (S2 prints its compass only as an unreadable image; S8 gives the same strings as C♭1–G♭7; S10 uses maker octave numbers) |
| Playable (recommended) | 23–103 (C♭1–G♮7) | see the JSON `range_provenance`; [Cited S1, S4, S8] |
| Range in the all-flat tuning | C♭1–G♭7 = 23–102 | [Cited S8] |
| Rimsky-Korsakov's range | C♭1–F♯7 = 23–102 | [Cited S1]; that his harp had 46 strings with F at the top [Estimate: inferred from the F♯7 top] |
| Optional top | G♯7 = 104: the unpedalled top G string retuned before the piece (S8: the three unpedalled strings "can also be tuned sharp or flat prior to performance"; "If necessary, tune high G to G♯"), or a top-G mechanism (S2 footnote: the top note "can be mechanically raised to G♮ or G♯"; S8: "A few modern harps have a pedal mechanism for the high G string") | [Cited S2, S8] |
| Lowest C, D and top G strings | fixed (no discs); retune by hand before the piece | [Cited S8] |
| Comfortable (recommended) | 36–96 (C2–C7) | [Estimate: see JSON; built from S1 and S3] |
| Transposition | 0. Written on two bracketed staves, like the pianoforte; treble and bass clefs at discretion | [Cited S2, S3] |
| Harmonics | written an octave below their sound, with a small circle | [Cited S2, S6] |
| Very sharp keys | write B, F♯, C♯ major as C♭, G♭, D♭: "It gives a greater vibrating-length for the strings" | [Cited S2] |
| Double sharps and double flats | impossible; respell | [Cited S1, S6, S9] |

**Harp-maker octave numbers.** S10 gives the Style 23 range as "0th Octave G to 7th Octave C". Harpists count
octaves from the top, so this is the same G7–C1 span [Estimate: the convention is not defined on any opened page;
it matches only if octave 0 holds the top G]. Never read these numbers as scientific octaves.

**Register character.**

| Register (sounding) | Character | Provenance |
|---|---|---|
| Lowest strings (about C1–B1) | "soft and dull"; chords there "without sonorousness"; fit for doubling a bass an octave below | [Cited S3]; MIDI span [Estimate: S3 names no pitches] |
| Low (about C2–B2) | "veiled, mysterious and beautiful"; rarely used beyond left-hand bass | [Cited S3]; span [Estimate: S3 names no pitches] |
| Middle and lower | long resonance that dies away gradually; blurs in quick changes | [Cited S1] |
| Upper | shorter, harder strings; the only register where rapid figures are clear | [Cited S1] |
| Top octave | "delicate crystalline sound"; struck violently it gives "a dry hard sound, similar to that of broken glass" | [Cited S3] |

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

**Power.** The harp is never a very powerful instrument; three or four harps in unison are needed to be heard
against a full orchestra playing forte [Cited S1]. Its effect grows with the number of harps [Cited S3]. Harmonics
are possible only played quite softly [Cited S1], and they are inaudible unless unaccompanied or under the pp of a
single orchestral group [Cited S2]. The faster a glissando, the louder it sounds [Cited S1].

**Hands.**

| Limit | Value | Provenance |
|---|---|---|
| Notes per hand | at most 4 (no little finger) | [Cited S1, S2, S6] |
| Chord span per hand | 4 notes within an octave; a tenth with a great stretch, less sonorous | [Cited S3] |
| | stretches up to and beyond a tenth are normal | [Cited S2] |
| | span should not exceed an octave and a fourth | [Cited S6] |
| Chords | always spread (arpeggiato) unless marked *non arpeggiato*; short chords marked "sec" | [Cited S1, S2] |
| Distance between hands | at least a sixth or an octave, or they get in each other's way | [Cited S3] |
| Arpeggios | in a quick tempo, at most about an octave per hand position | [Cited S3] |
| Long arpeggios | one part, hands alternating, 4 notes each | [Cited S2] |
| Repeated notes | on two strings tuned to the same pitch, at any speed; on one string only in slow tempos | [Cited S2, S3] |
| Low register | repeated left-hand figures sound muddy | [Cited S6] |

**Pedals: the main playability constraint.** Sonara should model the harp as 7 pitch classes, each in one of three
states, and check every passage against the current setting.

| Constraint | Value | Provenance |
|---|---|---|
| One pedal acts on every octave of its pitch class | yes: two octave strings can never differ | [Cited S2, S3, S9] |
| Most consecutive semitones in one setting | 4 | [Cited S7] |
| Largest gap between neighbouring strings | interval class 4 | [Cited S7] |
| Homophones (two strings on one pitch) | 9: B♯–C♮, C♯–D♭, D♯–E♭, E♮–F♭, E♯–F♮, F♯–G♭, G♯–A♭, A♯–B♭, B♮–C♭ | [Cited S2] |
| Pitches with no homophone | D♮, G♮, A♮ | [Cited S2, S3] |
| Chromatic scale | impossible even at a moderate tempo; chromatic runs are impracticable | [Cited S3] |
| Time per pedal change | "a small but appreciable fraction of time", "not always free from noise" | [Cited S2]; **no opened source gives a number in seconds or beats** |
| Simultaneous changes | opposite sides together are easy; three at once, or two on one side, are harder | [Cited S6] |
| | two to four changes at a time are common in the literature | [Cited S7] |
| Rests needed | a few beats or bars to set a glissando; warn the player a few bars before a modulation; one beat of rest helps chromatic set-ups | [Cited S2, S3, S6] |
| Pedal changes per modulation | one for a fifth or fourth, two for a major second, three for a minor third up, seven for a semitone up | [Cited S6] |
| Remote key changes | impossible on one harp in a fast tempo: use a second harp, or omit the harp | [Cited S1, S2, S3] |
| Wildly chromatic music | omit the harp or write for two, splitting the pedalling | [Cited S2] |
| A string being re-pedalled | out of action for that moment | [Cited S2] |

**Recommended rule for an agent** [Estimate: synthesis of the rows above, no source states it as a rule]: each
passage gets a pedal setting; a note outside it needs a pedal change; one change per foot per beat at a moderate
tempo; changes in a rest; more than four chromatic neighbours, a double sharp or flat, or two spellings of one
letter at once means the passage is not playable on one harp.

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

**Plucking is not an articulation (applies to all four cards).** The ordinary sound is a pluck, so `pizzicato`
(plucked bowed strings, in Sonara's vocabulary) is not listed. `sustain` stands for the ordinary undamped note
("ordinario"; "Normale" in S12; "Regular" in S11) [Cited S11, S12 for the library names]. It is a decaying pluck,
not a held tone [Estimate: mapping choice made for these cards so that a library's default patch matches an
existing id].

| Technique | Sonara id | What it is | Provenance |
|---|---|---|---|
| Ordinary pluck, ringing | `sustain` + `let_ring` | normal note; *laissez vibrer* (l.v.) cancels a damping instruction | [Cited S2, S4, S12] |
| Damped | `damped` | *sons étouffés*: the string is stopped right after the pluck; dry, like a short string pizzicato; it. *velare*, de. *dämpfen* | [Cited S2, S4, S12] |
| Harmonics | `harmonics` | only the 2nd partial: lower palm touches the string's midpoint; sounds an octave higher; written an octave below with "o" | [Cited S2, S3, S6, S12] |
| Glissando | `glissando` | a sweep over the strings in the current pedal setting; f or p, up or down, both hands, contrary motion | [Cited S2, S4, S6] |
| Enharmonic glissando | `glissando` | homophones leave only chord tones: dim7 chords; dom7 on G♭, A♭, B, D♭, E; dom9 on C, D, E♭, F, G, A, B♭ | [Cited S2, S4] |
| Bisbigliando | `bisbigliando` | "whispering": a quiet tremolo on a chord or on homophones; notated like a trill | [Cited S2, S4] |
| Tremolo, trill | `tremolo`, `trill` | two hands alternating; a harp trill is really a tremolo, often across two strings on one pitch; tolerable only high and pp | [Cited S2, S3, S4] |
| Près de la table | `pres_de_la_table` | plucked near the soundboard (p.d.l.t.); metallic, like a guitar or banjo | [Cited S2, S4, S12] |
| Pedal glissando | `portamento` (the pitch of a sounding string is bent by moving its pedal) | works in the low register, inaudible above C6 | [Cited S4] |
| Xylophonic | `xylophonic` | one hand mutes at the soundboard while the other plucks | [Estimate: standard harp technique; its only source found (S28) could not be opened] |
| Effects | (none proposed) | palm on the low strings gives a gong-like sound; nails along a string squeak; paper threaded through the strings | [Cited S4]; "FX" patch with scratches and palm hits [Cited S12] |

Glissando practice: the player crosses the whole compass in about one second; a glissando shorter than four or five
octaves sounds tame unless it loops [Cited S2]. Every one of the 7 strings in each octave must belong to the
setting, or the finger has to skip and the effect is lost [Cited S2]. Pianissimo glissandi belong high; forte
glissandi through the low and middle strings are embellishments only, because the strings ring on [Cited S1].
Harmonic chords: at most three notes, two in the left hand and one in the right [Cited S1]; the left hand can take
two to four if they are close, the right hand only one [Cited S2 (quoting Widor), S3, S6]. Harmonics are not
possible on the highest or lowest strings [Cited S2, S3].

**Harmonics range.** No opened source gives it in an unambiguous convention.

| Source | Says | Converted (sounding) |
|---|---|---|
| S12 | flageolet range "from G2 to C6" | 43–84 if C4 = 60; 55–96 if C3 = 60 [Estimate: S12 states no convention] |
| S4 | audio clips labelled G3 … E7 under "Harmonics on the low strings" | 55–100 if the labels are sounding pitches [Estimate: S4 does not say written or sounding] |
| S2 | 17 strings in the middle of the compass | pitches in an unreadable image |
| S3 | "only the two octaves above the last" | ambiguous wording |

Recommendation: sounding 55–96 (G3–C7), written 43–84 [Estimate: the C3 = 60 reading of S12 agrees with S4 and with
the rule that extremes do not work]. Caution (fact-check): G2–C6 written spans 25 diatonic strings, whereas S2 confines
harmonics to "the seventeen strings" in the middle of the compass [Cited S2], so the outer notes of this window are
doubtful; a narrower window of about 17 strings centred on it is safer [Estimate: S2's endpoints are in an image].

**How libraries implement the harp.**

| Library (manual read) | Mechanism | Provenance |
|---|---|---|
| VSL Synchron Harp | **Pedal mode:** the 7 strings per octave sit on the white keys, retuned by *pedalings*, so a glissando is a swipe over white keys; a guard ignores stray black keys. **Chromatic mode:** plays like a keyboard | [Cited S11] |
| | Articulations Regular, Harmonics and Bisbigliando, each with a ringing or a damped release | [Cited S11] |
| | Default key switches (VSL note names, convention not stated): Chromatic/Pedal A0/A♯0; pedal tuning on the black keys F♯1–G♯2 (a high velocity raises the pitch, a low one lowers it, a medium one resets it); pedaling stepper C♯1/D♯1; scale family F♯5–C♯6; scale variant F♯6–C♯7 | [Cited S11] |
| | A "Glissando Smoother" softens the attacks of detected glissando notes; velocity can come from a MIDI CC | [Cited S11] |
| Spitfire *Skaila Kanga - Harp Redux* (Kontakt) | Main Techniques patch with key switches; each articulation also comes as a separate patch | [Cited S12] |
| | Normale (velocity); sustain pedal (CC64) down = *laissez vibrer*; a key released with the pedal up = *sons étouffés* (damping on note-off); release triggers with a level control | [Cited S12] |
| | PDLT (velocity); Flageolet (velocity, "generally quiet"); Slid (single notes recorded as part of a glissando, for fast runs); FX | [Cited S12] |
| | Bisbigliando loudness on CC1 (mod wheel), not velocity; expression CC11 | [Cited S12] |
| | GUI pedal diagram D C B / E F G A, up = flat, middle = natural, down = sharp; with every pedal natural it plays chromatically; once one pedal is off natural, the black keys stop and the white keys follow the pedal scale; each pedal can be MIDI-CC learned | [Cited S12] |
| | "Glissandi Manual": key switches pick one of 6 scales (whole tone, harmonic minor, melodic minor, major, pentatonic, diminished). "Glissandi Legato": the **interval of two overlapping notes** picks the scale (minor 2nd whole tone, major 2nd harmonic minor, minor 3rd melodic minor, major 3rd major, perfect 4th pentatonic, diminished 5th dim7) | [Cited S12] |
| | 4,041 samples; several dynamics, round robins and release triggers | [Cited S12] |

Implication for Sonara [Estimate: design inference from S11, S12 and S25]: a pedal state (7 values) should be part of
the harp track's control data, not of the notes. A glissando can be rendered as fast notes over the set scale,
which S12's "Slid" articulation exists for. CC64 maps to `let_ring` against `damped`.

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- Most scores have one harp part; later composers write for two or three, sometimes compressed into one part
  [Cited S1]. Rimsky-Korsakov wrote for two harps in *Sadko*, *Kitezh* and *The Golden Cockerel*, and for three in
  *Mlada*; he says full orchestras should have three or four [Cited S1].
- "Every Symphony and Opera orchestra has at least a couple of Harps" (1914) [Cited S2]. Mahler 7 uses 2 harps
  [Cited S23].
- Several harps are divided into firsts and seconds with separate parts; separate parts sound richer than doubling
  one part [Cited S2, S3]. Two harps can share the pedalling of chromatic music [Cited S1, S2, S3].
- Unison doubling is for volume: three or four harps against an orchestral forte [Cited S1].
- `solo_or_section: both`: normally one player per part, sometimes massed in unison [Estimate: summary of the
  rows above].

### 7. Role in the orchestra and typical doublings

- Mainly a harmonic, accompanying instrument: chords and the figures that grow from them [Cited S1]; "nine-tenths of
  all orchestral Harp-music must be 'in arpeggio'" [Cited S2]. Like pizzicato strings, it is more an instrument of
  colour than of expression [Cited S1].
- Typical uses: single repeated notes, enharmonic "bell" unisons, bell-like melody notes, simple arpeggio
  accompaniments, low-middle chords, and bass octaves in quiet passages [Cited S2].
- Doublings: pizzicato strings and the harp form one plucked group [Cited S1]. Harmonics with flutes and clarinets
  in the middle register sound magical, and the harp blends best with horns, trombones and brass [Cited S3]. Pizz.
  cellos and basses and the harps reinforce sustained woodwind notes [Cited S1]. The piano, alone or combined with
  the harp, imitates the guzli or a soft peal of bells; S1 adds that the celesta is superseding the piano, but does
  not tie the celesta to the guzli [Cited S1].
- The harp became standard in the Romantic era; earlier composers used it rarely [Cited S9].
- In film the harp provides glissandi, arpeggios and "magic" colour
  [Estimate: general film-scoring practice, no source opened].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Facts | Provenance |
|---|---|---|
| Single-action harp in E♭ | 7 pedals, one notch; good in 8 keys only; obsolete after Érard's double action (1810–1811) | [Cited S2, S3] |
| Chromatic cross-strung harp (Lyon/Pleyel) | one string per semitone, no pedals; cannot play the pedal-harp glissandi; tone thought weaker | [Cited S1, S2] |
| Lever (Celtic, folk) harp | a lever raises one string a semitone; smaller than the pedal harp | [Cited S9]; a "Folk Harp" set exists in VCSL [Cited S14] |
| Smaller pedal harps (44–46 strings) | narrower range at the bottom | [Estimate: makers' catalogues not opened] |
| Electric/electro-acoustic harp | pickups for amplification | [Estimate: not researched] |

In the orchestra the harpist does not usually double on another instrument
[Estimate: no source opened; S1 and S2 never mention doubling].

### 9. General MIDI and existing Sonara metadata

- GM1 program **47 "Orchestral Harp"** (1-based; program-change value 46) [Cited S17]. The midi.org pages give no
  table (S19). In GM2, program 47 bank 0 is "Harp" and bank 1 "Yangqin" [Cited S18].
- `harp` is **not** in `data.rs` (16 instruments) and is listed as "Not covered" in
  `docs/orchestration-metadata.md` [Cited S25]. `gm.rs` holds only the channel-10 percussion map [Cited S25]. There is
  no data.rs verification table for this card.
- The vocabulary has no glissando, bisbigliando, pres_de_la_table, let_ring or damped id. These are proposed above
  [Cited S25].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence (as stated) | Coverage | Plays in the Sonara sampler? | Provenance |
|---|---|---|---|---|
| VSCO 2 CE `Harp.sfz` | CC0 1.0 Universal | 23 regions, keys 28–101 (E1–F7; sample names agree with C4 = 60); **one velocity layer** (0–127); no round robin; `ampeg_release=12` (s); ordinary pluck only, no harmonics, glissando or damping samples | yes: velocity, release and CC64 are supported | [Cited S13, S25] |
| VCSL "Concert Harp" | CC0 1.0 Universal | 45 WAVs at 23 pitches E1–F7, file names marking p, mp, mf and f (two layers per pitch: f plus one softer; E1 has f only); WAVs on the master branch; the SFZ files `Concert Harp.sfz` and `Folk Harp.sfz` exist only in the tag `v1.2.2-RC` (see [free_libraries.md](free_libraries.md), VCSL) | plays partly: both velocity layers sound together, because the sampler ignores velocity crossfades (Q6 in [../sfz-conformance.md](../sfz-conformance.md)); the SFZ file itself was not opened for this card | [Cited S14, S25]; the tag contents per free_libraries.md |
| Sonatina SO `Concert Harp.sfz` | CC Sampling Plus 1.0 | 21 regions, note names c2–c7; `loop_mode=one_shot`; `pitch_veltrack`, `cutoff`, `fil_veltrack` (velocity-to-brightness filter) | partly: the sampler has no filters, so it ignores the brightness and reports unsupported opcodes | [Cited S15, S25]; c2–c7 = 36–96 [Estimate: assumes c4 = 60; S16 warns that note names differ by an octave or two between DAWs] |
| VSL Synchron Harp, Spitfire Harp Redux | commercial EULAs | full techniques (see section 5) | no: Kontakt/VSL players need plugin hosting (M5) | [Cited S11, S12, S25] |

No free harp library found covers harmonics, bisbigliando, p.d.l.t. or recorded glissandi
[Estimate: only the three free sets above were inspected].

## guitar_classical: Classical guitar

```json
{"id": "guitar_classical", "family": "plucked", "names": {"en": "Classical guitar", "cs": "klasická kytara", "it": "chitarra classica", "de": "klassische Gitarre", "fr": "guitare classique", "score_abbreviation": "Gtr."}, "pitched": true, "transposition": -12, "clefs": ["treble"], "sounding_playable": [40, 83], "sounding_comfortable": [40, 76], "range_provenance": {"playable": "Estimate: low E2 = 40 is Cited S20 (open 6th string, 82.41 Hz); top B5 = 83 is derived from three cited facts that agree: S20 (19 frets above the open E4 first string), S5 (each string spans an octave plus a fifth) and S3 (a compass of three octaves and a fifth); no opened source names B5", "comfortable": "Estimate: E2-E5 (up to the 12th fret of the first string); no source states a comfortable range"}, "written_playable": [52, 95], "gm_program": 25, "gm_program_name": "Acoustic Guitar (nylon)", "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["sustain", "legato", "tremolo", "trill", "harmonics", "sul_ponticello"], "articulations_proposed_new": ["harmonics_natural", "harmonics_artificial", "sul_tasto", "rasgueado", "strum", "palm_mute", "bartok_pizzicato", "golpe", "vibrato_on", "let_ring", "damped", "glissando"], "articulations_provenance": "Cited S2, S3, S5, S20 (harmonics natural and artificial, shakes, reiterations / tremolo, slurred scales, rasgueado, strumming, vibrato, ponticello and tasto positions, damping); the mappings palm_mute = guitar 'pizzicato', bartok_pizzicato = 'snap', golpe = 'percussive effects' and glissando are Estimates (S5 gives only the headings)", "solo_or_section": "solo", "typical_players": {"classical": "0 (opera stage colour only)", "romantic": "0-1 (opera colour; Mahler 7, movement 4)", "film": "0-1 (often recorded separately)", "chamber": "1"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | classical guitar, guitar | [Cited S20] |
| Czech | klasická kytara | [Cited S24] |
| Italian | chitarra classica (chitarra) | [Cited S24, S2] |
| German | klassische Gitarre (Forsyth: "Guitarre", the old spelling) | [Cited S24, S2] |
| French | guitare classique (guitare) | [Cited S24, S3] |
| Score | "Gtr." / "Git."; strings numbered I (top) to VI (bottom) | abbreviation [Estimate: no source prints it]; string numbers [Cited S5] |

Family `plucked`. The id names the nylon-string classical guitar; steel-string, 12-string and electric guitars are
variants (section 8) [Estimate: id scope chosen for this card set].

### 2. Sound production

- Six strings; the right-hand thumb and fingers pluck, the left hand stops the strings on a fretted neck
  [Cited S2]. The classical guitar is usually plucked with the fingernails [Cited S5]. The thumb takes the three
  lower strings and the fingers one upper string each [Cited S3].
- Strings: nylon, or metal-wound nylon [Cited S20]; historically three silk strings wound with silver and three gut
  [Cited S3].
- *Barré*: the first finger lies across several strings "as a temporary fret" [Cited S2, S3].
- Stroke types: *apoyando* (rest stroke) and *tirando* (free stroke) [Cited S20].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

| Item | Value | Provenance |
|---|---|---|
| Standard tuning (sounding) | E2 A2 D3 G3 B3 E4 = 40 45 50 55 59 64 | all six notes [Cited S20] (E2 ≈ 82.41 Hz, E4 ≈ 329.63 Hz); tuning in fourths with a major third between the 2nd and 3rd strings [Cited S2, S3]; MIDI numbers [Estimate: conversion, C4 = 60] |
| Frets | most often 19 | [Cited S20] |
| Per string | an octave plus a fifth | [Cited S5] |
| Compass | three octaves and a fifth | [Cited S3] |
| Playable | 40–83 (E2–B5) | [Estimate: derived from S3, S5, S20] |
| Comfortable | 40–76 (E2–E5) | [Estimate: up to the 12th fret of string I] |
| Transposition | −12: treble clef, written an octave above the sound | [Cited S2, S3, S5] |
| Written playable | 52–95 (E3–B6) | [Estimate: derived] |
| Alternative tuning | a different tuning is sometimes used for pieces in E | [Cited S3]; the common drop-D (6th string to D2 = 38) [Estimate: not stated in an opened source] |
| Keys | flat keys are "incomparably more difficult" and need barré; keys whose bass can use a low open string (S3 names the keys A, E, G and F) allow chords of more than four notes | [Cited S3] |

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

- Feeble sonority: it does not combine with other instruments or with many voices [Cited S3]. "The sound of twelve
  guitars playing in unison is almost absurd" [Cited S3]. In an orchestra it is usually amplified or set against a
  thin texture [Estimate: modern practice; S5's "Balancing the guitar in an orchestra" is a video heading only].
- Melancholy, dreamy character [Cited S3].
- Agility: major and minor trills (shakes) work across the whole compass [Cited S3]. Slurred scales in pairs are
  "elegant, and tolerably sonorous" [Cited S3]. Scales in thirds, sixths and octaves are difficult at the extremes but
  usable at a moderately quick tempo [Cited S3]. Notes repeated 2–8 times are easy; long reiterations work best on
  the upper strings [Cited S3].
- Chords: at most six notes, one per string. Root-position seventh chords in stacked thirds are "next to
  impossible"; leaving out a middle string makes a chord unplayable for the thumb [Cited S3]. Broad spread chords and
  simple arpeggios suit it best [Cited S2]. Composers who do not play it often write parts that are very difficult and
  sound weak [Cited S3].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Sonara id | Provenance |
|---|---|---|
| Ordinary note, let ring | `sustain`, `let_ring` | [Cited S2] (Grainger's lines show how long strings should vibrate) |
| Slurs (hammer-on, pull-off) | `legato` | "scales bound by twos" [Cited S3]; the hammer-on/pull-off mechanism [Estimate: standard guitar technique, not described in an opened source] |
| Trill | `trill` | [Cited S3] |
| Tremolo (repeated notes / p-a-m-i) | `tremolo` | [Cited S20, S3] |
| Natural harmonics (partials 2–5) | `harmonics`, `harmonics_natural` | [Cited S2, S3] |
| Artificial harmonics | `harmonics_artificial` | left hand stops the note; right index touches the node, thumb plucks [Cited S2, S3] |
| Sul ponticello / sul tasto (hand position) | `sul_ponticello`, `sul_tasto` | [Cited S20] |
| Rasgueado | `rasgueado` (new: a flamenco strum in which the fingers flick out one after another across the strings) | [Cited S20]; S5 lists "Rasguardo" [Cited S5] |
| Strumming | `strum` (new: a chord swept by the thumb, fingers or a plectrum, without rasgueado's finger flicks) | [Cited S5]; Grainger's thumb strumming [Cited S2] |
| Vibrato | `vibrato_on` | [Cited S5] |
| "Pizzicato" (palm mute) | `palm_mute` | S5 has a "Pizzicato" heading [Cited S5]; that it means palm muting [Estimate: the page text gives only the heading] |
| Snap | `bartok_pizzicato` | S5 has a "Snap" heading [Cited S5]; the mapping [Estimate: the page text gives only the heading] |
| Percussive effects (tambora, golpe) | `golpe` (new: knocking on the soundboard or striking the strings near the bridge) | S5 has a "Percussive effects" heading [Cited S5]; the technique names [Estimate: the page text gives only the heading] |
| Damping with cloth or mutes | `damped` | [Cited S5] |
| Slide, bottleneck, nails along the string | `glissando` | [Cited S5] (bottlenecks, nails); the id choice [Estimate: no glissando-specific id exists for slides] |

Libraries: no orchestral guitar sample manual was opened. Commercial guitar libraries usually switch articulations
with key switches, separate patches and release noises [Estimate: not verified for any product]. Sonara's sampler
can play key switches and release samples [Cited S25].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One player per part [Cited S3]: guitars lose by being multiplied.
- Exception: Grainger's guitar-and-mandolin band, about forty players, split into sections that are each tuned to a
  chord [Cited S2].

### 7. Role in the orchestra and typical doublings

- "Can scarcely be regarded as a regular orchestral instrument" [Cited S2]. It gives Spanish "local colour" in opera:
  Rossini, *Il barbiere*, Act I; Weber, *Oberon*, Act I [Cited S2].
- Mahler, Symphony No. 7: guitar in the 4th movement only (Nachtmusik II), with the mandolin [Cited S23].
- Concertos: Rodrigo, *Concierto de Aranjuez*, and *Fantasía para un gentilhombre* [Cited S20].
- The guitar is the most common fretted instrument in the orchestra [Cited S5].
- Film: solo colour, often recorded apart from the orchestra [Estimate: general practice, no source opened].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Facts | Provenance |
|---|---|---|
| Steel-string acoustic | GM1 program 26 | [Cited S17] |
| 12-string | same tuning; the lowest four courses doubled in octaves, the top two in unison | [Cited S5] |
| Electric guitar | GM1 programs 27–31 | [Cited S17]; S5 has an electric-guitar heading |
| Lute, mandolin, banjo, ukulele | other plucked fretted instruments; mandolin, banjo and ukulele have been used in orchestras | [Cited S5] |

A guitarist may double on mandolin [Cited S3: a few days' study is enough for a guitarist to play the Don Giovanni
mandolin part].

### 9. General MIDI and existing Sonara metadata

- GM1 program **25 "Acoustic Guitar (nylon)"** [Cited S17]; 32 "Guitar Harmonics" [Cited S17]. GM2 variations of
  program 25: bank 1 "Ukulele", bank 2 "Opened Nylon-Strings Guitar", bank 3 "Nylon-Strings Guitar 2" [Cited S18].
- Not in `data.rs` [Cited S25]; no verification table.

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

- VSCO 2 CE (S13) and VCSL (S14) have **no** guitar folder or SFZ [Cited S13, S14]. Sonatina's instrument list has no
  guitar [Cited S15].
- The GM sound fonts in Sonara's research (FluidR3_GM, MuseScore_General: MIT) contain GM program 25, but they are
  SF2, which the Sonara sampler does not play [Cited S25].
- No free classical-guitar SFZ was found or verified in this session [Estimate: no other library was searched].

## lute: Lute

```json
{"id": "lute", "family": "plucked", "names": {"en": "Lute", "cs": "loutna", "it": "liuto", "de": "Laute", "fr": "luth", "score_abbreviation": "Lt."}, "pitched": true, "transposition": -12, "clefs": ["treble"], "sounding_playable": [36, 79], "sounding_comfortable": [43, 74], "range_provenance": {"playable": "Estimate: a Renaissance lute nominally in G with up to 10 courses; low C2 = 36 assumes a 10-course bass; top G5 = 79 is the open G4 treble course plus one octave, from S22's statement that some wooden frets glued to the body (which some consider anachronistic) let the highest courses be stopped up to a full octave above the open string. The octave of the open strings is not stated in S22", "comfortable": "Estimate: G2-D5 = 43-74, the six main courses (S22: 6-course tenor lute in G) and the lower part of the fingerboard"}, "written_playable": [48, 91], "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["sustain", "trill"], "articulations_proposed_new": ["let_ring"], "articulations_provenance": "Estimate: no lute technique source was opened; ornaments (trill) and ringing are general early-music practice", "solo_or_section": "solo", "typical_players": {"classical": "0", "romantic": "0", "film": "0-1 (period and fantasy scores)", "chamber": "1 (Baroque continuo)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | lute | [Cited S22] |
| Czech | loutna | [Cited S24] |
| Italian | liuto | [Cited S24] |
| German | Laute | [Cited S24] |
| French | luth | [Cited S24] |
| Score | "Lute" in full, or "Lt." | [Estimate: no source prints an abbreviation] |

Family `plucked`. The card describes the Renaissance lute; archlute and theorbo are variants [Estimate: id scope].

### 2. Sound production

- Strings arranged in courses: medieval lutes had 4–5, the late Renaissance up to 10, the Baroque 14 (sometimes 19)
  [Cited S22]. In the lower courses one string of a pair is tuned an octave higher [Cited S22].
- Gut strings; frets are loops of gut tied round the neck [Cited S22]. Around 1500 lutenists gave up the quill and
  plucked with the fingers [Cited S22].
- The lute family was historically important until the 17th century [Cited S2].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

| Item | Value | Provenance |
|---|---|---|
| 6-course tenor lute "in G" | the pattern "(G'G) (Cc) (FF) (AA) (dd) (g)", intervals of a fourth except a third between courses 3 and 4 | [Cited S22]; no fixed pitch standard before the 20th century [Cited S22] |
| Converted | G2 C3 F3 A3 D4 G4 = 43 48 53 57 62 67 | [Estimate: S22's letters are not strict Helmholtz (that would put the top at G3); the guitar-like intervals and a G4 top course are the usual modern reading] |
| Top | about an octave above the open top course: G5 = 79 | [Estimate: from S22's statement that wooden frets glued to the body, "considered anachronistic" by some, let the highest courses be stopped up to an octave above the open string; with tied gut frets alone the top is lower] |
| Extra bass courses | 7–10 course lutes reach about D2 or C2 (38 or 36) | [Estimate: not in an opened source] |
| Recommended | playable 36–79 (C2–G5); comfortable 43–74 (G2–D5: the six main courses and the lower fingerboard); written 48–91 at −12 | [Estimate: built from the rows above, see the JSON `range_provenance`] |
| Notation | tablature (Italian, Spanish, German, French); the tablature depends on the instrument | [Cited S22, S2] |
| Transposition | −12 if written on a treble staff like a guitar; 0 in at-pitch transcriptions; tablature has no pitch | [Estimate: no opened source states it; open question] |

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

Quiet: softer than the guitar, needs close miking in film and an exposed texture in the orchestra
[Estimate: no source opened]. Chords and polyphony, as on the guitar, are limited by the hand stretch and by which
courses are open [Estimate: by analogy with the guitar (S3); no lute source opened].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Sonara id | Provenance |
|---|---|---|
| Ordinary note, ringing | `sustain`, `let_ring` | [Estimate: general plucked-string practice; no lute technique source opened] |
| Ornaments (trills, mordents) | `trill` | [Estimate: general early-music practice] |

No lute sample-library manual was opened [Estimate: none searched].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

One player; the lute is a solo, song-accompaniment or continuo instrument [Cited S22 (continuo)];
`solo` [Estimate: summary].

### 7. Role in the orchestra and typical doublings

- Baroque: one of the instruments of the basso continuo; later replaced by keyboards [Cited S22].
- Modern orchestral use is mostly indirect: Respighi's *Ancient Airs and Dances* draw on lute music [Cited S22].
- Film: Jozef van Wissem wrote the music for Jarmusch's *Only Lovers Left Alive* [Cited S22]. Period films use lute
  colour in place of the guitar [Estimate: general film practice, no source opened].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Facts | Provenance |
|---|---|---|
| Theorbo | the double-necked bass lutes of the 16th–17th centuries; 12, 14 or 16 strings in unison pairs plus 8–11 open bass strings | [Cited S2] |
| Archlute; chitarrone ("Roman theorbo") | the largest theorbos; the chitarrone "stood as high as a tall man" | [Cited S2] |
| Archlute, theorbo, torban | long extensions for open bass strings outside the fingerboard | [Cited S22] |
| Baroque lute | the first six courses outline a D minor triad | [Cited S22] |

Lutenists commonly double on theorbo and archlute, and sometimes on Baroque guitar [Estimate: not sourced].

### 9. General MIDI and existing Sonara metadata

- GM1 has **no** lute program [Cited S17]. GM2 has no lute variation either [Cited S18]. Fallback: program 25
  "Acoustic Guitar (nylon)" [Estimate: closest plucked timbre].
- Not in `data.rs` [Cited S25].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

VCSL has no "Lutes" folder (its Chordophones are composite chordophones and zithers) [Cited S14]. VSCO 2 CE and
Sonatina have no lute [Cited S13, S15]. No free lute library was verified [Estimate: no wider search].

## mandolin: Mandolin

```json
{"id": "mandolin", "family": "plucked", "names": {"en": "Mandolin", "cs": "mandolína", "it": "mandolino", "de": "Mandoline", "fr": "mandoline", "score_abbreviation": "Mand."}, "pitched": true, "transposition": 0, "clefs": ["treble"], "sounding_playable": [55, 91], "sounding_comfortable": [55, 88], "range_provenance": {"playable": "Estimate: low G3 = 55 is Cited S21 (tuning G3 D4 A4 E5) and S2 (low G, 4th string); the top G6 = 91 is derived from S3 (a compass of about three octaves) above G3", "comfortable": "Estimate: derived from S2, a working compass from the low G of about two octaves and a major sixth (G3-E6 = 55-88)"}, "written_playable": [55, 91], "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["sustain", "staccato", "tremolo"], "articulations_proposed_new": ["strum", "let_ring"], "articulations_provenance": "Cited S2, S3, S21 (single staccato strokes, plectrum tremolo as the sustained note, four-course chords swept with the plectrum); let_ring is an Estimate", "solo_or_section": "solo", "typical_players": {"classical": "0-1 (Mozart, Don Giovanni)", "romantic": "0-1 (Verdi Otello; Mahler 7, 8, Das Lied von der Erde)", "film": "0-1", "chamber": "1"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | mandolin (older: mandoline) | [Cited S21, S2] |
| Czech | mandolína | [Cited S24] |
| Italian | mandolino | [Cited S2, S24] |
| German | Mandoline | [Cited S2, S24] |
| French | mandoline | [Cited S2, S24] |
| Score | "Mand." | [Estimate: no source prints it] |

Family `plucked`; the Neapolitan mandolin [Cited S2].

### 2. Sound production

- Small, pear-shaped, with a fretted neck; eight wire strings in four pairs tuned in unison [Cited S2, S5, S21].
- Played with a plectrum (tortoiseshell, bone or ivory in 1914) [Cited S2]; the plectrum strikes the two strings of a
  pair alternately [Cited S21]. Historic stringing: E gut, A steel, D copper, G silver-wound gut [Cited S3].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

| Item | Value | Provenance |
|---|---|---|
| Tuning | G3 D4 A4 E5 (55 62 69 76), like the violin | [Cited S21, S2, S3] |
| Working compass | from the low G, about two octaves and a major sixth: G3–E6 = 55–88 | words [Cited S2]; numbers [Estimate: derived] |
| Compass | "about three octaves", with all chromatic notes: G3–G6 = 55–91 | words [Cited S3]; numbers [Estimate: derived] |
| Recommended | playable 55–91, comfortable 55–88 | [Estimate: the wider figure as the outer limit] |
| Transposition | 0: treble clef at sounding pitch | [Cited S2, S3] |

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

- Tone "thin and nasal" but "piquant and original" [Cited S3]; "poor tinkley tone-quality" [Cited S2]. Soft: an
  exposed or lightly scored setting is needed [Estimate: inferred from S2 and S3 on its weak tone].
- Fingering as on the violin [Cited S2]. More a melodic than a harmonic instrument; four-note chords made by sweeping
  the plectrum over the four courses sound rather poor [Cited S3].
- No sustain beyond the pluck except through tremolo [Cited S2, S21].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Sonara id | Provenance |
|---|---|---|
| Single plectrum strokes | `staccato` (ordinary short notes), `sustain` | [Cited S2] ("single staccato notes") |
| Tremolo (the reiterated *tenuto*; the chief characteristic) | `tremolo` | [Cited S2, S21] |
| Chords swept with the plectrum | `strum` (defined in the guitar card) | [Cited S3] |
| Let ring | `let_ring` | [Estimate: general plucked-string practice, no mandolin source states it] |

No mandolin sample-library manual was opened. In libraries, tremolo is normally a separate patch or key switch,
because it is a sustained articulation [Estimate: not verified].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

One player in the orchestra [Estimate: S23 lists Mahler 7's "mandolin" in the singular, with no number given
(Cited S23); no opened source states a player count]. Grainger's guitar-and-mandolin band is an
exception [Cited S2]. Mandolin orchestras exist outside the symphony orchestra [Estimate: general knowledge, no source opened].

### 7. Role in the orchestra and typical doublings

- Opera: Mozart, *Don Giovanni*, "Deh vieni alla finestra" [Cited S2, S3, S21]; Grétry, *L'amant jaloux*
  [Cited S2]; Verdi, *Otello* (a guzla part usually played on the mandolin) [Cited S2, S21].
- In practice the Don Giovanni part is often played by first violins pizzicato or by guitars, which Berlioz
  condemns [Cited S2, S3].
- Mahler: Symphony No. 7 (4th movement only, with the guitar), No. 8, *Das Lied von der Erde* [Cited S21, S23].
  Also Schoenberg's Variations Op. 31, Stravinsky's *Agon*, Prokofiev's *Romeo and Juliet*, Webern Op. 10 and
  Respighi's *Feste romane* [Cited S21].
- Film: Italian or Mediterranean colour and serenades [Estimate: general practice, no film source opened].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Facts | Provenance |
|---|---|---|
| Mandola | the modern Italian mandola is a large tenor mandolin | [Cited S2] |
| Mandola, octave mandolin, mandocello | members of the mandolin family | [Cited S21] (names only) |
| Guitar, violin | a guitarist or violinist can learn the Don Giovanni part in a few days | [Cited S3] |

### 9. General MIDI and existing Sonara metadata

- GM1 has **no** mandolin program [Cited S17]. GM2: program 26, bank 2 "Mandolin" (a variation of "Steel-Strings
  Guitar") [Cited S18]. GM1 fallback: program 26 "Acoustic Guitar (steel)" [Estimate: what a GM2 device falls back
  to when the variation is missing].
- Not in `data.rs` [Cited S25].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

VSCO 2 CE, VCSL and Sonatina contain no mandolin [Cited S13, S14, S15]. No free mandolin SFZ was verified
[Estimate: no wider search].

## Open questions and disagreements between sources

1. **Harp low limit, 23 or 24.** S1 and S8 give C♭1 (23) as the bottom of the range. S4 and S10 give the string
   C1. S8 also says the lowest C string has no pedal and is normally tuned C♮, so C♭1 needs retuning before the
   piece. Recommendation: playable 23, and an agent warns that 23 needs the lowest string pre-tuned. Rejected
   alternative: 24, which would flag C♭1, a pitch both handbooks list.
2. **Harp high limit, 102, 103 or 104.** S1: F♯7 (102; probably an older 46-string harp, an Estimate inferred from that top note). S8: G♭7 in the all-flat
   frame, while also saying the top G has no pedal and is normally G♮, and that it can be tuned sharp or flat before
   the performance ("If necessary, tune high G to G♯"). S4: G7 (103). S2: a top note that can be mechanically raised
   to G♮ or G♯ (104). Recommendation: 103, the top string in its normal tuning. 104 needs the top string retuned
   before the piece or a top-G mechanism, which is the same kind of condition as C♭1 = 23 at the bottom. The
   asymmetry (23 in, 104 out) follows the ranges the sources list (S1 and S8 list C♭1; none lists G♯7 as the end of
   the range). Fact-check note: the earlier wording "104 only with a top-G mechanism" contradicted S8 and was
   corrected; whether to admit 104 is a schema decision.
3. **Harmonics range and note-name conventions** (section 5 of the harp card). S12's "G2 to C6" and S11's
   key-switch names do not state the middle-C convention, and S4's labels do not say written or sounding.
   Recommendation: sounding 55–96. To confirm, read a dated handbook page as text, or check the sample keys of an
   installed library.
4. **Harp chord span per hand.** S3: within an octave, a tenth with a great stretch. S2: up to and beyond a tenth is
   normal. S6: at most an octave and a fourth. Recommendation: comfortable within an octave, maximum a tenth.
5. **Harmonics per hand.** S1: a chord of harmonics has at most three notes (two in the left hand, one in the right).
   S2 (Forsyth, quoting Widor's *The Technique of the Modern Orchestra*): the left hand two to four. S3: two or three.
   S6: up to three in the left hand. Recommendation: left hand ≤ 3 close notes, right hand 1.
6. **Rimsky-Korsakov's "first to the fourth octave".** S1 says that of the whole harp range "only the notes of the
   first to the fourth octave are used", but the octave counting is not defined in the translation. It is not used
   for the comfortable range.
7. **Pedal-change time** has no numeric source (S2 is qualitative; S6 and S7 give counts, not durations). Sonara's
   check needs a number in beats. Ask a harpist, or find a dated handbook that gives one.
8. **Mandolin top note.** S2: two octaves and a major sixth (E6 = 88); S3: about three octaves (G6 = 91). Modern
   instruments with more frets probably go higher; no modern source states it.
9. **Lute transposition and pitch.** No opened source states whether lute parts in staff notation are written an
   octave high (−12, guitar convention) or at pitch (0). The octave of S22's tuning letters is also unresolved.
   The −12, 36–79 and comfortable 43–74 values are Estimates.
10. **GM for lute and mandolin.** GM1 has neither. The JSON stores `null`; the suggested fallbacks (25 for lute,
    26 for mandolin) are Estimates. Sonara must decide whether the schema should hold a fallback program.
11. **Guitar technique names.** S5 lists "Pizzicato", "Snap" and "Percussive effects" only as video headings, so the
    mappings to `palm_mute`, `bartok_pizzicato` and `golpe` are Estimates. `rasgueado`, `strum` and `golpe`
    are new ids outside the preferred list and need the vocabulary owner's approval. The harp's pedal glissando is
    mapped to the preferred `portamento` (one string, continuous pitch change), not to `glissando` (a sweep across
    strings).
12. **`sustain` for plucked instruments.** These cards map the ordinary decaying pluck to `sustain` so that library
    default patches match. The alternative is a new id such as `ordinario`; this is a vocabulary decision.
13. **Harp-maker octave numbering** ("0th octave G to 7th octave C", S10) is not defined on any opened page. It
    must never be parsed as scientific octaves.
14. **No free library** covers harp techniques beyond the ordinary pluck, or any guitar, lute or mandolin in SFZ
    (among VSCO 2 CE, VCSL and Sonatina). VCSL's harp has two dynamic layers per pitch (f plus mf; mp or p at the
    extremes; E1 has f only), four markings in all; its SFZ files exist only in the tag `v1.2.2-RC`, not on the master branch read for this card.

## Fact-check log

_Adversarial fact-check, 2026-10-30. Every source below was reopened in this session: S1, S2 and S3 as full OCR or
plain text, downloaded and searched; S12 as the PDF, downloaded and extracted with PDFKit; S13 and S15 as the raw
SFZ files; S14 as the GitHub directory listing (the GitHub API was rate-limited, the HTML listing worked); the rest
with a web fetch. S26 (VSL Academy, HTTP 502) and S27 (VSL product page, HTTP 404) still fail, so they stay uncited._

**Structure.** All four JSON blocks parse, carry exactly the 20 required keys in order (with the 6 `names` keys,
2 `range_provenance` keys and 4 `typical_players` keys), use the assigned ids (`harp`, `guitar_classical`, `lute`,
`mandolin`) and the valid family slug `plucked`. `written_playable` = `sounding_playable` − `transposition` holds
for all four. Every `articulations_existing_vocab` entry is one of the 15 ids. Every `articulations_proposed_new`
entry is a preferred spelling except `rasgueado`, `strum` and `golpe`, which are defined in one line in the guitar
card's section 5 and flagged in Open question 11. That meets the rule, so they were not renamed.

**data.rs.** `crates/sonara-midi/src/orchestration/data.rs` holds the 16 listed ids and none of the four ids here.
`docs/orchestration-metadata.md` lists harp as "Not covered". So the cards are right to have no "Verification of
data.rs" table. `Family::Plucked` ("plucked") exists in `instrument.rs`.

**Confirmed numbers (the source states them; conversions redone with C4 = 60).**

| Instrument | Item | Checked against | Result |
|---|---|---|---|
| harp | low C♭1 = 23 | S1 "[Music: C1[flat]-F7[sharp]]" with the transcriber's C4 = middle C; S8 "C♭1 to G♭7", "the C♭ three octaves below middle C" | confirmed |
| harp | top G7 = 103 | S4 "The range is from C1 - G7"; S8: the top G has no pedal and is normally G♮ | confirmed |
| harp | S1 top F♯7 = 102; S8 top G♭7 = 102 | S1, S8 | confirmed |
| harp | 47 strings, 7 pedals, D C B / E F G A, notches, 9 homophones, D/G/A without homophone | S2 (pp. 464–466), S4, S6, S7, S8 | confirmed |
| harp | transposition 0; two bracketed staves, treble and bass clefs at discretion | S2 | confirmed |
| harp | GM1 47 "Orchestral Harp"; GM2 47 bank 0 "Harp", bank 1 "Yangqin" | S17, S18 | confirmed |
| guitar | E2 A2 D3 G3 B3 E4 = 40 45 50 55 59 64; E2 ≈ 82.41 Hz | S20 | confirmed (the middle notes were upgraded from Estimate to Cited) |
| guitar | 19 frets → E4 + 19 = B5 = 83; S5 "one octave plus a fifth" per string (19 semitones); S3 "three octaves and a fifth" from E2 → 83 | S20, S5, S3 | three derivations agree; 40–83 stays an Estimate, as the card says |
| guitar | −12; written 52–95 | S2, S3, S5 ("All guitars sound an octave lower than written") | confirmed |
| guitar | GM1 25 "Acoustic Guitar (nylon)", 26 steel, 27–31 electric, 32 "Guitar Harmonics"; GM2 25 banks 1–3 | S17, S18 | confirmed |
| lute | S22 pattern "(G'G) (Cc) (FF) (AA) (dd) (g)", capitalisation exactly as quoted | S22 | confirmed; the conversion to 43–67 stays an Estimate |
| lute | no GM1 or GM2 lute | S17, S18 | confirmed |
| mandolin | G3 D4 A4 E5 = 55 62 69 76 | S21 ("in scientific pitch notation G3–D4–A4–E5"), S2, S3 | confirmed |
| mandolin | S2 working compass "about two octaves and a major sixth" from low G → E6 = 88; S3 "about three octaves ... with the chromatic intervals" → G6 = 91; sounding pitch in treble clef | S2, S3 | words confirmed; MIDI values are derived Estimates, as stated |
| mandolin | GM2 26 bank 2 "Mandolin"; no GM1 mandolin | S17, S18 | confirmed |
| harp libraries | VSCO `Harp.sfz`: 23 regions, keys 28–101, `lovel=0`/`hivel=127` throughout, `ampeg_release=12`, CC0; SSO: 21 regions, `lokey=c2` … `hikey=c7`, `loop_mode=one_shot`, `pitch_veltrack`, `cutoff`, `fil_veltrack`, CC Sampling Plus 1.0; VCSL: 45 WAVs, 23 pitches E1–F7, CC0 | S13, S14, S15 | confirmed |

**Spot-checked Cited claims (all confirmed).** S12: 4,041 samples; Normale with CC64 down = laissez vibrer and key
release with the pedal up = sons étouffés; Flageolet "from G2 to C6"; Bisbigliando on CC1; expression CC11; the
Glissandi Legato interval-to-scale table (minor 2nd whole tone ... dim. 5th diminished 7). S11: Pedal mode on the
white keys; default key switches A0/A♯0, F♯1–G♯2, C♯1/D♯1, F♯5–C♯6, F♯6–C♯7; Regular, Harmonics and Bisbigliando
with ringing or damped release; Glissando Smoother; velocity from a MIDI CC; no middle-C convention stated. S7: at
most 4 consecutive semitones, interval class 4, two to four pedal changes at a time. S6: four notes per hand, span
of at most an octave and a fourth, pedal changes per modulation (including a minor third up = three), glissandi
with one or both hands in parallel or contrary motion. S4: pedal glissando "inaudible above C6"; the dominant 7th
roots G♭, A♭, B, D♭, E and dominant 9th roots C, D, E♭, F, G, A, B♭; the extended effects (palm, nails, paper).
S2: Shakes and tremolos played hand over hand, pp only, bisbigliando (Strauss, *Sinfonia Domestica*); the harmonics
rules and the Widor quotation; sons étouffés, laissez vibrer, sons près de la table; "at least a couple of Harps";
glissando "in one second", "four or five octaves"; the 16 writing rules; the Grainger guitar band of forty guitars
and mandolines. S3: chord span, hand distance, arpeggio extent, the register character quotes, the harp shake "only
tolerable on the high notes", the guitar's compass and fingering, "twelve guitars", the mandolin stringing, and the
Don Giovanni substitution. S23: 2 harps; guitar and mandolin in movement 4 only. S24: all 16 foreign names.

**Changed (was → now, why).**

1. Harp JSON `range_provenance.playable` and section 3: "G♯7 = 104 is possible only on harps with a top-G
   mechanism" → 104 can be had by retuning the unpedalled top G before the piece (S8: "If necessary, tune high G to
   G♯") or with a mechanism (S2, S8). The old wording contradicted S8. `sounding_playable` stays [23, 103]; the
   23-in, 104-out asymmetry is now stated in Open question 2.
2. Harp section 7: "Celesta or piano with harp imitate the guzli [Cited S1]" → the piano, alone or with the harp,
   imitates the guzli. S1 mentions the celesta only as superseding the piano.
3. Harp section 3, sharp keys: "the strings sound longer" → S2's own words, "a greater vibrating-length for the
   strings" (a paraphrase that drifted).
4. Guitar section 3: "S3 names A, E and G" → S3 names the keys A, E, G and F.
5. Lute JSON `range_provenance.playable` and section 3 "Top" row: S22's octave-above-open statement is about extra
   wooden frets glued to the body, which some consider anachronistic. The wording now says so.
6. Open question 14 and the harp's section 10: "VCSL's harp has four dynamics" → two layers per pitch (f plus one
   softer marking), four markings in all.
7. Harp section 5, harmonics: added a caution that the recommended 25-string window is wider than S2's 17 strings.
8. Upgrades from Estimate to Cited, where the source states the fact: S8's octave designations (anchored to middle
   C); the guitar's middle open strings (S20 lists all six).

**Downgraded or narrowed citations.**

1. Harp section 2, "thumb and first three fingers; little finger not used": S4 removed from the citation. None of
   the opened S4 pages (harps, harp_player, harp_construction, harp_techniques) mentions fingering; S2 and S8 do.
2. Harp section 3, "47 diatonic strings, C1 to G7 [Cited S2, S4, S8, S10]": the C1–G7 span is now cited to S4
   only. S2's compass is an image, S8 uses the flat frame, and S10 uses maker octave numbers.
3. Harp section 2, "C-flat major scale [Cited S6, S8, S12]": S6 does not state the base tuning, so S2 and S8 now
   carry it, and S6 and S12 carry only the pedal positions.
4. Mandolin section 6, "[Cited S23 (Mahler 7: one mandolin)]" → Estimate. S23 lists "mandolin" with no number.

**Remaining concerns.**

- Harp top note 103 against 104 is a schema decision, not a source conflict (see Open question 2). Modern
  orchestration textbooks may give C♭1–G♯7 [Estimate: from memory, none was opened]. The VSL Academy range page is
  down (502).
- The harp harmonics range rests on vendor note names with no stated convention (S12) and unlabelled clips (S4). S2's
  17-string window is in an image.
- No source gives a pedal-change time in beats or seconds.
- The lute's octave (S22's letters are not strict Helmholtz) and its notation transposition (−12 against 0) are
  unresolved Estimates. So are the mandolin top (91) and the harp's comfortable range (36–96).
- S2 prints the harp's "most sonorous part of the instrument" as an image (rule 6, p. 474). If someone reads it from
  a scan, it would give the comfortable range a handbook source.

## Consistency review

_Consistency review, 2026-10-30: json against sections 3 and 9 and the Fact-check log; written = sounding minus
transposition; tag hygiene; quote length; paths._

- Lute section 3: added a "Recommended" row (playable 36–79, comfortable 43–74, written 48–91, Estimate). Before, the
  json's comfortable range (top D5 = 74) and written range appeared nowhere in the prose.
- Open question 9: the lute's comfortable range 43–74 is now named as an Estimate too.
- Fact-check log header: "S26 and S27 (VSL Academy) still return HTTP 502" corrected to match the Sources table
  (S26 VSL Academy, HTTP 502; S27 VSL product page, HTTP 404).
- Sources row S25: the absolute local checkout path was replaced by "paths relative to the repository root".
- Open question 5: "S2 (Widor)" now reads "S2 (Forsyth, quoting Widor's *The Technique of the Modern Orchestra*)".
  S2 is Forsyth's *Orchestration*; its harp chapter quotes the two-to-four left-hand harmonics rule and footnotes
  it to Widor, whose book is not a source of this card. Checked in the S2 OCR text; section 5 already said "S2
  (quoting Widor)".
- VCSL harp (section 10 row and Open question 14): the SFZ files `Concert Harp.sfz` and `Folk Harp.sfz` exist in the tag `v1.2.2-RC`; the row now says so, with the velocity-crossfade caveat (Q6) from the SFZ conformance record.
