# Bowed strings: instrument cards
_Researched 2026-10-30. MIDI note numbers, C4 = 60 = middle C. Ranges are sounding unless marked written.
Transposition = sounding minus written, in semitones._
## Sources
| # | Source | URL | Kind (handbook, manufacturer, library maker, spec, encyclopedia, other) | Opened in this session |
|---|---|---|---|---|
| S1 | Rimsky-Korsakov, _Principles of Orchestration_ (1922 English translation), Project Gutenberg eBook 33900: plain text, HTML edition and its notated examples `images/ms001.png`-`ms006.png`, `table_a.png` (the transcriber's note says the eBook uses octave numbering with middle C = C4) | https://www.gutenberg.org/ebooks/33900 (text: https://www.gutenberg.org/cache/epub/33900/pg33900.txt; HTML: https://www.gutenberg.org/files/33900/33900-h/33900-h.htm) | handbook | yes |
| S2 | Cecil Forsyth, _Orchestration_ (Macmillan / Stainer & Bell, 1914), Internet Archive item `orchestration00fors`: OCR text and page scans pp. 306, 383, 415, 439, 440, 442, 443, 451 (notated pitches read from the scans); fact-check re-read pp. 306, 383, 415, 429, 439, 443, 451 as page images (`https://archive.org/download/orchestration00fors/page/<page>.jpg`) and the OCR text (`orchestration00fors_djvu.txt`) | https://archive.org/details/orchestration00fors | handbook | yes |
| S3 | Timbre and Orchestration Resource (McGill ACTOR project): figure captions from Widor, _Technique of the Modern Orchestra_ (1906), cello chapter | https://timbreandorchestration.org/treatises/figures/widor | handbook (captioned figures) | yes |
| S4 | Vienna Symphonic Library Academy: Violin (page text read through the site's own content endpoint `https://www.vsl.co.at/vcms/pages/academy/strings/violin`) | https://www.vsl.co.at/academy/strings/violin | library maker | yes |
| S5 | Vienna Symphonic Library Academy: Viola (same endpoint, `/viola`) | https://www.vsl.co.at/academy/strings/viola | library maker | yes |
| S6 | Vienna Symphonic Library Academy: Cello (same endpoint, `/cello`) | https://www.vsl.co.at/academy/strings/cello | library maker | yes |
| S7 | Vienna Symphonic Library Academy: Double bass (same endpoint, `/double-bass`) | https://www.vsl.co.at/academy/strings/double-bass | library maker | yes |
| S8 | Wikipedia: Violin (wikitext read) | https://en.wikipedia.org/wiki/Violin | encyclopedia | yes |
| S9 | Wikipedia: Viola | https://en.wikipedia.org/wiki/Viola | encyclopedia | yes |
| S10 | Wikipedia: Cello | https://en.wikipedia.org/wiki/Cello | encyclopedia | yes |
| S11 | Wikipedia: Double bass | https://en.wikipedia.org/wiki/Double_bass | encyclopedia | yes |
| S12 | Wikipedia: String section | https://en.wikipedia.org/wiki/String_section | encyclopedia | yes |
| S13 | MIDI Manufacturers Association, _General MIDI System Level 1_ (RP-003, 1991/1994), PDF linked from the MIDI Association's GM1 page | https://midi.org/general-midi-level-1 (PDF: https://drive.google.com/file/d/1TTM2HI2JAp_XiTJYEJ8gPNvuJpIjJiTP/view) | spec | yes |
| S14 | Spitfire Audio, _Spitfire Symphony Orchestra User Manual_ v5 (PDF) | https://www.spitfireaudio.com/cdn/shop/files/Spitfire_Symphony_Orchestra_Manual_v5.pdf | library maker | yes |
| S15 | Sonatina Symphonic Orchestra (peastman/sso): README, repository tree, `Strings - Notation/1st Violins Legato.sfz`, `includes/1st-violins-sustain.sfz` | https://github.com/peastman/sso | library maker | yes |
| S16 | VSCO 2 Community Edition, `SFZ` branch file list and the string `.sfz` files (`ViolinEns-KS`, `SViolin-KS`, `ViolaEns-KS`, `CelloEns-KS`, `Contrabass-KS` and single-articulation files), plus `LICENSE` (CC0 1.0) | https://github.com/sgossner/VSCO-2-CE/tree/SFZ | library maker | yes |
| S17 | Apple, _Logic Pro User Guide for iPad_: "Studio Strings keyswitch mapping" | https://support.apple.com/guide/logicpro-ipad/lpipe5675ccd/ipados | manufacturer | yes |
| S18 | sfzformat.com: `octave_offset` opcode (default note-name convention) | https://sfzformat.com/opcodes/octave_offset/ | spec | yes |
| S19 | Czech Wikipedia: Housle | https://cs.wikipedia.org/wiki/Housle | encyclopedia | yes |
| S20 | Czech Wikipedia: Viola | https://cs.wikipedia.org/wiki/Viola | encyclopedia | yes |
| S21 | Czech Wikipedia: Violoncello | https://cs.wikipedia.org/wiki/Violoncello | encyclopedia | yes |
| S22 | Czech Wikipedia: Kontrabas | https://cs.wikipedia.org/wiki/Kontrabas | encyclopedia | yes |
| S23 | Sonara repository (read only): `crates/sonara-midi/src/orchestration/data.rs`, `articulation.rs`, `docs/orchestration-metadata.md`, `docs/sample-libraries.md`, `docs/orchestral-audio-engine-research.md` §5, `crates/sonara-media/src/sfz.rs` | (read-only checkout of the Sonara repository; paths relative to its root) | other | yes |

## violin: Violin

```json
{"id": "violin", "family": "strings", "names": {"en": "Violin", "cs": "housle", "it": "violino", "de": "Violine (Geige)", "fr": "violon", "score_abbreviation": "Vn."}, "pitched": true, "transposition": 0, "clefs": ["treble"], "sounding_playable": [55, 105], "sounding_comfortable": [55, 93], "range_provenance": {"playable": "Cited S4 (VSL: range G3-A7, harmonics to D8); S8 gives G3 as lowest and up to C8 by stopping at the fingerboard end and/or artificial harmonics", "comfortable": "Cited S2 (p. 306: simplify passages above top A = A6, read from the scan) and S1 (practical extreme limit A6, read from ms001.png; the transcription prints A7)"}, "written_playable": [55, 105], "gm_program": 41, "gm_program_name": "Violin", "gm_percussion_notes": [], "existing_sonara_id": "violin", "articulations_existing_vocab": ["sustain", "legato", "staccato", "spiccato", "tenuto", "marcato", "pizzicato", "tremolo", "trill", "col_legno", "harmonics", "sul_ponticello", "con_sordino"], "articulations_proposed_new": ["detache", "martele", "sautille", "ricochet", "loure", "portato", "sul_tasto", "flautando", "bartok_pizzicato", "left_hand_pizzicato", "col_legno_battuto", "col_legno_tratto", "tremolo_measured", "fingered_tremolo", "harmonics_natural", "harmonics_artificial", "trill_half", "trill_whole", "portamento", "glissando", "fortepiano", "crescendo_swell", "non_vibrato", "vibrato_on", "su_una_corda", "behind_the_bridge"], "articulations_provenance": "Cited S4 (VSL technique and notation lists), S14 (Spitfire patch list incl. Marcato Attack, Legato), S15 (Sonatina articulations), S17 (Logic Studio Strings keyswitches)", "solo_or_section": "both", "typical_players": {"classical": "Estimate: about 6-10 per violin part in a Classical-era orchestra; no source read states Haydn/Mozart section sizes", "romantic": "16 Vn I + 14 Vn II (full orchestra) [Cited S1]; Wagner, Die Walkure: 16 + 16 [Cited S12]", "film": "16 Vn I + 14 Vn II as recorded for Spitfire Symphony Orchestra [Cited S14]", "chamber": "1 per part; string quartet = two violins, viola, cello [Cited S5, S8]"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Source |
|---|---|---|
| English | violin (also "fiddle") | [Cited S2] (heading "The Violin (or Fiddle)"), [Cited S8] |
| Czech | housle (plural noun) | [Cited S19] |
| Italian | violino | [Cited S2], [Cited S4] |
| German | Violine (or Geige) | [Cited S2], [Cited S4] |
| French | violon | [Cited S2], [Cited S4] |
| Score | "1st Violins (or Vn)", "2nd Violins (or Vn)"; Rimsky-Korsakov writes "Vns I", "Vns II" | [Cited S14], [Cited S1] |

- Family: strings; the soprano member of the violin family (violin, viola, cello) [Cited S4]. In the score the
  violins are two separate staves, Violin I and Violin II; both are the same instrument type [Cited S4, S14]
  (Spitfire: "Exactly the same instruments as the 1sts").
- Sonara modelling: `violin` is the instrument type; Violin I and Violin II are parts (staves) of it, not ids
  [Estimate: follows the id convention of these cards and of S23 `docs/orchestration-metadata.md`].

### 2. Sound production
- Bowed with horsehair coated in rosin; friction makes the string stick and slip; bow speed and pressure set
  loudness, too little pressure gives a weak fundamental with whistling partials, too much a rasp [Cited S4].
- Contact point: loudest near the bridge, quieter and with fewer overtones towards the fingerboard [Cited S4].
- Pitch is changed by stopping the string on a fretless fingerboard; positions 1-7 are "low", 8-11 "high",
  solo playing up to the 14th [Cited S4]; Wikipedia gives the 13th as the highest practical position [Cited S8].
- Four strings tuned in fifths G3, D4, A4, E5 (55, 62, 69, 76) [Cited S4, S8]; strings are numbered from the top:
  E = 1st string [Cited S2].
- Plucked (pizzicato) with the right hand, or the left hand (marked +) [Cited S4].
- Curved bridge: no more than two strings sound at once; triple and quadruple stops are arpeggiated (two lowest,
  then two highest) [Cited S4].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
Non-transposing, treble clef [Cited S4]. Written = sounding.

| Item | Pitch | MIDI | Source |
|---|---|---|---|
| Lowest (open G) | G3 | 55 | [Cited S4, S8] |
| Scordatura lowest | down to D3 "on rare occasions" | 50 | [Cited S8] |
| Highest stopped (VSL range) | A7 | 105 | [Cited S4] |
| Highest by fingerboard end / artificial harmonics | C8 | 108 | [Cited S8] |
| Highest harmonic (VSL) | D8 | 110 | [Cited S4] |
| Practical limit for orchestral parts (Piston, via Wikipedia) | E7 | 100 | [Cited S8] |
| "Extreme high notes up to" E7 need positions above the seventh (scan p. 306: treble clef, third ledger line under an 8va line) | E7 | 100 | [Cited S2] |
| "Simplify any passages above top A" (scan p. 306: treble clef, above the fourth ledger line) | A6 | 93 | [Cited S2] |
| Rimsky-Korsakov's practical extreme limit (image ms001.png: treble clef, note above the fourth ledger line, no 8va visible; the eBook text and alt text say "A7") | A6 (recommended reading) | 93 | [Cited S1] |
| Above E6 (beyond the soprano voice) the tone "lose[s] in expression and warmth" | E6 | 88 | [Cited S1] |
| Spitfire section range "curated ... to fit within the reasonable demands of professional players" | G3-C#7 | 55-97 | [Cited S14] |

Per-string ranges (VSL; in brackets the top harmonic) [Cited S4]:

| String | Range | MIDI | Character (VSL) |
|---|---|---|---|
| G (4th) | G3-C5 (G5) | 55-72 (79) | dark, sonorous, tends to roughness low; intense, soulful high |
| D (3rd) | D4-G5 (D6) | 62-79 (86) | very full, mellow; vocal cantilena |
| A (2nd) | A4-D6 (A6) | 69-86 (93) | more mellow than D |
| E (1st) | E5-A7 (D8) | 76-105 (110) | lustrous, metallic, dominates middle voices; bright but less full high up |

- Register breaks: VSL says registers with fixed boundaries describe the violin poorly; colour depends on the
  string chosen and the technique; the underlying sound is homogeneous [Cited S4]. Rimsky-Korsakov: E string
  brilliant, A and D sweeter and weaker, covered G harsh; open strings clearer and more powerful but less
  expressive [Cited S1]. Each lower string should not be taken above about the 4th position (octave or ninth
  of the open string) in orchestral writing [Cited S1].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Agility: the most active and flexible string instrument (order: violin, viola, cello, double bass) [Cited S1].
  Long fast chromatic figures are unsuitable for strings and better given to woodwinds [Cited S1]. Above top A
  (A6) the fingers must close up, so simplify passages [Cited S2].
- Keys: easiest are major keys from Eb to E and minor from C to B; hardest Db, Gb, Cb, F#, C# major [Cited S2].
- Bow limits: only notes on the same or adjacent strings can be slurred; long forte slurs are impossible;
  a pianissimo held slur lasts at most about 4 bars of moderate time, about half that for many-note slurs [Cited S2].
- Detache is for f/mf passages and tops out at about allegro quavers; sautille cannot go above mp [Cited S2];
  VSL: sautille only p or mp, ricochet up to p-mf [Cited S4].
- Double stops: easiest with an open string; with both stopped, unison to tenth is common, thirds to octaves easy
  within one position [Cited S4]; double stops above the 3rd position are "practically unknown in the orchestra"
  [Cited S2].
- Fingered tremolo: limit of an augmented fourth [Cited S2]; VSL: commonly a third, up to a diminished fifth
  [Cited S4].
- Harmonics: flute-like, thinner than normal notes; playable pp to ff; natural harmonics louder than artificial
  [Cited S4]. Rimsky-Korsakov: cold, little expression, weak, use sparingly [Cited S1].
- Mute: removes partials above about 2000 Hz; quieter, darker, thinner [Cited S4]; "dull" in soft passages,
  a hiss in loud ones, volume always greatly reduced [Cited S1]; allow at least a couple of bars of moderate time to fit
  it, slightly less to remove it [Cited S2].
- Sul ponticello: brighter, glassy, eerie, more partials [Cited S4]; in practice mostly used as bowed tremolo
  [Cited S2]. Sul tasto/flautando: softer, fewer partials, flute-like [Cited S4].
- Bowed tremolo has less sound than the corresponding legato or fingered tremolo [Cited S2].
- Pizzicato: little expression; resonant on open strings, dry and hard in high positions; never as fast as arco
  [Cited S1]; switching pizz to arco needs about half a second [Cited S2].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique | Vocabulary id | Meaning / limits | Source |
|---|---|---|---|
| Long note | `sustain` | normale long; Spitfire "Long", Sonatina "Sustain", Logic "Sustain" | [Cited S14, S15, S17] |
| Legato (slur) | `legato` | notes in one bow; library legato is monophonic and triggered by overlapping notes | [Cited S2, S14, S15] |
| Detache | `detache` (new) | separate bows, bow stays on the string | [Cited S2, S4] |
| Martele | `martele` (new) | hammered stroke at the point, abrupt stop on the string | [Cited S2, S4] |
| Staccato | `staccato` | several short notes in one bow (VSL); Spitfire "Short 0'5 / 1'0" | [Cited S4, S14] |
| Spiccato | `spiccato` | bouncing, one stroke per note, dot without slur | [Cited S4, S14] |
| Sautille | `sautille` (new) | fast uncontrolled bounce at mid-bow, p-mp only | [Cited S2, S4] |
| Ricochet / jete | `ricochet` (new) | thrown bow, 3-4 bounces per stroke | [Cited S2, S4] |
| Loure | `loure` (new) | pulsed notes in one bow, legato impression | [Cited S2, S4] |
| Portato | `portato` | slightly accented notes on one stroke | [Cited S4] |
| Tenuto | `tenuto` | full value | [Cited S4] |
| Marcato | `marcato` | heavy accented attack; Spitfire "Marcato Attack", Sonatina "Marcato" | [Cited S14, S15] |
| Pizzicato | `pizzicato` | plucked, `arco` to resume | [Cited S2, S4] |
| Bartok (snap) pizzicato | `bartok_pizzicato` | string snaps back on the fingerboard; percussive | [Cited S4, S14] |
| Left-hand pizzicato | `left_hand_pizzicato` (new) | plucked by the left hand, marked + | [Cited S4] |
| Col legno | `col_legno`, `col_legno_battuto`, `col_legno_tratto` | struck (battuto) or bowed (tratto) with the stick; no legato | [Cited S2, S4] |
| Bowed tremolo | `tremolo` (unmeasured), `tremolo_measured` | measured = strokes match the notated division; must be synchronised | [Cited S4, S14] |
| Fingered tremolo | `fingered_tremolo` (new) | left-hand alternation of two notes, smooth bow | [Cited S2, S4] |
| Trill | `trill`, `trill_half`, `trill_whole` | half or whole step; Spitfire also minor/major 3rd trills | [Cited S4, S14, S17] |
| Harmonics | `harmonics`, `harmonics_natural`, `harmonics_artificial` | artificial = stop + touch a fourth above, sounds two octaves up | [Cited S4, S14] |
| Sul ponticello | `sul_ponticello` | at the bridge | [Cited S2, S4] |
| Sul tasto / flautando | `sul_tasto`, `flautando` | Spitfire records both separately (flautando = "flute-like", tasto = soft normale) | [Cited S4, S14] |
| Mute | `con_sordino` | `senza sordino` removes it | [Cited S4] |
| Portamento / glissando | `portamento`, `glissando` | audible slide | [Cited S4, S17] |
| Vibrato control | `non_vibrato`, `vibrato_on` | Spitfire "molto vib", senza vib | [Cited S14] |
| On one string | `su_una_corda` (new) | melody kept on one named string, e.g. sul G | [Cited S4, S14] |
| Behind the bridge | `behind_the_bridge` (new) | bowing between bridge and tailpiece, indeterminate pitch | [Cited S4] |
| Fortepiano, swell | `fortepiano`, `crescendo_swell` | Logic "Fortepiano Slow/Fast", "Crescendo Slow/Fast" | [Cited S17] |

New ids (one line each): `detache` = separate bow strokes on the string, one note per stroke; `martele` =
hammered, abruptly stopped stroke with the bow left on the string; `sautille` = fast light off-the-string
bounce at the middle of the bow; `ricochet` = thrown bow bouncing several notes in one stroke (jete);
`loure` = several gently pulsed notes in one bow; `left_hand_pizzicato` = string plucked by a left-hand finger;
`fingered_tremolo` = rapid two-note alternation by the left hand under a smooth bow; `su_una_corda` = a passage
kept on one named string for its colour; `behind_the_bridge` = bowing the short string segment between bridge
and tailpiece [Cited S2, S4 for the techniques; the ids are new].

How libraries implement them:
| Library (document read) | Articulation selection | Dynamics / expression | Legato | Source |
|---|---|---|---|---|
| Spitfire Symphony Orchestra (manual v5) | each articulation is a patch; red keyswitch range on the Kontakt keyboard; several keys at once layer articulations; UACC (CC-based) option | Dynamics control crossfades recorded layers (Modwheel/CC1); vibrato crossfade on CC21; Expression = CC11 inside CC7 | hold the first note while playing the next (overlap); in Performance Legato, velocity picks the transition when playing slowly: 1-19 portamento, 20-84 fingered, 85-127 bowed | [Cited S14] |
| Sonatina Symphonic Orchestra | one SFZ per articulation, or a KS file switched by keys outside the range | Notation patches: velocity = volume, CC1 = marcato attack strength; Performance patches: CC1 = volume of long notes; CC21 = vibrato amount on most strings | monophonic, overlap a new note; the SFZ uses `trigger=first` and `trigger=legato` groups | [Cited S15] |
| VSCO 2 CE | KS file: C2 sustain vibrato, C#2 tremolo, D2 spiccato, D#2 pizzicato (= MIDI 36-39 with SFZ c4 = 60) | 2 velocity layers (`hivel` 62/127), 2 round robins | no legato patch | [Cited S16, S18] |
| Logic Pro Studio Strings (iPad guide) | keyswitches from MIDI 24: 24 Sustain, 25 Tremolo, 26 Accented Sustain, 27 Fortepiano Slow, 28 Fortepiano Fast, 29 Spiccato, 30 Staccato, 31 Pizzicato, 32 Trill Semi, 33 Trill Whole; 34-41 Fall Long/Short, Scoop Slow/Fast, Crescendo Slow/Fast, Glissando Up/Down (violins, violas, cellos only) | not stated on the page read | not stated on the page read | [Cited S17] |
| VSL Academy | describes notation, not a product mapping | n/a | n/a | [Cited S4] |

- Vendor octave names: Apple's guide calls MIDI 24 "C0", so its middle C (60) is "C3"; SFZ files use c4 = 60
  [Cited S17, S18]. Convert by MIDI number, never by name.

### 6. Solo vs section (typical number of players, divisi, unison doubling)
| Orchestra | Vn I | Vn II | Source |
|---|---|---|---|
| Full | 16 | 14 | [Cited S1] |
| Medium | 12 | 10 | [Cited S1] |
| Small | 8 | 6 | [Cited S1] |
| Larger orchestras | 20-24 (others in proportion) | | [Cited S1] |
| Large orchestra (VSL) / Spitfire recording | 16 | 14 | [Cited S4, S14] |
| Wagner, Die Walkure | 16 | 16 | [Cited S12] |

Violin I vs Violin II (one instrument type, two parts):
| Aspect | Violin I | Violin II | Source |
|---|---|---|---|
| Role | higher part, usually the main melody | "lower sisters": an octave below, or in unison, filling the middle register; recent composers sometimes give them higher parts | [Cited S4] |
| Melody | soprano-alto register and extra-high compass usually goes to Vn I, sometimes to Vn II or both in unison (fuller, no loss of quality) | | [Cited S1] |
| Strength | an extra desk is usual and they generally have a more powerful tone | with the violas, a secondary part | [Cited S1] |
| Seating (modern) | left of the conductor; principal = leader/concertmaster | right of the 1sts, next to the violas | [Cited S14, S12] |
| Seating (19th century) | antiphonal: Vn I, cello, viola, Vn II | opposite side | [Cited S12] |
| Library range | G3-C#7 | G3-C#7 (identical) | [Cited S14] |
| Library articulations | Spitfire Vn I has more patches (e.g. Long Sul Tasto, major/minor-third trills) than Vn II | | [Cited S14] |

- Divisi: by desk (desks 1, 3, 5 upper; 2, 4, 6 lower) or by stand position (outside player upper) [Cited S1,
  S12]; threefold divisi is harder to balance and usually done by desks [Cited S1, S12]; four or more parts are
  rare and reduce volume, useful in piano [Cited S1]. Rimsky-Korsakov prefers dividing Vn II with violas over
  Vn I with violas because the two groups are similar in size and seated together [Cited S1].
- Section sound vs solo: sections play "in chorus"; measured tremolo must be absolutely synchronised [Cited S4];
  the jete is more effective from a large body than from one player [Cited S2]; Spitfire "CS Blend" = half of
  each desk muted [Cited S14]. Section samples are a separate recording from solo violin in every library read
  [Cited S14, S15, S16].

### 7. Role in the orchestra and typical doublings
- Melody carrier; Vn I + Vn II in octaves, or double octaves with violas and cellos, for climaxes [Cited S4, S1].
- Good blend with woodwinds (woodwinds add power, strings soften them); poor blend with trumpet, trombone, tuba;
  horn blends best with violins, especially with cellos added; violin pizzicato blends with harp [Cited S4].
- Vn I in the upper register doubling high woodwinds is a Rimsky-Korsakov example of "choice resonance"
  [Cited S1]. Typical accompaniment: Vn II and violas tremolando in the middle, cellos as bass [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Historical: violino piccolo (Bach; a three-quarter violin a minor third higher), violetta marina, viola pomposa;
  all obsolete [Cited S2].
- Scordatura, e.g. Mahler Symphony 4, solo violin a whole tone higher (A3, E4, B4, F#5) [Cited S4].
- Five- to seven-string violins exist; seven strings is about the practical maximum [Cited S8]; electric violins
  use magnetic or piezo pickups [Cited S8].
- Orchestral violinists do not normally double on another instrument in a work [Estimate: no source read
  discusses doubling for violinists].

### 9. General MIDI and existing Sonara metadata
- GM Level 1 program 41 "Violin" (group 41-48 "Strings"); related: 45 "Tremolo Strings", 46 "Pizzicato
  Strings", 49 "String Ensemble 1", 50 "String Ensemble 2" [Cited S13]. GM1 requires CC1, CC7, CC10, CC11, CC64,
  velocity response and pitch bend default +/-2 semitones; middle C = key 60 [Cited S13].
- data.rs: `violin`, transposition 0, playable 55-108 and comfortable 55-100 both cited to Wikipedia, the 13
  `BOWED` articulations with an Estimate provenance [Cited S23].

### Verification of data.rs
| Field | data.rs today | Verified value | Verdict (confirmed / corrected / still estimate) | Source |
|---|---|---|---|---|
| playable range | 55-108 (Cited Wikipedia) | 55-105 (VSL G3-A7 stopped); C8 = 108 only by fingerboard end or artificial harmonics | corrected (low confirmed) | [Cited S4, S8] |
| comfortable range | 55-100 (Cited Wikipedia, E7 "practical limit") | 55-93 (A6: Forsyth and Rimsky-Korsakov); E7 kept as the orchestral extreme | corrected | [Cited S1, S2, S8] |
| transposition | 0 | 0 (non-transposing) | confirmed | [Cited S4] |
| articulations | 13 BOWED ids, Estimate | all 13 are documented for the violin; add the new ids above | confirmed (provenance now Cited) | [Cited S4, S14, S15] |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Violin coverage | Sonara sampler notes | Source |
|---|---|---|---|---|
| VSCO 2 CE (SFZ branch) | CC0 1.0 | solo violin: vibrato sustain (+ quiet), tremolo, spiccato, pizzicato, KS file, keys 55-96; violin section: sustain vibrato (+ quiet), tremolo, spiccato, pizzicato, KS, keys 55-86 | uses only opcodes the sampler supports; no legato, mute, harmonics or col legno | [Cited S16, S23] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 (README); Sonara's research doc marks bundling "unresolved, no" | separate 1st and 2nd Violins: sustain, marcato, staccato, legato, pizzicato, col legno, tremolo, harmonics; two solo violins (spiccato, non-vibrato variants); 1st violins sustain mapped G3-C7 (55-96) | CC1/CC21 crossfades not played; `trigger=first/legato` is parsed as a plain attack (sfz.rs maps every trigger except `release` to attack), so legato transitions are not reproduced | [Cited S15, S23] |
| BBCSO Discover, Spitfire LABS, Berlin Free Orchestra | proprietary EULA, own player only | not inspected | needs plugin hosting | [Cited S23] |
| Spitfire Symphony Orchestra (commercial) | proprietary | Vn I 33 and Vn II 29 patch entries listed (incl. FX) | needs plugin hosting | [Cited S14] |

## viola: Viola

```json
{"id": "viola", "family": "strings", "names": {"en": "Viola", "cs": "viola", "it": "viola", "de": "Bratsche (also Viola)", "fr": "alto", "score_abbreviation": "Va."}, "pitched": true, "transposition": 0, "clefs": ["alto", "treble"], "sounding_playable": [48, 93], "sounding_comfortable": [48, 81], "range_provenance": {"playable": "Cited S5 (VSL: range C3-A6, harmonics to E7)", "comfortable": "Cited S1 (practical extreme limit A5: text and ms002.png agree, the HTML alt text says A7); S2 gives E6 as the orchestral maximum and about C6 for ordinary routine; S5 calls C3-G5 the characteristic range"}, "written_playable": [48, 93], "gm_program": 42, "gm_program_name": "Viola", "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["sustain", "legato", "staccato", "spiccato", "tenuto", "marcato", "pizzicato", "tremolo", "trill", "col_legno", "harmonics", "sul_ponticello", "con_sordino"], "articulations_proposed_new": ["detache", "martele", "sautille", "ricochet", "loure", "portato", "sul_tasto", "flautando", "bartok_pizzicato", "left_hand_pizzicato", "col_legno_battuto", "col_legno_tratto", "tremolo_measured", "fingered_tremolo", "harmonics_natural", "harmonics_artificial", "trill_half", "trill_whole", "portamento", "glissando", "fortepiano", "crescendo_swell", "non_vibrato", "vibrato_on", "su_una_corda", "behind_the_bridge"], "articulations_provenance": "Cited S5 (VSL technique list), S2 (technique as violin, a fifth lower), S14 (Spitfire violas patch list), S15 (Sonatina violas and solo viola), S17 (Logic Studio Strings)", "solo_or_section": "both", "typical_players": {"classical": "Estimate: about 4-6 in a Classical-era orchestra; no source read states it", "romantic": "12 (full), 8 (medium), 4 (small orchestra) [Cited S1]", "film": "12 as recorded for Spitfire Symphony Orchestra [Cited S14]", "chamber": "1 in a string quartet [Cited S5]"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Source |
|---|---|---|
| English | viola (older English: "tenor") | [Cited S2] |
| Czech | viola (archaic or rare "bráč") | [Cited S20] |
| Italian | viola | [Cited S2, S5] |
| German | Bratsche; VSL gives "Viola" | [Cited S2], [Cited S5] |
| French | alto | [Cited S2, S9]; VSL lists "viola" for French, which disagrees (see open questions) [Cited S5] |
| Score | "Violas (or Va)" | [Cited S14] |

- Family: strings; alto (and tenor) member of the violin family [Cited S5, S1].

### 2. Sound production
- As the violin (bowed, stopped, plucked), tuned a fifth lower: C3, G3, D4, A4 (48, 55, 62, 69) [Cited S5, S9].
- The two lower strings are covered (metal-wound) [Cited S2]. The body is too small for its pitch, which is why
  the viola is "a betwixt-and-between instrument" and varies more in size than any other string instrument
  [Cited S2]; it is more prone to wolf notes [Cited S2].
- Bridge curvature: the same double and multiple stops as on the violin; chords are arpeggiated [Cited S5].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
Non-transposing; alto clef (middle C on the middle line), treble clef "from the second octave above middle C";
change clef only for longer passages [Cited S5]. Forsyth: alto clef for all passages "up to about" F5 = 77
(scan p. 383: alto clef, notehead on the third ledger line above, the same glyph pattern as the treble-clef E6 on
the same page), treble only when a whole passage lies in the top octave, "say from" E5 upwards (treble clef, top
space) [Cited S2].

| Item | Pitch | MIDI | Source |
|---|---|---|---|
| Lowest (open C) | C3 | 48 | [Cited S5, S9] |
| Highest (VSL range) | A6 | 93 | [Cited S5] |
| Highest harmonic (VSL) | E7 | 100 | [Cited S5] |
| "Unwise to go beyond" in the orchestra (scan p. 383: treble clef, third ledger line above) | E6 | 88 | [Cited S2] |
| Natural harmonic A a fourth higher may be added (footnote p. 383) | A6 | 93 | [Cited S2] |
| "The last three or four semitones ... can be dispensed with", leaving three octaves | about C6 | about 84 | [Cited S2] |
| Rimsky-Korsakov's practical extreme limit (text "A5"; ms002.png: alto clef, note on the fourth ledger line) | A5 | 81 | [Cited S1] |
| Characteristic range | C3-G5 | 48-79 | [Cited S5] |
| Spitfire section range | C3-F#6 | 48-90 | [Cited S14] |

Per-string ranges and colour (VSL) [Cited S5]:
| String | Range | MIDI | Character |
|---|---|---|---|
| C | C3-D4 | 48-62 | powerful, resonant, rich and dark; foreboding; the same notes sound more powerful on the cello |
| G | G3-A4 | 55-69 | rather bland, weaker than the same notes on the violin; filler and figuration |
| D | D4-E5 | 62-76 | rather bland |
| A | A4-E6 (A6) | 69-88 (93) | penetrating, austere, nasal, distinctive; pain and sentiment when forte |

- Rimsky-Korsakov: viola A string "more biting in quality and slightly nasal"; viola D string sweeter and weaker;
  covered strings rather harsh [Cited S1].
- Positions: generally the 5th position is the highest on C, G and D strings, the 8th (extreme 11th) on the A
  string [Cited S5]; without a thumb position the viola player relies on violin technique, and beyond the
  5th position on the 1st string technique "peters out" except with brilliant players [Cited S2].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Agility: second after the violin [Cited S1]. Judge viola passages "in alt" as if they were violin passages an
  octave higher [Cited S2]. Players who spend "99 per cent" of their time in the two lower octaves lose freedom at
  the top [Cited S2].
- Chromatic passages are clearer on the viola than on the violin but better slurred than bowed; very fast bowed
  chromatic scales are almost impossible [Cited S2].
- Double stops up to the octave; avoid wide spreads and long high-position playing; quadruple stops in high
  positions do not occur in orchestral literature [Cited S5].
- Mute: no string instrument "answers so well to the mute as the Viola" [Cited S2]; VSL describes the same mute
  effect as for violin (partials above 2 kHz reduced) [Cited S5].
- Section colour: violas as a group have "an austere charm" used for melodic tasks at dramatic moments,
  especially in opera [Cited S5].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
- Technique list identical to the violin's, transposed a fifth down: VSL lists the same bowings, pizzicato
  (incl. Bartok), col legno, tremolo (bowed, measured, fingered), harmonics, ponticello/tasto, portamento,
  glissando, con sordino, louré, portato, ricochet [Cited S5]; Forsyth says all violin bowing, harmonics,
  tremolos and chords apply "when transposed down a perfect fifth" [Cited S2]. The articulation table of the
  `violin` card applies; `su_una_corda` here is typically "sul C" (Spitfire "Long Sul C") [Cited S14].
- Libraries: Spitfire violas: 29 patches incl. Flautando, Harmonics, Sul C, Sul Pont, Super Sul Tasto, Col Legno,
  Bartok, Spiccato, measured tremolo, m2/M2 trills, Marcato Attack [Cited S14]; Sonatina: Violas = sustain,
  marcato, staccato, legato, pizzicato, col legno, tremolo, harmonics; Solo Viola = sustain, marcato, staccato,
  legato, pizzicato, harmonics [Cited S15]; VSCO 2 CE: viola section sustain (+ quiet), tremolo, spiccato,
  pizzicato, KS at C2-D#2 (36-39) [Cited S16, S18]; Logic Studio Strings: same keyswitch map as violins
  [Cited S17]. Controls as in the violin card (Spitfire CC1/CC11/CC21; Sonatina CC1/CC21; overlap legato)
  [Cited S14, S15].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- Section: 12 (full), 8 (medium), 4 (small orchestra) [Cited S1]; 12 in VSL's large orchestra (the list
  16-14-12-10-8 is printed on the VSL violin, cello and double-bass pages, not on the viola page) and in Spitfire's
  recording [Cited S4, S6, S7, S14].
- Fewer players than violins is one reason cantabile is given less often to violas [Cited S1].
- Divisi: violas are divided more often than either violin group, but mostly in simple two-part harmony filling;
  elaborate violin-style subdivision is rare in viola parts [Cited S2];
  Rimsky-Korsakov's preferred pairing is Vn II div. + violas div. [Cited S1].
- The solo viola "is seldom found" in orchestral writing [Cited S1].

### 7. Role in the orchestra and typical doublings
- Middle voice; melodies given to violas are generally doubled by other strings or woodwind [Cited S1].
- Viola + violin: mellow and full, violins dominate; violins + violas in octaves need cellos in unison for a firm
  foundation [Cited S5]. Viola + cello: cellos dominate; in octaves good for tenor cantilenas [Cited S5].
- Violas often play parallel to the double basses two octaves higher, reinforcing their partials [Cited S7].
- Rimsky-Korsakov: violas and double basses in octaves is a listed doubling [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Forsyth covers the viola alta, violotta and viola d'amore as related instruments [Cited S2].
- Five-string violas exist for a wider range [Cited S9]; viola da braccio is the historical name ("of the arm")
  [Cited S9].
- An orchestral violist may be asked to play viola d'amore in a few works [Estimate: Forsyth treats it but no
  source read states orchestral doubling practice].

### 9. General MIDI and existing Sonara metadata
- GM Level 1 program 42 "Viola" [Cited S13].
- data.rs has no `viola`; `docs/orchestration-metadata.md` lists viola as "Not covered" [Cited S23]. This card is
  the proposed new entry.

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence | Viola coverage | Source |
|---|---|---|---|
| VSCO 2 CE | CC0 1.0 | section only: sustain vibrato (+ quiet), tremolo, spiccato, pizzicato, KS; keys 48-86 | [Cited S16] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 | section (8 articulations) and solo viola (6) | [Cited S15] |
| Spitfire Symphony Orchestra | proprietary | 12 players, 29 patches | [Cited S14] |

## cello: Cello

```json
{"id": "cello", "family": "strings", "names": {"en": "Cello (violoncello)", "cs": "violoncello", "it": "violoncello", "de": "Violoncello (Cello)", "fr": "violoncelle", "score_abbreviation": "Vc."}, "pitched": true, "transposition": 0, "clefs": ["bass", "tenor", "treble"], "sounding_playable": [36, 88], "sounding_comfortable": [36, 76], "range_provenance": {"playable": "Cited S3 (lowest C2; Widor's cello concerto reaches E6) and S6 (VSL: C2-A5 stopped, harmonics to A7)", "comfortable": "Cited S3 (Widor: E5 highest note recommended for orchestral use) and S2 (p. 415: E5 a reasonable upward limit, read from the scan)"}, "written_playable": [36, 88], "gm_program": 43, "gm_program_name": "Cello", "gm_percussion_notes": [], "existing_sonara_id": "cello", "articulations_existing_vocab": ["sustain", "legato", "staccato", "spiccato", "tenuto", "marcato", "pizzicato", "tremolo", "trill", "col_legno", "harmonics", "sul_ponticello", "con_sordino"], "articulations_proposed_new": ["detache", "martele", "sautille", "ricochet", "loure", "portato", "sul_tasto", "flautando", "bartok_pizzicato", "left_hand_pizzicato", "col_legno_battuto", "col_legno_tratto", "tremolo_measured", "fingered_tremolo", "harmonics_natural", "harmonics_artificial", "trill_half", "trill_whole", "portamento", "glissando", "fortepiano", "crescendo_swell", "non_vibrato", "vibrato_on", "su_una_corda", "behind_the_bridge"], "articulations_provenance": "Cited S6 (VSL technique list), S3 (Widor: pizzicato, double stops, harmonics), S14 (Spitfire celli patch list), S15 (Sonatina celli and solo cello), S17 (Logic Studio Strings)", "solo_or_section": "both", "typical_players": {"classical": "Estimate: about 3-6, often reading one 'Bassi' part with the double basses [the shared part is Cited S12]", "romantic": "10 (full), 6 (medium), 3 (small orchestra) [Cited S1]; Wagner, Die Walkure: 12 [Cited S12]", "film": "10 as recorded for Spitfire Symphony Orchestra [Cited S14]", "chamber": "1 in a string quartet [Cited S5]"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Source |
|---|---|---|
| English | cello, violoncello | [Cited S2, S10] |
| Czech | violoncello (colloquially "cello") | [Cited S21] |
| Italian | violoncello ("little big viola") | [Cited S2, S6] |
| German | (Violon)Cello | [Cited S6] |
| French | violoncelle | [Cited S2, S6] |
| Score | "Cellos (or Violoncello or VC)"; Rimsky-Korsakov writes "'Cellos" | [Cited S14, S1] |

- Family: strings; "tenor and bass instrument of the violin family" [Cited S6].

### 2. Sound production
- Bowed, stopped, plucked like the violin; played seated, supported by an endpin [Cited S2, S10]. The bow
  attack is normally from the bass side upwards [Cited S2].
- Four strings in fifths C2, G2, D3, A3 (36, 43, 50, 57), an octave below the viola [Cited S10, S2].
- Thumb position (thumb as a moveable nut) for high notes, in use since about 1740 [Cited S6]; regular fingering
  up to B4, thumb positions above [Cited S3]; VSL: thumb used from the 7th position [Cited S6].
- In lower positions four fingers serve three diatonic notes and the hand commands only a major third at a time
  [Cited S2].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
Non-transposing; bass clef mainly, tenor and treble for higher passages [Cited S6]. Forsyth: tenor clef for a
middle-high band, treble for anything continually above middle C [Cited S2]. Older scores (Beethoven, Bruckner)
sometimes write treble clef an octave higher than it sounds [Cited S2, S6]: an import filter must know the
convention of the edition.

| Item | Pitch | MIDI | Source |
|---|---|---|---|
| Lowest (open C) | C2 | 36 | [Cited S3, S6] |
| Highest stopped (VSL range) | A5 | 81 | [Cited S6] |
| Highest harmonic (VSL) | A7 | 105 | [Cited S6] |
| Solo writing: Widor's own concerto | up to E6 | 88 | [Cited S3] |
| Orchestral upper limit (Widor) | E5 | 76 | [Cited S3] |
| "Reasonable upward-limit for orchestral writing" (scan p. 415: treble clef, top space) | E5 | 76 | [Cited S2] |
| Natural harmonic A a fourth higher may be included (footnote p. 415) | A5 | 81 | [Cited S2] |
| Rimsky-Korsakov's practical extreme limit (ms003.png: bass clef, above the third ledger line) | A4 | 69 | [Cited S1] |
| Spitfire section range | C2-Bb5 | 36-82 | [Cited S14] |
| Top of regular fingering; thumb above | B4 | 71 | [Cited S3] |
| Pizzicato upper limit (Widor) | about A4 | 69 | [Cited S3] |
| Harmonics, any note | C4-A5 | 60-81 | [Cited S3] |

Per-string ranges and colour (VSL) [Cited S6]:
| String | Range | MIDI | Character |
|---|---|---|---|
| C | C2-F3 | 36-53 | weighty, dark, powerful; sonorous fundamental bass |
| G | G2-C4 | 43-60 | less power and sustain; carries bass lines; resonant pizzicato on C and G |
| D | D3-G4 | 50-67 | "most musical and soulful"; intimate lyrical cantilena |
| A | A3-A5 (A7 harm.) | 57-81 (105) | bright, penetrating, incisive, dominant |

- Rimsky-Korsakov: the A string is bright with a "chest-voice" timbre; passionate cantabile is usually laid on it
  [Cited S1]. Widor calls four G-string notes "bad" in tone; Forsyth doubts this applies generally [Cited S3, S2].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Agility: third after violin and viola [Cited S1]. Avoid figures built on repeated fourths and continual skips of
  a fourth: each shift costs time [Cited S2]. Wild jumps between thumb position and low positions are hard
  [Cited S2].
- Range of expression: calm and solemn low, passionate at the top; register transitions smooth [Cited S6].
- Double stops: easiest with an open string; best sounding in fourths, fifths and sixths; widest on adjacent
  strings an octave (an eleventh in solo literature, with the thumb) [Cited S6]; Widor gives recommended ranges
  for fifths and sixths and limits difficult thirds to fortissimo [Cited S3].
- Fingered tremolo: most common interval a third; up to a fourth, or a fifth with the thumb position [Cited S6].
- Quadruple stops played piano are usually done divisi in orchestras [Cited S6].
- Pizzicato above about A4 is ineffective [Cited S3]; Forsyth places the effective pizz limit at the "top Bb"
  that ends a Humperdinck passage (Hänsel und Gretel, Ex. 266, p. 429) [Cited S2]. On the scan the last note sits
  in the space above the third ledger line of the bass clef (A4 position, key signature of four sharps), so the
  octave is the fourth: Bb4 (A#4) = 70, within a semitone of Widor [Estimate: my reading of the p. 429 scan].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
- Technique list as for the violin [Cited S6]; specific to the cello: thumb-position octaves and artificial
  harmonics [Cited S6]; arpeggiated pizzicato works in both directions on the cello (not downward on the violin)
  [Cited S3]; sul ponticello tremolo cello bass in Wagner's Meistersinger [Cited S3].
- Libraries: Spitfire Celli: Long, CS, CS Blend, Flautando, Harmonics, Molto Vib, Sul C, Sul Pont, Super Sul Tasto,
  Marcato Attack, Shorts (Spiccato, 0'5, 1'0, Brushed, Col Legno, Harmonics, Pizzicato, Bartok), measured and
  unmeasured tremolo, trills [Cited S14]; Sonatina: Celli = 8 articulations incl. legato and harmonics; Solo Cello =
  sustain, marcato, staccato, legato, pizzicato, harmonics, vibrato baked in (no CC21) [Cited S15]; VSCO 2 CE:
  cello section sustain (+ quiet), tremolo, spiccato, pizzicato, KS keys at C6-D#6 (84-87, above the 36-77 key
  range), 4 velocity layers in the KS file [Cited S16, S18]; Logic Studio Strings: same map as violins incl. falls,
  scoops, glissandi [Cited S17].
- Controls and legato mechanics as in the violin card [Cited S14, S15].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- Section: 10 (full), 6 (medium), 3 (small) [Cited S1]; 10 in VSL's large orchestra and Spitfire [Cited S6, S14];
  12 in Wagner's Walkure [Cited S12].
- Divisi: "6 'Cellos div. a 3" is Rimsky-Korsakov's own marking example [Cited S1]; cellos divided with violas or
  with basses are listed combinations [Cited S1].
- A section must keep high passages simple: what one player can do, a dozen cannot [Cited S2].
- Solo cello in the orchestra and cello ensembles ("cello choir", e.g. William Tell overture) [Cited S10].

### 7. Role in the orchestra and typical doublings
- Bass line with the double basses in octaves (the "classic" combination, a sustaining bass even when soft)
  [Cited S6, S1]; tenor cantilena in the middle register [Cited S6].
- Cello + violin in octaves: expansive cantilenas, cellos can drown violins [Cited S6]; cello + viola: cellos
  dominate, violas mellow the top [Cited S6]; cello + horn blend well with violins [Cited S4].
- Cello melody doubled by bassoons (Rimsky-Korsakov, Antar) [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Violoncello piccolo (five strings, top E4) [Cited S6]; viola pomposa, possibly Bach's "violoncello piccolo"
  [Cited S2]; viola da gamba, the older fretted instrument [Cited S2].
- Fractional sizes (7/8, 3/4, ...) share range and usage [Cited S10].
- Orchestral cellists do not normally double on other instruments [Estimate: no source read discusses it].

### 9. General MIDI and existing Sonara metadata
- GM Level 1 program 43 "Cello" [Cited S13].
- data.rs: `cello`, transposition 0, playable 36-93 as an Estimate ("the upper limit is not stated by a source
  that was read"), no comfortable range, the 13 `BOWED` articulations as an Estimate [Cited S23].

### Verification of data.rs
| Field | data.rs today | Verified value | Verdict (confirmed / corrected / still estimate) | Source |
|---|---|---|---|---|
| playable range | 36-93 (Estimate) | 36-88: C2 confirmed; top E6 from Widor's solo writing; VSL's stopped range ends at A5 (81), harmonics to A7 | corrected | [Cited S3, S6] |
| comfortable range | none | 36-76 (E5: Widor and Forsyth agree) | corrected (new value) | [Cited S2, S3] |
| transposition | 0 | 0 (non-transposing; beware old octave-treble notation) | confirmed | [Cited S6] |
| articulations | 13 BOWED ids, Estimate | all 13 documented for the cello | confirmed (provenance now Cited) | [Cited S6, S14, S15] |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence | Cello coverage | Source |
|---|---|---|---|
| VSCO 2 CE | CC0 1.0 | section only: sustain vibrato (+ quiet), tremolo, spiccato, pizzicato, KS; keys 36-77 | [Cited S16] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 | Celli (8 articulations) and Solo Cello (6) | [Cited S15] |
| Spitfire Symphony Orchestra | proprietary | 10 players, about 30 patches; solo cello (Caroline Dale) in the Solo Strings | [Cited S14] |

## double_bass: Double bass

```json
{"id": "double_bass", "family": "strings", "names": {"en": "Double bass", "cs": "kontrabas", "it": "contrabbasso", "de": "Kontrabass", "fr": "contrebasse", "score_abbreviation": "Cb."}, "pitched": true, "transposition": -12, "clefs": ["bass", "tenor", "treble"], "sounding_playable": [23, 67], "sounding_comfortable": [28, 55], "range_provenance": {"playable": "Cited S7 (VSL: B0-G4 sounding, five-string B0; harmonics to G6) and S11 (B0 on five-string basses)", "comfortable": "Cited S11 (Classical orchestral parts rarely exceed E1-G3) and S1 (practical extreme limit written G4 = sounding G3, ms004.png)"}, "written_playable": [35, 79], "gm_program": 44, "gm_program_name": "Contrabass", "gm_percussion_notes": [], "existing_sonara_id": "double_bass", "articulations_existing_vocab": ["sustain", "legato", "staccato", "spiccato", "tenuto", "marcato", "pizzicato", "tremolo", "trill", "col_legno", "harmonics", "sul_ponticello", "con_sordino"], "articulations_proposed_new": ["detache", "martele", "loure", "portato", "sul_tasto", "flautando", "bartok_pizzicato", "left_hand_pizzicato", "slap_pizzicato", "col_legno_battuto", "col_legno_tratto", "tremolo_measured", "fingered_tremolo", "harmonics_natural", "harmonics_artificial", "trill_half", "trill_whole", "portamento", "glissando", "fortepiano", "crescendo_swell", "non_vibrato", "vibrato_on"], "articulations_provenance": "Cited S7 (VSL technique list incl. jazz slap), S2 (harmonics, chords, pizzicato), S14 (Spitfire basses patch list), S15 (Sonatina basses and solo bass), S17 (Logic Studio Strings)", "solo_or_section": "both", "typical_players": {"classical": "Estimate: about 2-4, reading the shared 'Bassi' part an octave below the cellos [shared part Cited S12, S2]", "romantic": "8-10 (full), 4-6 (medium), 2-3 (small orchestra) [Cited S1]; Wagner, Die Walkure: 8 [Cited S12]", "film": "8 as recorded for Spitfire Symphony Orchestra, all with low C extensions [Cited S14]", "chamber": "Estimate: 1 when present (string quintet, octets); no source read lists chamber works"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Source |
|---|---|---|
| English | double bass, upright bass, contrabass, string bass, bass | [Cited S11], [Cited S2] ("or, more simply, The Bass") |
| Czech | kontrabas (colloquially basa) | [Cited S22] |
| Italian | contrabbasso (VSL); Forsyth prints "Contrabasso" (OCR text, scan not viewed); older "violone" | [Cited S7], [Cited S2] |
| German | Kontrabass | [Cited S2, S7] |
| French | contrebasse | [Cited S2, S7] |
| Score | "Basses (or Contrabass or CB)"; Rimsky-Korsakov writes "D. basses", Forsyth's examples "D.-Basses" | [Cited S14, S1, S2] |

- Family: strings; the contrabass of the string section, historically closer to the viol (gamba) family: sloping
  shoulders, tuning in fourths [Cited S7].

### 2. Sound production
- Bowed (French overhand bow or German underhand bow), stopped, plucked; played standing or on a high stool
  [Cited S7]. Machine heads instead of pegs [Cited S11].
- Tuning in fourths E1, A1, D2, G2 (28, 33, 38, 43); five-string basses add B0 (sometimes C1) [Cited S7, S11].
- Thumb position above G3 [Cited S11]. Chromatic fingering repeats 2-4 between neighbouring open strings in the
  two lower octaves [Cited S2].
- Long ringing: fortissimo open strings can ring more than 10 s; stopped notes about a third shorter; the G string
  rings longest; players damp with the hand [Cited S7].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
**Transposition -12:** "the Bass is always a transposing-instrument. The actual sounds are an octave below the
written notes" (p. 436, fn.), in every clef (p. 442) [Cited S2]; notated an octave higher than it sounds [Cited
S7, S11]. Exception: harmonics are sometimes written at actual pitch (often in treble clef) [Cited S2, S7].
Clefs: bass regularly, tenor for the highest register, treble occasionally (harmonics; and in Classical-era solo
passages, where it sounded two octaves lower) [Cited S2, S7]. Spitfire: basses read bass clef and sound an octave
lower [Cited S14].

| Item | Sounding | MIDI | Written | Source |
|---|---|---|---|---|
| Lowest, four strings | E1 | 28 | E2 | [Cited S7, S11] |
| Lowest, C extension (four semitones: Eb, D, Db, C) | C1 | 24 | C2 | [Cited S2, S11] |
| Lowest, five-string (modern usual: B0; Forsyth: C, or a newer B-natural five-string in fourths) | B0 | 23 | B1 | [Cited S7, S11, S2] |
| Highest stopped (VSL range) | G4 | 67 | G5 | [Cited S7] |
| Fingerboard top "typically near" (solo) | D5 | 74 | D6 | [Cited S11] |
| Highest harmonic (VSL) | G6 | 91 | (harmonics often at pitch) | [Cited S7] |
| Classical orchestral parts rarely exceed | E1-G3 | 28-55 | E2-G4 | [Cited S11] |
| Occasional in standard repertoire; Prokofiev Kije solo | A3; D4/Eb4 | 57; 62/63 | | [Cited S11] |
| Rimsky-Korsakov practical extreme (ms004.png: bass clef, note on third ledger line = written G4) | G3 | 55 | G4 | [Cited S1] |
| Forsyth ensemble upper limit (scan p. 443: tenor clef third ledger line = written D5, printed "sounding" D4) | D4 | 62 | D5 | [Cited S2] |
| Forsyth pizzicato "working upward limit" (scan p. 451: bass clef third ledger line = written G4) | G3 | 55 | G4 | [Cited S2] |
| Below written D3 the bass "lose[s] in expression" (ms006.png: bass clef middle line) | D2 | 38 | D3 | [Cited S1] |
| Spitfire section range (all players with low C extension) | C1-F#3 | 24-54 | | [Cited S14] |

Per-string ranges (VSL, sounding) [Cited S7]: B B0-F#2 (23-42); E E1-B2 (28-47); A A1-E3 (33-52); D D2-D4 (38-62);
G G2-G4 (43-67).

- Low-note practice: most professional orchestral players use four strings with a C extension (fingered,
  wooden "fingers" or a lever "machine") [Cited S11]; Forsyth (1914) found five-strings in French and Belgian
  orchestras, and advised "keep the Basses up" in ensembles [Cited S2]; Wagner's Rheingold Eb pedal asks half
  the basses to retune [Cited S2]. Classical "Bassi" parts below E1 were and are re-arranged up an octave
  [Cited S2, S11].
- Colour: B and E strings full and dark, pitch of the lowest notes hard to perceive; A string clearer; D and G
  strings share the cello's range but sound fuller and darker [Cited S7]; lacks high partials, first six partials
  prominent [Cited S7]. Rimsky-Korsakov: duller on E and A, more penetrating on D and G [Cited S1].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Least agile of the strings; "dull in tone and of little flexibility" for melody [Cited S1]; nonetheless capable
  of virtuoso passages in modern orchestral writing [Cited S7, S2].
- Pizzicato must be much slower than on violins [Cited S1]; high pizzicato becomes dry and snappy [Cited S2].
- Open strings have limited dynamic potential [Cited S7]. Large finger spacing and high string tension make
  playing physically demanding [Cited S11].
- Chords: possible but rarely advisable; better written divisi, except fifths and octaves with an open string
  [Cited S2].
- Natural harmonics are particularly easy on the bass (better on higher strings) [Cited S2].
- Mute: a heavy, well-fitting mute matters most on violas, cellos and basses; Corder gives 2 lb as the minimum
  weight of a bass mute [Cited S2].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
- Technique list as for the other strings (detache, martele, spiccato, staccato, louré, portato, col legno,
  pizzicato incl. Bartok, left-hand pizzicato, tremolo bowed/measured/fingered, harmonics, ponticello/tasto,
  portamento, glissando, con sordino, vibrato pizzicato) [Cited S7]. Jazz: slap bass, walking bass [Cited S7].
  New id `slap_pizzicato` = jazz pizzicato in which the string is pulled so it slaps the fingerboard on the beat
  [Cited S7 for the technique; the id is new].
- Sautille and ricochet are left out of this list [Estimate: VSL's bass page repeats the generic bowing list;
  no source read shows them as idiomatic orchestral bass strokes].
- Libraries: Spitfire Basses: Long, Flautando, Harmonics, Sul Pont (+ distorted), Super Sul Tasto, Marcato Attack,
  Shorts (Spiccato, Spicc-Pizz, Staccato Dig, 0'5, 1'0, Col Legno, Harmonics, Pizzicato, Bartok), measured and
  unmeasured tremolo, m2/M2 trills, no con sordino patches [Cited S14]; Sonatina: Basses = sustain, marcato,
  staccato, legato, pizzicato, col legno, tremolo (no harmonics); Solo Bass adds harmonics [Cited S15]; VSCO 2 CE:
  contrabass sustain non-vibrato, sustain vibrato (+ quiet), tremolo, spiccato, pizzicato, KS keys C6-E6 (84-88),
  samples centred on 28-59 and mapped at sounding pitch over keys 24-60 [Cited S16, S18]; Logic Studio Strings:
  Double Bass, like the full "Sections" patch, gets keyswitches 24-33 only (no falls, scoops, crescendos,
  glissandi; those are marked for violins, violas and cellos only) [Cited S17].
- Library mapping is at sounding pitch in every library inspected (VSCO keys 24-60; Spitfire range C1-F#3)
  [Cited S16, S14]: Sonara must convert written notes by -12 before playing them.

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- Section: 8-10 (full), 4-6 (medium), 2-3 (small) [Cited S1]; 8 in VSL's large orchestra and Spitfire [Cited S7,
  S14].
- Seating: in an arc behind the cellos, standing or on stools, one or two rows [Cited S12].
- Divisi in octaves sounds dull (few high partials) [Cited S7]. Strauss's Also sprach Zarathustra uses four desks of basses, the
  lowest part for five-string or C-extension basses [Cited S2].
- A double bass solo in the orchestra is "practically unknown" [Cited S1]; for solo playing strings are often tuned a whole
  tone higher (scordatura) [Cited S7].

### 7. Role in the orchestra and typical doublings
- Fundamental bass; rarely independent except pedal notes; usually in octaves or unison with cellos, or doubling
  bassoons [Cited S1]. Bass pizzicato under cello arco an octave above is common [Cited S7].
- Classical era: one "Bassi" part shared with the cellos, sounding an octave lower; independent parts become
  common from Beethoven and Rossini on [Cited S12, S2].
- Violas two octaves above reinforce the bass's partials; cellos + basses in octaves are the classic bass
  [Cited S7]. Bass clarinet and contrabassoon came to support the four-string bass's weaker sound [Cited S7].

### 8. Variants and related instruments (when a player changes instrument)
- Three-string bass of the 18th-19th centuries (tuned in fourths, occasionally in fifths), more powerful,
  clearer and harder but shorter in the low register [Cited S2, S7]; violone [Cited S2, S7]; Vuillaume's octobass [Cited S2].
- Five- and six-string basses (low B0 and/or high C3); fifths tuning C1-G1-D2-A2 (Red Mitchell, Joel Quarrington);
  classical "solo tuning" a whole tone higher, F#1-B1-E2-A2; A1-D2-G2-C3 is a much less common Eastern European
  solo-tuning variant [Cited S11].
- Electric upright bass and travel basses [Cited S11]. Orchestral bassists may switch to bass guitar in some
  modern scores [Estimate: no source read covers it].

### 9. General MIDI and existing Sonara metadata
- GM Level 1 program 44 "Contrabass" [Cited S13]; GM program 33 "Acoustic Bass" is a separate (jazz/pop) sound
  [Cited S13 for the name; its use for upright jazz bass is an Estimate: the spec defines no timbres].
- data.rs: `double_bass`, transposition -12, playable 24-67 as an Estimate, comfortable 28-55 cited to Wikipedia,
  13 `BOWED` articulations as an Estimate [Cited S23].

### Verification of data.rs
| Field | data.rs today | Verified value | Verdict (confirmed / corrected / still estimate) | Source |
|---|---|---|---|---|
| playable range | 24-67 (Estimate) | 23-67: low B0 (five-string; C1 = 24 with a C extension), top G4 stopped | corrected (low end); top confirmed and now cited | [Cited S7, S11] |
| comfortable range | 28-55 (Cited Wikipedia) | 28-55: E1-G3; Rimsky-Korsakov's written G4 and Forsyth's pizz limit agree; Forsyth's arco ensemble limit is D4 (62) | confirmed | [Cited S11, S1, S2] |
| transposition | -12 | -12 (written an octave above sounding) | confirmed | [Cited S2, S7, S11] |
| articulations | 13 BOWED ids, Estimate | all 13 documented (Sonatina basses lack harmonics, Spitfire basses lack con sordino patches) | confirmed (provenance now Cited) | [Cited S7, S14, S15] |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence | Double bass coverage | Source |
|---|---|---|---|
| VSCO 2 CE | CC0 1.0 | section ("BKCtbss" samples): sustain NV, sustain VB (+ quiet), tremolo, spiccato, pizzicato, KS; keys 24-60 | [Cited S16] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 | Basses (7 articulations) and Solo Bass (6) | [Cited S15] |
| Spitfire Symphony Orchestra | proprietary | 8 players, 24 patch entries, C1 extension | [Cited S14] |

## Open questions and disagreements between sources
0. **Conventions used in this card.**
   - `sounding_playable` = the stopped (fingered) range stated by the VSL Academy page of the instrument
     (S4-S7, one source and one convention for all four), extended only when a handbook documents a
     higher written note in the repertoire (cello: Widor's concerto, S3). Harmonic ceilings are listed in
     section 3, not in `playable`. Wikipedia-only extremes (violin C8, bass D5) are recorded as
     disagreements [Estimate: editorial rule of this card, chosen for consistency across the group].
   - `sounding_comfortable` = the handbook upper limit for ordinary orchestral writing (S1, S2, S3), the
     value at least two sources support where possible [Estimate: editorial rule of this card].
   - Pitches that S1 and S2 print only as notation were read from the page images; each such reading
     names the clef and ledger-line position so it can be checked [Estimate: my reading of the scans;
     the scans themselves are cited].
1. **Violin top (playable).** VSL: A7 = 105 stopped, D8 = 110 harmonic [Cited S4]; Wikipedia: up to C8 = 108 by
   stopping at the fingerboard end and/or artificial harmonics [Cited S8]. Recommended 105 for consistency with the
   other three cards (VSL stopped range); harmonics belong to a separate check [Estimate: editorial rule].
2. **Violin comfortable top.** Forsyth and Rimsky-Korsakov: A6 = 93 [Cited S1, S2]; Piston via Wikipedia: E7 = 100
   "practical limit for orchestral parts" [Cited S8]; Spitfire section: C#7 = 97 [Cited S14]. Recommended 93 as
   "comfortable", with 100 remaining playable and inside the orchestral extreme [Estimate: the two handbooks define
   the ordinary limit, Piston's value is the extreme].
3. **Rimsky-Korsakov transcription.** The Gutenberg text gives violin "A7", viola "A5"; the HTML alt text gives "A7"
   for both [Cited S1]. The notated images read as A6 (violin, treble clef, above the fourth ledger line) and A5
   (viola, alto clef, on the fourth ledger line) [Estimate: my reading of ms001.png and ms002.png]. The A6 reading
   matches Forsyth's "top A" [Cited S2]. A human should confirm against the 1922 print.
   Fact-check (second, independent reading of the same images, enlarged 5x): ms001 = treble clef, notehead in the
   space above the fourth ledger line, no 8va = A6 (93); ms002 = alto clef, fourth ledger line through the notehead
   = A5 (81); ms003 = bass clef, space above the third ledger line = A4 (69); ms004 = bass clef, third ledger line
   through the notehead = written G4, sounding G3 (55); ms005 = treble clef, third ledger line = E6 (88); ms006 =
   bass clef, middle line, "(written sound)" = written D3, sounding D2 (38) [Estimate: reading of the images]. The
   glyph in ms001 is the same "note above the fourth ledger line" pattern as Forsyth's printed "top A" (p. 306),
   which supports A6. RK adds that higher notes "given in Table A" need caution [Cited S1], so the practical
   limit must lie below Table A's violin top; `table_a.png` shows the violin E-string run continuing several notes
   above that point and ending, after a dotted line (presumably 8va), on a harmonic [Estimate: the 600 px image is
   too coarse to name the last note]. The transcription "A7" is therefore most likely an octave error.
4. **Viola comfortable top.** Rimsky-Korsakov A5 = 81 [Cited S1]; VSL characteristic range to G5 = 79 [Cited S5];
   Forsyth: three-octave ordinary compass to about C6 = 84, maximum E6 = 88 [Cited S2]; Spitfire F#6 = 90
   [Cited S14]. Recommended 81 (the conservative handbook value, between VSL and Forsyth) [Estimate: editorial].
5. **Viola French name.** Forsyth "Fr. Alto" and Wikipedia "the French use the term alto" [Cited S2, S9]; VSL lists
   "French, italian: viola" [Cited S5]. Recommended "alto".
6. **Cello top.** VSL stopped A5 = 81 [Cited S6]; Spitfire section Bb5 = 82 [Cited S14]; Widor's solo concerto E6 = 88
   [Cited S3]; data.rs had A6 = 93 (Estimate) [Cited S23]. Recommended 88; virtuoso repertoire may go higher, but no
   source read documents it.
7. **Cello comfortable top.** Widor and Forsyth E5 = 76 [Cited S2, S3]; Rimsky-Korsakov A4 = 69 [Cited S1].
   Recommended 76 (two sources).
8. **Double bass low end.** Four-string E1 = 28; C extension C1 = 24 (most professional orchestral players per
   Wikipedia) [Cited S11]; five-string B0 = 23 (VSL: "has now become indispensable") [Cited S7]. Recommended
   playable 23 for the instrument type; a project could later carry a per-instrument "lowest string" setting
   [Estimate: design suggestion].
9. **Double bass top.** VSL G4 = 67 [Cited S7] vs Wikipedia fingerboard top D5 = 74 [Cited S11]. Recommended 67 for
   orchestral/section use; solo works above 67 will be flagged AbovePlayable.
10. **Double bass comfortable top.** Wikipedia E1-G3 and Rimsky-Korsakov G3 = 55 [Cited S11, S1] vs Forsyth's arco
    ensemble limit D4 = 62 [Cited S2]. Recommended 55.
11. **Forsyth's cello pizzicato limit** "top Bb" has no octave in the OCR; the original research did not check the
    page scan and used Widor's A4 instead [Cited S3]. The fact-check below resolves the octave.
    Fact-check: the p. 429 scan (Ex. 266, Humperdinck) ends the cello passage on a note in the space above the
    third ledger line of the bass clef (four sharps), i.e. the A4 position; the text calls it "top Bb", so the
    value is Bb4/A#4 = 70, one semitone above Widor's A4 = 69 [Estimate: my reading of the scan; the text is Cited
    S2]. Resolved: the two handbooks agree within a semitone.
12. **Legato in the Sonara sampler.** Sonatina's legato patches rely on `trigger=first` / `trigger=legato`
    [Cited S15]; Sonara's parser treats every trigger except `release` as an attack [Cited S23], and VSCO 2 CE has no
    legato patch [Cited S16]. So "the strings play a legato melody" cannot be rendered as true legato with the free
    SFZ libraries today; it needs `trigger=first/legato` support or a plugin-hosted library.
13. **Dynamics on CC1.** Spitfire and Sonatina performance patches put dynamics on CC1 and vibrato on CC21
    [Cited S14, S15]; GM1 defines CC1 only as "modulation" and CC11 as expression [Cited S13]; the Sonara sampler
    plays CC7/CC11 but no CC-driven crossfades [Cited S23]. An agent-facing "dynamics" control will need a
    per-library mapping.
14. **Classical-era section sizes and chamber line-ups** for this card are Estimates: no source read gives
    Haydn/Mozart string counts.
15. **Score abbreviations.** Spitfire uses Vn, Va, VC, CB [Cited S14]; the card uses "Vn.", "Va.", "Vc.", "Cb."
    [Estimate: common engraving style, not checked against a style guide].

## Fact-check log
_Adversarial fact-check, 2026-10-30. Every source below was reopened in this session: VSL Academy pages through the
content endpoint (`https://www.vsl.co.at/vcms/pages/academy/strings/<instrument>`, full text grepped); Wikipedia
wikitext (`?action=raw`) for Violin, Viola, Cello, Double bass, String section and the four Czech pages; the
Gutenberg plain text, HTML alt texts and images `ms001`-`ms006`, `table_a` of S1; Forsyth OCR text and page images
pp. 306, 383, 415, 429, 439, 443, 451 (S2); the S3 Widor caption page; the GM1 PDF (S13); the Spitfire manual
(S14, text extracted with macOS PDFKit); Sonatina README and SFZ files (S15); VSCO 2 CE KS files, tree and LICENSE
(S16); the Logic iPad keyswitch table parsed by column (S17); data.rs and sfz.rs in the repository (S23)._

**A. Key numeric fields (6 per instrument, 24 checked, 24 confirmed, 0 changed in the json).**

| Instrument | Field | Value in card | Check | Verdict |
|---|---|---|---|---|
| violin | sounding_playable | 55-105 | VSL "Range of the violin: G3 - A7 (harmonic D8)"; G3 = 55, A7 = 105 | confirmed |
| violin | sounding_comfortable | 55-93 | Forsyth p. 306 "top A" glyph = treble clef, space above 4th ledger = A6 = 93; RK ms001 same pattern = A6 | confirmed (RK text "A7" is a transcription error, open question 3) |
| violin | transposition / written | 0 / 55-105 | VSL "non-transposing ... treble clef" | confirmed |
| violin | clefs | treble | VSL | confirmed |
| violin | GM | 41 Violin | GM1 PDF Table 2 | confirmed |
| viola | sounding_playable | 48-93 | VSL "C3 - A6 (harmonic E7)"; C3 = 48, A6 = 93 | confirmed |
| viola | sounding_comfortable | 48-81 | RK text "A5"; ms002 alto clef, 4th ledger line through note = A5 = 81 | confirmed |
| viola | transposition / written | 0 / 48-93 | VSL | confirmed |
| viola | clefs | alto, treble | VSL "alto clef, and in treble clef from the second octave above middle C"; Forsyth p. 383 | confirmed |
| viola | GM | 42 Viola | GM1 PDF | confirmed |
| cello | sounding_playable | 36-88 | Widor caption: C2 "lowest note possible", concerto "up to an E6" = 88; VSL C2-A5 (harm. A7) | confirmed (88 is solo repertoire; see concerns) |
| cello | sounding_comfortable | 36-76 | Widor caption "C2 and E5 ... highest note recommended ... orchestral"; Forsyth p. 415 treble clef top space = E5 = 76 | confirmed |
| cello | transposition / written | 0 / 36-88 | VSL; old "treble clef at the octave" noted (VSL, Forsyth p. 415) | confirmed |
| cello | clefs | bass, tenor, treble | VSL | confirmed |
| cello | GM | 43 Cello | GM1 PDF | confirmed |
| double_bass | sounding_playable | 23-67 | VSL "B0 - G4 (harmonic G6)"; B0 = 23, G4 = 67 | confirmed |
| double_bass | sounding_comfortable | 28-55 | Wikipedia "rarely ... exceed ... E1 to G3"; RK ms004 written G4 = sounding G3 = 55 | confirmed |
| double_bass | transposition | -12 | Forsyth p. 436 fn. "always a transposing-instrument ... an octave below"; p. 442 "Whatever the register or clef the written part remains an octave above"; VSL | confirmed |
| double_bass | written_playable | 35-79 | = sounding + 12 (B1-G5) | confirmed |
| double_bass | clefs | bass, tenor, treble | Forsyth p. 442 (bass; tenor for the highest register; treble for harmonics) | confirmed |
| double_bass | GM | 44 Contrabass | GM1 PDF | confirmed |
| all | GM extras | 45, 46, 49, 50, 33; group 41-48 "Strings"; CC1/7/10/11/64; bend +/-2; key 60 | GM1 PDF pp. 3, 5 | confirmed |

Structure: all four json blocks parse, carry exactly the 20 required keys in order, `names` has the six keys,
family `strings`, ids `violin`, `viola`, `cello`, `double_bass`, `existing_sonara_id` matches data.rs (viola
null), every `articulations_existing_vocab` id is in the 15-id vocabulary, no proposed id duplicates it, and the
invented ids (`detache`, `martele`, `sautille`, `ricochet`, `loure`, `left_hand_pizzicato`, `fingered_tremolo`,
`su_una_corda`, `behind_the_bridge`, `slap_pizzicato`) each have a one-line definition. Written = sounding minus
transposition in all four blocks.

Conversion sweep: every note-name/MIDI pair in the card body (68 table rows and 63 inline pairs, computed as
(octave + 1) x 12 + pitch class, C4 = 60) was recomputed by script; no conversion error was found (the only
mismatches flagged were rows that deliberately pair a written pitch with its sounding MIDI number, which are
correct). The SFZ convention c4 = 60 (S18, not reopened) is consistent with the files: the VSCO violin files
start at `g3` = 55, the open G, and the cello files at `c2` = 36, the open C [Cited S16].

**B. Other Cited claims spot-checked (36 checked: 31 confirmed as written, 4 corrected, 1 open question resolved).**

| # | Claim | Source reopened | Verdict |
|---|---|---|---|
| 1 | Spitfire: Vn I 16 / Vn II 14 / Va 12 / Vc 10 / Cb 8 players; ranges G3-C#7, G3-C#7, C3-F#6, C2-Bb5, C1-F#3 (scientific naming: violins start on G3) | S14 | confirmed |
| 2 | Spitfire Performance Legato: slow 1-19 portamento, 20-84 fingered, 85-127 bowed | S14 | confirmed |
| 3 | Spitfire CC11 expression within CC7; CC21 vibrato; CS Blend = half of each desk muted; "Exactly the same instruments as the 1sts" | S14 | confirmed |
| 4 | Spitfire patch counts Vn I 33, Vn II 29, Va 29, Cb 24, Vc 32 ("about 30"); basses have no CS (con sordino) patches | S14 | confirmed |
| 5 | Spitfire basses all with low C extension; basses read bass clef and sound an octave lower | S14 | confirmed |
| 6 | Logic Studio Strings keyswitches 24-41; 34-41 marked only for "Violins, Violas, Cellos" | S17 (table parsed by column) | confirmed; note added that the "Sections" patch also lacks 34-41 |
| 7 | Sonatina: licence CC Sampling Plus 1.0; string articulation lists; Basses without harmonics; Solo Bass with harmonics | S15 README | confirmed |
| 8 | Sonatina: notation = velocity volume + CC1 marcato attack; performance = CC1 volume; CC21 vibrato except solo violins and solo cello | S15 README | confirmed |
| 9 | Sonatina 1st Violins Legato uses `trigger=first` / `trigger=legato`; 1st violins sustain mapped g3-c7 (55-96) | S15 SFZ files | confirmed |
| 10 | Sonara sfz.rs maps every trigger except `release` to attack | S23 `crates/sonara-media/src/sfz.rs` lines 389-391 | confirmed |
| 11 | VSCO 2 CE: ViolinEns 55-86, SViolin 55-96, ViolaEns 48-86, CelloEns 36-77, Contrabass 24-60 (keycentres 28-59); KS c2-d#2 = 36-39 (sus vib, trem, spic, pizz); cello KS 84-87 with 4 velocity layers; contrabass KS 84-88; CC0 | S16 | confirmed |
| 12 | RK section table 16/14/12/10/8-10, 12/10/8/6/4-6, 8/6/4/3/2-3; larger orchestras 20-24 first violins; divisi by desks 1,3,5 / 2,4,6; Vns II + violas preferred | S1 text | confirmed |
| 13 | RK: viola A string "more biting ... slightly nasal"; bass duller on E and A; bass "rarely plays an independent part"; solo viola "seldom found", bass solo "practically unknown" | S1 text | confirmed |
| 14 | Wikipedia String section: Walküre 16-16-12-12-8; "Bassi" shared part; outside player takes upper divisi part; alternating desks favoured in threefold divisi; 19th-century antiphonal seating Vn I, Vc, Va, Vn II; basses in an arc behind the cellos | S12 | confirmed |
| 15 | Wikipedia Violin: G3 lowest; D3 by scordatura "on rare occasions"; E7 "practical limit for orchestral violin parts" (Piston); up to C8; 13th position; seven strings maximum | S8 | confirmed |
| 16 | Wikipedia Double bass: E1 / C1 / B0; fingerboard top near D5; E1-G3 Classical parts, occasional A3, Kijé D4/Eb4; thumb position above G3; most professional orchestral players use a C extension; machine tuners | S11 | confirmed |
| 17 | Wikipedia Cello: fractional sizes identical in "construction, range, and usage"; cello choir, William Tell | S10 | confirmed |
| 18 | Widor captions: regular fingering to B4; pizz limit about A4; harmonics C4-A5; arpeggiated pizz both directions; four "bad" G-string notes; Meistersinger sul ponticello tremolo | S3 | confirmed |
| 19 | VSL violin: positions 1-7 low, 8-11 high, up to 14th in solo; sautillé only p/mp; ricochet p-mf; fingered tremolo third, up to diminished fifth; mute reduces partials over 2000 Hz; soprano of the violin family | S4 | confirmed |
| 20 | VSL viola: per-string ranges; characteristic range C3-G5; 5th position on lower strings, 8th (11th) on A; German "Viola", "French, italian: viola"; "austere charm"; violins + violas need cellos in unison | S5 | confirmed |
| 21 | VSL cello: thumb position "widespread use since 1740", from 7th position; quadruple stops piano usually divisi; violoncello piccolo fifth string E4; section 10 | S6 | confirmed |
| 22 | VSL double bass: open strings ring > 10 s ff; slap bass in jazz; solo scordatura a whole tone up; divisi in octaves dull; violas two octaves above; bass clarinet and contrabassoon support; B0 "has now become indispensable"; first six partials prominent | S7 | confirmed |
| 23 | Forsyth: positions p. 306; double stops above 3rd position "practically unknown in the orchestra"; fingered tremolo limit augmented fourth; easy keys Eb-E major, C-B minor; pizz to arco "about half a second" | S2 | confirmed |
| 24 | Forsyth viola: names "Fr. Alto, It. Viola, Ger. Bratsche", "(or Tenor)"; C and G strings covered; "betwixt-and-between"; wolf notes; "answers so well to the mute"; E6 limit, last 3-4 semitones dispensable, harmonic A6 | S2 pp. 381-384 | confirmed |
| 25 | Forsyth viola alto-clef limit "up to about" | S2 p. 383 image | **corrected**: D5 (74) -> F5 (77) |
| 26 | Forsyth cello: four fingers serve three diatonic notes; command of "only a major third"; E5 orchestral limit p. 415, harmonic A5 | S2 | confirmed |
| 27 | Forsyth bass: five-string with bottom C (French and Belgian orchestras) and newer B-natural five-stringer (p. 439 image); C extension Eb-D-Db-C; Rheingold Eb pedal, half the basses retune; "keep the Basses up"; Corder 2 lb mute; Zarathustra fugue four desks, lowest part for five strings or C mechanism; ensemble limit written D5 = sounding D4 (p. 443, tenor clef); pizz limit written G4 (p. 451) | S2 | confirmed |
| 28 | Forsyth related instruments: violino piccolo three-quarter size a minor third higher; Vuillaume octobass; Ritter viola alta, violotta; three-string bass | S2 | confirmed |
| 29 | Czech names: housle; viola (archaic "bráč"); violoncello (colloquially cello); kontrabas (colloquially basa) | S19-S22 | confirmed; spelling corrected to "bráč" |
| 30 | Viola section 6: "12 in VSL's large orchestra [Cited S5]" | S5, S4, S6, S7 | **corrected**: the VSL viola page has no section list; re-cited to S4, S6, S7 |
| 31 | Cello section 4: "Double stops: most common interval a third; up to a fourth, or a fifth with the thumb [Cited S6]" | S6 | **corrected**: VSL says this of the fingered tremolo; double stops are best in fourths, fifths, sixths, widest an octave (eleventh in solo literature with the thumb) |
| 32 | Double bass section 8: "solo tuning A1-D2-G2-C3 [Cited S11]" | S11 | **corrected**: standard solo tuning is F#1-B1-E2-A2; A-D-G-C is a "much less-commonly used" Eastern European variant |
| 33 | Violin chamber "string quartet = two violins, viola, cello [Cited S5]" | S5, S8 | confirmed (S5 states the four-part two violins / viola / cello ensemble); S8 added as the more direct source |
| 34 | Forsyth cello pizz "top Bb" (open question 11) | S2 p. 429 image | resolved to Bb4/A#4 = 70 by scan reading (tagged Estimate) |
| 35 | data.rs violin 55-108 / 55-100 cited Wikipedia; cello 36-93 Estimate, no comfortable; double_bass -12, 24-67 Estimate, 28-55 cited; all three `BOWED` (13 ids) with `ARTICULATION_ESTIMATE`; no viola entry | S23 data.rs lines 44-58, 81-134; orchestration-metadata.md line 75 | confirmed: all three "Verification of data.rs" tables quote data.rs correctly |
| 36 | RK "practical extreme limit" images ms001-ms006 | S1 images | confirmed the original researcher's readings (A6, A5, A4, written G4, E6, written D3) with a second independent reading |

**C. Changes made (was -> now).** 1. Viola section 3: Forsyth alto-clef limit "about D5 (second ledger line)" ->
"about F5 = 77 (third ledger line)", plus the E5-upwards treble-clef note. 2. Viola section 6: "[Cited S5, S14]" ->
"[Cited S4, S6, S7, S14]". 3. Cello section 4: the third/fourth/fifth intervals moved from double stops to a new
fingered-tremolo line; double stops restated from VSL. 4. Cello section 4 and open question 11: Forsyth "top Bb"
resolved to Bb4 = 70 [Estimate: scan reading]. 5. Double bass section 8: solo tuning restated. 6. Double bass
section 5: Logic note extended to the "Sections" patch. 7. Viola section 1: "brac" -> "bráč". 8. Violin json
`typical_players.chamber`: "[Cited S5]" -> "[Cited S5, S8]". 9. Open question 3: second reading of all six RK
images and the Table A argument added. 10. Sources row S2: pages re-read listed. No json range, transposition,
clef or GM value changed.

**D. Downgraded to Estimate:** none. Every Cited claim that was checked is stated by its source (four needed a
re-citation or rewording, listed above). Claims not individually re-checked (for example the VSL per-string
colour adjectives and the Rimsky-Korsakov and VSL blend remarks in section 7) remain as the original researcher
tagged them.

**E. Remaining concerns.**
- RK's violin limit: the Gutenberg text and alt text print "A7", the image reads A6; two readers agree on A6, but a
  check against the 1922 print or the Russian original is still advisable.
- Viola `sounding_comfortable` top 81 rests on one handbook value (RK A5); VSL's characteristic range ends at 79,
  Forsyth's ordinary compass at about 84 and his maximum at 88.
- Cello `sounding_playable` top 88 rests on one solo example (Widor's concerto, E6); VSL's stopped range ends at 81.
  It is a solo-repertoire value, not an orchestral one.
- Violin playable top 105 (VSL stopped) vs data.rs 108 (Wikipedia, fingerboard end or artificial harmonics) and
  double bass 23 (five-string B0) vs 24 (C extension) are editorial conventions of this card, not source conflicts.
- Spitfire's manual names notes with C4 = middle C (inferred from violins starting on G3), while Logic names MIDI 24
  "C0" (middle C = C3); downstream agents must convert by MIDI number, never by name.

## Consistency review
_Consistency review, 2026-10-30: json against sections 3 and 9, the "Verification of data.rs" tables and the Fact-check
log; written = sounding minus transposition; tag hygiene; quote length; paths._

- `double_bass` json `articulations_proposed_new`: added `loure`. Section 5 lists louré in the VSL bass technique
  list [Cited S7] and excludes only sautille and ricochet. This supersedes the Fact-check log's "0 changed in the
  json", which describes the file before this review.
- Sources row S23: the absolute local checkout path was replaced by "paths relative to the repository root".
- Open question 11: the opening sentence no longer says the scan "was not checked". It now says the original research
  did not check it and that the fact-check paragraph resolves the octave.
- Violin section 1: removed a reference to the research brief; the Estimate now names the id convention of these
  cards.
