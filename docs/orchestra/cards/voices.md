# Voices: choir and solo voice: instrument cards
_Researched 2026-10-30. MIDI note numbers, C4 = 60 = middle C. Ranges are sounding unless marked written.
Transposition = sounding minus written, in semitones._
## Sources
| # | Source | URL | Kind (handbook, manufacturer, library maker, spec, encyclopedia, other) | Opened in this session |
|---|---|---|---|---|
| S1 | N. Rimsky-Korsakov, _Principles of Orchestration_ (ed. Maksimilian Shteinberg, trans. Edward Agate; Gutenberg text of the 1964 Dover reprint of the 1922 English translation, per its title page and transcriber's notes), Chapter V and Chapter VI "Voices", incl. Table F (chorus and soloists) as engraved images `table_f_chorus.png`, `table_f_soloists.png` | https://www.gutenberg.org/ebooks/33900 (text: https://www.gutenberg.org/cache/epub/33900/pg33900.txt; images: https://www.gutenberg.org/files/33900/33900-h/images/table_f_chorus.png, .../table_f_soloists.png) | handbook | yes (full text of Ch. V-VI read; Table F images read at 6-10x zoom) |
| S2 | H. Berlioz, _A Treatise on Modern Instrumentation and Orchestration_, trans. Mary Cowden Clarke (Novello, 1882), chapter "Voices" (OCR text; the music staves are lost in OCR, only prose is used) | https://archive.org/details/treatiseonmodern00berl (text: https://archive.org/download/treatiseonmodern00berl/treatiseonmodern00berl_djvu.txt) | handbook | yes (prose of the Voices chapter) |
| S3 | Wikipedia, "Voice type" | https://en.wikipedia.org/wiki/Voice_type | encyclopedia | yes |
| S4 | Wikipedia, "Fach" | https://en.wikipedia.org/wiki/Fach | encyclopedia | yes (first 100,000 characters) |
| S5 | Wikipedia, "Tenor" | https://en.wikipedia.org/wiki/Tenor | encyclopedia | yes |
| S6 | Wikipedia, "Alto" | https://en.wikipedia.org/wiki/Alto | encyclopedia | yes |
| S7 | Wikipedia, "Bass (voice type)" | https://en.wikipedia.org/wiki/Bass_(voice_type) | encyclopedia | yes |
| S8 | Wikipedia, "SATB" | https://en.wikipedia.org/wiki/SATB | encyclopedia | yes |
| S9 | Wikipedia, "Choir" | https://en.wikipedia.org/wiki/Choir | encyclopedia | yes (first 100,000 characters) |
| S10 | Wikipedia, "Chamber choir" | https://en.wikipedia.org/wiki/Chamber_choir | encyclopedia | yes |
| S11 | Czech Wikipedia, "Hlasový obor" (voice types, Czech names, Helmholtz ranges) | https://cs.wikipedia.org/wiki/Hlasový_obor | encyclopedia | yes |
| S12 | Wikipedia, "General MIDI" (program list 1-128; cites the MIDI Association's "GM 1 Sound Set" page) | https://en.wikipedia.org/wiki/General_MIDI | encyclopedia | yes |
| S13 | Wikipedia, "General MIDI Level 2" (bank variations) | https://en.wikipedia.org/wiki/General_MIDI_Level_2 | encyclopedia | yes |
| S14 | The MIDI Association, "General MIDI Level 1" (the page carries no sound table; the spec PDF is members-only) | https://midi.org/general-midi-level-1 | spec | yes |
| S15 | Wikipedia, "Humming" | https://en.wikipedia.org/wiki/Humming | encyclopedia | yes |
| S16 | Sound On Sound, review of Spitfire Audio _Eric Whitacre Choir_ | https://www.soundonsound.com/reviews/spitfire-audio-eric-whitacre-choir | other (magazine review of a library) | yes |
| S17 | Spitfire Audio, _Originals Epic Choir_ product page | https://www.spitfireaudio.com/originals-epic-choir | library maker | yes |
| S18 | EastWest, _Hollywood Choirs_ product page | https://www.soundsonline.com/hollywood-choirs | library maker | yes |
| S19 | MusicTech, review of EastWest _Hollywood Choirs_ | https://musictech.com/reviews/eastwest-hollywood-choirs-review/ | other (magazine review) | yes |
| S20 | Epicomposer, review of EastWest/Quantum Leap _Hollywood Choirs_ (2018-03-29) | https://epicomposer.com/east-west-quantum-leap-hollywood-choirs-review/ | other (review) | yes |
| S21 | Wikipedia, "Symphonic Choirs" (EastWest library, WordBuilder) | https://en.wikipedia.org/wiki/Symphonic_Choirs | encyclopedia | yes |
| S22 | Soundiron, _Voices of Rapture_ collection product page (solo S, A, T, B) | https://soundiron.com/collections/all-products/products/voices-of-rapture | library maker | yes |
| S23 | Sonatina Symphonic Orchestra (SSO): README (instrument list, licence) and the Chorus SFZ files `Chorus - Notation/Mixed Chorus.sfz`, `Large Chrous.sfz`, `Chorus - Performance/Mixed Chorus.sfz`, `includes/mixed-chorus.sfz`, plus the sample file tree | https://github.com/peastman/sso | library maker | yes |
| S24 | VSCO 2 Community Edition, file tree of branch `SFZ` (GitHub API) | https://github.com/sgossner/VSCO-2-CE/tree/SFZ | library maker | yes (searched for choir/vocal files: none) |
| S25 | Versilian Community Sample Library (VCSL), file tree (GitHub API) | https://github.com/sgossner/VCSL | library maker | yes (top-level categories: Aerophones, Chordophones, Electrophones, Idiophones, Membranophones; no vocal category) |
| S26 | Sonara repository: `docs/orchestral-audio-engine-research.md` section 5 and `docs/sample-libraries.md` | (read-only checkout of the Sonara repository; paths relative to its root) | other (project docs) | yes |
| S27 | Sonara repository: `crates/sonara-midi/src/orchestration/data.rs`, `articulation.rs`, `instrument.rs` (grep), `crates/sonara-midi/src/gm.rs`, `docs/orchestration-metadata.md`, `crates/sonara-media/src/sfz.rs` line 298 | (read-only checkout of the Sonara repository; paths relative to its root) | other (project code) | yes |
| S28 | EastWest, _Hollywood Choirs_ user manual (PDF mirror) | https://www.bax-shop.nl/downloads/products/9000-0047-1517/ew-hollywood-choirs-user-manual.pdf | library maker | no (download returned an error page and an empty file; never cited) |
| S29 | S. Adler, _The Study of Orchestration_ | (copyrighted print) | handbook | no (not readable; never cited) |
| S30 | Spitfire LABS "Choir" page | https://labs.spitfireaudio.com/choir (redirects to splice.com) | library maker | no (redirect not followed; never cited) |

Reading notes that apply to every card below:

- **Table F (S1) tiers.** Rimsky-Korsakov prints, for every voice, a "normal" bracket, solid "except." brackets
  and dotted extensions. For the chorus he defines them: "The exceptional register may be considered as two notes
  above and below the normal octave. The dotted lines extended still further indicate the limits upon which a
  composer may rely in very exceptional cases" [Cited S1]. For soloists: "A bracket under the notes defines the
  normal octave ... Within these limits the composer may write freely ... the notes above it are exceptional and
  should be used for the culminating points" [Cited S1]. The engraving shows no accidentals [Cited S1], so a printed
  B may stand for B or Bb [Estimate: the table is diatonic, accidentals are not printed].
- **One mapping rule for all ten cards.** `sounding_playable` = the outer limits (dotted lines) of Table F, widened
  only where Berlioz (S2, the second handbook) states a wider limit for the same voice in prose.
  `sounding_comfortable` = Table F's normal bracket plus the solid exceptional brackets (dotted part excluded); this
  matches data.rs, where `comfortable` is the limit of ordinary parts, not a tessitura [Cited S27]. The bare normal
  octave (the tessitura) is given in section 3 of each card. Wikipedia, Fach and Czech values are recorded for
  comparison and never widen a range.
- **Reading accuracy.** Note counts and ledger lines were checked against stem direction (stems turn down from the
  middle line); every chorus normal bracket spans exactly an octave and every chorus solid bracket exactly two notes,
  as S1's text requires. The solid/dotted boundary of the soloists' upper brackets is read at 8-10x zoom and, after
  the fact-check, by a pixel calibration against the chorus rows (see "Fact-check log"); it is uncertain by one
  diatonic step where a card says so.
- **Octave names.** Helmholtz (Czech S11, older texts): C = C2 = 36, c = C3 = 48, c1 = C4 = 60, c2 = C5 = 72,
  c3 = C6 = 84; contra A1 = A1 = 33. SFZ note names in Sonara's parser: "`c4` is 60" [Cited S27], so SSO's sample
  names are read in that convention.
- **Clef slug `treble_8vb`:** a treble clef with an 8 below; it sounds an octave lower than written, "the standard
  clef for tenor parts in scores" [Cited S5]. On a shared tenor/bass staff both parts are in bass clef [Cited S8] and
  the octave transposition then does not apply.
- **New articulation ids used in this file** (same spelling in every card): `vowel`, `humming`, `crescendo_swell`,
  `sforzando`, `vibrato_on`, `non_vibrato`, `portamento`, `falsetto` (male voices only) from the shared list, and two
  invented ids: `text` = singing words or syllables (lyrics) rather than one sustained vowel; `whisper` = unvoiced,
  breath-only singing or speaking.

## choir_soprano: Sopranos (choir section)

```json
{"id": "choir_soprano", "family": "choir",
 "names": {"en": "Sopranos (choir)", "cs": "Soprány (sbor)", "it": "Soprani (coro)", "de": "Sopran (Chor)", "fr": "Sopranos (chœur)", "score_abbreviation": "S."},
 "pitched": true, "transposition": 0, "clefs": ["treble"],
 "sounding_playable": [60, 84], "sounding_comfortable": [62, 81],
 "range_provenance": {"playable": "Cited S1: Table F, Chorus, Soprano, outer (dotted) limits C4-C6 read from the Gutenberg engraving table_f_chorus.png; Berlioz S2 gives C4-Bb5, inside it", "comfortable": "Cited S1: Table F normal octave F4-F5 plus the solid exceptional brackets D4-E4 and G5-A5"},
 "written_playable": [60, 84], "gm_program": 53, "gm_program_name": "Choir Aahs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento"],
 "articulations_provenance": "Cited S1 (legato, staccato: 'the rapid change from staccato to legato'; vowels), S15 (humming), S16 (looped sustains, legato, 'mmm', breathy ooh, non-vib to vib), S19 and S20 (staccato, legato, WordBuilder text), S21 (whispers); Estimate for tenuto, marcato, sforzando, crescendo_swell, portamento: general vocal notation practice, no read source lists them for choirs",
 "solo_or_section": "section",
 "typical_players": {"classical": "Estimate: 4-10 per part; Baroque choirs were 'usually quite small' (Cited S9)", "romantic": "Cited S1: full chorus 32 per part, medium 16-20, small 8-10; women often predominate and the firsts get more voices", "film": "Cited S17: Epic Choir 50 singers in two sections (S+A, T+B); Cited S16: Eric Whitacre Choir 22 singers SATB", "chamber": "Cited S10: a chamber choir has roughly 8 to 40 singers in total"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | sopranos, 1st/2nd sopranos | [Cited S2] ("1st Soprano", "2nd Soprano") |
| Czech | soprán (S); section: soprány | [Cited S11] for "Soprán (S)"; plural and "sbor" [Estimate: standard Czech] |
| Italian | soprani | [Cited S2] (Berlioz's example is labelled "Soprani. Tenori. Bassi."; the OCR reads "Basai") |
| German | Sopran | [Cited S4] (Fach names "Lyrischer Sopran" etc.); "Chor" [Estimate: standard German] |
| French | sopranos (chœur) | [Estimate: standard French; no French source read] |

- Score: the four elemental chorus types are "soprano, alto or contralto, tenor and bass", sub-divided into "firsts"
  and "seconds" as "Sopr. I, Sopr. II" [Cited S1]; the SATB abbreviation letters are S, A, T, B [Cited S8].
- Family slug `choir`; the Sonara enum already has `Family::Choir` with slug "choir" [Cited S27].

### 2. Sound production
- The voice has registers: "chest voice, middle and head voice in women; chest voice, mixed voice and falsetto in
  men"; Rimsky-Korsakov leaves their exact limits to singing teachers [Cited S1].
- Vowels shape the tone: "In women's voices the easiest vowel on high notes is a" [Cited S1].
- Humming is "a wordless tone with the mouth closed, forcing the sound to emerge from the nose" [Cited S15].
- A chorus sustains without gaps because singers need not breathe together: "each singer may take a slight rest from
  time to time" [Cited S1]; library makers record long notes the same way ("some singers pause for a breath while
  others continue the note") [Cited S16].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier (Table F, chorus soprano) | Notes | MIDI | Source |
|---|---|---|---|
| very exceptional (dotted) | C4; B5-C6 | 60; 83-84 | [Cited S1] |
| exceptional (solid) | D4-E4; G5-A5 | 62-64; 79-81 | [Cited S1] |
| normal octave (tessitura) | F4-F5 | 65-77 | [Cited S1] |
| Berlioz, school range of the soprano | C4-Bb5 | 60-82 | [Cited S2] |
| Berlioz, second sopranos ("all the notes will sound well") | B3-G5 | 59-79 | [Cited S2] |
| Czech "soprán" (c1-c3) | C4-C6 | 60-84 | [Cited S11] |
| Wikipedia, solo soprano "typical" | C4-C6 | 60-84 | [Cited S3] |

- "The range of choral voices is slightly more limited than that of soloists" [Cited S1].
- Firsts and seconds: "the normal octave and the exceptionally low register should be allotted to the 'seconds', the
  same octave and the exceptionally high register to the 'firsts'" [Cited S1].
- Non-transposing; treble clef [Cited S1] (Table F prints the chorus soprano in treble clef).
- Register breaks: no read source gives a numeric passaggio for sopranos; S1 declines to fix register limits
  [Cited S1].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- High voices made to sing low notes "lose ... all their power, and their distinctive quality of tone" but do not
  produce bad sounds; second sopranos forced too high do [Cited S2].
- Soft sustained Andante: "use only notes of the medium" which "can be sustained without the least effort in a
  pianissimo"; brilliant or violent choruses are written higher, but without constant high notes [Cited S2].
- Agility: "Voices possess the greatest amount of flexibility in their normal octave"; women's voices are more supple
  than men's and sopranos are the most agile [Cited S1]. Choral melody is more limited than solo melody in range and
  mobility; "Not more than two or three notes should be written on one syllable except for fanciful and whimsical
  effects" [Cited S1].
- Hard for every voice: "Intervals bigger than fourths in quick succession and chromatic scales are extremely
  difficult"; skips of an octave or more from a short note should be avoided [Cited S1].
- Breath: choristers stagger breathing, so pauses for breath are less critical than for soloists [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Musical evidence | Library implementation (source) |
|---|---|---|
| sustain / vowel | vowel sustains; choice of vowel matters on long high notes [Cited S1] | looped vowel sustains; EastWest Hollywood Choirs (EW HC) vowel patches crossfade 3 dynamic layers (mp, mf, ff) with the mod wheel CC1 [Cited S20]; vowels Oo, Oh, Ah, Ee [Cited S19]; SSO Mixed Chorus: one looped sample per semitone [Cited S23] |
| legato | "extended melodic figures sung legato on one syllable" need care [Cited S1] | Eric Whitacre Choir (EWC): monophonic legato patches slide between overlapping notes up to an octave apart; pitch bend only in legato patches [Cited S16]; EW HC: legato only on Ah and Oh, two speeds [Cited S19]; "polyphonic legato" [Cited S20] |
| staccato | "rapid change from staccato to legato" is possible [Cited S1] | EW HC staccato variations inside the vowel patches [Cited S19], two round robins [Cited S20]; Spitfire Epic Choir "Short Staccato Syllables", also as a keyswitch preset [Cited S17] |
| humming | wordless, mouth closed [Cited S15] | EWC 'mmm', 'Breathy mmms', 'Mmm aah mmm' [Cited S16]; Epic Choir "Long Mmm" [Cited S17]; GM2 program 54 bank 1 "Humming" [Cited S13] |
| text | words and syllables; few notes per syllable in chorus [Cited S1] | EW HC WordBuilder: words typed "in plain English, phonetically or in Votox", preset phrases in English, German, Italian, Spanish and Latin [Cited S19]; 781 pre-built phrases [Cited S18]; EW Symphonic Choirs WordBuilder [Cited S21]; EWC has no word builder by design [Cited S16] |
| whisper | (no handbook read) | EW Symphonic Choirs: solo whispers (soprano, alto, boy), not playable through WordBuilder [Cited S21]; EWC "Breathy ooh" [Cited S16] |
| vibrato_on / non_vibrato | (no handbook read) | EWC "Nonvib to vib" style [Cited S16]; EW HC: CC1 raises dynamics and vibrato together, not separately [Cited S20] |
| crescendo_swell, sforzando | [Estimate: notated dynamics, not a separate patch] | rendered with the dynamics controller (EW HC CC1 [Cited S20]; Epic Choir "Expression" and "Dynamics" faders [Cited S17]) |
| tenuto, marcato, portamento | [Estimate: general vocal notation] | no read library documents a dedicated choir patch for them |

- Articulation switching in EWC: keyswitches, MIDI CC, velocity or playing speed, and per-patch MIDI channels
  [Cited S16].
- Sonara's sampler plays velocity layers, keyswitches, release samples, CC 64/7/11 and pitch bend, but no CC-driven
  layers or crossfades [Cited S26], so mod-wheel vowel crossfades (EW HC, SSO Performance) cannot play in it.

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- Size: full chorus about 32 per part, medium 16-20, small 8-10; "The number of women will often predominate, and
  more voices are given to the 'firsts' than to the 'seconds'" [Cited S1]. Chamber choir: roughly 8 to 40 singers in
  total [Cited S10].
- Divisi: "One part may be divided into as many as three different parts, 3 sopranos, 3 altos etc." [Cited S1];
  "SSAATTBB, where each voice is divided into two parts" [Cited S9]; a women's choir is usually soprano and alto,
  "two parts in each" [Cited S9]. Berlioz recommends six-part choruses with first and second sopranos [Cited S2].
- Unison: "Dividing kindred voices in octaves is seldom done" (Sopr. I/Sopr. II) [Cited S1].

### 7. Role in the orchestra and typical doublings
- Melody: "one melodic part may predominate (the upper part for preference)" [Cited S1]; altos may be added to the
  sopranos to strengthen the melody [Cited S1].
- Octaves: sopranos with tenors are "the most beautiful and natural" octave pairing [Cited S1].
- Doubling: women's voices are most naturally doubled in unison "by violins, violas, clarinets and oboes";
  trombones and trumpets overpower the voice [Cited S1]; "Doubling choral parts by instruments is generally a good
  plan" [Cited S1].
- Placement: in symphonic choirs the choir often stands behind the orchestra, highest voices on the left [Cited S9].

### 8. Variants and related instruments (when a player changes instrument)
- Children: "Children's voices are of excellent effect in large choirs" [Cited S2]; a boys' choir is a separate
  sampled section in EW Symphonic Choirs [Cited S21]; "treble" names a young singer of either sex whose voice has
  not yet changed and who sings in the mezzo-soprano range (originally boy sopranos) [Cited S3].
- Mezzo-sopranos sit with second sopranos or first altos [Cited S1].

### 9. General MIDI and existing Sonara metadata
- GM1 assigns sound names to program numbers but does not define their acoustics [Cited S14]. Program 53
  "Choir Aahs", 54 "Voice Oohs (or Doos)", 55 "Synth Voice or Synth Choir", numbered 1-128 [Cited S12]. GM2 bank-1
  variations: 53 "Choir Aahs 2", 54 "Humming", 55 "Analog Voice" [Cited S13].
- Card uses 53 Choir Aahs: the only GM program that names a choir [Cited S12].
- Sonara: no voice or choir instrument exists in data.rs (16 instruments) and docs list "voices" as not covered
  [Cited S27]; `gm.rs` holds only the channel-10 percussion map [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage for sopranos | Plays in Sonara sampler? |
|---|---|---|---|
| Sonatina Symphonic Orchestra, "Chorus" (Mixed, Large) | CC Sampling Plus 1.0 [Cited S23]; bundling unresolved [Cited S26] | female samples g4-c6 (67-84), one per semitone, looped, vowel not named [Cited S23] | Notation patch: yes, but its velocity filter is ignored (filters unsupported); Performance patch needs CC1 crossfades: no [Cited S23, S26] |
| VSCO 2 CE | CC0 [Cited S26] | no choir or vocal samples in branch SFZ [Cited S24] | n/a |
| VCSL | CC0 [Cited S26] | no vocal category [Cited S25] | n/a |
| FluidR3_GM, MuseScore_General | MIT, notices [Cited S26] | GM sets of 128 programs [Cited S26], so program 53 [Estimate: a complete GM set includes it] | no, SF2 is not played [Cited S26] |
| Spitfire Eric Whitacre Choir | commercial (price not read) | SATB tutti and per-section mics, vowels aah/ooh/oh/mmm [Cited S16] | no: proprietary player [Cited S26] |
| Spitfire Originals Epic Choir | £/$/€29 perpetual or Splice subscription, needs the INSTRUMENT plugin [Cited S17] | Sopranos & Altos section [Cited S17] | no [Cited S26] |
| EastWest Hollywood Choirs | commercial (price not read) | female choir; reliable "up to a Top Bb" [Cited S19] | no [Cited S26] |

## choir_alto: Altos (choir section)

```json
{"id": "choir_alto", "family": "choir",
 "names": {"en": "Altos (choir)", "cs": "Alty (sbor)", "it": "Contralti (coro)", "de": "Alt (Chor)", "fr": "Altos (chœur)", "score_abbreviation": "A."},
 "pitched": true, "transposition": 0, "clefs": ["treble"],
 "sounding_playable": [53, 77], "sounding_comfortable": [55, 74],
 "range_provenance": {"playable": "Cited S1, S2: Table F, Chorus, Contralto, outer limits G3-F5 (dotted E5-F5) read from table_f_chorus.png; low end widened to F3 by Berlioz's contralto 'from the low F to the high Eb' (F3-Eb5); Wikipedia's choral alto F3-F5 (S6) agrees", "comfortable": "Cited S1: Table F solid exceptional G3-A3, normal octave B3-B4, solid exceptional C5-D5"},
 "written_playable": [53, 77], "gm_program": 53, "gm_program_name": "Choir Aahs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S15 (humming), S16 (sustains, legato, 'mmm', non-vib to vib), S19 and S20 (staccato, legato, WordBuilder text), S21 (whispers by altos); Estimate for tenuto, marcato, sforzando, crescendo_swell, portamento: general vocal notation practice",
 "solo_or_section": "section",
 "typical_players": {"classical": "Estimate: 4-10 per part; Baroque choirs were 'usually quite small' (Cited S9)", "romantic": "Cited S1: full chorus 32 per part, medium 16-20, small 8-10", "film": "Cited S17: Epic Choir records sopranos and altos as one section of a 50-voice choir; Cited S16: Eric Whitacre Choir 22 singers SATB", "chamber": "Cited S10: roughly 8 to 40 singers in total"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | altos; contraltos | [Cited S1] (chorus types "soprano, alto or contralto, tenor and bass") |
| Czech | alt (A); kontraalt (cA) is a separate, lower type | [Cited S11]; plural "alty" [Estimate: standard Czech] |
| Italian | contralto | [Cited S2] (Berlioz discusses "the Italian word Contralto"); plural "contralti" [Estimate] |
| German | Alt | [Cited S4] ("Dramatischer Alt", "Tiefer Alt") |
| French | altos (chœur) | [Estimate: no French source read] |

- Score: "Altos", "Altos I / Altos II" in Rimsky-Korsakov's divisi tables [Cited S1]; letter A in SATB [Cited S8].
- Family `choir` [Cited S27].
- "the terms alto and contralto are not interchangeable"; many choral altos "would more accurately be called
  mezzo-sopranos" [Cited S6].

### 2. Sound production
- Women's registers: chest, middle and head voice [Cited S1]; Berlioz counts the contralto among the "female voices,
  or high voices" (women, children, artificial sopranos) [Cited S2].
- The alto part may be sung by contraltos, mezzo-sopranos and countertenors together, "three vocal timbres" on the
  same notes [Cited S6].
- Humming: wordless tone, mouth closed, sound through the nose [Cited S15].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier (Table F, chorus contralto) | Notes | MIDI | Source |
|---|---|---|---|
| very exceptional (dotted) | (none below); E5-F5 | 76-77 | [Cited S1] |
| exceptional (solid) | G3-A3; C5-D5 | 55-57; 72-74 | [Cited S1] |
| normal octave (tessitura) | B3-B4 | 59-71 | [Cited S1] |
| Berlioz, contralto (school theory) | F3-Eb5 | 53-75 | [Cited S2] |
| Wikipedia, choral alto (the range caption carries "citation needed") | F3-F5 | 53-77 | [Cited S6] |
| Czech "alt" (f-f2) | F3-F5 | 53-77 | [Cited S11] |

- Disagreement: S1's lowest choral alto note is G3, S2 and S6 give F3; the card takes F3 for `playable` by the
  mapping rule (Berlioz widens) and keeps G3 as the comfortable limit.
- Above F5 Berlioz divides the second sopranos again "in order that the contr'altos may not be forced to scream out
  notes too high for them" [Cited S2].
- Non-transposing; treble clef in Table F [Cited S1].
- Register breaks: no numeric passaggio in any read source [Cited S1] (S1 declines to fix register limits).

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Weakness: in Paris, "the contralto part ... would be so weak as to miss the greater portion of the effects
  assigned to it"; contralto voices are plentiful in Italy, scarce in France [Cited S2].
- Contraltos in their medium "become lost" when doubling tenors in the high register [Cited S2].
- Women's voices are more supple than men's; flexibility is greatest in the normal octave [Cited S1].
- Choral melody: at most two or three notes per syllable except for whimsical effects [Cited S1].
- Women's voices suffer more than men's from orchestral harmony in their own register [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Library implementation (source) |
|---|---|
| sustain / vowel | EW HC vowel loops, mod wheel CC1 crossfades mp/mf/ff [Cited S20]; EWC looped long notes with staggered breathing [Cited S16]; SSO: female samples start at g4 (67), so alto notes below G4 play the male samples in "Mixed Chorus" [Cited S23] |
| legato | EWC monophonic legato on the main vowels, overlapping notes up to an octave apart [Cited S16]; EW HC legato only on Ah and Oh [Cited S19] |
| staccato | EW HC staccato variations [Cited S19]; Epic Choir "Short Staccato Syllables" (separate and keyswitch presets) [Cited S17] |
| humming | EWC 'mmm' styles [Cited S16]; Epic Choir "Long Mmm" [Cited S17] |
| text | EW HC WordBuilder (plain English, phonetic, Votox) [Cited S19]; Symphonic Choirs WordBuilder for the SATB sections [Cited S21] |
| whisper | EW Symphonic Choirs solo whispers include alto [Cited S21] |
| vibrato_on / non_vibrato | EWC "Nonvib to vib" [Cited S16] |
| crescendo_swell, sforzando, tenuto, marcato, portamento | [Estimate: notated dynamics and accents, rendered with the dynamics controller; no dedicated alto patch read] |

- EW Symphonic Choirs' SATB sections offer "Normal, Legato, Staccato, and Slurred" articulations [Cited S21].
- Sonara's sampler has no CC crossfades [Cited S26], so dynamics would need velocity layers or CC7/CC11 volume.

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- Full chorus about 32 per part, medium 16-20, small 8-10 [Cited S1].
- Divisi: a three-part women's chorus is "Sopr. I/Sopr. II/Altos or Sopr./Altos I/Altos II" [Cited S1]; up to three
  alto parts [Cited S1]; in Berlioz's six-part plan "the true contralto voices ... must necessarily sing the second
  soprano part" [Cited S2].
- Unison pairings: sopranos and altos "produce ample and vigorous tone"; altos and tenors give "a peculiar mixed tone
  quality, somewhat bizarre and seldom used" [Cited S1].

### 7. Role in the orchestra and typical doublings
- Harmony and inner voice; altos may join the sopranos to strengthen a melody [Cited S1].
- Octaves: altos with basses are one of "the most beautiful and natural combinations" [Cited S1].
- Doubling: women's voices by violins, violas, clarinets and oboes [Cited S1].
- Placement: often behind the orchestra in symphonic choirs [Cited S9].

### 8. Variants and related instruments (when a player changes instrument)
- Countertenors (male altos) may sing the part [Cited S6]; Berlioz's "haute-contre" was a high male tenor, not a
  female voice [Cited S2].
- Mezzo-sopranos are classed with second sopranos or first altos [Cited S1].

### 9. General MIDI and existing Sonara metadata
- GM 53 "Choir Aahs" (1-based) [Cited S12]; GM1 names programs without defining acoustics [Cited S14]; GM2 bank-1
  variation of 54 is "Humming" [Cited S13].
- No choir or voice metadata in data.rs; voices listed as not covered [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage for altos | Plays in Sonara sampler? |
|---|---|---|---|
| SSO "Chorus" | CC Sampling Plus 1.0 [Cited S23] | no separate alto section: female g4-c6 and male g2-f#4 samples [Cited S23] | Notation patch yes (filter ignored), Performance patch no [Cited S23, S26] |
| VSCO 2 CE, VCSL | CC0 [Cited S26] | no vocal samples [Cited S24, S25] | n/a |
| GM SoundFonts (FluidR3_GM, MuseScore_General) | MIT [Cited S26] | program 53 [Estimate: complete GM set] | no [Cited S26] |
| Spitfire EWC | commercial | alto section mic [Cited S16] | no [Cited S26] |
| Spitfire Epic Choir | £/$/€29 [Cited S17] | Sopranos & Altos combined [Cited S17] | no [Cited S26] |
| EW Hollywood Choirs | commercial | women's choir only, no separate altos [Cited S20] | no [Cited S26] |

## choir_tenor: Tenors (choir section)

```json
{"id": "choir_tenor", "family": "choir",
 "names": {"en": "Tenors (choir)", "cs": "Tenory (sbor)", "it": "Tenori (coro)", "de": "Tenor (Chor)", "fr": "Ténors (chœur)", "score_abbreviation": "T."},
 "pitched": true, "transposition": -12, "clefs": ["treble_8vb", "bass"],
 "sounding_playable": [48, 70], "sounding_comfortable": [50, 69],
 "range_provenance": {"playable": "Cited S1, S2: Table F, Chorus, Tenor, printed in treble clef as written C4-A5 = sounding C3-A4 (read from table_f_chorus.png; the -12 octave reading is an Estimate: inference from the treble-8vb clef convention in S5); top widened to Bb4 by Berlioz (tenor 'up to the A and Bb above'; 'the Bb must be fixed as the limit of a tenor singer's chest voice')", "comfortable": "Cited S1: Table F solid exceptional D3-E3, normal octave F3-F4, solid exceptional G4-A4 (sounding)"},
 "written_playable": [60, 82], "gm_program": 53, "gm_program_name": "Choir Aahs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento", "falsetto"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S2 (head voice / falsetto for tenors), S15 (humming), S16 (sustains, legato, 'mmm'), S19 and S20 (staccato, legato, WordBuilder text); Estimate for tenuto, marcato, sforzando, crescendo_swell, portamento, whisper: general vocal notation practice",
 "solo_or_section": "section",
 "typical_players": {"classical": "Estimate: 4-10 per part; Baroque choirs were 'usually quite small' (Cited S9)", "romantic": "Cited S1: full chorus 32 per part, medium 16-20, small 8-10; men usually fewer than women", "film": "Cited S17: Epic Choir records tenors and basses as one section of a 50-voice choir; Cited S16: Eric Whitacre Choir 22 singers SATB", "chamber": "Cited S10: roughly 8 to 40 singers in total"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | tenors; 1st/2nd tenors | [Cited S2] |
| Czech | tenor (T) | [Cited S11]; plural "tenory" [Estimate: standard Czech] |
| Italian | tenori | [Cited S2] (example labelled "Soprani. Tenori. Bassi."; the OCR reads "Basai") |
| German | Tenor | [Cited S4] ("Lyrischer Tenor", "Heldentenor") |
| French | ténors (chœur) | [Estimate: no French source read] |

- Score: "Ten. I / Ten. II" [Cited S1]; letter T in SATB [Cited S8]. Family `choir` [Cited S27].

### 2. Sound production
- Men's registers: "chest voice, mixed voice and falsetto" [Cited S1].
- Head notes (falsetto) "are of great beauty in tenor singers", reaching "Eb and F above the stave" (read as Eb5,
  F5, because the same text places the tenor's chest limit Bb4 "above" the tenor-clef stave), but choristers rarely
  use them [Cited S2].
- "In women's voices the easiest vowel on high notes is a, for men it is o" [Cited S1].
- Berlioz praises "mixed" and "veiled" emission as valuable in solo and choral singing [Cited S2].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier (Table F, chorus tenor; printed in treble clef, converted -12) | Written | Sounding | MIDI sounding | Source |
|---|---|---|---|---|
| very exceptional (dotted) | C4 | C3 | 48 | [Cited S1] |
| exceptional (solid) | D4-E4; G5-A5 | D3-E3; G4-A4 | 50-52; 67-69 | [Cited S1] |
| normal octave (tessitura) | F4-F5 | F3-F4 | 53-65 | [Cited S1] |
| Berlioz, school range ("C below the stave (C clef on fourth line) up to the A and Bb above") | | C3-A4/Bb4 | 48-70 | [Cited S2] |
| Wikipedia, choral tenors "approximately B2 up to A4" | | B2-A4 | 47-69 | [Cited S5] |
| Czech "tenor" (c-c2) | | C3-C5 | 48-72 | [Cited S11] |

- Transposition: tenor parts are written in treble clef with an 8 below, sounding an octave lower [Cited S5]; on its
  own staff the tenor "is usually written in treble clef marked an octave down", on a shared staff with the basses in
  bass clef [Cited S8]. Table F itself prints the tenor in plain treble clef [Cited S1]; written C4-A5 can only be a
  tenor range if it sounds an octave lower [Estimate: inference from the clef convention in S5].
- Berlioz wrote the tenor in C clef on the fourth line [Cited S2] (historical; not used by this card).
- Limits: tenors reaching A and Bb are "common in France and Italy, but are rarer in Germany" [Cited S2]; "of twenty
  basses taken at random, ten of them, at least, could give, in chest voice, a high F#; while in the same number of
  tenors, one would not be found who could give, in chest voice, a tolerably good high C" [Cited S2].
- Second tenors are the most limited voices: their low D, C and B "would have scarcely any sonority" [Cited S2].
- Above the playable top: when a melody leads the first tenors to the B ("a very dangerous note, and one to be
  shunned"), Berlioz brings in second sopranos and contraltos in unison [Cited S2]. The OCR garbles the accidental
  ("BJJs"); it is read as B natural, B4 [Estimate: the same chapter fixes Bb4 as the chest-voice limit].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- The higher voice in each sex is the more agile: "the tenor voice in men" [Cited S1].
- Upper register: tenors forced to A and Bb outside their means "give utterance to notes false, forced, and of bad
  quality" [Cited S2].
- Low register: tenors descending too low are supported by the first basses, which keeps the vocal character
  [Cited S2].
- Choral melody is limited in range and mobility; two or three notes per syllable at most [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Library implementation (source) |
|---|---|
| sustain / vowel | EW HC men's choir vowel loops (Oo, Oh, Ah, Ee) [Cited S19], CC1 crossfade of 3 dynamic layers [Cited S20]; SSO male samples g2-f#4 (43-66), so G4-A4 fall on female samples in "Mixed Chorus" [Cited S23] |
| legato | EWC legato patches: overlapping notes up to an octave apart [Cited S16]; EW HC legato on Ah and Oh, two speeds [Cited S19] |
| staccato | EW HC staccato inside vowel patches [Cited S19]; Epic Choir "Short Staccato Syllables" for Tenors & Basses [Cited S17] |
| humming | Epic Choir "Long Mmm" [Cited S17]; EWC 'mmm' [Cited S16] |
| text | EW HC WordBuilder; consonant patches Mm, D, RR (tongue roll) [Cited S19] |
| falsetto | no read library documents a choral falsetto patch [Estimate] |
| vibrato_on / non_vibrato | EWC "Nonvib to vib" [Cited S16] |
| crescendo_swell, sforzando, tenuto, marcato, portamento, whisper | [Estimate: notated, rendered with the dynamics controller] |

- Sonara's sampler: velocity layers, keyswitches, CC 64/7/11; no CC crossfades [Cited S26].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- Sizes as in the other choir cards: 32 / 16-20 / 8-10 per part [Cited S1].
- Divisi: men's three-part chorus "Ten. I/Ten. II/Bass or Ten./Bass I/Bass II" [Cited S1]; a men's chorus is
  "usually consisting of two tenors, baritone, and bass" [Cited S9]; Berlioz's six-part chorus has first and second
  tenors [Cited S2].
- In a close forte chord S1 recommends 2 tenors doubling the upper parts an octave lower: "the tenors sing in the
  soprano octave" [Cited S1].
- Unison: tenors and basses "produce ample and vigourous tone" [Cited S1].

### 7. Role in the orchestra and typical doublings
- Octaves with sopranos: "the most beautiful and natural" combination [Cited S1].
- Doubling: men's voices by "violas, 'cellos, bassoons and horns" [Cited S1].
- Help across the range: second sopranos and contraltos can join the tenors in unison on dangerous high notes
  [Cited S2].
- Placement: behind the orchestra in symphonic choirs; men often at the back in a cappella settings [Cited S9].

### 8. Variants and related instruments (when a player changes instrument)
- Baritones are classed with second tenors or first basses [Cited S1].
- Haute-contre: a high tenor singing the top of the first tenor's compass in chest voice [Cited S2].
- Countertenors sing alto in falsetto [Cited S5, S6].

### 9. General MIDI and existing Sonara metadata
- GM 53 "Choir Aahs" [Cited S12]; GM1 does not define the acoustics [Cited S14].
- No voice metadata in data.rs; voices not covered [Cited S27]. The `Transposition` field already models octave
  transposition (double bass -12) [Cited S27], so -12 fits the existing type.

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage for tenors | Plays in Sonara sampler? |
|---|---|---|---|
| SSO "Chorus" | CC Sampling Plus 1.0 [Cited S23] | male samples g2-f#4 (43-66), looped, no separate tenor section [Cited S23] | Notation patch yes (filter ignored); Performance patch no [Cited S23, S26] |
| VSCO 2 CE, VCSL | CC0 [Cited S26] | none [Cited S24, S25] | n/a |
| GM SoundFonts | MIT [Cited S26] | program 53 [Estimate: complete GM set] | no [Cited S26] |
| Spitfire EWC | commercial | tenor section mic [Cited S16] | no [Cited S26] |
| Spitfire Epic Choir | £/$/€29 [Cited S17] | Tenors & Basses combined [Cited S17] | no [Cited S26] |
| EW Hollywood Choirs | commercial | men's choir (tenors and basses combined) [Cited S20] | no [Cited S26] |

## choir_bass: Basses (choir section)

```json
{"id": "choir_bass", "family": "choir",
 "names": {"en": "Basses (choir)", "cs": "Basy (sbor)", "it": "Bassi (coro)", "de": "Bass (Chor)", "fr": "Basses (chœur)", "score_abbreviation": "B."},
 "pitched": true, "transposition": 0, "clefs": ["bass"],
 "sounding_playable": [36, 66], "sounding_comfortable": [43, 62],
 "range_provenance": {"playable": "Cited S1, S2: Table F, Chorus, Bass, outer (dotted) limits C2-E4 read from table_f_chorus.png; top widened to F#4 by Berlioz ('ten of them, at least, could give, in chest voice, a high F#', used by Gluck for all basses); the C2 bottom is corroborated by Berlioz's North-German and Russian basso profondo 'D's and C's below the stave' (S2)", "comfortable": "Cited S1: Table F solid exceptional G2-A2, normal octave B2-B3, solid exceptional C4-D4"},
 "written_playable": [36, 66], "gm_program": 53, "gm_program_name": "Choir Aahs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento", "falsetto"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S2 (head notes of basses only in light buffo style), S15 (humming), S16 (sustains, legato, 'mmm'), S19 and S20 (staccato, legato, WordBuilder text); Estimate for tenuto, marcato, sforzando, crescendo_swell, portamento, whisper: general vocal notation practice",
 "solo_or_section": "section",
 "typical_players": {"classical": "Estimate: 4-10 per part; Baroque choirs were 'usually quite small' (Cited S9)", "romantic": "Cited S1: full chorus 32 per part, medium 16-20, small 8-10", "film": "Cited S17: Epic Choir records tenors and basses as one section of a 50-voice choir; Cited S16: Eric Whitacre Choir 22 singers SATB", "chamber": "Cited S10: roughly 8 to 40 singers in total"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | basses; 1st basses (baritones), 2nd basses | [Cited S2] |
| Czech | bas (B); oktávista (O) for the lowest Russian type | [Cited S11]; plural "basy" [Estimate: standard Czech] |
| Italian | bassi; basso profondo, basso cantante | [Cited S2] ("Bassi" in the example; the OCR reads "Basai"), [Cited S1] (basso cantante, basso profondo) |
| German | Bass | [Cited S4] ("Jugendlicher Bass", "Spielbass") |
| French | basses (chœur); "contrebasses" for Russian octavists | [Cited S1] (translator's footnote on "Contrebasses voices"); "basses" [Estimate] |

- Score: "Basses I / Basses II" [Cited S1]; letter B in SATB [Cited S8]. Family `choir` [Cited S27].

### 2. Sound production
- Men's registers: chest, mixed, falsetto [Cited S1]. Head notes of basses and baritones suit only "an extremely
  light style of music, such as that of our French comic operas" and are never used in a noble chorus [Cited S2].
- Vowels: "The vowel i softens the penetrating quality of the top notes of a bass voice, and the vowel a adds to the
  extension of range in the very lowest compass" [Cited S1].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier (Table F, chorus bass, bass clef) | Notes | MIDI | Source |
|---|---|---|---|
| very exceptional (dotted) | C2-F2; E4 | 36-41; 64 | [Cited S1] |
| exceptional (solid) | G2-A2; C4-D4 | 43-45; 60-62 | [Cited S1] |
| normal octave (tessitura) | B2-B3 | 47-59 | [Cited S1] |
| Berlioz, school range ("from F below the stave (F clef) up to D and Eb above") | F2-D4/Eb4 | 41-63 | [Cited S2] |
| Berlioz, second basses' extreme low ("the Eb, and even the D below the stave") | Eb2, D2 | 39, 38 | [Cited S2] |
| Berlioz, chest high F# for half of the basses | F#4 | 66 | [Cited S2] |
| Czech "bas" (E-e1); "oktávista" (A1-a) | E2-E4; A1-A3 | 40-64; 33-57 | [Cited S11] |
| Wikipedia, bass voice | E2-E4, low extreme C2; Russian religious music: A2 drone, in rare cases doubled by A1 | 40-64; 36; 45/33 | [Cited S7] |
| Berlioz, North-German and Russian basso profondo: "D's and C's below the stave" given "without preparation" | D2, C2 | 38, 36 | [Cited S2] |

- Octavists: "one or two bass singers may be found who are able to go still lower than the limit of the exceptional
  range (they are called octavists)"; their notes "can only be used when the whole chorus is singing quite piano"
  and are "hardly applicable except in unaccompanied choruses" [Cited S1].
- Disagreement: Berlioz also says many basses "lose much of their sonority after the low C or B, and it is useless to
  write for them either G's or F's" [Cited S2], against S1's solid G2-A2. The card keeps S1 for `comfortable`.
- Non-transposing; bass clef [Cited S1].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Very low notes: "Fine effects are always to be obtained from the extreme low notes of the second basses" when the
  singers "have time to take them firmly ... preceded by sufficient time for breath, and written upon a sonorous
  syllable"; they need slow notes and few words [Cited S2].
- "choral vocalisations in the lowest part of the scale are of detestable effect" [Cited S2].
- High register: forced notes such as high F and F# suit "rude and fierce accents" better than the tenors' natural
  notes on the same pitches, if led up to [Cited S2].
- Baritones (first basses) from low A to high G: "the low A will be heavy, and vague; and the high G excessively
  forced" [Cited S2].
- Men's voices are less supple than women's [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Library implementation (source) |
|---|---|
| sustain / vowel | EW HC men's choir vowel loops with a CC1 crossfade of mp/mf/ff [Cited S20]; SSO male samples from g2 (43), nothing lower [Cited S23] |
| legato | EWC legato: overlapping notes up to an octave apart [Cited S16]; EW HC on Ah and Oh [Cited S19] |
| staccato | Epic Choir "Short Staccato Syllables" (Tenors & Basses) [Cited S17]; EW HC [Cited S19] |
| humming | Epic Choir "Long Mmm" [Cited S17] |
| text | EW HC WordBuilder [Cited S19, S20] |
| vibrato_on / non_vibrato | EWC "Nonvib to vib" [Cited S16]; EW HC vibrato tied to CC1 [Cited S20] |
| falsetto, whisper, crescendo_swell, sforzando, tenuto, marcato, portamento | [Estimate: no dedicated choral bass patch read] |

- Sonara's sampler: no CC crossfades [Cited S26].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- 32 / 16-20 / 8-10 per part [Cited S1].
- Divisi: "Bass I / Bass II"; in a close forte chord "the 1st basses in the alto octave and the 2nd basses are
  independent" [Cited S1]. Berlioz divides basses so that the first bass takes a note "higher by a third, by a fifth,
  or by an octave, than the too-low note of the second bass" [Cited S2].
- Octaves inside the section: "except perhaps in the basses ... when ... it is required to double the bass part in
  octaves" [Cited S1].

### 7. Role in the orchestra and typical doublings
- Harmonic foundation; altos and basses in octaves are "the most beautiful and natural" [Cited S1].
- Doubling: men's voices by violas, cellos, bassoons and horns [Cited S1]. Long sustained double-bass notes with the
  voice "produce a peculiar throbbing effect" (said of solo accompaniment) [Cited S1].
- Unison with tenors gives "volume of sound ... so powerful an accent" (Gluck, _Iphigenia in Tauris_) [Cited S2].

### 8. Variants and related instruments (when a player changes instrument)
- Baritones sing first bass or second tenor [Cited S1, S2].
- Basso profondos and oktavists "are able to reach much lower than" the C2 low extreme; traditional Russian
  religious music calls for an A2 drone, in rare cases doubled by A1 [Cited S7]. Berlioz: such basses in North
  Germany and Russia are given "D's and C's below the stave" to sustain [Cited S2].

### 9. General MIDI and existing Sonara metadata
- GM 53 "Choir Aahs" [Cited S12]; acoustics undefined by GM1 [Cited S14].
- No voice metadata in data.rs [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage for basses | Plays in Sonara sampler? |
|---|---|---|---|
| SSO "Chorus" | CC Sampling Plus 1.0 [Cited S23] | male samples g2-f#4 (43-66); nothing below G2 [Cited S23] | Notation patch yes (filter ignored); Performance patch no [Cited S23, S26] |
| VSCO 2 CE, VCSL | CC0 [Cited S26] | none [Cited S24, S25] | n/a |
| GM SoundFonts | MIT [Cited S26] | program 53 [Estimate: complete GM set] | no [Cited S26] |
| Spitfire EWC | commercial | bass section mic [Cited S16] | no [Cited S26] |
| Spitfire Epic Choir | £/$/€29 [Cited S17] | Tenors & Basses combined [Cited S17] | no [Cited S26] |
| EW Hollywood Choirs | commercial | men's choir [Cited S20] | no [Cited S26] |

## voice_soprano: Soprano (solo voice)

```json
{"id": "voice_soprano", "family": "choir",
 "names": {"en": "Soprano", "cs": "Soprán", "it": "Soprano", "de": "Sopran", "fr": "Soprano", "score_abbreviation": "Sopr."},
 "pitched": true, "transposition": 0, "clefs": ["treble"],
 "sounding_playable": [60, 88], "sounding_comfortable": [62, 83],
 "range_provenance": {"playable": "Cited S1: Table F, Soloists, Soprano, outer (dotted) limits C4-E6 read from table_f_soloists.png; Wikipedia's typical C4-C6 (S3) lies inside; Fach (S4) extends to A3 (spinto, dramatic) and F6 (coloratura), recorded, not used", "comfortable": "Cited S1: Table F solid exceptional D4-E4, normal bracket F4-G5, solid exceptional A5-B5 (upper solid/dotted boundary read at 8x zoom)"},
 "written_playable": [60, 88], "gm_program": 54, "gm_program_name": "Voice Oohs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento"],
 "articulations_provenance": "Cited S1 (legato, staccato, florid passages, vowels), S15 (humming), S22 (true legato vowels, vowel staccatos, Latin syllable sustains and staccatos for a solo soprano), S21 (whispers); Estimate for tenuto, marcato, trill, sforzando, crescendo_swell, portamento, vibrato control: general vocal practice, no read source lists them",
 "solo_or_section": "solo",
 "typical_players": {"classical": "1", "romantic": "1", "film": "1 (solo libraries record one singer, Cited S22)", "chamber": "1"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | soprano | [Cited S1, S3] |
| Czech | soprán (S) | [Cited S11] |
| Italian | soprano; "soprano giusto" | [Cited S1] |
| German | Sopran (Lyrischer Sopran, Dramatischer Sopran, Koloratursopran, Soubrette) | [Cited S4] |
| French | soprano | [Estimate: no French source read] |

- Score abbreviation "Sopr." [Cited S1]. Family `choir` (the shared slug for voices) [Cited S27].
- Sub-types named by S1: "light soprano, soprano giusto, lyric soprano, dramatic soprano" and the intermediate
  "mezzo-carattere" [Cited S1]; S3 lists coloratura, soubrette, lyric, spinto and dramatic [Cited S3].

### 2. Sound production
- Women's registers: chest, middle and head voice; the teacher's work is to make the transitions imperceptible
  "on all the vowels" [Cited S1].
- Classes: "lyric" (more suppleness) and "dramatic" ("more powerful and of greater range") [Cited S1].
- Wagner created "a type of powerful dramatic soprano, of extensive range, combining the quality and scope of the
  soprano and mezzo-soprano voices" [Cited S1].
- Solo singers are classified by tessitura, choral singers by range [Cited S3].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier / source | Notes | MIDI | Source |
|---|---|---|---|
| Table F very exceptional (dotted) | C4; C6-E6 | 60; 84-88 | [Cited S1] |
| Table F exceptional (solid) | D4-E4; A5-B5 | 62-64; 81-83 | [Cited S1] |
| Table F normal bracket (tessitura) | F4-G5 (the printed bracket spans a ninth, checked at 10x zoom) | 65-79 | [Cited S1] |
| Wikipedia "typical soprano" | C4-C6 | 60-84 | [Cited S3] |
| Czech soprán (c1-c3) | C4-C6 | 60-84 | [Cited S11] |
| Fach: coloratura (lyric, dramatic) | C4-F6 | 60-89 | [Cited S4] |
| Fach: soubrette | C4-C6 | 60-84 | [Cited S4] |
| Fach: lyric | B3-C6 | 59-84 | [Cited S4] |
| Fach: spinto, full dramatic | A3-C6 | 57-84 | [Cited S4] |
| Fach: high dramatic | F3-C6 | 53-84 | [Cited S4] |

- Non-transposing; treble clef [Cited S1].
- Use of the tiers: "the notes above it are exceptional and should be used for the culminating points of a passage
  or for climaxes, the notes below, for the fall or decline of a melody" [Cited S1].
- Register breaks: S1 gives none ("the position and exact limits of register" are left to the singing teacher)
  [Cited S1].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Agility: sopranos are the most agile voices; flexibility is greatest in the normal octave [Cited S1].
- Fast passages: diatonic scales and arpeggios in thirds come easiest; wide intervals in quick succession and
  chromatic scales "are extremely difficult"; a very high note should be prepared by a step or a leap of a fourth,
  fifth or octave [Cited S1].
- Vowels: on high notes "a" is easiest for women; long florid passages are often written on "ah" [Cited S1].
- Breath: "Breath cannot be taken in the middle of a word"; the part needs rests [Cited S1].
- Fatigue: long stretches in unusual registers "will weary both singer and listener" [Cited S1].
- Balance: strings are the most transparent accompaniment, then woodwind, horns, trombones, trumpets; "a singer is
  more easily overpowered by long sustained notes than by short detached ones" [Cited S1]; women's voices suffer
  more than men's from harmony in their own register [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Musical evidence | Library implementation (source) |
|---|---|---|
| sustain / vowel | vowel choice on long high notes [Cited S1] | Soundiron Voices of Rapture (VoR) soprano: vowel sustains Ah, Eh, Ee, Mm, Oh, Oo in piano and forte [Cited S22] |
| legato | cantabile legato aria [Cited S1] | VoR "True Legato Vowels: Ah and Oo core vowels" [Cited S22] |
| staccato | "rapid change from staccato to legato" [Cited S1] | VoR single-vowel staccatos and 23 Latin "Poly-Staccatos" with round robin [Cited S22] |
| text | syllables and words, diction [Cited S1] | VoR 12 Latin "Poly-Sustains" with speed control and simulated legato [Cited S22] |
| humming | [Cited S15] | VoR "Mm" sustains and staccatos [Cited S22] |
| whisper | (no handbook read) | EW Symphonic Choirs solo soprano whispers [Cited S21] |
| trill, portamento, vibrato_on, non_vibrato, crescendo_swell, sforzando, tenuto, marcato | [Estimate: standard vocal notation] | no read solo library documents them |

- VoR offers "Articulation Key-Switching" and CC and host automation [Cited S22]; the CC numbers are not stated
  [Cited S22].
- Sonara's sampler: keyswitches and velocity layers yes, CC crossfades no [Cited S26].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- One singer. Two sopranos in one work: write "for one slightly higher than the other" [Cited S1].
- Duets: Sopr./Ten. are "related within an octave"; Sopr./M.-sopr. "related in thirds"; Sopr./Bass in twelfths
  should not approach each other [Cited S1].
- Solo with chorus: a solo female voice stands out against a male chorus; same-timbre combinations need the soloist
  _a piena voce_ and the chorus _piano_ [Cited S1].

### 7. Role in the orchestra and typical doublings
- Aria (cantabile) and recitative; florid singing "requires a light accompaniment" [Cited S1].
- Doubling in unison by violins, violas, clarinets and oboes, only for isolated phrases; trombones and trumpets
  overpower the voice [Cited S1].
- Climaxes outside the normal range should be supported "in the register in which the voice is placed" [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Fach subtypes as in section 3 [Cited S4]; treble: a young female or male singer with an unchanged voice
  (originally the boy soprano) [Cited S3].
- Mezzo-carattere voices combine lyric and dramatic qualities [Cited S1].

### 9. General MIDI and existing Sonara metadata
- No GM program for a solo singer was found [Estimate: the GM1 "Ensemble" group read in S12 (49-56) holds only
  choir and synth voices]; 54 "Voice Oohs (or Doos)" is the nearest [Cited S12]; GM2 bank 1 of 54 is "Humming"
  [Cited S13].
- No voice metadata in data.rs; "voices" not covered [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage | Plays in Sonara sampler? |
|---|---|---|---|
| Soundiron Voices of Rapture (soprano Nichole Dechaine) | $249 collection (Kontakt Player); $119 per single voice, listed as Kontakt full version; NI serial registration [Cited S22] | vowel sustains, legato, staccatos, Latin syllables [Cited S22] | no: proprietary player [Cited S26] |
| EW Symphonic Choirs | commercial | solo soprano whispers [Cited S21] | no [Cited S26] |
| SSO, VSCO 2 CE, VCSL | CC Sampling Plus 1.0 / CC0 [Cited S23, S26] | no solo voice (SSO has only "Chorus" Mixed and Large) [Cited S23, S24, S25] | n/a |
| GM SoundFonts | MIT [Cited S26] | 54 Voice Oohs [Estimate: complete GM set] | no [Cited S26] |

## voice_mezzo_soprano: Mezzo-soprano (solo voice)

```json
{"id": "voice_mezzo_soprano", "family": "choir",
 "names": {"en": "Mezzo-soprano", "cs": "Mezzosoprán", "it": "Mezzosoprano", "de": "Mezzosopran", "fr": "Mezzo-soprano", "score_abbreviation": "M.-sopr."},
 "pitched": true, "transposition": 0, "clefs": ["treble"],
 "sounding_playable": [57, 84], "sounding_comfortable": [59, 79],
 "range_provenance": {"playable": "Cited S1: Table F, Soloists, Mezzo-soprano, outer (dotted) limits A3-C6 read from table_f_soloists.png; Wikipedia's typical A3-A5 (S3) lies inside; Fach G3-B5 (S4) recorded, not used", "comfortable": "Cited S1: Table F solid exceptional B3-C4, normal octave D4-D5, solid exceptional E5-G5; the last solid note G5 read by pixel calibration against the chorus rows, whose counts S1's text fixes (fact-check log); +-1 diatonic step"},
 "written_playable": [57, 84], "gm_program": 54, "gm_program_name": "Voice Oohs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento"],
 "articulations_provenance": "Cited S1 (legato, staccato, florid passages, vowels), S15 (humming); Estimate for the rest: general vocal practice; no read library records a solo mezzo-soprano",
 "solo_or_section": "solo",
 "typical_players": {"classical": "1", "romantic": "1", "film": "1", "chamber": "1"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | mezzo-soprano | [Cited S1, S3] |
| Czech | mezzosoprán (mS) | [Cited S11] |
| Italian | mezzosoprano | [Estimate: standard Italian] |
| German | Mezzosopran (Koloratur-, Lyrischer, Dramatischer Mezzosopran) | [Cited S4] |
| French | mezzo-soprano | [Estimate] |

- S1 calls it the voice "between sop. and alto" and abbreviates "M.-sopr." [Cited S1]. Family `choir` [Cited S27].
- S3 lists lyric, coloratura and dramatic mezzo-sopranos [Cited S3].

### 2. Sound production
- Registers chest, middle, head [Cited S1]; lyric and dramatic classes [Cited S1].
- "Meyerbeer ... was the first to write for a special type of heavy mezzo-soprano and baritone" [Cited S1].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier / source | Notes | MIDI | Source |
|---|---|---|---|
| Table F very exceptional (dotted) | A3; A5-C6 | 57; 81-84 | [Cited S1] |
| Table F exceptional (solid) | B3-C4; E5-G5 | 59-60; 76-79 | [Cited S1] |
| Table F normal octave (tessitura) | D4-D5 | 62-74 | [Cited S1] |
| Wikipedia typical mezzo-soprano | A3-A5 | 57-81 | [Cited S3] |
| Czech mezzosoprán (a-a2) | A3-A5 | 57-81 | [Cited S11] |
| Fach: coloratura, lyric, dramatic mezzo | G3-B5 | 55-83 | [Cited S4] |

- Non-transposing; treble clef [Cited S1]. No numeric register breaks in read sources [Cited S1].
- Disagreement: Fach's G3 is a tone below S1's dotted A3; the card keeps A3 (rule: only Berlioz widens).

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Flexibility greatest in the normal octave; women's voices more supple than men's [Cited S1].
- Fast writing: scales and arpeggios in thirds easiest; chromatic scales and wide fast leaps very difficult
  [Cited S1].
- Vowel "a" easiest on high notes for women [Cited S1]. Breath only between words [Cited S1].
- Accompaniment: strings most transparent; sustained harmony in the voice's own register hurts women's voices more
  [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Evidence | Library implementation |
|---|---|---|
| sustain / vowel, legato, staccato | [Cited S1] | no solo mezzo library was read; Voices of Rapture covers soprano, alto, tenor and bass only [Cited S22] |
| humming | [Cited S15] | GM2 54 bank 1 "Humming" [Cited S13] |
| text, whisper, trill, portamento, vibrato_on, non_vibrato, crescendo_swell, sforzando, tenuto, marcato | [Estimate: general vocal practice] | none read |

- Practical substitute: a solo alto or soprano patch transposed into range [Estimate: no mezzo library found].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- One singer. Duet pairings: "M.-sopr./Bar." related within an octave; Sopr./M.-sopr. in thirds, "may move in unison,
  in thirds and sixths, and admit very largely of the crossing of parts" [Cited S1].
- In a chorus, mezzo-sopranos are classed with second sopranos or first altos [Cited S1].

### 7. Role in the orchestra and typical doublings
- Arias and ensembles; doubling of women's voices by violins, violas, clarinets, oboes for isolated phrases
  [Cited S1].
- Example roles named by S1: Lioubava's aria (_Sadko_), Fairy Spring's aria (_Snegourotchka_) [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Fach subtypes (section 3) [Cited S4]; the Wagnerian dramatic soprano covers soprano and mezzo range [Cited S1].
- Many choral altos are mezzo-sopranos [Cited S6].

### 9. General MIDI and existing Sonara metadata
- No GM solo-singer program found [Estimate: GM1 "Ensemble" group 49-56 read in S12 has none]; nearest 54
  "Voice Oohs (or Doos)" [Cited S12].
- No voice metadata in data.rs [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage | Plays in Sonara sampler? |
|---|---|---|---|
| SSO, VSCO 2 CE, VCSL | CC Sampling Plus 1.0 / CC0 [Cited S23, S26] | no solo voice [Cited S23, S24, S25] | n/a |
| Soundiron Voices of Rapture | $249 collection (Kontakt Player) / $119 per single voice (Kontakt full version) [Cited S22] | no mezzo; the "Alto" (Kindra Scharich) is the nearest [Cited S22] | no [Cited S26] |
| GM SoundFonts | MIT [Cited S26] | 54 Voice Oohs [Estimate: complete GM set] | no [Cited S26] |

## voice_alto: Contralto (solo alto voice)

```json
{"id": "voice_alto", "family": "choir",
 "names": {"en": "Contralto (alto)", "cs": "Alt", "it": "Contralto", "de": "Alt", "fr": "Contralto", "score_abbreviation": "C.-alto"},
 "pitched": true, "transposition": 0, "clefs": ["treble"],
 "sounding_playable": [53, 79], "sounding_comfortable": [55, 77],
 "range_provenance": {"playable": "Cited S1: Table F, Soloists, Contralto, outer (dotted) limits F3-G5 read from table_f_soloists.png; Wikipedia's typical F3-F5 (S3) lies inside; Fach deep contralto C3-E5 and dramatic contralto F3-A5 (S4) recorded, not used", "comfortable": "Cited S1: Table F solid exceptional G3-A3, normal octave B3-B4, solid exceptional C5-F5; the boundary note F5 read as solid by pixel calibration against the chorus rows, whose counts S1's text fixes (fact-check log); +-1 diatonic step"},
 "written_playable": [53, 79], "gm_program": 54, "gm_program_name": "Voice Oohs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S15 (humming), S22 (solo alto: true legato vowels, vowel sustains and staccatos, Latin syllables), S21 (alto whispers); Estimate for tenuto, marcato, trill, sforzando, crescendo_swell, portamento, vibrato control",
 "solo_or_section": "solo",
 "typical_players": {"classical": "1", "romantic": "1", "film": "1 (Cited S22: one singer per solo library)", "chamber": "1"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | contralto (solo); alto | [Cited S1, S3] |
| Czech | alt (A); kontraalt (cA) for the deepest type | [Cited S11] |
| Italian | contralto | [Cited S2] |
| German | Alt (Dramatischer Alt, Tiefer Alt) | [Cited S4] |
| French | contralto | [Estimate: no French source read] |

- Score: S1 writes "C.-alto" [Cited S1]. Family `choir` [Cited S27].
- "alto and contralto are not interchangeable nor synonymous"; a solo contralto usually has a wider range than a
  choral alto part and is "more comfortable in the lower part" of the voice [Cited S6].

### 2. Sound production
- Registers chest, middle, head; lyric and dramatic classes [Cited S1].
- S3 lists coloratura, lyric and dramatic contraltos [Cited S3].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier / source | Notes | MIDI | Source |
|---|---|---|---|
| Table F very exceptional (dotted) | F3; G5 | 53; 79 | [Cited S1] |
| Table F exceptional (solid) | G3-A3; C5-F5 | 55-57; 72-77 | [Cited S1] |
| Table F normal octave (tessitura) | B3-B4 | 59-71 | [Cited S1] |
| Wikipedia typical contralto | F3-F5 | 53-77 | [Cited S3] |
| Czech alt (f-f2); kontraalt (d-d2) | F3-F5; D3-D5 | 53-77; 50-74 | [Cited S11] |
| Fach dramatic contralto | F3-A5 | 53-81 | [Cited S4] |
| Fach deep contralto | C3-E5 | 48-76 | [Cited S4] |

- Non-transposing; treble clef [Cited S1]. No numeric register breaks in read sources [Cited S1].
- Disagreement: S11 and S4 put the deepest contraltos at D3 or C3; this card follows S1's F3 (the rule widens only
  with Berlioz, who gives no solo contralto limit).

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Flexibility greatest in the normal octave; women more supple than men [Cited S1].
- Example of contralto agility named by S1: "Lell's first song" (_Snegourotchka_) [Cited S1].
- Vowel "a" easiest on high notes; breath only between words [Cited S1].
- Women's voices suffer more than men's from orchestral harmony in their own register [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Library implementation (source) |
|---|---|
| sustain / vowel | Voices of Rapture (VoR) alto (Kindra Scharich): vowel sustains Ah, Eh, Ee, Mm, Oh, Oo, piano and forte [Cited S22] |
| legato | VoR true legato vowels, Ah and Oo core, extra vowels for some singers [Cited S22] |
| staccato | VoR vowel staccatos and Latin poly-staccatos with round robin [Cited S22] |
| text | VoR Latin poly-sustains with simulated legato [Cited S22] |
| humming | VoR "Mm" [Cited S22]; GM2 54 bank 1 "Humming" [Cited S13] |
| whisper | EW Symphonic Choirs solo alto whispers [Cited S21] |
| trill, portamento, vibrato_on, non_vibrato, crescendo_swell, sforzando, tenuto, marcato | [Estimate: none documented by a read library] |

- VoR switches articulations by keyswitch [Cited S22]; Sonara's sampler plays keyswitches [Cited S26].

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- One singer. Duets: "C.-alto/Bass" related within an octave; Sopr./C.-alto related in fifths "should progress nearer
  to one another" [Cited S1].

### 7. Role in the orchestra and typical doublings
- Doubling of women's voices by violins, violas, clarinets and oboes, only for isolated phrases [Cited S1].
- Trouser and character roles: S1's contralto examples are "The 3 songs of Lell" (_Snegourotchka_) [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Countertenors ("male alto") sing this range [Cited S6].
- Mezzo-sopranos often sing alto parts [Cited S6].

### 9. General MIDI and existing Sonara metadata
- No GM solo-singer program found [Estimate: GM1 "Ensemble" 49-56 read in S12 has none]; nearest 54 "Voice Oohs
  (or Doos)" [Cited S12].
- No voice metadata in data.rs [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage | Plays in Sonara sampler? |
|---|---|---|---|
| Soundiron Voices of Rapture, "Alto" | $249 collection (Kontakt Player) / $119 per single voice (Kontakt full version) [Cited S22] | sustains, legato, staccatos, Latin syllables [Cited S22] | no [Cited S26] |
| EW Symphonic Choirs | commercial | alto whispers [Cited S21] | no [Cited S26] |
| SSO, VSCO 2 CE, VCSL | CC Sampling Plus 1.0 / CC0 [Cited S23, S26] | no solo voice [Cited S23, S24, S25] | n/a |
| GM SoundFonts | MIT [Cited S26] | 54 Voice Oohs [Estimate: complete GM set] | no [Cited S26] |

## voice_tenor: Tenor (solo voice)

```json
{"id": "voice_tenor", "family": "choir",
 "names": {"en": "Tenor", "cs": "Tenor", "it": "Tenore", "de": "Tenor", "fr": "Ténor", "score_abbreviation": "Ten."},
 "pitched": true, "transposition": -12, "clefs": ["treble_8vb"],
 "sounding_playable": [48, 74], "sounding_comfortable": [50, 72],
 "range_provenance": {"playable": "Cited S1: Table F, Soloists, Tenor, printed in treble clef as written C4-D6 = sounding C3-D5 (read from table_f_soloists.png; the -12 octave reading is an Estimate: inference from the treble-8vb clef convention in S5); Wikipedia's typical C3-C5 (S3) lies inside; S5's operatic Bb2 and Fach B2 (S4) recorded, not used", "comfortable": "Cited S1: Table F solid exceptional D3-E3, normal octave F3-F4, solid exceptional G4-C5 (sounding); the last solid note C5 (written C6) read by pixel calibration against the chorus rows, whose counts S1's text fixes (fact-check log); +-1 diatonic step"},
 "written_playable": [60, 86], "gm_program": 54, "gm_program_name": "Voice Oohs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento", "falsetto"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S2 (head voice / falsetto of tenors), S15 (humming), S22 (solo tenor: true legato vowels, vowel sustains and staccatos, Latin syllables); Estimate for tenuto, marcato, trill, sforzando, crescendo_swell, portamento, vibrato control, whisper",
 "solo_or_section": "solo",
 "typical_players": {"classical": "1", "romantic": "1", "film": "1 (Cited S22: one singer per solo library)", "chamber": "1"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | tenor | [Cited S1, S5] |
| Czech | tenor (T) | [Cited S11] |
| Italian | tenore (tenore di grazia, tenore robusto) | [Cited S5] |
| German | Tenor (Lyrischer Tenor, Jugendlicher Heldentenor, Heldentenor, Spieltenor) | [Cited S4] |
| French | ténor; "haute-contre" for the old French high tenor | [Cited S2] (haute-contre); "ténor" [Estimate] |

- Score abbreviation "Ten." [Cited S1]. Family `choir` [Cited S27].
- S1's sub-types: "light tenor, tenorino-altino, ... lyric tenor, dramatic tenor" [Cited S1].

### 2. Sound production
- Men's registers chest, mixed, falsetto [Cited S1]. Head notes "are of great beauty in tenor singers" up to "Eb and F
  above the stave" (read as Eb5, F5); "Clever tenor singers ... give, forte, certain high notes such as A, B, and even
  C, in head voice, or in chest voice, at pleasure"; for most, Bb4 is "the limit of a tenor singer's chest voice"
  [Cited S2].
- Wagner wrote a tenor "possessing the attributes and compass of the tenor and baritone together" [Cited S1].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier / source | Written | Sounding | MIDI sounding | Source |
|---|---|---|---|---|
| Table F very exceptional (dotted) | C4; D6 | C3; D5 | 48; 74 | [Cited S1] |
| Table F exceptional (solid) | D4-E4; G5-C6 | D3-E3; G4-C5 | 50-52; 67-72 | [Cited S1] |
| Table F normal octave (tessitura) | F4-F5 | F3-F4 | 53-65 | [Cited S1] |
| Wikipedia typical tenor | | C3-C5 | 48-72 | [Cited S3] |
| Wikipedia, opera writing | | Bb2-C5 | 46-72 | [Cited S5] |
| Tenor subtypes: leggero C3-E5 (some F5); lyric C3-D5; spinto C3-C5; dramatic B2-B4 (some C5); buffo C3-C5 | | | | [Cited S5] |
| Fach: lyric C3-C5; Jugendlicher Heldentenor C3-C5; Charaktertenor B2-C5; Heldentenor B2-C5; Spieltenor B2-B4 | | | | [Cited S4] |
| Czech tenor (c-c2) | | C3-C5 | 48-72 | [Cited S11] |

- Transposition -12: an 8 under the treble clef means the notes sound an octave below the written pitch, and this
  is the standard clef for tenor parts [Cited S5]. Table F prints the tenor in plain treble clef [Cited S1], so
  reading its written C4-D6 as sounding C3-D5 is an inference [Estimate: from the clef convention in S5];
  Berlioz used the C clef on the fourth line [Cited S2].
- High C: "A few tenor roles in the standard repertoire call for a 'tenor C' (C5)", some optional or interpolated
  [Cited S5].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- The tenor is the most agile male voice [Cited S1]. For men the easiest vowel on high notes is "o" [Cited S1].
- Lyric versus dramatic: the dramatic voice "is more powerful and of greater range", the lyric more supple
  [Cited S1].
- Fast writing rules as for all voices: thirds and scales easy, chromatic scales and wide fast leaps hard [Cited S1].
- Breath only between words; rests needed [Cited S1].
- Accompaniment: "As a rule women's voices suffer more than men's when they come in contact with harmony in a
  register similar to their own" [Cited S1]; "a singer is more easily overpowered by long sustained notes than by
  short detached ones" [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Library implementation (source) |
|---|---|
| sustain / vowel | Voices of Rapture (VoR) tenor (Brian Thorsett): vowel sustains Ah, Eh, Ee, Mm, Oh, Oo, piano and forte [Cited S22] |
| legato | VoR true legato vowels (Ah, Oo core) [Cited S22] |
| staccato | VoR vowel staccatos and Latin poly-staccatos [Cited S22] |
| text | VoR Latin poly-sustains [Cited S22] |
| falsetto | head voice to Eb5/F5 [Cited S2]; no read library documents a falsetto patch [Estimate] |
| humming, whisper, trill, portamento, vibrato_on, non_vibrato, crescendo_swell, sforzando, tenuto, marcato | [Estimate: none documented by a read library] |

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- One singer. Duets: Sopr./Ten. related within an octave, moving in tenths, sixths, thirds; Ten./Ten. pairs sing in
  unison and thirds and cross [Cited S1]. Two tenors in one work: write one slightly higher [Cited S1].
- A solo male voice stands out against a women's chorus [Cited S1].

### 7. Role in the orchestra and typical doublings
- Doubling of men's voices in unison "by violas, 'cellos, bassoons and horns"; trombones and trumpets overpower
  [Cited S1].
- S1's tenor examples: Sadko's aria, Tsar Berendey's cavatinas [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Fach and Italian subtypes (section 3) [Cited S4, S5]; countertenor (falsetto, alto range) [Cited S5];
  haute-contre [Cited S2].

### 9. General MIDI and existing Sonara metadata
- No GM solo-singer program found [Estimate: GM1 "Ensemble" 49-56 read in S12 has none]; nearest 54 "Voice Oohs
  (or Doos)" [Cited S12].
- No voice metadata in data.rs; octave transposition is already a supported `Transposition` value (double bass -12)
  [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage | Plays in Sonara sampler? |
|---|---|---|---|
| Soundiron Voices of Rapture, "Tenor" | $249 collection (Kontakt Player) / $119 per single voice (Kontakt full version) [Cited S22] | sustains, legato, staccatos, Latin syllables [Cited S22] | no [Cited S26] |
| SSO, VSCO 2 CE, VCSL | CC Sampling Plus 1.0 / CC0 [Cited S23, S26] | no solo voice [Cited S23, S24, S25] | n/a |
| GM SoundFonts | MIT [Cited S26] | 54 Voice Oohs [Estimate: complete GM set] | no [Cited S26] |

## voice_baritone: Baritone (solo voice)

```json
{"id": "voice_baritone", "family": "choir",
 "names": {"en": "Baritone", "cs": "Baryton", "it": "Baritono", "de": "Bariton", "fr": "Baryton", "score_abbreviation": "Bar."},
 "pitched": true, "transposition": 0, "clefs": ["bass"],
 "sounding_playable": [45, 69], "sounding_comfortable": [47, 67],
 "range_provenance": {"playable": "Cited S1: Table F, Soloists, Baritone, outer (dotted) limits A2-A4 read from table_f_soloists.png; Wikipedia's typical A2-A4 (S3) agrees; Fach G2-B4 (S4) and Czech G2-G4 (S11) recorded, not used", "comfortable": "Cited S1: Table F solid exceptional B2-C3, normal octave D3-D4, solid exceptional E4-G4; the boundary note G4 read as solid by pixel calibration against the chorus rows, whose counts S1's text fixes (fact-check log); +-1 diatonic step"},
 "written_playable": [45, 69], "gm_program": 54, "gm_program_name": "Voice Oohs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento", "falsetto"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S2 (head notes of baritones only in light buffo style), S15 (humming); Estimate for the rest: general vocal practice; no read library records a solo baritone",
 "solo_or_section": "solo",
 "typical_players": {"classical": "1", "romantic": "1", "film": "1", "chamber": "1"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | baritone | [Cited S1, S3] |
| Czech | baryton (Bt); basbaryton (bBt) | [Cited S11] |
| Italian | baritono ("baritono leggero") | [Cited S4] |
| German | Bariton (Lyrischer Bariton, Kavalierbariton, Charakterbariton, Heldenbariton) | [Cited S4] |
| French | baryton ("baryton-martin") | [Cited S1, S4] |

- S1: baritone is "between tenor and bass"; abbreviation "Bar." [Cited S1]. Family `choir` [Cited S27].

### 2. Sound production
- Men's registers chest, mixed, falsetto [Cited S1]; head notes of baritones and basses have "something revolting,
  everywhere but in buffo music" [Cited S2].
- Berlioz values "mixed" and "veiled" emission in men's voices [Cited S2].
- "Meyerbeer ... was the first to write for a special type of heavy mezzo-soprano and baritone" [Cited S1].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier / source | Notes | MIDI | Source |
|---|---|---|---|
| Table F very exceptional (dotted) | A2; A4 | 45; 69 | [Cited S1] |
| Table F exceptional (solid) | B2-C3; E4-G4 (G4 is the boundary note, read as solid; see fact-check log) | 47-48; 64-67 | [Cited S1] |
| Table F normal octave (tessitura) | D3-D4 | 50-62 | [Cited S1] |
| Wikipedia typical baritone | A2-A4 | 45-69 | [Cited S3] |
| Czech baryton (G-g1); basbaryton (F-f1) | G2-G4; F2-F4 | 43-67; 41-65 | [Cited S11] |
| Fach light baritone / baryton-martin | B2-B4 | 47-71 | [Cited S4] |
| Fach lyric baritone | A2-A4 | 45-69 | [Cited S4] |
| Fach Kavalierbariton, Verdi baritone | A2-G#4 | 45-68 | [Cited S4] |
| Fach Heldenbariton | G2-F#4 | 43-66 | [Cited S4] |
| Fach lyric bass-baritone | F2-F#4 | 41-66 | [Cited S4] |
| Berlioz, choral first basses or baritones (low A heavy, high G forced) | A2-G4 | 45-67 | [Cited S2] |

- Non-transposing; bass clef in Table F [Cited S1]. Treble-8 notation for baritones is sometimes seen [Estimate: no
  read source states it].
- Disagreement: Czech S11 G2-G4 and Fach Heldenbariton G2 extend below A2; light baritone B4 above A4. The card keeps
  S1's A2-A4.
- Comfortable top G4 (67) sits inside Fach lyric A2-A4 and Kavalierbariton A2-G#4 [Cited S4]. Berlioz's "high G
  excessively forced" concerns choral first basses, not soloists [Cited S2].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Men's voices are less supple than women's; the baritone is less agile than the tenor [Cited S1].
- For men the easiest vowel on high notes is "o" [Cited S1].
- Choral baritones: low A "heavy, and vague", high G "excessively forced" [Cited S2].
- S1's baritone examples: Griaznov's aria (_The Tsar's Bride_), Miskir's aria, the Venetian song (_Sadko_)
  [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Evidence | Library implementation |
|---|---|---|
| sustain / vowel, legato, staccato, text | [Cited S1] | no solo baritone library was read; Voices of Rapture covers soprano, alto, tenor and bass [Cited S22] |
| humming | [Cited S15] | GM2 54 bank 1 "Humming" [Cited S13] |
| falsetto | buffo only [Cited S2] | none read |
| whisper, trill, portamento, vibrato_on, non_vibrato, crescendo_swell, sforzando, tenuto, marcato | [Estimate] | none read |

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- One singer. Duets: "M.-sopr./Bar." related within an octave; Sopr./Bar. in tenths "is fairly common"
  [Cited S1].
- In a chorus baritones join the second tenors or first basses [Cited S1].

### 7. Role in the orchestra and typical doublings
- Doubling of men's voices by violas, cellos, bassoons and horns, in isolated phrases only [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Fach subtypes (section 3) [Cited S4]; baryton-martin as the light, high type [Cited S1, S4]; bass-baritone
  [Cited S4, S11].

### 9. General MIDI and existing Sonara metadata
- No GM solo-singer program found [Estimate: GM1 "Ensemble" 49-56 read in S12 has none]; nearest 54 "Voice Oohs
  (or Doos)" [Cited S12].
- No voice metadata in data.rs [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage | Plays in Sonara sampler? |
|---|---|---|---|
| SSO, VSCO 2 CE, VCSL | CC Sampling Plus 1.0 / CC0 [Cited S23, S26] | no solo voice [Cited S23, S24, S25] | n/a |
| Soundiron Voices of Rapture | $249 collection (Kontakt Player) / $119 per single voice (Kontakt full version) [Cited S22] | no baritone; tenor or bass are the nearest [Cited S22] | no [Cited S26] |
| GM SoundFonts | MIT [Cited S26] | 54 Voice Oohs [Estimate: complete GM set] | no [Cited S26] |

## voice_bass: Bass (solo voice)

```json
{"id": "voice_bass", "family": "choir",
 "names": {"en": "Bass", "cs": "Bas", "it": "Basso", "de": "Bass", "fr": "Basse", "score_abbreviation": "B."},
 "pitched": true, "transposition": 0, "clefs": ["bass"],
 "sounding_playable": [40, 65], "sounding_comfortable": [43, 64],
 "range_provenance": {"playable": "Cited S1: Table F, Soloists, Bass, outer (dotted) limits E2-F4 read from table_f_soloists.png; Wikipedia's E2-E4 (S3, S7) lies inside; basso profondo C2 (S4, S7) recorded, not used", "comfortable": "Cited S1: Table F solid exceptional G2-A2, normal octave B2-B3, solid exceptional C4-E4; the boundary note E4 read as solid by pixel calibration against the chorus rows, whose counts S1's text fixes (fact-check log); +-1 diatonic step"},
 "written_playable": [40, 65], "gm_program": 54, "gm_program_name": "Voice Oohs", "gm_percussion_notes": [],
 "existing_sonara_id": null,
 "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
 "articulations_proposed_new": ["vowel", "text", "humming", "whisper", "crescendo_swell", "sforzando", "vibrato_on", "non_vibrato", "portamento", "falsetto"],
 "articulations_provenance": "Cited S1 (legato, staccato, vowels), S2 (head notes only in buffo style), S15 (humming), S22 (solo bass: true legato vowels, vowel sustains and staccatos, Latin syllables); Estimate for tenuto, marcato, trill, sforzando, crescendo_swell, portamento, vibrato control, whisper",
 "solo_or_section": "solo",
 "typical_players": {"classical": "1", "romantic": "1", "film": "1 (Cited S22: one singer per solo library)", "chamber": "1"},
 "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)
| Language | Name | Provenance |
|---|---|---|
| English | bass | [Cited S1, S7] |
| Czech | bas (B); basso profondo (cB); oktávista (O) | [Cited S11] |
| Italian | basso (basso cantante, basso buffo, basso profondo) | [Cited S1, S7] |
| German | Bass (Hoher Bass, Jugendlicher Bass, Spielbass, Schwerer Spielbass, Seriöser Bass) | [Cited S4, S7] |
| French | basse | [Estimate: no French source read; S7 gives no French names] |

- Score abbreviation "B." (Czech letter "B") [Cited S11]; S1 writes "Bass" in full [Cited S1]. Family `choir`
  [Cited S27].

### 2. Sound production
- Men's registers chest, mixed, falsetto [Cited S1]; bass head notes only in buffo style [Cited S2].
- "basso profondo (deep bass) ... signifies great resonance in the deep register" [Cited S1].
- Vowel "i" softens the bass's top notes; "a" extends the lowest compass [Cited S1].

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)
| Tier / source | Notes | MIDI | Source |
|---|---|---|---|
| Table F very exceptional (dotted) | E2-F2; F4 | 40-41; 65 | [Cited S1] |
| Table F exceptional (solid) | G2-A2; C4-E4 (E4 is the boundary note, read as solid; see fact-check log) | 43-45; 60-64 | [Cited S1] |
| Table F normal octave (tessitura) | B2-B3 | 47-59 | [Cited S1] |
| Wikipedia bass | E2-E4; "low extreme" C2; Harvard Dictionary E2-C4 | 40-64; 36; 40-60 | [Cited S7] |
| Wikipedia typical bass (Voice type) | E2-E4 | 40-64 | [Cited S3] |
| Czech bas (E-e1); basso profondo (C-c1); oktávista (A1-a) | E2-E4; C2-C4; A1-A3 | 40-64; 36-60; 33-57 | [Cited S11] |
| Fach: dramatic bass-baritone, young bass, Spielbass | E2-F4 | 40-65 | [Cited S4] |
| Fach: schwerer Spielbass, lyric and dramatic seriöser Bass | C2-F4 | 36-65 | [Cited S4] |

- The bass tessitura "is normally defined by the outermost lines of the bass clef" (G2-A3) [Cited S7].
- Basso profondo: "A basso profondo can hardly ever go beyond B or C above the stave" (B3/C4, 59/60) [Cited S2].
- Non-transposing; bass clef [Cited S1]. E4 is the boundary note of Table F's upper solid bracket; the pixel
  calibration in the fact-check log reads it as solid (comfortable top E4 = 64), +-1 diatonic step.

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits
- Men's voices are less supple than women's; the bass is the least agile [Estimate: S1 says only that the higher
  voice is the more agile, "the tenor voice in men"].
- Low notes need slow notes, time for breath and a sonorous syllable (said of chorus basses) [Cited S2].
- S1's bass example: the Bass Aria in _Sadko_ [Cited S1].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)
| Technique (id) | Library implementation (source) |
|---|---|
| sustain / vowel | Voices of Rapture (VoR) bass (Joseph Trumbo): vowel sustains Ah, Eh, Ee, Mm, Oh, Oo, piano and forte [Cited S22] |
| legato | VoR true legato vowels (Ah, Oo core) [Cited S22] |
| staccato | VoR vowel staccatos and Latin poly-staccatos [Cited S22] |
| text | VoR Latin poly-sustains [Cited S22] |
| humming | VoR "Mm" [Cited S22] |
| falsetto | buffo style only [Cited S2]; no library read |
| whisper, trill, portamento, vibrato_on, non_vibrato, crescendo_swell, sforzando, tenuto, marcato | [Estimate: none documented] |

### 6. Solo vs section (typical number of players, divisi, unison doubling)
- One singer. Duets: "C.-alto/Bass" within an octave; Ten./Bass related in fifths; Sopr./Bass in twelfths "crossing
  of parts is out of the question"; "M.-sopr./Bass" in tenths "fairly common" [Cited S1].

### 7. Role in the orchestra and typical doublings
- Doubling of men's voices by violas, cellos, bassoons and horns, in isolated phrases [Cited S1].
- Long sustained double-bass notes under a singer "produce a peculiar throbbing effect" [Cited S1].

### 8. Variants and related instruments (when a player changes instrument)
- Basso cantante, basso buffo, basso profondo; oktavists reaching below C2; bass-baritone [Cited S7, S11, S1].

### 9. General MIDI and existing Sonara metadata
- No GM solo-singer program found [Estimate: GM1 "Ensemble" 49-56 read in S12 has none]; nearest 54 "Voice Oohs
  (or Doos)" [Cited S12].
- No voice metadata in data.rs [Cited S27].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)
| Library | Licence (as stated) | Coverage | Plays in Sonara sampler? |
|---|---|---|---|
| Soundiron Voices of Rapture, "Bass" | $249 collection (Kontakt Player) / $119 per single voice (Kontakt full version) [Cited S22] | sustains, legato, staccatos, Latin syllables [Cited S22] | no [Cited S26] |
| SSO, VSCO 2 CE, VCSL | CC Sampling Plus 1.0 / CC0 [Cited S23, S26] | no solo voice [Cited S23, S24, S25] | n/a |
| GM SoundFonts | MIT [Cited S26] | 54 Voice Oohs [Estimate: complete GM set] | no [Cited S26] |

## Open questions and disagreements between sources

1. **Table F reading.** All values from S1 were read off a small engraved image (581x327 and 620x517 px); the
   Gutenberg HTML links no larger version. Chorus brackets pass S1's own tests (each normal bracket is one octave;
   each solid bracket two notes). Soloist brackets are not bound by that text: the soprano's normal bracket is
   printed under F4-G5 (a ninth), and the upper solid brackets cover two notes (soprano A5-B5), three (mezzo E5-G5,
   baritone E4-G4, bass C4-E4) or four (contralto C5-F5, tenor written G5-C6). Rule used for the last solid note
   (fact-check): the engraver ends each solid segment at or just past the centre of the last solid note head and
   10-19 px before the first dotted head, as measured on the chorus rows whose counts the text fixes; the boundary
   notes contralto F5, baritone G4 and bass E4 sit at -0.5 to +1.5 px from the solid end, so they are read as solid.
   Residual uncertainty: one diatonic step at each upper comfortable limit. A higher-resolution scan of the 1922
   Russian or English edition should confirm these before they are pinned in data.rs.
2. **Accidentals in Table F.** The table prints no accidentals, so "B" limits (alto normal B3-B4, bass normal B2-B3,
   Berlioz's chest limit Bb4) may be B or Bb in the author's intent.
3. **Two tiers or three?** S1 has three tiers (normal, exceptional, very exceptional). data.rs has two (`playable`,
   `comfortable`). This file maps comfortable to normal + solid exceptional, to match data.rs's "limit of ordinary
   parts" meaning. A third optional field (`tessitura`) would carry S1's normal octave, which is what an agent should
   aim melodies at. Owner decision.
4. **Alto low end.** S1 G3 vs Berlioz F3 and Wikipedia F3 (with "citation needed"). Card: playable F3, comfortable
   G3.
5. **Bass extremes.** S1 dotted C2 (Russian octavists) vs Berlioz F2 (school), Eb2/D2 (second basses) and "useless to
   write ... G's or F's" for many basses; top S1 E4 vs Berlioz F#4 (Gluck). Card: playable C2-F#4, comfortable
   G2-D4. A choir without octavists should treat C2-F2 as unreachable; a profile per choir type (Russian, Western,
   amateur) may be needed. Berlioz himself confirms the low end for North-German and Russian basso profondo
   ("D's and C's below the stave") [Cited S2].
6. **Tenor top.** S1 A4 vs Berlioz Bb4 (chest limit) and falsetto Eb5/F5 (my reading of "Eb and F above the stave").
   Card: playable Bb4. Falsetto notes are excluded.
7. **Fach "as written".** S4 says its ranges are "as written on sheet music", yet its tenor ranges (C3-C5) are
   sounding pitches. Its source for the numbers is not named (Kloiber is cited for role lists). Fach values are only
   recorded, never used for a range here.
8. **Czech vs English baritone.** S11 gives G2-G4, S3 and S1 give A2-A4.
9. **Wikipedia's "typical" solo ranges (S3) are narrower than S1's soloists' outer limits**, which include climax
   notes (soprano D6-E6, tenor D5). If Sonara wants a conservative `playable` for amateur singers, S3 is the better
   source; for professional soloists, S1.
10. **GM for solo voices.** GM1 has no solo-singer program in the part read (programs 49-56); the cards use 54
    "Voice Oohs" as nearest (Estimate). The full GM1 table on midi.org was not readable (S14 has no table, the PDF is
    members-only); 53/54/55 are cited from Wikipedia's list (S12), which cites midi.org.
11. **EastWest Hollywood Choirs details** come from product page and reviews (S18-S20); the manual (S28) was not
    readable. Reviews disagree on legato quality (S19 names legato on Ah and Oh at two speeds; S20 calls the
    "polyphonic legato" unconvincing and notes no proper marcato). Singer counts for EW HC were not found.
12. **Choir sizes** often quoted for planning (chamber 16-24, symphonic 60-150) were not found in a read source. Read values:
    S1 per part 32 (full), 16-20 (medium), 8-10 (small), i.e. about 128, 64-80 and 32-40 in total [Estimate: 4 x per
    part]; S10 chamber choir 8-40 in total; libraries 22 (S16) and 50 (S17).
13. **Free choir samples for Sonara.** The only free SFZ choir found is SSO "Chorus" (one looped vowel per semitone,
    female G4-C6, male G2-F#4, licence CC Sampling Plus 1.0, bundling unresolved per S26). Its Performance patch needs
    CC1 crossfades, which Sonara's sampler does not play; the Notation patch plays but its velocity filter is
    ignored. VSCO 2 CE and VCSL have no voices. No free solo-voice library was found. Spitfire LABS Choir (S30) was not
    read.
14. **Articulation vocabulary.** Proposed new ids `text` and `whisper` are invented here; `vowel` would need a
    parameter (ah, oh, oo, ee, mm) to be useful to an agent, which the current id-only vocabulary cannot express.

## Fact-check log

Adversarial check of this file, 2026-10-30. Every URL below was reopened in this session (WebFetch, or curl for the
Gutenberg, archive.org and GitHub files); nothing was taken from the original researcher's downloads.

**1. Ranges, transposition, written range, clefs, GM (all ten cards).**

| Check | Result |
|---|---|
| Table F chorus rows (S1, table_f_chorus.png, re-downloaded, read at 4x and as a pixel dump) | Confirmed note by note: soprano dotted C4 / solid D4-E4 / normal F4-F5 / solid G5-A5 / dotted B5-C6; contralto solid G3-A3 / normal B3-B4 / solid C5-D5 / dotted E5-F5 (no dotted part below); tenor (written, plain treble clef) dotted C4 / solid D4-E4 / normal F4-F5 / solid G5-A5; bass dotted C2-F2 / solid G2-A2 / normal B2-B3 / solid C4-D4 / dotted E4. Note counts checked against the stem-down note on the middle line (B4 treble, D3 bass). |
| Table F soloist rows (table_f_soloists.png) | Outer limits confirmed: soprano C4-E6, mezzo A3-C6, contralto F3-G5, tenor written C4-D6, baritone A2-A4, bass E2-F4. Lower brackets confirmed (dotted then two solid notes in every row). Soprano normal bracket F4-G5 (a ninth) confirmed. Upper boundary notes: see the calibration table. |
| Rimsky-Korsakov prose (S1, pg33900.txt) | Confirmed: tier definitions, "normal octave" text for soloists, chorus sizes 32 / 16-20 / 8-10, octavists note, vowels, flexibility, doubling by violins, violas, clarinets, oboes / violas, cellos, bassoons, horns, divisi, duet relationships, Meyerbeer/Wagner note, example arias (Lioubava, Lell, Griaznov, Bass Aria). |
| Berlioz (S2, OCR re-downloaded) | Confirmed: bass "F below the stave (F clef) up to D and Eb above" = F2-D4/Eb4; tenor "C below the stave (C clef on fourth line) up to the A and Bb above" = C3-A4/Bb4; contralto F3-Eb5; soprano C4-Bb5; second sopranos B3-G5; second basses Eb2, D2; half of basses F#4 in chest voice; tenor head notes to "Eb and F above the stave"; Bb as tenor chest limit; baritones "low A ... high G". |
| Wikipedia S3, S5, S6, S7, S8; Fach S4 (incl. offset 60,000 for the bass Fach); Czech S11 | All quoted ranges confirmed; Helmholtz conversions re-done (c1 = 60, a = 57, f = 53, d = 50, G = 43, F = 41, E = 40, C = 36, A1 = 33): all correct. |
| Transposition and written ranges | Recomputed written = sounding - transposition for all ten JSON blocks: all match. Tenor -12 rests on the treble-8vb convention [Cited S5, S8]; S1 prints the tenor in plain treble clef, so the -12 reading of S1 stays an Estimate, as the card says. |
| GM | 53 "Choir Aahs", 54 "Voice Oohs (or Doos)", 55 "Synth Voice or Synth Choir", numbered 1-128 (S12); GM2 bank 1: 53 "Choir Aahs 2", 54 "Humming", 55 "Analog Voice" (S13); S14 reopened: names per program, "the acoustic characteristics of the each sound are not defined", no table on the page. No midi.org page with the GM1 table could be opened (the old specifications URL returns 404). |
| JSON structure | All ten blocks have exactly the 20 required keys in order and the six name keys; ids are the assigned ones in the assigned order; family "choir" is a valid slug; existing articulation ids are all in the 15-id vocabulary; proposed ids are from the preferred list except `text` and `whisper`, which the file defines in its reading notes; comfortable inside playable everywhere. |
| data.rs (task item 4) | data.rs has no voice or choir instrument (16 instruments; docs/orchestration-metadata.md lists "voices" as not covered), so no "Verification of data.rs" table applies; `existing_sonara_id` null is correct. `Family::Choir` with slug "choir" exists in instrument.rs; `comfortable` = "ordinary orchestral writing, inside playable" (docs). |

**Pixel calibration of the solid/dotted boundary (x in original pixels, 620 px soloist image, 581 px chorus image;
head centre from the head pixels in the dump).**

| Row | Last solid pixel | Last solid note, head centre, delta | Next note, head centre, delta | Reading |
|---|---|---|---|---|
| chorus soprano (S1 text fixes solid = G5-A5) | 424 | A5 ~422, +2 | B5 ~441, -17 | calibration: included |
| chorus contralto (text fixes solid = C5-D5) | 409 | D5 ~404, +5 | E5 ~424, -15 | calibration: included |
| solo soprano | ~479 (dashes from 483) | B5 ~477, +2 | C6 ~496, -17 | B5 solid |
| solo tenor (written) | 521 | C6 ~513, +8 | D6 ~531, -10 | C6 solid (sounding C5) |
| solo mezzo | ~466 (then hidden by the staff line; dashes visible from 482) | G5 ~459, +3..+7 | A5 ~477, -11..-15 | G5 solid |
| solo contralto | 460 | F5 ~459, +1.5 | G5 ~477, -17 | F5 solid |
| solo baritone | 458 | G4 ~459, -0.5 | A4 ~477, -19 | G4 solid (changed) |
| solo bass | 459 | E4 ~459, +0.5 | F4 ~477, -18 | E4 solid (changed) |

The lower brackets show the mirror pattern (solid starts 2-3 px before the first solid head centre; baritone B2,
bass G2), which confirms the card's lower comfortable limits.

**2. Changes made (was -> now, why).**

| Card / place | Was | Now | Why |
|---|---|---|---|
| voice_baritone `sounding_comfortable` and section 3 | [47, 65] (B2-F4); solid E4-F4, dotted G4-A4 | [47, 67] (B2-G4); solid E4-G4, dotted A4 | the card treated identical boundary geometry two ways (contralto F5 included, baritone G4 and bass E4 excluded); calibration reads all three as solid |
| voice_bass `sounding_comfortable` and section 3 | [43, 62] (G2-D4); solid C4-D4, dotted E4-F4 | [43, 64] (G2-E4); solid C4-E4, dotted F4 | same calibration |
| mezzo, contralto, tenor, baritone, bass `range_provenance.comfortable` | "at the solid/dotted boundary, read at 8x/10x zoom" | one stated rule (pixel calibration against the chorus rows, +-1 diatonic step) | consistency; values for mezzo, contralto and tenor unchanged |
| Soundiron Voices of Rapture rows (soprano, mezzo, alto, tenor, baritone, bass section 10) | "$249 / $119 per voice, Kontakt Player" | $249 collection (Kontakt Player); $119 single voice listed as Kontakt full version | S22 lists the single voices as "Kontakt Full Version" |
| choir_soprano section 8, voice_soprano section 8 | "treble" = "the pre-pubescent male voice" | a young singer of either sex with an unchanged voice in the mezzo-soprano range (paraphrase of S3) | S3's actual definition |
| choir_bass section 3 table | "Russian drones on A1" (33) | A2 drone, in rare cases doubled by A1 (45/33) | S7's wording |
| choir_bass section 8 | oktavists reach C2 "or lower" (presented as a quote) | "are able to reach much lower than" the C2 low extreme | the quoted words are not in S7 |
| choir_alto section 3 | "tagged citation needed" | the range caption carries "citation needed" | S6 places the tag on the caption |
| choir_bass range_provenance, section 3, section 8, open question 5 | (none) | Berlioz corroborates the dotted C2: North-German and Russian basso profondo given "D's and C's below the stave" | read in S2 |
| voice_bass section 3 | (none) | "A basso profondo can hardly ever go beyond B or C above the stave" | read in S2 |
| choir_tenor section 3 | (none) | Berlioz: the B above the chest limit is "a very dangerous note, and one to be shunned" for first tenors (accidental garbled in OCR; B natural is an Estimate) | read in S2 |
| voice_baritone section 3 | (none) | comfortable G4 is inside Fach lyric and Kavalier baritone ranges; Berlioz's "excessively forced" high G concerns choral first basses | sanity check of the change |
| Open question 1, reading notes | boundary notes listed as unresolved | rule and residual uncertainty stated | follows the calibration |

**3. Spot-checked Cited claims (all confirmed).** S16 (22 singers; monophonic legato, overlapping notes up to an
octave apart; pitch bend only in legato patches; 'Breathy ooh', 'Mmm aah mmm', 'Nonvib to vib'; staggered
breathing; keyswitch, CC, velocity, speed, per-patch MIDI channel; no word building). S17 (50 singers; Sopranos &
Altos / Tenors & Basses; Long Mmm, Short Staccato Syllables incl. keyswitch preset; £/$/€29 perpetual; Splice
INSTRUMENT; Expression and Dynamics). S18 (781 pre-built phrases). S19 (Oo, Oh, Ah, Ee; legato only on Ah and Oh,
two speeds; WordBuilder plain English, phonetic, Votox; presets in five languages; Mm, D, RR; "up to a Top Bb").
S20 (CC1 crossfades mp/mf/ff and also sets vibrato; two staccato round robins; polyphonic legato unconvincing; no
proper marcato; women's and men's choirs only). S21 (SATB Normal, Legato, Staccato, Slurred; whispers for soprano,
alto, boy, not via WordBuilder). S22 (singers, vowels, 23 Poly-Staccatos, 12 Poly-Sustains, keyswitching, CC and
host automation). S23 (SSO README licence CC Sampling Plus 1.0; female samples g4-c6 and male g2-f#4, mapped
`lokey=g4` etc. in "Chorus - Notation/Mixed Chorus.sfz", which also uses `fil_veltrack`). S9, S10, S13, S15 quotes.
S26 (sampler: velocity layers, keyswitches, CC 64/7/11, pitch bend; no CC-driven layers, crossfades or filters).

**4. Downgraded to Estimate.** None: every Cited range value was found in its source. The one new inference added
(Berlioz's garbled accidental read as B natural) is tagged Estimate.

**5. Remaining concerns.**
- Table F is a 620 px engraving; the upper comfortable limits of all six soloists rest on a +-1 diatonic-step
  reading, even after calibration. A better scan should be checked before pinning in data.rs.
- S1 prints the tenor in plain treble clef; the -12 reading is an inference (Estimate), as is S1's intent for
  unprinted accidentals (B vs Bb).
- `choir_tenor` lists clefs `treble_8vb` and `bass`, but `written_playable` [60, 82] assumes `treble_8vb`; on a shared
  bass-clef staff written equals sounding. `treble_8vb` is a clef slug this file defines; no shared clef vocabulary
  was checked.
- GM program numbers rest on Wikipedia (S12, S13); the midi.org table is only in the members' PDF. Using 54 "Voice
  Oohs" for solo voices is an Estimate.
- Berlioz's four-voice ranges are the "school" theory he himself calls "insufficient and hazardous" for choirs;
  they widen `playable` (alto F3, tenor Bb4, bass F#4) by this file's mapping rule.
- `text` and `whisper` are invented articulation ids; `vowel` needs a parameter the id-only vocabulary lacks.
- Most French names and Czech plurals remain Estimates.

## Consistency review

_Consistency review, 2026-10-30: json against sections 3 and 9 and the Fact-check log; written = sounding minus
transposition; tag hygiene; quote length; paths._

- `choir_tenor` and `voice_tenor` json `range_provenance.playable`: the "-12 applied" octave reading of Table F is
  now tagged as an Estimate (an inference from the treble-8vb convention, S5). Before, the json called it Cited while
  section 3 and the Fact-check log call it an Estimate. No number changed.
- `voice_tenor` section 3: added the same Estimate clause for reading Table F's written C4-D6 as sounding C3-D5.
- `choir_soprano` section 8: the 16-word Wikipedia (S3) definition of "treble" is now a paraphrase (the quote licence
  allows non-public-domain quotes of at most 15 words).
- Fact-check log, change table, "treble" row: the "Now" cell is a paraphrase of S3 instead of the same 16-word quote.
- `voice_tenor` section 3: the 17-word Wikipedia (S5) sentence on the treble-8vb clef is now a paraphrase.
- Sources rows S26 and S27: the absolute local checkout paths were replaced by "paths relative to the repository
  root".
- Open question 12: removed a reference to the research brief; the sizes are now described as often quoted for
  planning.
