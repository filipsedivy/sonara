# Unpitched orchestral percussion: instrument cards
_Researched 2026-10-30. MIDI note numbers, C4 = 60 = middle C. Ranges are sounding unless marked written.
Transposition = sounding minus written, in semitones._
## Sources
| # | Source | URL | Kind (handbook, manufacturer, library maker, spec, encyclopedia, other) | Opened in this session |
|---|---|---|---|---|
| S1 | MIDI Association, "GM 1 Sound Set" page: GM1 instrument patch map (1-128) and the "General MIDI Level 1 Percussion Key Map" (channel 10, keys 35-81). Read as the Wayback Machine snapshot of 2021-01-25, because the live midi.org URLs return 404 and the spec PDF download needs a login. | https://web.archive.org/web/20210125203751/https://www.midi.org/specifications-old/item/gm-level-1-sound-set | spec | yes |
| S2 | Roland SC-88 Pro Owner's Manual, chapter 8 appendix, "SC-55 Drum set" table (manual p. 174): kits STANDARD/JAZZ, ROOM, POWER, ELECTRONIC, TR-808, BRUSH, ORCHESTRA (PC 49); legend "←" = same as STANDARD, "*" = two voices, [EXC] = exclusive group. HTML text copy on manualsdir.com. | https://www.manualsdir.com/manuals/196005/roland-sc-88-pro.html?page=176 | manufacturer | yes |
| S3 | N. Rimsky-Korsakov, "Principles of Orchestration" (tr. E. Agate), Project Gutenberg eBook 33900: "Percussion instruments producing indefinite sounds"; "Use of percussion instruments for rhythm and colour"; "Economy in orchestral colour". | https://www.gutenberg.org/cache/epub/33900/pg33900.txt | handbook | yes |
| S4 | Cecil Forsyth, "Orchestration" (Macmillan/Stainer & Bell, 1914), archive.org full text (Cornell copy): instrument list pp. 10-11; "Unmusical percussion instruments" Nos. 1-10, pp. 22-41. | https://archive.org/download/cu31924022381440/cu31924022381440_djvu.txt | handbook | yes |
| S5 | Ch.-M. Widor, "The Technique of the Modern Orchestra" (tr. E. Suddard, 1906), archive.org full text: chapter III, Side Drum, Muffled Drums, Tenor Drum, Tambourine, Tabor, Triangle, Castanets, Cymbals, Bass Drum, Gong (pp. 106-122). | https://archive.org/download/techniqueofmoder00widouoft/techniqueofmoder00widouoft_djvu.txt | handbook | yes |
| S6 | Wikipedia, "Snare drum". | https://en.wikipedia.org/wiki/Snare_drum | encyclopedia | yes |
| S7 | Wikipedia, "Bass drum" (concert bass drum section). | https://en.wikipedia.org/wiki/Bass_drum | encyclopedia | yes |
| S8 | Wikipedia, "Clash cymbals". | https://en.wikipedia.org/wiki/Clash_cymbals | encyclopedia | yes |
| S9 | Wikipedia, "Suspended cymbal". | https://en.wikipedia.org/wiki/Suspended_cymbal | encyclopedia | yes |
| S10 | Wikipedia, "Cymbal". | https://en.wikipedia.org/wiki/Cymbal | encyclopedia | yes |
| S11 | Wikipedia, "Gong" (section "Chau gong (tam-tam)"; "Tam-tam" redirects here). Read up to character 100,000 of 108,000; the unread tail is references and footer. | https://en.wikipedia.org/wiki/Gong | encyclopedia | yes |
| S12 | Wikipedia, "Triangle (musical instrument)". | https://en.wikipedia.org/wiki/Triangle_(musical_instrument) | encyclopedia | yes |
| S13 | Wikipedia, "Tambourine" (first 100,000 characters). | https://en.wikipedia.org/wiki/Tambourine | encyclopedia | yes |
| S14 | Wikipedia, "Castanets". | https://en.wikipedia.org/wiki/Castanets | encyclopedia | yes |
| S15 | Wikipedia, "Woodblock (instrument)". | https://en.wikipedia.org/wiki/Woodblock_(instrument) | encyclopedia | yes |
| S16 | Wikipedia, "Temple blocks". | https://en.wikipedia.org/wiki/Temple_blocks | encyclopedia | yes |
| S17 | Wikipedia, "Jingle bell" ("Sleigh bells" redirects here). | https://en.wikipedia.org/wiki/Jingle_bell | encyclopedia | yes |
| S18 | Wikipedia, "Maraca". | https://en.wikipedia.org/wiki/Maraca | encyclopedia | yes |
| S19 | Wikipedia, "Shaker (musical instrument)". | https://en.wikipedia.org/wiki/Shaker_(musical_instrument) | encyclopedia | yes |
| S20 | Wikipedia, "Clapper (musical instrument)" (section whip/slapstick; "Slapstick (instrument)" redirects here). | https://en.wikipedia.org/wiki/Clapper_(musical_instrument) | encyclopedia | yes |
| S21 | Wikipedia, "Orchestra" (instrumentation by era; first 100,000 characters). | https://en.wikipedia.org/wiki/Orchestra | encyclopedia | yes |
| S22 | Wikipedia, "Percussion section". | https://en.wikipedia.org/wiki/Percussion_section | encyclopedia | yes |
| S23 | Wikipedia API query of the interlanguage links (cs, it, de, fr) of the English articles above, with their redirects. These are article titles, not score terms. | https://en.wikipedia.org/w/api.php?action=query&prop=langlinks&titles=Snare_drum%7CBass_drum%7CClash_cymbals%7CSuspended_cymbal%7CTam-tam%7CTriangle_(musical_instrument)%7CTambourine%7CCastanets%7CWood_block%7CTemple_block%7CSleigh_bells%7CMaracas%7CShaker_(musical_instrument)%7CSlapstick_(instrument)%7CGong&lllimit=max&format=json&redirects=1 | encyclopedia | yes |
| S24 | VSCO 2 Community Edition, branch `SFZ`: `GM-StylePerc.sfz` (read and parsed) and the repository file tree (GitHub API, 3,399 entries, sample names of `Percussion/` and `VSCO 1 Percussion/`). | https://raw.githubusercontent.com/sgossner/VSCO-2-CE/SFZ/GM-StylePerc.sfz ; https://api.github.com/repos/sgossner/VSCO-2-CE/git/trees/SFZ?recursive=1 | library maker | yes |
| S25 | VSCO 2 CE `LICENSE` (CC0 1.0 Universal) and `README.md` (branch master). | https://github.com/sgossner/VSCO-2-CE | library maker | yes |
| S26 | VCSL (Versilian Community Sample Library): README (licence CC0, layers and round robins) and the folder listings `Idiophones/Struck Idiophones`, `Membranophones/Struck Membranophones`. Mappings not inspected. | https://github.com/sgossner/VCSL | library maker | yes |
| S27 | Sonatina Symphonic Orchestra (peastman/sso): `LICENSE` (Creative Commons Sampling Plus 1.0), `README.md`, `Sonatina Symphonic Orchestra/Percussion/All Unpitched Percussion.sfz` (read and parsed; its note names are converted with the SFZ default c4 = 60, which the file does not state: d2 = 38 ... d#5 = 75), folder `Sonatina Symphonic Orchestra/Samples/Percussion`. | https://github.com/peastman/sso | library maker | yes |
| S28 | Spitfire Audio Help Centre, "What are the Untuned Percussion instruments used in BBC SO Discover?" (key list C2-D4). | https://support.spitfireaudio.com/en/articles/11815993 | library maker | yes |
| S29 | Big Fish Audio, "London Orchestral Percussion" manual (PDF; Kontakt 7): user interface, keyboard layout, licence. | https://www3.bigfishaudio.com/downloads/LondonOrchestralPercussionManual.pdf | library maker | yes |
| S30 | Apple, Logic Pro User Guide for Mac, "Use the Articulation Set Editor in Logic Pro for Mac". | https://support.apple.com/guide/logicpro/manage-articulations-articulation-set-editor-lgcp33a49091/mac | manufacturer | yes |
| S31 | Sonara repository (read locally): `crates/sonara-midi/src/gm.rs`, `crates/sonara-midi/src/drums.rs`, `crates/sonara-midi/src/orchestration/data.rs` and `articulation.rs`, `crates/sonara-dsp/src/processors/drums.rs`, `crates/sonara-app/src/catalog.rs`, `docs/sample-libraries.md`, `docs/orchestral-audio-engine-research.md`, the file list of `golden/numpy/drums/`. | local checkout of the Sonara repository, branch `research/orchestra-instruments` (no public URL) | other | yes |

Not opened, so never cited: the GM1 spec PDF (login wall), Vienna Symphonic Library pages (the site serves a JavaScript shell with no text), Adler, Blatter, the GM2 spec, any Roland PDF (encrypted; S2 is the HTML text copy).

Convention for the JSON field `clefs`: every card lists `percussion` (the neutral percussion clef on a one-line or five-line stave) as the modern engraving default, beside the handbook notations its section 3 cites (single line, conventional treble or bass clef) [Estimate: modern engraving practice; no source read states it]. Where section 3 names no clef, `percussion` comes from this convention alone.

## Percussion mapping (GM1, Roland GS Orchestra kit, Sonara names)

How to read the table. GM1 column: S1 (channel 10, keys 35-81; "only these sounds are supported by General MIDI Level 1 devices") [Cited S1]. GS column: the ORCHESTRA set, program 49, of the SC-55 drum-set table [Cited S2]. The extracted table gives the sounds row by row but not each row's number. The numbers below are derived by aligning the rows with the STANDARD column, whose names follow the GM order (row "Kick Drum2" = 35 ... row "Open Triangle" = 81), and the grid is labelled C2-C7, so 36 = C2 in Roland's naming [Cited S2; the numbering is derived, see Open questions]. Sonara columns: `GM_MAP` (34 notes) and `DRUM_NAMES` (20 names) in `gm.rs`, the `cinematic` style in `drums.rs` [Cited S31].

Rule for the JSON field `gm_percussion_notes`: it lists the GM1 channel-10 notes whose GM1 name denotes the instrument, exact match first. The two cymbal cards are the one exception: they list the kit crash (and for the suspended cymbal only 49) as the only GM1 cymbals of that kind, flagged as a substitution in their section 9. Instruments that exist only in GS (castanets, sleigh bells, shaker) or nowhere (tam-tam, temple blocks, whip) have an empty list; nearest substitutes are named in prose only [Estimate: modelling rule chosen for this file].

| id | GM1 note and GM name [S1] | GS ORCHESTRA kit, PC 49 [S2] | Sonara `GM_MAP` today [S31] | Sonara `DRUM_NAMES` [S31] | In the `cinematic` style? [S31] | Gap today |
|---|---|---|---|---|---|---|
| snare_drum | 38 Acoustic Snare (exact); 40 Electric Snare (kit); 37 Side Stick (rim click) | 38 and 40 Concert SD; 37 Side Stick (←) | 38, 40 `snare`; 37 `rim` | `snare` = 38, `rim` = 37 | no row; the optional fill plays `SNARE` 38 | no name for roll, snares off, flam, rimshot |
| bass_drum | 35 Acoustic Bass Drum; 36 Bass Drum 1 | 35 Concert BD2; 36 Concert BD1 | 35, 36 `kick` | `kick` = 36 | yes: `KICK` 36 on beat 1 | `kick` is the kit drum; no concert bass drum, no roll |
| clash_cymbals | no exact sound; nearest 49 Crash Cymbal 1, 57 Crash Cymbal 2 (kit crashes) | 57 Concert Cymbal2; 59 Concert Cymbal1 | 49, 57 `crash`; 59 `ride` | `crash` = 49 | only with `crash_on_first` (49) | clash pair not distinct from kit crash; GS 59 collides with `ride` |
| suspended_cymbal | none; nearest 49/57 crash, 51/59 ride, 55 splash | none; in this kit 49 and 51 are Timpani c# and d#, 57 and 59 are Concert Cymbal2 and 1, only 55 Splash Cymbal stays as STANDARD | `crash`, `ride`, `splash`, `china`, `ride_bell` | `crash`, `ride`, `ride_bell` | no | no suspended cymbal, no roll or swell |
| tam_tam | none | none | none | none | no | missing |
| triangle | 80 Mute Triangle; 81 Open Triangle | 80, 81 (←), exclusive group EXC5 | not mapped (no 80, 81) | none | no | missing, although GM1 has it |
| tambourine | 54 Tambourine | 54 (←) | 54 `tambourine` | `tambourine` = 54 | no | no shake, roll, thumb roll |
| castanets | none | 39 Castanets (ORCHESTRA only); 85 Castanets (all seven kits of the SC-55 table read) | 39 is `clap` | none | no | missing |
| wood_block | 76 Hi Wood Block; 77 Low Wood Block | 76, 77 (←) | 76 `block_high`; 77 `block_low` | none | no | covered by `GM_MAP` only |
| temple_blocks | none | none | none | none | no | missing |
| sleigh_bells | none | 83 Jingle Bell (STANDARD, ←) | none | none | no | missing |
| maracas | 70 Maracas | 70 (←) | 69, 70 `shaker` | `shaker` = 70 | no | called `shaker` |
| shaker | none (nearest 69 Cabasa, 70 Maracas) | 82 Shaker (STANDARD, ←) | 69, 70 `shaker` | `shaker` = 70 | no | name collides with GM maracas |
| whip | none | none (STANDARD row 28 "Slap" becomes Pedal Hi-hat in ORCHESTRA) | none | none | no | missing |

Further facts for the mapping:

- The GS ORCHESTRA kit also changes 27-30 (Closed, Pedal and Open Hi-hat, Ride Cymbal1), 41-53 (Timpani F, F#, G ... e, f, chromatic, F2-F3 at C4 = 60) and adds 88 Applause (two voices) [Cited S2, numbering derived]. Every other row is "←", so 54 Tambourine, 70 Maracas, 76/77 Wood Blocks, 80/81 Triangles, 82 Shaker, 83 Jingle Bell and 85 Castanets keep their STANDARD sounds [Cited S2].
- Collision: Sonara's `cinematic` rows `TOM_FLOOR` (41), `TOM_LOW` (45) and `KICK` (36) [Cited S31] would sound as Timpani F, Timpani A and Concert BD1 on a GS ORCHESTRA kit, and the `clap` (39) of other styles as Castanets [Cited S2, S31]. Likewise `CRASH` 49 (added by `crash_on_first`) and `RIDE` 51 would sound as Timpani c# and d#, and the hi-hats 42/44/46 as Timpani F#, G# and A# [Cited S2, S31; numbering derived, see Open question 1].
- GM1 notes absent from `GM_MAP`: 58, 65, 66, 67, 68, 71, 72, 73, 74, 78, 79, 80, 81 (13 notes; 34 + 13 = 47) [Cited S1, S31]. The built-in Drum Kit returns no sound for an unmapped note (`_ => return None`) [Cited S31], so a GM triangle part is silent today.
- The 29 synthesized drum sounds have golden vectors in `golden/numpy/drums/` (for example `shaker`, `block_high`, `crash`) [Cited S31]. Changing `GM_MAP` touches GLD-01 data; an orchestral percussion map is better added beside it than inside it [Estimate: reading of the project rules, not an owner decision].
- Orchestral sounds with no Sonara name today: concert bass drum, snare roll, snares-off snare, clash cymbals as distinct from a kit crash, suspended cymbal and its roll, tam-tam, triangle (open and muted), castanets, sleigh bells, temple blocks, whip, and any roll of any instrument; maracas exist only under the name `shaker` [Cited S31 for what exists; the list of gaps is derived from it].
- Players: no source read gives the number of percussionists of an orchestra. S21 lists the instruments per era and says "Section percussionists play parts assigned to them by the principal percussionist" [Cited S21]. A late-Romantic or modern orchestra usually has a timpanist plus about 2-4 percussionists, each covering several instruments of this file [Estimate: general orchestral practice, no source read].

## snare_drum: Snare drum

```json
{"id": "snare_drum", "family": "percussion", "names": {"en": "Snare drum", "cs": "malý buben", "it": "tamburo militare", "de": "kleine Trommel", "fr": "caisse claire", "score_abbreviation": "S.D."}, "pitched": false, "transposition": null, "clefs": ["single_line", "treble", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate), S5", "comfortable": "n/a, unpitched: Cited S4, S5"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [38, 40], "existing_sonara_id": "snare_drum", "articulations_existing_vocab": ["roll", "marcato", "con_sordino"], "articulations_proposed_new": ["flam", "drag", "rimshot", "rim", "snares_off"], "articulations_provenance": "Cited S4 (roll, flam, drag, accents, muffled), S5 (sans timbre), S6 (rimshot, cross-stick)", "solo_or_section": "solo", "typical_players": {"classical": "rare; military and Turkish colour in opera (Estimate)", "romantic": "1 player, often also covering other battery instruments (Estimate)", "film": "1 player or a sampled ensemble of snares (Estimate)", "chamber": "1 percussionist with a multi-percussion set-up (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | snare drum; side drum (British, "Side-Drum") | [Cited S6] [Cited S4] [Cited S5] |
| Czech | "Vířivý buben" (title of the cs Wikipedia article) | [Cited S23] |
| Czech | "malý buben" as the usual score term (recommended for `names.cs`) | [Estimate: common Czech usage; no Czech score or handbook was read] |
| Italian | "Tamburo militare" (Forsyth); "Tamburo piccolo" (Widor); "Rullante" (it Wikipedia title) | [Cited S4] [Cited S5] [Cited S23] |
| German | "Kleine Trommel" | [Cited S4] [Cited S5] [Cited S23] |
| French | "Caisse claire"; "Tambour (militaire)"; Widor "Tambour" | [Cited S4] [Cited S5] [Cited S23] |
| Score | entry marked "S.D." when it shares a stave | [Cited S4] |
| Family | percussion; Forsyth's "Unmusical percussion instruments" No. 1, a membrane instrument | [Cited S4] |

Recommendation: keep all three Italian names as aliases; "tamburo militare" goes in `names.it` because it is the heading term in S4 [Estimate: choice of the primary alias].

### 2. Sound production

| Fact | Provenance |
|---|---|
| Two parchment heads: the upper "batter-head" and the lower "snare-head"; the snares (catgut, two or three to a dozen) are stretched across the snare-head | [Cited S4] |
| A lever (strainer) engages or disengages the snares; with snares off the drum "resembles that of a floor tom-tom" | [Cited S6] |
| The snared drum sounds about an octave higher than the same drum unsnared; its brilliance depends on the snares | [Cited S4] |
| Shell of brass (1914); sheepskin heads (1906) | [Cited S4] [Cited S5] |
| Two hard-wood sticks with a small olive-shaped knob | [Cited S4] |
| Typical diameter 14 in (36 cm); orchestral and kit shells about 6 in (15 cm) deep | [Cited S6] |
| Technique rests on double strokes per hand: the long roll is LL-RR, the second stroke a controlled rebound | [Cited S4] |
| The snares are "a fundamental ingredient in the pressed (buzz) drum roll" | [Cited S6] |
| No definite pitch; "suited to all keys" | [Cited S5] |
| Modern heads are usually plastic | [Estimate: general knowledge, no source read] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range and no transposition: "Pitch indeterminate" in Forsyth's table [Cited S4]; no definite pitch [Cited S5].
- Notation: a single line, or conventionally a treble-clef stave [Cited S4] [Cited S5]. With another part on the same stave, stems up, entry "S.D.", exit by a rest [Cited S4]. Modern parts often use a five-line stave with the neutral percussion clef [Estimate: engraving practice, no source read].
- Register: the struck spot (centre, edge, rim) changes the colour; there are no register breaks in the pitched sense [Estimate: percussion practice; S6 does not discuss it].
- MIDI: libraries put the snare on arbitrary keys (VSCO 37-41, SSO 39-40, BBC SO Discover E2) [Cited S24] [Cited S27] [Cited S28].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Dynamics | the long roll is "equally effective p or f", but its crescendo cannot be spread over as many bars as the kettle-drums' | [Cited S4] |
| Soft end | responds "audibly to the softest strokes, even with a wire brush" | [Cited S6] |
| Colour | "hard, dry, and bright, with something perky, obstinate, and combative about it"; calls up war and military bustle | [Cited S4] |
| Register group | Rimsky counts the side drum among the medium instruments (high: triangle, castanets, little bells; medium: tambourine, rute, side drum, cymbals; deep: bass drum, gong) | [Cited S3] |
| Agility | "practically any rhythmic combinations" are intelligible on it | [Cited S4] |
| Single strokes | Forsyth: single detached notes "should never be written"; Widor: the single stroke "is rarely used" | [Cited S4] [Cited S5] |
| End of a roll | write a detached finishing note; a sudden stop sounds untidy except in the faintest pianissimo | [Cited S4] |
| Combination | side-drum tremolo with sustained trumpet and horn chords; with triangle and tambourine it suits harmony in the upper register | [Cited S3] |
| Power | bass drum, cymbals, gong and a ff side-drum tremolo "is sufficient to overpower any orchestral tutti" | [Cited S3] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| single and accented strokes | `marcato` | accents [Cited S4] | velocity layers: SSO p/mf/ff (lovel 0-63, 64-79, 80-127) [Cited S27] |
| long roll (double-stroke) and pressed (buzz) roll | `roll` | [Cited S4] [Cited S6] | VSCO: separate keys 39 (`rollSN`) and 41 (`rollNS`) [Cited S24], read as snares on and off [Estimate: file names only]; SSO: key 40 plays `snare-roll.wav` while held (`ampeg_release=0.1`) and a `trigger=release_key` region replays the same file from `offset=150243` on key release; the group also sets `transpose=-1 tune=-50` [Cited S27]; that the release region is the roll's end stroke, and whether the held sample loops (no `loop_mode` in the SFZ; it depends on loop points in the WAV), are Estimates; London Orchestral Percussion: rolls at the top of each instrument's mapped keys [Cited S29] |
| flam (open, closed) | `flam` (preferred list) | [Cited S4] | no flam samples in VSCO, SSO or BBC SO Discover lists [Cited S24] [Cited S27] [Cited S28] |
| drag (2-6 strokes before an accent) | `drag` (preferred list) | [Cited S4] | none found |
| rimshot (head and rim together) | `rimshot` (preferred list) | [Cited S6] | VSCO 1 raw samples `snare1_rimshot_f/mf/fff`, `snare1_rim_fff` [Cited S24] |
| rim click, cross-stick | `rim` (preferred list) | [Cited S6] | GM1 37 Side Stick [Cited S1] |
| snares off, "sans timbre" | `snares_off` (new: snare drum played with the snares disengaged; Fr. "sans timbre" [Cited S5]; It. "senza corde" [Estimate: usual Italian score marking; no source read gives it]) | [Cited S4] [Cited S5] [Cited S6] | VSCO: 40 hit and 41 roll "NS" [Cited S24] (no snares [Estimate: file names only]); VSCO 1 raw `snare1_nosnare_*` [Cited S24]; BBC SO Discover lists only "Tenor Drum (Snares Off)" at D3 [Cited S28] |
| muffled (cloth over the head) | `con_sordino` (listed in the JSON beside `snares_off`; Widor says sans timbre is obtained "either by loosening the snares, or by muffling", the choice left to the player; see Open question 18) | [Cited S5] [Cited S4] | none found |
| taps, ghost notes | velocity, not an id | ghost notes "very light filler notes" [Cited S6] | VSCO key 37 "Snare2-taps" [Cited S24] |
| brushes | beater, not an id (see Open questions) | [Cited S6] | none found |

Mechanisms. Unpitched libraries put each technique on its own key, use velocity for dynamics and round robin for repeated strokes: SSO alternates left-hand and right-hand samples with `seq_length=2` [Cited S27]; VSCO `GM-StylePerc.sfz` uses only `lovel`/`hivel` and `seq_*` [Cited S24]. London Orchestral Percussion maps basic hits from C3 upward and rolls at the top of each instrument's keys, with gaps between articulation types; key switches exist only on its two timpani patches (C0 single hits ... F0 rolls) [Cited S29]. The manual text read names no MIDI CC assignments (dynamics follow velocity, with a Vel Sens control) [Cited S29]. In Logic Pro an Articulation Set turns articulation IDs into key switches, controllers or program changes for third-party libraries, with Permanent, Momentary and Toggle switch modes [Cited S30]. The Sonara sampler plays velocity layers, round robin, key switches, release triggers (`trigger=release`) and exclusive groups, and no filters or CC-driven crossfades [Cited S31]; whether it honours SSO's `trigger=release_key` is not stated (Open questions).

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One drum per part in the orchestra [Estimate: general practice]. Military funerals use several muffled side drums with a muffled bass drum [Cited S4].
- No divisi. Doubling another percussion part is a matter of players, not of unison [Estimate: general orchestral practice; no source read states it].

### 7. Role in the orchestra and typical doublings

- In the theatre "in continual use since the time of Meyerbeer and Rossini"; concert opportunities fewer; "its principal effect is its entry" [Cited S4]. Widor in 1906: "I do not think it has ever been employed in a symphony" [Cited S5]; the later repertoire contradicts this: Ravel's Bolero, Nielsen's Fifth Symphony [Cited S6].
- Rimsky: playful rhythms for triangle, tambourine, castanets and side drum; strokes on strong beats, syncopations and sforzandi [Cited S3].
- Combined with the bass drum in marches and funeral "Drums" [Cited S4].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Fact | Provenance |
|---|---|---|
| tenor drum | between side and bass drum, wooden shell, no snares, "sombre and antique" | [Cited S4] [Cited S5] |
| tabor (Provence) | long drum; Forsyth: a single hemp snare; Widor: "without timbre" (see Open questions) | [Cited S4] [Cited S5] |
| field/marching, rope-tension snare | VSCO 1 has `drum3_marching` and `OldSnare`; VCSL has "Snare Drum, Rope Tension" | [Cited S24] [Cited S26] |
| player changes | the snare player usually also takes tambourine, triangle or cymbals in the same work | [Estimate: general practice] |

### 9. General MIDI and existing Sonara metadata

- GM1: 38 Acoustic Snare, 40 Electric Snare, 37 Side Stick [Cited S1]. No melodic program [Cited S1].
- GS ORCHESTRA kit: 38 and 40 Concert SD [Cited S2].
- Sonara: `data.rs` has `snare_drum` ("Snare drum", `Family::Percussion`, `PitchModel::Unpitched`, articulations `[Marcato, Roll]`, `ARTICULATION_ESTIMATE`) [Cited S31]. `GM_MAP` names 38 and 40 `snare`, 37 `rim`; `DRUM_NAMES` `snare` = 38 [Cited S31]. `drum_pattern` adds ghost snares when `variation` > 0 and four snare strokes in the fill [Cited S31].

### Verification of data.rs

| Field | data.rs today | Verified value | Verdict (confirmed / corrected / still estimate) | Source |
|---|---|---|---|---|
| playable range | none (`PitchModel::Unpitched`) | none, pitch indeterminate | confirmed | S4, S5 |
| comfortable range | none | none | confirmed | S4, S5 |
| transposition | none (unpitched) | none; written on a single line or treble stave as a convention | confirmed | S4, S5 |
| articulation list | `[marcato, roll]` | `roll`, `marcato`, `con_sordino` (muffled drum); plus `flam`, `drag`, `rimshot`, `rim` (preferred list) and `snares_off` (new) | corrected (extended) | S4, S5, S6 |
| articulation provenance | `Estimate` "no source was read" | Cited | corrected | S4, S5, S6 |
| family and name | Percussion, "Snare drum" | percussion; snare drum or side drum | confirmed | S4, S6 |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 1.0 Universal [Cited S25] | `GM-StylePerc.sfz`: 37 `Snare2-taps`, 38 `HitSN`, 39 `rollSN`, 40 `HitNS`, 41 `rollNS` [Cited S24]; reading SN as snares on and NS as no snares is an inference from the VSCO 1 `nosnare` file names [Estimate: not documented]; raw VSCO 1 folders `drum1`, `drum2`, `drum3_marching`, `OldSnare` with pppp-fff, no-snare, rim, rimshot, click samples [Cited S24] |
| VCSL | "Creative Commons 0" [Cited S26] | Snare Drum Modern 1-3, Rope Tension, Legacy Snares (mapping not inspected) [Cited S26] |
| Sonatina Symphonic Orchestra | Creative Commons Sampling Plus 1.0 [Cited S27] | key 39: 3 velocity layers × 2 hands; key 40: roll with a `release_key` region [Cited S27] |
| BBC SO Discover (Spitfire) | proprietary EULA, runs only in its own player [Cited S31] | E2 Snare Drum, one key [Cited S28] |
| London Orchestral Percussion (Big Fish Audio) | single end-user licence, no redistribution of samples [Cited S29] | snares with Crush, Weight/Focus, sequencer; Kontakt 7 [Cited S29] |

## bass_drum: Bass drum

```json
{"id": "bass_drum", "family": "percussion", "names": {"en": "Bass drum", "cs": "velký buben", "it": "gran cassa", "de": "große Trommel", "fr": "grosse caisse", "score_abbreviation": "B.D."}, "pitched": false, "transposition": null, "clefs": ["single_line", "bass", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate), S5", "comfortable": "n/a, unpitched: Cited S4, S5"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [35, 36], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato", "con_sordino"], "articulations_proposed_new": ["damped", "let_ring", "rute"], "articulations_provenance": "Cited S4 (roll, damp, muffled, Ruthe), S5 (roll with single or double stick), S7 (rolls, repetitions)", "solo_or_section": "solo", "typical_players": {"classical": "18th-century opera, as local colour for Eastern scenes, with cymbals and triangle (Cited S4)", "romantic": "1 player (Estimate)", "film": "1 player, often several drums layered in samples (Estimate)", "chamber": "rare (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | bass drum, concert bass drum | [Cited S7] |
| Czech | "Basový buben" (cs Wikipedia title) | [Cited S23] |
| Czech | "velký buben" as the usual score term (recommended for `names.cs`) | [Estimate: common Czech usage; no Czech source read] |
| Italian | "Gran cassa", "cassa grande", or simply "cassa"; in old scores "Tamburo grande" or "Tamburo grosso"; also "gran tamburo" | [Cited S4] [Cited S5] [Cited S7] |
| German | "Grosse Trommel"; also "Basstrommel" | [Cited S4] [Cited S5] [Cited S7] |
| French | "Grosse caisse" | [Cited S4] [Cited S5] [Cited S7] |
| Score | entries marked "B.D." or "G.C." (grosse caisse) | [Cited S4] |
| Family | percussion, membrane instrument of indefinite pitch | [Cited S4] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| Wooden cylindrical shell, narrow in depth for its diameter, parchment heads at both ends, tensioned by leather braces or rods; no snares | [Cited S4] |
| Concert bass drum "roughly 40 inches in diameter and 20 inches in width", double-headed, rod-tensioned; usually in a frame that can be set at any angle; the largest drum of the orchestra | [Cited S7] |
| Beater: a heavy stick with a large knob padded with felt; Widor: a single felt or cork knob (mailloche simple) or a double knob (mailloche double) | [Cited S4] [Cited S5] |
| "Ruthe" (birch broom) in Mozart's Il Seraglio and in Mahler's Second Symphony, 3rd movement | [Cited S4] |
| Slow irregular vibrations "something of the effect of the deepest 32 ft. pipes of the Organ" | [Cited S4] |
| No definite pitch; the orchestra can make it seem pitched (an illusion of coincident vibrations); tuning has been attempted "but the result obtained is not worth the trouble" | [Cited S4] [Cited S5] |
| A slack head "begins to sing" sympathetically in concert; a cloth should cover it when out of action | [Cited S4] |
| Size does not dictate volume; pitch and sound vary with size | [Cited S7] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range and no transposition [Cited S4] [Cited S5].
- Notation: a bass-clef stave by convention, or a single line [Cited S4] [Cited S5]. The cymbals usually share the stave, with two sets of stems and each entry named [Cited S4].
- Register: none in the pitched sense; the striking spot changes the colour [Estimate: percussion practice, no source read].
- MIDI keys in libraries: VSCO 36 (hit) and 32-35 (rubs), SSO 38, BBC SO Discover C2 [Cited S24] [Cited S27] [Cited S28].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| pp | resembles distant artillery and thunder; a single deep pp stroke gives "farawayness, of solemnity, even of hopeless desolation" | [Cited S4] |
| Uses | marks the high lights or the climax of a crescendo; conveys awe and solemnity with its pp notes | [Cited S4] |
| Agility | quick rhythmic figures are out of place: each note damps the one before; single strokes only at moderate speed | [Cited S4] |
| Decay | the head vibrates "for some little time after a mf or f stroke": write the real length, add "damp" for a short note | [Cited S4] |
| Roll | practicable and effective, with the ordinary stick, a double-headed stick, or (less effectively) two kettle-drum sticks; a favourite pianissimo opening (Liszt, Strauss's Also sprach Zarathustra) | [Cited S4] |
| Roll, Widor | with the mailloche held by the middle, "a roll resembling distant thunder"; with kettle-drum sticks, a roll deeper than the timpani's | [Cited S5] |
| Imitation | cannon: Rimsky's Tsar Saltan (bass drum without cymbals), Berlioz's Marche hongroise | [Cited S3] [Cited S5] |
| Register group | a deep instrument; its tremolo goes with trombone chords or low cello and bass notes | [Cited S3] |
| Balance | with the cymbals, the bass drum "must be thrown somewhat into the background" | [Cited S5] |
| Techniques | "rolls, repetitions and unison strokes" | [Cited S7] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| single stroke, accented climax stroke | `marcato` | [Cited S4] [Cited S7] | velocity: SSO crossfades a p and an f sample between velocity 70 and 90 (`xfin`/`xfout`) [Cited S27] |
| roll (one stick, double-headed stick, two timpani sticks) | `roll` | [Cited S4] [Cited S5] | VSCO 1 raw `bdrum_roll1`, `_roll_long1`, `_roll_short_1/2`, `_roll-quietest` [Cited S24] |
| muffled (cover drawn over, head slackened, or both) | `con_sordino` | [Cited S4] | VSCO 1 raw `bdrum_muted_ppp` ... `fff` [Cited S24] |
| damped short note ("damp") | `damped` (preferred list) | [Cited S4] | none found |
| let vibrate | `let_ring` (preferred list) | write the real length [Cited S4] | release time of the sample; none labelled |
| birch switch | `rute` (new: the drum or shell struck with a bundle of twigs, Ger. Rute/Ruthe) | [Cited S4] [Cited S3] | none found |
| rub (friction) | none | VSCO `bassdrum_rub1-4` on keys 32-35 [Cited S24] | meaning of "rub" not documented [Estimate: the sample name is the only evidence; VSCO documents none (Open question 15)] |

Libraries map the bass drum to one key with velocity layers (VSCO, SSO) [Cited S24] [Cited S27]; London Orchestral Percussion adds a pattern sequencer and a Crush effect to its bass drums [Cited S29]. Mechanisms common to this file are described under snare_drum, section 5.

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One player and one drum normally [Estimate: general orchestral practice; no source read gives a player count (Open question 9)]. Beethoven's Battle of Vittoria uses three bass drums, one in the orchestra and two for the cannonade [Cited S4].
- Some players fasten a cymbal plate to the bass drum and play both at once; Forsyth calls it "an unfortunate accident" that destroys the drum's tone [Cited S4].

### 7. Role in the orchestra and typical doublings

- The "Batterie" (bass drum, cymbals, triangle), the English "Kitchen", entered opera in the 18th century as local colour for Eastern scenes [Cited S4].
- Classical uses: "Climactic single strokes", "Adding weight to loud tutti sections" [Cited S7].
- Constantly paired with cymbals; Forsyth warns against the "zim-boom" that destroys harmony and melody [Cited S4]. Rimsky: vigorous, straightforward rhythm for bass drum, cymbals and gong [Cited S3].
- Doublings: low strings, contrabassoon and organ under a pp roll (Also sprach Zarathustra: deep C for divided basses, double bassoon and organ) [Cited S4].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Fact | Provenance |
|---|---|---|
| tenor drum | smaller, no snares | [Cited S4] |
| kit bass drum (kick) | pedal-played drum of the drum kit; Sonara's `kick` | [Estimate: not read in S7's kit section] |
| bass drum with attached cymbal | condemned by Forsyth | [Cited S4] |
| gong drum (single-headed concert bass drum) | exists in orchestras | [Estimate: general knowledge, no source read] |

### 9. General MIDI and existing Sonara metadata

- GM1: 35 Acoustic Bass Drum, 36 Bass Drum 1 [Cited S1]. No melodic program; GM1 117 Taiko Drum is a different drum [Cited S1].
- GS ORCHESTRA kit: 35 Concert BD2, 36 Concert BD1 [Cited S2].
- Sonara: no `bass_drum` in `data.rs`; `GM_MAP` 35 and 36 = `kick`; `DRUM_NAMES` `kick` = 36; the `cinematic` style strikes `KICK` on beat 1 with floor and low toms [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | `GM-StylePerc.sfz` 36 hit, 32-35 rubs; VSCO 1 raw `drums/bass`: ppp-fff hits, muted hits, rolls, a side hit (39 files) [Cited S24] |
| VCSL | CC0 [Cited S26] | Bass Drum 1, Bass Drum 2, Bass Drum 3 - Legacy [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | key 38: p and f with a velocity crossfade [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | C2 Bass Drum [Cited S28] |
| London Orchestral Percussion | end-user licence [Cited S29] | "thunderous bass drums" with Crush and sequencer [Cited S29] |

## clash_cymbals: Clash cymbals

```json
{"id": "clash_cymbals", "family": "percussion", "names": {"en": "Clash cymbals", "cs": "činely", "it": "piatti", "de": "Becken", "fr": "cymbales", "score_abbreviation": "Cym."}, "pitched": false, "transposition": null, "clefs": ["single_line", "bass", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate), S10", "comfortable": "n/a, unpitched: Cited S4, S10"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [49, 57], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["let_ring", "damped", "choke"], "articulations_provenance": "Cited S4 (two-plate stroke, two-plate roll), S5 (damped or let vibrate, clash roll), S8 (crash), S10 (l.v., secco, damped against the body)", "solo_or_section": "solo", "typical_players": {"classical": "18th-century opera, as local colour for Eastern scenes (Cited S4)", "romantic": "1 player, one pair (Estimate)", "film": "1 player; several pairs in samples (Estimate)", "chamber": "rare (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | clash cymbals; also concert, orchestral, crash or hand cymbals | [Cited S8] |
| Czech | "činely" | [Estimate: the cs Wikipedia has no clash-cymbal article in S23; common Czech word] |
| Italian | "Piatti" or "Cinelli"; it Wikipedia title "Piatti orchestrali" | [Cited S4] [Cited S5] [Cited S8] [Cited S23] |
| German | "Becken" (full word "Schallbecken"; each plate "Schale" or "Teller"); de Wikipedia title "Paarbecken" | [Cited S4] [Cited S8] [Cited S23] |
| French | "Cymbales" | [Cited S4] [Cited S8] |
| Score | "Cym." or "Piatti"; sometimes "C.C."; "with stick" or "with mallet" means the suspended cymbal instead | [Cited S4] [Cited S8] |
| Family | percussion, metal instrument of indefinite pitch | [Cited S4] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| Two large circular brass plates of equal size, each with a saucer-shaped centre and a strap; slightly tapered so they meet at the edges | [Cited S4] |
| Tone varies with size, weight and alloy; small, poor plates add "a sense of vulgarity" | [Cited S4] |
| Three weights: French/leggero/light, Viennese/medio/medium, Germanic/Wagnerian/heavy; 14 in to 22 in diameter | [Cited S8] |
| The smallest and thickest have the higher pitch; the largest the greatest volume | [Cited S8] |
| A proper crash is "played like a flam": the bottoms touch before the tops meet | [Cited S8] |
| Holding the plates parallel to the floor after the crash sends most sound to the audience | [Cited S8] |
| Crash cymbals are damped by pressing them against the player's body | [Cited S10] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range and no transposition [Cited S4] [Cited S10].
- Notation: on the bass-drum stave (bass clef by convention) or one line; two sets of stems [Cited S4]. A tie, even into a rest, means the vibration must not be stopped [Cited S5].
- Register: none; size and weight set the colour [Cited S8].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Dynamics | "anything from pp to fff" (except the hard-stick stroke, which belongs to the suspended plate) | [Cited S4] |
| Character | "martial and brilliant in the extreme" when large and of proper metal | [Cited S4] |
| Sustain | vibration "continues so long" that the exact length must be written | [Cited S4] |
| Markings | "laissez vibrer" (l.v.), "secco" | [Cited S10] |
| Use | "the effect of the Cymbals is in inverse proportion to their use" | [Cited S4] |
| Register group | medium instrument; vigorous rhythm; strokes on strong beats, syncopations, sforzandi; cymbals go with harmony in the lower register | [Cited S3] |
| Soft stroke | the soft two-plate stroke with p bass drum (Berlioz's Requiem, Meyerbeer's Les Huguenots) | [Cited S4] [Cited S5] |
| Agility | a two-plate roll is possible but varies "from a rather commonplace ... pianissimo to a fortissimo of some violence" | [Cited S4] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| two-plate stroke (crash) | `marcato` (accented) or plain note | [Cited S4] [Cited S8] | VSCO 49: `cymbal-crash1` pp/mp/mf/ff × 2 round robins [Cited S24]; SSO 45 `piatti.wav` one sample [Cited S27]; BBC SO Discover B3 Piatti [Cited S28] |
| two-plate roll (plates agitated against each other; Tannhäuser overture) | `roll` | [Cited S4] [Cited S5] | none found as a clash roll |
| let vibrate | `let_ring` (preferred list) | [Cited S10] [Cited S5] | sample tail; VSCO raw `crash_hit_ff_loose` vs `_tight`, `_verytight` [Cited S24] |
| damped (secco) | `damped` (preferred list) | [Cited S5] [Cited S10] | none labelled |
| choke right after the crash | `choke` (preferred list) | damping against the body [Cited S10] | BBC SO Discover A3 "Stopped Cymbal" (which cymbal is not stated) [Cited S28] |

A damped or choked cymbal in a sampler needs a note-off that cuts the tail: either a short `ampeg_release` or an exclusive group that a "choke" key stops (`group`/`off_by`), which the Sonara sampler plays [Cited S31]; SSO uses this for the triangle, not for cymbals [Cited S27].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One matched pair per player [Cited S8]. Several pairs in one work (different weights) happen in late-Romantic scores [Estimate: no source read].
- One player may cover clash and suspended cymbal, which composers often write on one part [Cited S8].

### 7. Role in the orchestra and typical doublings

- Part of the 18th-century "Batterie" with bass drum and triangle [Cited S4].
- Continually associated with the bass drum, "though there is no good reason why they should be" [Cited S4].
- Climax and accent instrument with brass; with bass drum, gong and side drum ff it can overpower any tutti [Cited S3].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Fact | Provenance |
|---|---|---|
| suspended cymbal | a single plate on a stand, struck with sticks or mallets (own card) | [Cited S8] [Cited S9] |
| ancient cymbals (crotales) | small, tuned (Berlioz: one pair in B-flat, one in F, in Les Troyens and Roméo); pitched, not in this file | [Cited S4] |
| Chinese clash cymbals | swung, bells released: less damping, longer sustain | [Cited S8] |
| marching cymbals | marched in a drumline | [Cited S8] |

### 9. General MIDI and existing Sonara metadata

- GM1 has no clash cymbals; 49 Crash Cymbal 1 and 57 Crash Cymbal 2 are drum-kit crashes and are the nearest substitute [Cited S1; substitution is an Estimate].
- GS ORCHESTRA kit: 57 Concert Cymbal2, 59 Concert Cymbal1 [Cited S2]. That they are clash cymbals is not stated [Estimate: inferred from Roland's name "Concert Cymbal"; S2 gives names only]. On that kit note 49 sounds Timpani c#, so of the two GM crash notes only 57 stays a cymbal there [Cited S2; numbering derived, see Open question 1].
- Sonara: no `clash_cymbals` in `data.rs`; `GM_MAP` 49 and 57 `crash`, 59 `ride`; `DRUM_NAMES` `crash` = 49; `crash_on_first` adds 49 on beat 1 [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | key 49 crash pp-ff × 2 RR; VSCO 1 raw `Cymbals/clash` (loose, tight, very tight; pp-fff) [Cited S24] |
| VCSL | CC0 [Cited S26] | Clash Cymbals 1, Clash Cymbals 2 [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | key 45 piatti, one sample [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | B3 Piatti; A3 Stopped Cymbal [Cited S28] |
| London Orchestral Percussion | end-user licence [Cited S29] | "expressive cymbals" with a swell creator [Cited S29] |

## suspended_cymbal: Suspended cymbal

```json
{"id": "suspended_cymbal", "family": "percussion", "names": {"en": "Suspended cymbal", "cs": "zavěšený činel", "it": "piatto sospeso", "de": "hängendes Becken", "fr": "cymbale suspendue", "score_abbreviation": "sus. cym."}, "pitched": false, "transposition": null, "clefs": ["single_line", "bass", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S10 (indefinite pitch)", "comfortable": "n/a, unpitched: Cited S10"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [49], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["crescendo_swell", "let_ring", "damped", "choke", "bowed", "scrape"], "articulations_provenance": "Cited S4 (hard and soft stick strokes, two-stick roll, finger damping), S5 (suspended roll), S9 (mallet roll, bow, scrape), S10 (roll swell, coin or triangle-beater scrape, bell)", "solo_or_section": "solo", "typical_players": {"classical": "not used (Estimate)", "romantic": "1 player, often the cymbal player (Estimate)", "film": "1-2 players; swells are a staple (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | suspended cymbal; "sus. cym." or "sus. cymb." | [Cited S9] |
| Czech | "zavěšený činel" | [Estimate: no cs article in S23] |
| Italian | "Piatto sospeso" (it Wikipedia title; S9 itself gives no Italian name) | [Cited S23] |
| German | "hängendes Becken" | [Estimate: no de article in S23] |
| French | "cymbale suspendue" | [Estimate: no fr article in S23] |
| Score | "with stick" or "with mallet" over a cymbal part; in Russian notation a "+" over the note | [Cited S8] |
| Family | percussion, metal, indefinite pitch | [Cited S10] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| Played with a stick or beater rather than struck against another cymbal | [Cited S9] |
| Forsyth's method (2): a single plate struck with a hard side-drum stick (only ff, "at moments of wild excitement") or a soft kettle-drum stick (any dynamic; in pp "a deep gong-like effect", the OCR of the dynamic is unclear) | [Cited S4] |
| Forsyth's method (4): a plate hung up and rolled with two soft kettle-drum sticks, beaten at opposite sides of the circumference; the plate should not be held in the hand | [Cited S4] |
| Roll: a single-stroke roll on the outside edge with soft mallets, one on each side | [Cited S9] |
| A contrabass bow drawn slowly across the rim [Cited S9]; scraping with a triangle beater [Cited S9], or a coin or triangle beater drawn rapidly across the ridges for a "zing" [Cited S10] | [Cited S9] [Cited S10] |
| The bell gives a higher "pinging" pitch than the rest of the cymbal | [Cited S10] |
| Larger cymbals are usually louder with longer sustain | [Cited S10] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range: "the majority of cymbals are of indefinite pitch" [Cited S10]; no transposition.
- Notation: shares the cymbal part or the bass-drum stave [Cited S8] [Cited S4]; the roll uses the ordinary roll notation with "Soft Drum-sticks" or "Timpani sticks" written above [Cited S4].
- Zones instead of registers: bell, bow (middle), edge [Cited S10 for the bell; the rest is an Estimate].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Swell | a roll with two mallets on opposite sides swells smoothly from barely audible to an overwhelming climax | [Cited S10] |
| Crescendo | "no other sound in the orchestra quite so exciting as a long crescendo finely performed with two Kettle-Drum sticks" | [Cited S4] |
| Evenness | Widor: for piano passages or a crescendo, one cymbal hung by its strap gives "a perfectly even and continuous roll"; the sound "swells with great regularity" | [Cited S5] |
| Diminuendo | a rapid diminuendo "by means of the finger-tips without any sticks" | [Cited S4] |
| Combination | cymbals struck with drum sticks under sustained trumpet and horn chords | [Cited S3] |
| Physical | one-handed rolls (other hand holding the plate) cause oscillation and are less effective | [Cited S4] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| stroke with soft mallet | `marcato` or plain note | [Cited S4] | VSCO 51 `susCymb1-hit` pp/mp/f/fff × 2 RR [Cited S24] |
| stroke with hard stick | plain note, beater "stick" | [Cited S4] | VSCO 59 `susCymb1-hitstick` pp/mp/f/fff × 2 RR [Cited S24] |
| roll | `roll` | [Cited S4] [Cited S9] | BBC SO Discover G2 "Sus. Cymbal Roll" [Cited S28]; SSO 41-43 `cymbal_roll-a` f/mf/p and 44 `cymbal_roll-b` as fixed recordings [Cited S27] |
| crescendo roll (swell) | `crescendo_swell` (preferred list) | [Cited S10] [Cited S4] | VSCO 47/48/50 `susCymb1-cresc` short/median/long as fixed-length samples [Cited S24]; London Orchestral Percussion "cymbal swell creator" with a drawn curve, time in seconds or beats [Cited S29] |
| let vibrate, damped, choke | `let_ring`, `damped`, `choke` (preferred list) | [Cited S4] [Cited S10] | BBC SO Discover A3 "Stopped Cymbal" [Cited S28] |
| bowed | `bowed` (new: a bass bow drawn across the edge of a cymbal or tam-tam) | [Cited S9] (S10 does not mention bowing) | VSCO 101-104 `susCymb1-bow-1..4` [Cited S24] |
| scrape | `scrape` (new: a beater drawn across the surface of a cymbal or tam-tam) | [Cited S9] | VSCO raw `susCymb1-scrape1/2`; VSCO 1 `susp_FX_scrape_*` [Cited S24] |
| bell stroke | none (zone) | [Cited S10] | VSCO raw `susCymb1-hit-bell_pp/mf/fff` [Cited S24] |

A swell recorded at a fixed length only fits one tempo; a CC-driven swell or a GUI "swell creator" fits any length [Cited S29 for the GUI tool; the tempo problem is an Estimate]. The Sonara sampler has no CC crossfades [Cited S31].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One cymbal per part; composers often condense clash and suspended cymbal into one part [Cited S8]. Several suspended cymbals of different sizes in one set-up are common [Estimate: general percussion practice; no source read states it].

### 7. Role in the orchestra and typical doublings

- Swells into climaxes and soft shimmering colour; in Widor's two roll types it is the piano/crescendo option [Cited S5].
- Doubles trumpets and horns on sustained chords [Cited S3]; timpani rolls and crescendos [Estimate: general orchestration practice; no source read pairs it with the timpani].

### 8. Variants and related instruments (when a player changes instrument)

- Clash cymbals (own card) [Cited S8]. Kit crash, ride, splash and China cymbals, which Sonara names [Cited S31]. Sizzle cymbal, finger cymbals (SSO 59-60) [Cited S27 for finger cymbals; sizzle is an Estimate].

### 9. General MIDI and existing Sonara metadata

- GM1 has no suspended cymbal; a stick stroke is nearest to 49 Crash Cymbal 1 [Cited S1; substitution is an Estimate]. Supporting evidence: S9 says most drum kits contain "at least two suspended cymbals: a crash cymbal and a ride cymbal" [Cited S9]; against it, VSCO's GM-style map puts its suspended-cymbal mallet hits on 51 and stick hits on 59, the GM ride notes [Cited S24]. On a GS ORCHESTRA kit 49 sounds Timpani c# [Cited S2; numbering derived, see Open question 1]. GM1 melodic program 120 is "Reverse Cymbal", an effect, not this instrument [Cited S1].
- GS ORCHESTRA kit: no suspended cymbal [Cited S2].
- Sonara: none; `crash`, `ride`, `ride_bell`, `splash`, `china` are kit cymbals [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | the richest free set read: hits (mallet, stick, bell), three crescendo lengths, four bowed notes, scrapes; VSCO 1 `Cymbals/susp` (29 files: scrapes, hard-mallet and metal hits) [Cited S24] |
| VCSL | CC0 [Cited S26] | Suspended Cymbal 1, Suspended Cymbal 2 [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | four roll samples (41-44) [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | F2 hit, G2 roll, A3 stopped [Cited S28] |

## tam_tam: Tam-tam

```json
{"id": "tam_tam", "family": "percussion", "names": {"en": "Tam-tam", "cs": "tam-tam", "it": "tam-tam", "de": "Tamtam", "fr": "tam-tam", "score_abbreviation": "T.-t."}, "pitched": false, "transposition": null, "clefs": ["single_line", "bass", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate), S5, S11", "comfortable": "n/a, unpitched: Cited S5, S11"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["let_ring", "damped", "crescendo_swell", "scrape", "bowed"], "articulations_provenance": "Cited S4 (single stroke, crescendo roll, damping), S5 (damp before a harmony change), S11 (priming, damped notes, scrape, bow)", "solo_or_section": "solo", "typical_players": {"classical": "not used before Gossec, 1790/1791 (Cited S11)", "romantic": "1 player (Estimate)", "film": "1 player; large tam-tam hits and scrapes are common (Estimate)", "chamber": "rare (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | tam-tam; in scores often "gong", which players usually read as tam-tam | [Cited S11] |
| Forsyth | "The Gong. Fr. It. and Ger. Tam-tam." | [Cited S4] |
| Widor | "The Gong (ital., Tam-tam. Ger., Tam-tam. Fr., Tam-tam.)" | [Cited S5] |
| Rimsky-Korsakov | "Chinese gong" | [Cited S3] |
| Czech | "tam-tam" | [Estimate: the cs title for the Gong article is "Gong" [Cited S23]; no cs tam-tam article read] |
| German spelling | "Tamtam" (modern) vs "Tam-tam" (Widor) | [Estimate for "Tamtam"] [Cited S5] |
| Score | one line without clef | [Cited S4] |
| Score abbreviation | "T.-t." | [Estimate: common abbreviation, no source read] |
| Family | percussion, metal, indefinite pitch | [Cited S4] [Cited S11] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| A broad circular plate of thick hammered metal, edge turned over, "the shape of a shallow sieve"; Chinese, Japanese and Burmese gongs judged superior (1914) | [Cited S4] |
| Chau gong (tam-tam): almost flat except the turned-up rim; a flat surface "vibrates in multiple modes, giving a crash rather than a tuned note"; 20-200 cm diameter | [Cited S11] |
| Beater: "A soft Bass-Drum stick is generally used"; larger gong, larger and softer hammer | [Cited S4] [Cited S11] |
| Priming: light strokes before the main one make the instrument "speak" sooner, with a shorter delay for the sound to "bloom" | [Cited S11] |
| A single heavy blow on an unprepared gong "produces nothing but a dull unpleasant sound"; it needs "a continual, persistent, and graduated attack" | [Cited S4] |
| No definite pitch, yet "it seems to borrow the tonality of the chord in which it is struck" | [Cited S5] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition [Cited S4] [Cited S5] [Cited S11].
- Notation: one line without clef; write exact note lengths because of the long vibration [Cited S4]. Forsyth's instrument table (pp. 10-11) gives the Gong "Pitch indeterminate" and "Conventional bass-clef or a single line" [Cited S4; the table's columns were aligned from the OCR text, consistent with the Wind-Machine row's bass clef that the text confirms].
- A tuned (bossed) gong is pitched and belongs in a separate pitched id (section 8) [Cited S11 for the distinction; the id split is a recommendation].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Dynamics (Forsyth) | "cannot be used in the orchestra for much else than a single p or mp stroke"; a crescendo roll pp to ff is possible | [Cited S4] |
| Dynamics (Widor) | "terrifying when struck forte, and threatening even when struck piano" | [Cited S5] |
| Character | "by turns solemn, mysterious, and terrifying" (theatre from La Vestale to Madame Butterfly); Berlioz: "for scenes of mourning or for the dramatic depiction of extreme horror" | [Cited S4] [Cited S11] |
| Sustain | long-sustained vibrations; damp it before the harmony changes, otherwise it seems to hold the previous chord "like a Pianoforte without dampers" | [Cited S5] |
| Damping | "somewhat difficult to damp a large gong" | [Cited S4] |
| Register group | deep; tremolo on the gong with trombone chords or low cellos and basses | [Cited S3] |
| Altitude | Widor orders cymbals, gong and bass drum as three "sonorous altitudes" | [Cited S5] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| single stroke (primed) | plain note or `marcato` | [Cited S4] [Cited S11] | VSCO `gongHit` p/mf/f/fff on key 46 [Cited S24]; SSO 67-69 `tamtam-a/b/c` [Cited S27]; BBC SO Discover B2 Tam Tam [Cited S28] |
| roll, crescendo roll | `roll`, `crescendo_swell` | [Cited S4] [Cited S3] | VSCO 1 raw `gong_roll_long_f/ff`, `gong_roll_medium_f/ff` [Cited S24] |
| let vibrate | `let_ring` | [Cited S4] | the sample tail; a long `ampeg_release` (VSCO global `ampeg_release=3`) [Cited S24] |
| damped, quickly damped notes (Rite of Spring) | `damped` | [Cited S5] [Cited S11] | none labelled |
| scrape with a triangle beater (Rite of Spring) | `scrape` (new, defined under suspended_cymbal) | [Cited S11] | VSCO `gongscrape` pp/mf on key 42; VSCO 1 `gong_scrape_1-3` [Cited S24] |
| bowed (Crumb, rosined bass bow) | `bowed` (new) | [Cited S11] | none found |
| water gong (lowered into water after the stroke) | none (effect) | [Cited S11] | none found |
| "smack" | none | VSCO 1 raw `gong_smack_*` [Cited S24] | meaning not documented [Estimate: the sample name is the only evidence; VSCO documents none (Open question 15)] |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One instrument and one player normally [Estimate: general orchestral practice; no source read gives a player count (Open question 9)]. Parsifal at Bayreuth: four tam-tams tuned to the four bell notes, plus a fifth rolled continuously [Cited S4]; S11 confirms that four tam-tams are used at Bayreuth [Cited S11].

### 7. Role in the orchestra and typical doublings

- First orchestral use by Gossec in 1790 (another passage of the same article: the funeral march for Mirabeau, 1791) [Cited S11].
- A single soft stroke has "a startlingly picturesque effect" but can hardly be used more than once in a work [Cited S4].
- Imitating a church bell (Rimsky's Ivan the Terrible) [Cited S3]; reinforcing stage bells (Parsifal) [Cited S4].
- Doublings: trombones, low strings, bass drum; with bass drum, cymbals and ff side drum it overpowers any tutti [Cited S3].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Fact | Provenance |
|---|---|---|
| tuned gong (bossed, nipple gong) | raised centre, "most often but not always tuned to various pitches"; 15-50 cm or larger | [Cited S11] |
| term "gong" | in Western symphonic music "reserved for the bossed type only"; a footnote says modern orchestras use "gong" and "tam-tam" interchangeably | [Cited S11] |
| sets of tuned gongs | Messiaen (Des canyons aux étoiles) | [Cited S11] |
| Puccini | used "gongs and tam-tams" in his operas | [Cited S11] |
| tuned-gong ranges | not stated by any source read; a pitched `gong_tuned` id needs its own source | [Estimate: design judgement; S11 gives tuned-gong sizes but no pitch ranges] |
| player changes | the tam-tam is usually one station of a multi-instrument player | [Estimate: general orchestral practice; no source read states it] |

### 9. General MIDI and existing Sonara metadata

- GM1: no tam-tam or gong on channel 10 and no melodic program [Cited S1].
- GS ORCHESTRA kit: none [Cited S2].
- Sonara: none in `data.rs`, `GM_MAP` or `DRUM_NAMES` [Cited S31]. VSCO's GM-style map puts gong sounds on 42 and 46, which are hi-hat notes in GM1 [Cited S24] [Cited S1].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | keys 42 (scrape) and 46 (hit); VSCO 1 `varMetal/Gong`: hits pppp-fff incl. hard, rolls, scrapes, smacks (29 WAV files; a `gong` subfolder adds 6 MP3s and a zip). Whether this "Gong" is a tam-tam is not stated [Cited S24] |
| VCSL | CC0 [Cited S26] | Gong 1, Gong 2 (type not stated in the listing) [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | three tam-tam samples, keys 67-69 [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | B2 Tam Tam [Cited S28] |

## triangle: Triangle

```json
{"id": "triangle", "family": "percussion", "names": {"en": "Triangle", "cs": "triangl", "it": "triangolo", "de": "Triangel", "fr": "triangle", "score_abbreviation": "Trgl."}, "pitched": false, "transposition": null, "clefs": ["single_line", "treble", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate), S12", "comfortable": "n/a, unpitched: Cited S12"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [81, 80], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["let_ring", "damped"], "articulations_provenance": "Cited S4 (strokes, groups, roll), S5 (tremolo, pp at the top), S12 (roll, hand damping), S1 (Mute/Open Triangle)", "solo_or_section": "solo", "typical_players": {"classical": "18th-century opera, as local colour for Eastern scenes (Cited S4)", "romantic": "1 player (Estimate)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | triangle | [Cited S4] [Cited S12] |
| Czech | "Triangl" (cs title "Triangl (hudební nástroj)") | [Cited S23] |
| Italian | "Triangolo" | [Cited S4] [Cited S5] [Cited S23] |
| German | "Triangel" | [Cited S4] [Cited S5] [Cited S23] |
| French | "Triangle" | [Cited S4] [Cited S5] [Cited S23] |
| Score | single line or treble-clef stave; abbreviation "Trgl." | [Cited S4] [Cited S5] for the stave; [Estimate: common score abbreviation; no source read gives it] for the abbreviation |
| Family | percussion, metal, indefinite pitch | [Cited S4] [Cited S12] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| A small steel bar bent into a triangle, struck with a beater of the same metal; moderate size, neither "tinkley" nor so large as to acquire a distinct note; "as clear as crystal" | [Cited S4] |
| Made of aluminium, beryllium copper, brass, bronze, iron or steel; struck with a metal rod; high-pitched ringing tone | [Cited S12] |
| Hung from a clip so that it vibrates freely; held in the hand, one side can be damped by the fingers | [Cited S12] |
| Some triangles give "a double sound"; one stroke should give a single sound | [Cited S5] |
| "Theoretically" of indefinite pitch | [Cited S12] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition [Cited S4] [Cited S12].
- Notation: single line or treble stave [Cited S4] [Cited S5]; a roll with three strokes through the stem [Cited S12]; "the tremolo is written as for the Drums" [Cited S5].
- Zones: pianissimo is played at the top of the triangle, where the rod moves only about an inch [Cited S5].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Power | "considerable powers" in f and ff; keep its marking below the instruments it plays with; it asserts itself "even in the tutti ff" | [Cited S4] |
| Climax | "it suffices to add the Triangle, in order to convert red-heat into white-heat" (Widor, quoted by Forsyth) | [Cited S4] [Cited S5] |
| Soft | pp with soft strings and light woodwind "is charming"; single soft strokes with flute, oboe and harp harmonics | [Cited S4] |
| Agility | single, double, triple strokes; groups of 2-5 notes before the main note; odd numbers are better because first and last strokes go the same direction | [Cited S4] [Cited S5] |
| Register group | high, with castanets and little bells; goes with harmony in the upper register; tremolo with woodwind and violin trills | [Cited S3] |
| Use | marks the rhythm alone; "absolutely indispensable in the orchestra" | [Cited S5] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| open stroke | plain note, `let_ring` | [Cited S4] [Cited S1: "Open Triangle"] | GM 81 [Cited S1]; SSO 57 open (2 RR) [Cited S27]; VSCO 79 and 81 `Triangle3-Hit`, `Triangle6-Hit` (two triangles) [Cited S24] |
| muted stroke (hand on the triangle) | `damped` (here: struck while held damped, GM "Mute Triangle") | [Cited S12] [Cited S1] | GM 80 [Cited S1]; SSO 55 muted (3 RR) [Cited S27]; VSCO 78 and 80 `HitFM` (meaning undocumented) [Cited S24] |
| half-open | none | — | SSO 56 half-open (3 RR) [Cited S27] |
| roll (between two sides) | `roll` | [Cited S4] [Cited S12] | SSO 58: held roll sample, `trigger=release_key` tail, 300 Hz high-pass filter [Cited S27]; VSCO raw `Triangle3-Roll`, `Triangle6-Roll` [Cited S24] |
| choke of an open note | `damped` | [Cited S12] | GS: 80/81 in exclusive group EXC5, "Percussion sound of the same number will not be heard at the same time" [Cited S2]; SSO: the open triangle (`group=2`) is cut by the muted and half-open strokes (`off_by=1`) [Cited S27] |
| accented stroke, grace groups | `marcato`; grace notes as separate notes | [Cited S4] [Cited S5] | velocity [Estimate: inferred from the libraries read, which map dynamics to velocity (snare_drum section 5); none labels an accent sample] |

The exclusive-group choke (open cut by muted) is the one percussion mechanism every source read agrees on: GM pairs the two sounds [Cited S1], GS makes them exclusive [Cited S2], SSO implements it [Cited S27], and the Sonara sampler plays `group`/`off_by` [Cited S31].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One triangle and one player [Estimate: general orchestral practice; no source read gives a player count (Open question 9)]. Liszt's E-flat piano concerto raises it to a solo instrument [Cited S4] [Cited S12].

### 7. Role in the orchestra and typical doublings

- One of the "Batterie" with bass drum and cymbals [Cited S4]; in Brahms's Fourth Symphony "the only appearance of non-timpani percussion in a Brahms symphony" [Cited S12].
- A single stroke to set free a new phrase (Meistersinger overture, "the acme of good taste"; one note at the end of Siegfried Act 2) [Cited S4].
- Doublings: woodwind and violin trills (with a triangle tremolo), the octave of flute, oboe and harp harmonics [Cited S3] [Cited S4]; Lalo's Rapsodie, triangle ppp with woodwind and upper strings [Cited S4].

### 8. Variants and related instruments (when a player changes instrument)

- Sizes from small to "a large, low-pitched bass triangle" [Cited S12]; VSCO 1 has four triangles, VSCO 2 two (`Triangle3`, `Triangle6`) [Cited S24].
- Related high metal: finger cymbals, bell tree, mark tree (SSO, VSCO, VCSL) [Cited S27] [Cited S24] [Cited S26].

### 9. General MIDI and existing Sonara metadata

- GM1: 80 Mute Triangle, 81 Open Triangle [Cited S1]; GS: the same, EXC5 [Cited S2].
- Sonara: none. `GM_MAP` has no 80 or 81, so the built-in Drum Kit plays nothing for a GM triangle [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | keys 78-81 (two triangles, hit and `HitFM`); raw `HitM`, `Roll`; VSCO 1 four triangle folders [Cited S24] |
| VCSL | CC0 [Cited S26] | Triangles [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | muted, half-open, open (round robins), roll; choke group [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | E3 Triangle [Cited S28] |

## tambourine: Tambourine

```json
{"id": "tambourine", "family": "percussion", "names": {"en": "Tambourine", "cs": "tamburína", "it": "tamburino", "de": "Schellentrommel", "fr": "tambour de basque", "score_abbreviation": "Tamb."}, "pitched": false, "transposition": null, "clefs": ["single_line", "treble", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate)", "comfortable": "n/a, unpitched: Cited S4"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [54], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["thumb_roll", "shake_stroke"], "articulations_provenance": "Cited S4 (knuckles, shake roll, thumb), S5 (hand, rustle, thumb), S13 (thumb roll with beeswax or rosin, wrist roll)", "solo_or_section": "solo", "typical_players": {"classical": "Mozart among the earliest (Cited S13)", "romantic": "1 player (Estimate)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | tambourine | [Cited S4] [Cited S13] |
| Czech | "Tamburína" | [Cited S23] |
| Italian | "Tamburino" (Forsyth, Widor); "tamburello" (it Wikipedia) | [Cited S4] [Cited S5] [Cited S13] [Cited S23] |
| German | "Schellentrommel" (Forsyth, Widor); "Baskische Trommel", "Tambourin" (Forsyth); "Tamburin" (de Wikipedia) | [Cited S4] [Cited S5] [Cited S23] |
| French | "Tambour de Basque" | [Cited S4] [Cited S5] |
| Warning | German "Tambourin" also names the Provençal long drum; French "tambourin" is the long drum of Provence | [Cited S4] [Cited S13] |
| Score | single line or treble stave; abbreviation "Tamb." | [Cited S4] [Cited S5] for the stave; [Estimate: common score abbreviation; no source read gives it] for the abbreviation |
| Family | percussion, membrane with jingles, indefinite pitch | [Cited S4] |

Recommendation: `names.de` = "Schellentrommel" (both handbooks), aliases "Tamburin"; `names.it` = "tamburino", alias "tamburello" [Estimate: choice of primary alias].

### 2. Sound production

| Fact | Provenance |
|---|---|
| A small wooden hoop with a parchment head on one side, tightened by brass rods and nuts; the hoop is cut away for pairs of loose metal plates ("jingles") on a wire | [Cited S4] |
| A frame of wood or plastic with pairs of metal jingles ("zills"); the headless version is the "jingle ring" | [Cited S13] |
| Three ways of playing (Forsyth): (1) striking the head with the knuckles; (2) shaking the hoop, "practically a roll on the jingles"; (3) rubbing the thumb on the head, a partial tremolo of the jingles | [Cited S4] |
| Widor: (1) back of the hand; (2) shaking gives "a metallic rustle rather than a roll"; (3) gliding the thumb gives a temporary roll, "requires some skill" | [Cited S5] |
| Thumb roll: the player "lightly licks the tip of their thumb or uses beeswax or rosin"; shake roll by rotating the wrist | [Cited S13] |
| A fourth method: laid on a kettle-drum and played with kettle-drum sticks (muffled-drum tone with an edge of jingles) | [Cited S4] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition [Cited S4]. Notation: single line or treble stave [Cited S4] [Cited S5]; the shake written like a trill or a string tremolo [Cited S5].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| ff roll | sudden ff roll "effective at moments of rhythmic excitement", above all with the orchestra in its upper register; the tone "soon becomes tiresome. Its proper place is the Theatre" | [Cited S4] |
| pp | "less generally useful"; tie its notes to the rhythmic pattern or it sounds "like a copyist's mistake" | [Cited S4] |
| Register group | Rimsky lists it fourth, among the medium instruments; with triangle and side drum it suits harmony in the upper register; tremolo with woodwind and violin trills | [Cited S3] |
| Agility | "capable of various rhythmic figures" | [Cited S3] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| struck (knuckles, hand) | plain note, `marcato` | [Cited S4] [Cited S5] | GM 54 [Cited S1]; VSCO 54 `Tamb1-Hit` [Cited S24]; SSO 52 hit f/mf [Cited S27] |
| shake roll | `roll` | [Cited S4] [Cited S13] | VSCO 55 `Tamb1-Roll` [Cited S24]; SSO 54 roll [Cited S27] |
| single shake | `shake_stroke` (new: one short shake gesture of a jingle or rattle instrument, as opposed to a sustained roll; not the brass `shake`) | [Cited S24] [Cited S27] show it as a separate sample | VSCO 53 `Tamb1-Shake` [Cited S24]; SSO 53 shake [Cited S27] |
| thumb roll | `thumb_roll` (new: tambourine roll made by rubbing a moistened or rosined thumb along the head) | [Cited S4] [Cited S5] [Cited S13] | none found |
| on a timpani with timpani sticks | none (rare effect) | [Cited S4] | none found |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One tambourine and one player [Estimate: general orchestral practice; no source read gives a player count (Open question 9)].

### 7. Role in the orchestra and typical doublings

- Dance and local colour: Berlioz's Carnaval romain, Elgar's Cockaigne (crotchets "battuto colla mano" with bass drum, cello and bass pizzicato), the Arab Dance of Tchaikovsky's Nutcracker (thumb method) [Cited S4]; Bizet's Carmen [Cited S5]; Mozart "among the earliest western composers" to use it [Cited S13].

### 8. Variants and related instruments (when a player changes instrument)

- Jingle ring (headless) [Cited S13]; tabor (Provençal long drum, confusable by name) [Cited S4].

### 9. General MIDI and existing Sonara metadata

- GM1 54 Tambourine [Cited S1]; GS ORCHESTRA kit 54 unchanged [Cited S2].
- Sonara: `GM_MAP` 54 `tambourine`, `DRUM_NAMES` `tambourine` = 54, a synthesized sound with golden data [Cited S31]. No shake, roll or thumb roll.

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | keys 53 shake, 54 hit, 55 roll [Cited S24] |
| VCSL | CC0 [Cited S26] | Tambourine 1, 2 and 3-5 Legacy [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | 52 hit (2 layers), 53 shake, 54 roll [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | C3 Tambourine [Cited S28] |

## castanets: Castanets

```json
{"id": "castanets", "family": "percussion", "names": {"en": "Castanets", "cs": "kastaněty", "it": "castagnette", "de": "Kastagnetten", "fr": "castagnettes", "score_abbreviation": "Cast."}, "pitched": false, "transposition": null, "clefs": ["single_line", "treble", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Cited S4 (Pitch indeterminate), S5", "comfortable": "n/a, unpitched: Cited S5"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": [], "articulations_provenance": "Cited S4 (rhythms), S5 (any rhythmic combination, accents), S14 (roll, handle castanets for sustained rolls)", "solo_or_section": "solo", "typical_players": {"classical": "rare (Estimate)", "romantic": "1 player, Spanish colour (Estimate)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | castanets | [Cited S4] [Cited S14] |
| Czech | "Kastaněty" | [Cited S23] |
| Italian | "Castagnette" (Widor; Forsyth's OCR "Gaitagnette"); "Nacchere" (it Wikipedia) | [Cited S5] [Cited S4] [Cited S23] |
| German | "Kastagnetten" | [Cited S4] [Cited S5] [Cited S23] |
| French | "Castagnettes" | [Cited S4] [Cited S5] [Cited S23] |
| Spanish | "Castañuelas" | [Cited S5] [Cited S4] |
| Score | single line without clef; abbreviation "Cast." | [Cited S5] for the line; [Estimate: common score abbreviation; no source read gives it] for the abbreviation |
| Family | percussion, wood, indefinite pitch | [Cited S4] |

Recommendation: `names.it` = "castagnette" (both handbooks), alias "nacchere" (modern Italian usage per S23) [Estimate: choice].

### 2. Sound production

| Fact | Provenance |
|---|---|
| Each pair is two hollow pieces of hard wood clicked together by the fingers; the left hand plays a simpler rhythm on a larger pair ("macho"), the right hand the full rhythm on a smaller pair ("hembra") | [Cited S4] |
| A pair of concave shells joined at one edge by a string, traditionally chestnut; fibreglass increasingly popular | [Cited S14] |
| Mounted on a handle "for use in the orchestra and military bands" | [Cited S4] |
| Machine castanets (on a base) are easier to play but alter the sound; handle castanets "were developed for use in orchestral music", useful for sustained and loud rolls | [Cited S14] |
| Made of wood or of iron, one pair in each hand (Widor); Saint-Saëns asks for both in Samson et Dalila | [Cited S5] [Cited S4] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition [Cited S4]; single line without clef [Cited S5]. Forsyth's instrument table (pp. 10-11) gives "Conventional treble-clef or a single line" for the Castanets [Cited S4; columns aligned from the OCR text].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Rhythm | a triple-time quaver rhythm whose middle quaver splits into semiquavers, a triplet or demisemiquavers | [Cited S4] |
| Agility | "any rhythmic combinations imaginable"; can mark the accented beat like a triangle | [Cited S5] |
| Register group | high, with triangle and little bells | [Cited S3] |
| Economy | like the piano, "too characteristic" to be used often | [Cited S3] |
| Roll | possible in each of the three ways of holding them | [Cited S14] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| click (single stroke, accent) | plain note, `marcato` | [Cited S5] | SSO 61, 2 round robins; the second has `pitch_keycenter=c4` on the c#4 key, so it plays a semitone sharp [Cited S27]; BBC SO Discover F3 [Cited S28]; GS 85 (all seven kits of the SC-55 table) and 39 (ORCHESTRA kit) [Cited S2] |
| roll (carretilla) | `roll` | [Cited S14] | none found |
| rhythmic patterns | sequences of notes, not ids | [Cited S4] | — |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One player; a pair per hand when hand-held [Cited S5]. On stage, dancers or their companions keep the rhythms going [Cited S4].

### 7. Role in the orchestra and typical doublings

- Spanish colour: Bizet's Carmen, Rimsky-Korsakov's Capriccio espagnol, Ravel's Rapsodie espagnole [Cited S14].
- Alternating with other percussion (Samson et Dalila, Act 3 scene 2) [Cited S5].

### 8. Variants and related instruments (when a player changes instrument)

- Hand castanets, handle (paddle) castanets, machine castanets, iron castanets [Cited S14] [Cited S5].

### 9. General MIDI and existing Sonara metadata

- GM1: no castanets [Cited S1]. GS: 85 Castanets in all seven kits of the SC-55 drum-set table (other GS sets were not read), 39 Castanets in the ORCHESTRA kit [Cited S2].
- Sonara: none; 39 is `clap` [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27]; bundling judged "unresolved" in Sonara's research [Cited S31] | one stroke, 2 round robins, key 61 [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | F3 Castanets [Cited S28] |
| VSCO 2 CE, VCSL | CC0 [Cited S25] [Cited S26] | none in the file tree or folder listing [Cited S24] [Cited S26] |

## wood_block: Wood block

```json
{"id": "wood_block", "family": "percussion", "names": {"en": "Wood block", "cs": "dřevěný blok", "it": "cassa di legno", "de": "Holzblock", "fr": "bloc de bois", "score_abbreviation": "W.B."}, "pitched": false, "transposition": null, "clefs": ["single_line", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Estimate (high/low pair of indefinite pitch; S15 does not discuss pitch)", "comfortable": "n/a: Estimate"}, "written_playable": null, "gm_program": 116, "gm_program_name": "Woodblock", "gm_percussion_notes": [76, 77], "existing_sonara_id": null, "articulations_existing_vocab": ["marcato", "roll"], "articulations_proposed_new": [], "articulations_provenance": "Estimate: single strokes and rolls are general practice; S15 only states stick versus mallet strokes", "solo_or_section": "solo", "typical_players": {"classical": "not used (Estimate)", "romantic": "rare (Estimate)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | woodblock, wood block; "clog box" or "tap box" in ragtime and jazz | [Cited S15] |
| Czech | "dřevěný blok" | [Estimate: no cs article in S23] |
| Italian | "cassa di legno" | [Cited S15] |
| German | "Holzblock" or "Holzblocktrommel" | [Cited S15] |
| French | "bloc de bois" or "tambour de bois" | [Cited S15] |
| Score | "W.B." or "Wd. Bl." | [Estimate: common abbreviations, no source read] |
| Family | percussion, wood; "a small slit drum made from a single piece of wood" | [Cited S15] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| Western orchestral woodblock of teak or another hardwood, a rectangular or cylindrical block with one or two longitudinal cavities | [Cited S15] |
| Plastic versions: "granite blocks", the "jam block" | [Cited S15] |
| A stick gives "a sharp crack"; a rounder mallet, soft or hard, "a deeper-pitched and fuller knocking sound" | [Cited S15] |
| On the drum kit it is clamped to the rear rim of the bass drum | [Cited S15] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition; usually a high and a low block [Estimate: GM1 lists Hi and Low Wood Block (S1); S15 does not discuss pitch].
- Notation: a single line, or two lines for high and low [Estimate: engraving practice; no source read describes wood-block notation].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Colour | dry, sharp crack with sticks; fuller knock with mallets | [Cited S15] |
| Dynamics | short decay at every level; loud strokes risk cracking a wooden block | [Estimate: no source read] |
| Agility | very fast single strokes and rolls are possible with two sticks | [Estimate: general percussion practice; S15 does not discuss agility] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| stroke, high and low block | plain note, `marcato` | [Cited S15] for the strokes | GM 76/77 [Cited S1]; SSO 74 low, 75 high [Cited S27]; BBC SO Discover D4 [Cited S28] |
| roll | `roll` | [Estimate: general percussion practice, as the json provenance says; S15 names only stick and mallet strokes] | none found |
| beater (stick, mallet) | beater, not an id | [Cited S15] | none found |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One player, usually as part of a multi-percussion station [Estimate: general orchestral practice; no source read gives a player count (Open question 9)].

### 7. Role in the orchestra and typical doublings

- Rhythmic, comic or clock-like effects; no orchestral repertoire is named in S15 [Cited S15 for the absence; examples would be Estimates].

### 8. Variants and related instruments (when a player changes instrument)

- Temple blocks ("a set of four or more woodblocks"), the Chinese wooden fish (muyu), slit drums, plastic jam and granite blocks [Cited S15] [Cited S16]. Log drum: VSCO maps its log drum to the GM wood-block notes [Cited S24].

### 9. General MIDI and existing Sonara metadata

- GM1: 76 Hi Wood Block, 77 Low Wood Block on channel 10; melodic program 116 "Woodblock" in the Percussive family 113-120 [Cited S1]. GS ORCHESTRA: 76, 77 unchanged [Cited S2].
- Sonara: `GM_MAP` 76 `block_high`, 77 `block_low` (golden synthesized sounds); no entry in `DRUM_NAMES` [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VCSL | CC0 [Cited S26] | Woodblock folder [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | low and high block, one sample each [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | D4 Woodblock [Cited S28] |
| VSCO 2 CE | CC0 [Cited S25] | no wood block; keys 76/77 play `LogDrumHi/Lo` [Cited S24] |

## temple_blocks: Temple blocks

```json
{"id": "temple_blocks", "family": "percussion", "names": {"en": "Temple blocks", "cs": "temple bloky", "it": "temple block", "de": "Tempelblock", "fr": "bloc chinois", "score_abbreviation": "T.Bl."}, "pitched": false, "transposition": null, "clefs": ["percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a: Cited S16 (not commonly treated as pitched percussion)", "comfortable": "n/a: Cited S16"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["marcato", "roll"], "articulations_proposed_new": ["glissando"], "articulations_provenance": "Estimate: strokes, rolls and a sweep across the set are general practice; S16 names no techniques", "solo_or_section": "solo", "typical_players": {"classical": "not used (Estimate)", "romantic": "not used (Estimate)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | temple blocks; also tone blocks, skulls, wooden bells, dragons' mouths | [Cited S16] |
| Czech | "temple bloky" | [Estimate: no cs article in S23] |
| Italian | "Temple block" (it Wikipedia title) | [Cited S23] |
| German | "Tempelblock" (de Wikipedia title) | [Cited S23] |
| French | "Bloc chinois" (fr Wikipedia title) | [Cited S23] |
| Score | "T.Bl."; usually a five-line stave, one line per block or a pitched-looking layout | [Estimate: common abbreviation and engraving practice; no source read gives either] |
| Family | percussion, wood | [Cited S16] |

Warning: "Chinese block" in English often means the wood block or wooden fish, while French "bloc chinois" is the title for temple blocks [Cited S15] [Cited S23]; ids, not names, must carry the meaning [Estimate: recommendation].

### 2. Sound production

| Fact | Provenance |
|---|---|
| "A carved hollow wooden instrument with a large slit", descended from the muyu (wooden bell) of East Asia | [Cited S16] |
| Traditional forms rounder and bell-like; modern instruments often rectangular; plastic "granite blocks" | [Cited S16] |
| Played in groups of at least four; the usual set of five is normally tuned to a pentatonic scale; chromatic and diatonic sets exist, but "they are not commonly treated as pitched percussion" | [Cited S16] |
| Sound like a woodblock but with "a darker, more hollow timbre" | [Cited S16] |
| Beaters: rubber or yarn mallets, or sticks | [Estimate: S16 does not mention beaters] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- Treated as unpitched [Cited S16]. A set of five gives five relative heights; MIDI needs five keys in a fixed high-to-low order [Estimate: modelling recommendation].
- No transposition.

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Colour | darker and hollower than a woodblock | [Cited S16] |
| Imitation | in Leroy Anderson's Sleigh Ride and The Syncopated Clock they imitate a ticking clock and a galloping horse | [Cited S16] |
| Melody | can double melodic motifs (John Barnes Chance, Incantation and Dance; Variations on a Korean Folk Song) | [Cited S16] |
| Dynamics, agility | short decay, fast patterns across the set | [Estimate: general percussion practice; S16 does not discuss dynamics or agility] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| stroke on one block | plain note, `marcato` | [Estimate: general percussion practice; S16 names no techniques] | none in VSCO, VCSL, SSO or BBC SO Discover lists [Cited S24] [Cited S26] [Cited S27] [Cited S28] |
| roll | `roll` | [Estimate: general percussion practice; S16 names no techniques] | none found |
| sweep across the set | `glissando` (preferred list) | [Estimate: general percussion practice; S16 names no techniques] | none found |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One set, one player [Estimate: general orchestral practice; no source read gives a player count (Open question 9)].

### 7. Role in the orchestra and typical doublings

- Entered the orchestra after early jazz drummers used them as exotic instruments [Cited S16]. Light-music and film effects (clock, hooves) [Cited S16].

### 8. Variants and related instruments (when a player changes instrument)

- Woodblock ("temple blocks consist of a set of woodblocks"), muyu, plastic granite blocks [Cited S15] [Cited S16].

### 9. General MIDI and existing Sonara metadata

- GM1: none; the nearest are 76/77 wood blocks and melodic program 116 Woodblock [Cited S1; substitution is an Estimate]. GS ORCHESTRA: none [Cited S2]. Sonara: none [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE, VCSL | CC0 [Cited S25] [Cited S26] | none found in the file tree or folder listing [Cited S24] [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | none [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | none in the untuned patch [Cited S28] |

## sleigh_bells: Sleigh bells

```json
{"id": "sleigh_bells", "family": "percussion", "names": {"en": "Sleigh bells", "cs": "rolničky", "it": "sonagli", "de": "Schellen", "fr": "grelots", "score_abbreviation": "Sl. B."}, "pitched": false, "transposition": null, "clefs": ["single_line", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Estimate (no handbook read describes them)", "comfortable": "n/a: Estimate"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["shake_stroke"], "articulations_provenance": "Estimate for the techniques; hit and shake exist as separate samples in VSCO (Cited S24)", "solo_or_section": "solo", "typical_players": {"classical": "rare (Estimate)", "romantic": "1 player (Mahler 4: Cited S17)", "film": "1 player (Estimate)", "chamber": "rare (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | sleigh bells, jingle bells | [Cited S17] |
| Czech | "Rolnička" (cs title, singular); "rolničky" (plural) | [Cited S23] for the title; [Estimate: Czech plural of the cs title; no Czech score or handbook was read] for the plural form as the score term |
| Italian | "sonagli" | [Estimate: the it title linked from "Jingle bell" is "Campane tintinnanti" [Cited S23]] |
| German | "Schellen" | [Estimate: the de title linked from "Jingle bell" is "Glockenstab" [Cited S23], which looks like a different object] |
| French | "Grelot (instrument)" (fr title); "grelots" | [Cited S23] |
| Score | "Sl. B." | [Estimate: common score abbreviation; no source read gives it] |
| Family | percussion, metal, indefinite pitch | [Estimate: inferred from S17's description of metal bells; no handbook read classes them] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| In the simplest form, one sheet of metal is bent into a rough sphere around a loose ball bearing or short rod; some are made of two halves crimped together, and larger ones may hold a glass marble | [Cited S17] |
| Several bells on a strap or handle, shaken or struck against the hand | [Estimate: S17 does not describe mounts or technique] |
| "Little bells" appear in Rimsky's high group; whether he means sleigh bells is unclear | [Cited S3] (see Open questions) |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition [Estimate: no handbook read describes sleigh bells; S17 states no pitch]. Single line [Estimate: engraving practice; no source read gives the notation].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Colour | bright jingle "especially in large numbers" | [Cited S17] |
| Dynamics | quiet to loud; library samples run from quiet to loud hits | [Cited S24] for the samples; the rest [Estimate: general percussion practice; no source read states a dynamic range] |
| Agility | steady shaken rhythms; accents by striking the mount | [Estimate: general percussion practice; S17 does not describe technique] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| hit (accent) | plain note, `marcato` | [Estimate: general percussion practice; no source read describes the technique, only VSCO's sample names (S24)] | VSCO 82 `Sleighbells_Hit` (2 RR); VSCO 1 raw `sleighbell1_hit_*`, `_hit_loud`, `_hit_quiet` [Cited S24]; SSO 70 f and mf [Cited S27] |
| single shake | `shake_stroke` (new, defined under tambourine) | [Estimate: general percussion practice; no source read describes the technique, only VSCO's sample names (S24)] | VSCO 1 raw `sleighbell1_shake1`, `sleighbell2_shake1` [Cited S24] |
| continuous shake | `roll` | [Estimate: general percussion practice; no source read describes a sustained shake] | none labelled |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One set, one player [Estimate: general orchestral practice; no source read gives a player count (Open question 9)].

### 7. Role in the orchestra and typical doublings

- Mahler's Symphony No. 4 (1901) and Prokofiev's Lieutenant Kijé Suite (1934) call for them [Cited S17]. Winter and sleigh-ride imagery (Mozart's German Dance K. 605 No. 3, Leroy Anderson's Sleigh Ride) [Estimate: not stated in a source read].

### 8. Variants and related instruments (when a player changes instrument)

- Crotal bell (European ancestor) [Cited S17]; tambourine jingles, jingle ring [Cited S13]; bell tree (SSO 71, VSCO 83-86) [Cited S27] [Cited S24].

### 9. General MIDI and existing Sonara metadata

- GM1: none [Cited S1]. GS: 83 Jingle Bell in the STANDARD set and every kit that keeps it, including ORCHESTRA [Cited S2]. GM1 melodic program 113 is "Tinkle Bell", not defined as sleigh bells [Cited S1].
- Sonara: none [Cited S31]. VSCO's GM-style map puts sleigh bells on 82, where GS has Shaker [Cited S24] [Cited S2].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | key 82 hit; VSCO 1 raw two sets of hits and shakes [Cited S24] |
| VCSL | CC0 [Cited S26] | Sleigh Bells [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | key 70, two dynamics [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | none [Cited S28] |

## maracas: Maracas

```json
{"id": "maracas", "family": "percussion", "names": {"en": "Maracas", "cs": "rumba koule", "it": "maracas", "de": "Maracas", "fr": "maracas", "score_abbreviation": "Mar."}, "pitched": false, "transposition": null, "clefs": ["single_line", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Estimate (S18 gives no pitch statement)", "comfortable": "n/a: Estimate"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [70], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["shake_stroke"], "articulations_provenance": "Cited S18 (shaking, rhythmic role), S19 (shakers may be struck for accents); roll is an Estimate", "solo_or_section": "solo", "typical_players": {"classical": "not used (Estimate)", "romantic": "not used (Estimate)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | maracas (singular maraca); "sometimes called shaker or chac-chac" | [Cited S18] |
| Czech | "Rumba koule" | [Cited S23] |
| Italian | "Maracas" | [Cited S23] |
| German | "Maracas" | [Cited S23] |
| French | "Maraca" (fr title); "maracas" | [Cited S23] |
| Score | "Mar." or "Mrcs." | [Estimate: common score abbreviation; no source read gives it] |
| Family | percussion, rattle (shaken idiophone) | [Cited S18] for the shaking; the class name is an Estimate |

### 2. Sound production

| Fact | Provenance |
|---|---|
| Gourd rattles, or balls of leather, wood or plastic, with a wooden stick through the shell as handle; a few pebbles inside | [Cited S18] |
| "The contents and materials affect the instrument's timbre and volume" | [Cited S18] |
| "Shaken by a handle and usually played as part of a pair"; the internal elements strike the inside of the container | [Cited S18] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition; one line, often two (left and right maraca) [Estimate: engraving practice; S18 gives no pitch or notation].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Role | "a primarily rhythmic role, accentuating the beat"; Latin styles (son cubano, salsa, mambo, samba) | [Cited S18] |
| Accents | shakers "may also be struck for a greater accent on certain beats" | [Cited S19] |
| Dynamics, agility | soft to moderately loud; fast subdivisions; each pair can alternate | [Estimate: general percussion practice; S18 does not discuss dynamics or agility] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| shake (one gesture) | `shake_stroke` (new, defined under tambourine) | [Cited S18] | VSCO 1 raw `maraca1.wav` to `maraca5.wav` and `maraca_shake.wav`, unmapped [Cited S24]; GM 70 [Cited S1] |
| accent | `marcato` | [Cited S19] | velocity [Estimate: inferred from the libraries read, which map dynamics to velocity (snare_drum section 5); none labels an accent sample] |
| continuous shake | `roll` | [Estimate: general percussion practice; S18 and S19 describe shaking, not a sustained roll] | none found |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One pair, one player [Cited S18 for the pair; the rest is an Estimate].

### 7. Role in the orchestra and typical doublings

- Latin-American colour; orchestral works are not named in the source read [Cited S18]; Varèse's Ionisation and Latin-themed orchestral pieces use them [Estimate: no source read].

### 8. Variants and related instruments (when a player changes instrument)

- Cabasa (GM 69), shakers, caxixi, egg shaker [Cited S1] [Cited S19].

### 9. General MIDI and existing Sonara metadata

- GM1 70 Maracas [Cited S1]; GS ORCHESTRA 70 unchanged [Cited S2].
- Sonara: `GM_MAP` names 69 (Cabasa) and 70 (Maracas) `shaker`; `DRUM_NAMES` `shaker` = 70 [Cited S31]. A GM maracas part plays Sonara's synthesized `shaker` (golden sound) [Cited S31]. Through VSCO's `GM-StylePerc.sfz` the same note 70 plays `Ratchet1-Fast`, a ratchet [Cited S24].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | six raw maraca files (`maraca1` to `maraca5`, `maraca_shake`) in `VSCO 1 Percussion/varWood`, not mapped in `GM-StylePerc.sfz`, the branch's only unpitched-percussion SFZ [Cited S24]; that SFZ plays a ratchet on GM 70 [Cited S24] |
| VCSL | CC0 [Cited S26] | no maracas folder; Cabasa and Shakers instead [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | none; cabasa and shakers instead [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | none [Cited S28] |

## shaker: Shaker

```json
{"id": "shaker", "family": "percussion", "names": {"en": "Shaker", "cs": "šejkr", "it": "shaker", "de": "Shaker", "fr": "shaker", "score_abbreviation": "Shk."}, "pitched": false, "transposition": null, "clefs": ["single_line", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Estimate (S19 gives no pitch statement)", "comfortable": "n/a: Estimate"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["roll", "marcato"], "articulations_proposed_new": ["shake_stroke"], "articulations_provenance": "Cited S19 (shaken back and forth, struck for accents); roll is an Estimate", "solo_or_section": "solo", "typical_players": {"classical": "not used (Estimate)", "romantic": "not used (Estimate)", "film": "1 player; common in hybrid scores (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | shaker: "various percussive musical instruments used for creating rhythm" | [Cited S19] |
| Czech | "šejkr" | [Estimate: no cs article in S23] |
| Italian | "Shaker (strumento musicale)" | [Cited S23] |
| German | "Shaker (Musik)" | [Cited S23] |
| French | "Instrument à secouer" (fr title); "shaker" in practice | [Cited S23]; [Estimate: common French usage; no French source was read] for the practice |
| Score | "Shk." | [Estimate: common score abbreviation; no source read gives it] |
| Family | percussion, shaken idiophone; a generic class, not one instrument | [Cited S19] |

Scope: this id is the tube or egg shaker class. Maracas have their own id; cabasa, caxixi and rainstick are variants [Cited S19 for the class members; the split is a recommendation].

### 2. Sound production

| Fact | Provenance |
|---|---|
| A container "partially full of small loose objects such as beans", sounding when they collide with each other or the walls | [Cited S19] |
| Examples: rainstick, caxixi, egg shaker | [Cited S19] |
| Moved back and forth in the air rather than struck; "most may also be struck for a greater accent" | [Cited S19] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition; single line [Estimate: engraving practice; S19 gives no pitch or notation].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Role | "often used in rock and other popular styles of music to provide the ride pattern" | [Cited S19] |
| Dynamics, agility | soft; continuous subdivisions; accents by striking | [Cited S19] for accents; the rest [Estimate: general percussion practice; S19 does not discuss dynamics] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| single shake | `shake_stroke` (new, defined under tambourine) | [Cited S19] | SSO 64-66 `shaker-a/b/c` (plus unmapped `shaker-hi/lo` files) [Cited S27]; VSCO 1 raw "Camo's Shaker" `shake1-7` [Cited S24]; GS 82 Shaker [Cited S2] |
| accent (struck) | `marcato` | [Cited S19] | velocity [Estimate: inferred from the libraries read, which map dynamics to velocity (snare_drum section 5); none labels an accent sample] |
| continuous shake | `roll` | [Estimate: general percussion practice; S19 describes single shakes back and forth, not a sustained roll] | none labelled |

A shaker groove alternates forward and backward strokes of different colour, which libraries cover with two or more samples [Estimate: the SSO a/b/c naming suggests it, but no documentation was read].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One player [Estimate: general orchestral practice; no source read gives a player count (Open question 9)].

### 7. Role in the orchestra and typical doublings

- Not an orchestral-handbook instrument: a full-text search of S3, S4 and S5 finds no shaker, maracas, whip, wood block, temple blocks or sleigh bells [Cited S3] [Cited S4] [Cited S5]. Pop, film and hybrid scoring [Estimate: S19 names only rock and other popular styles; film and hybrid use is general scoring practice].

### 8. Variants and related instruments (when a player changes instrument)

- Maracas, cabasa, caxixi, rainstick, egg shaker [Cited S18] [Cited S19] [Cited S1].

### 9. General MIDI and existing Sonara metadata

- GM1: no "shaker"; the nearest are 69 Cabasa and 70 Maracas [Cited S1; substitution is an Estimate]. GS: 82 Shaker in STANDARD and ORCHESTRA [Cited S2]. VSCO's `GM-StylePerc.sfz` plays `Ratchet1-Crank`, `-Fast` and `-Slow` on 69, 70 and 71, so a GM cabasa or maracas note plays a ratchet there [Cited S24].
- Sonara: `shaker` is the name of GM 69 and 70 in `GM_MAP`, `DRUM_NAMES` `shaker` = 70 [Cited S31]. The GS 82 Shaker and Sonara's `shaker` (70) are different notes for the same word [Cited S2] [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VSCO 2 CE | CC0 [Cited S25] | Camo's Shaker, 7 raw samples [Cited S24] |
| VCSL | CC0 [Cited S26] | Shaker, Large; Shaker, Small; Shaker - Legacy [Cited S26] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | keys 64-66 [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | none [Cited S28] |

## whip: Whip (slapstick)

```json
{"id": "whip", "family": "percussion", "names": {"en": "Whip", "cs": "bič", "it": "frusta", "de": "Peitsche", "fr": "fouet", "score_abbreviation": "Whip"}, "pitched": false, "transposition": null, "clefs": ["single_line", "percussion"], "sounding_playable": null, "sounding_comfortable": null, "range_provenance": {"playable": "n/a, unpitched: Estimate (S20 gives no pitch statement)", "comfortable": "n/a: Estimate"}, "written_playable": null, "gm_program": null, "gm_program_name": null, "gm_percussion_notes": [], "existing_sonara_id": null, "articulations_existing_vocab": ["marcato"], "articulations_proposed_new": [], "articulations_provenance": "Cited S20 (single crack; two-handed louder than one-handed)", "solo_or_section": "solo", "typical_players": {"classical": "not used (Estimate)", "romantic": "1 player (Mahler 5 and 6: Cited S20)", "film": "1 player (Estimate)", "chamber": "1 percussionist (Estimate)"}, "keyboard": null}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | whip; slapstick (one-handed type) | [Cited S20] |
| Czech | "bič" | [Estimate: no cs article in S23] |
| Italian | "frusta" | [Cited S20] |
| German | "Peitsche" | [Cited S20] |
| French | "fouet" | [Cited S20] |
| Score | the native word for "whip" is written in the score | [Cited S20] |
| Family | percussion, wooden clapper | [Cited S20] |

### 2. Sound production

| Fact | Provenance |
|---|---|
| "Two wooden boards joined by a hinge at one end"; the player holds the handles and hits the boards together | [Cited S20] |
| Slapstick: a spring hinge, playable with one hand, "cannot produce sounds as loud as a whip requiring both hands" | [Cited S20] |
| The sound is "reminiscent of the crack of a whip" | [Cited S20] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

- No pitch range, no transposition; single line [Estimate: engraving practice; S20 gives no pitch or notation].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Aspect | Fact | Provenance |
|---|---|---|
| Dynamics | loud by nature; the two-handed whip is louder than the slapstick | [Cited S20] |
| Agility | one crack per gesture; a two-handed whip needs time to reopen, so rapid repeats need a slapstick or two instruments | [Estimate: inferred from the hinged boards S20 describes; no source states a repeat speed] |
| Character | imitates a whip on a horse (Anderson's Sleigh Ride) | [Cited S20] |

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Technique | Vocabulary id | Source | Library implementation |
|---|---|---|---|
| crack | plain note, `marcato` | [Cited S20] | VCSL "Slapstick" folder [Cited S26]; VSCO 1 `varWood/Slapstick.zip` (archive, contents not inspected) [Cited S24] |

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One player [Estimate: general orchestral practice; no source read gives a player count (Open question 9)].

### 7. Role in the orchestra and typical doublings

- Mahler's Symphonies No. 5 and 6, Ravel's Piano Concerto in G (opening crack), Britten's The Young Person's Guide to the Orchestra, Leroy Anderson's Sleigh Ride [Cited S20 for the works; "opening crack" is an Estimate].

### 8. Variants and related instruments (when a player changes instrument)

- Slapstick (spring hinge) [Cited S20]; ratchet, a neighbouring "special effect" instrument (Strauss's Till Eulenspiegel) [Cited S4].

### 9. General MIDI and existing Sonara metadata

- GM1: none [Cited S1]. GS ORCHESTRA: none; STANDARD row 28 "Slap" is replaced by Pedal Hi-hat in ORCHESTRA [Cited S2]; whether GS "Slap" is a slapstick is not stated (Open questions).
- Sonara: none [Cited S31].

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage |
|---|---|---|
| VCSL | CC0 [Cited S26] | Slapstick folder [Cited S26] |
| VSCO 2 CE | CC0 [Cited S25] | `Slapstick.zip` in the raw VSCO 1 folder, not mapped [Cited S24] |
| Sonatina Symphonic Orchestra | CC Sampling Plus 1.0 [Cited S27] | none [Cited S27] |
| BBC SO Discover | proprietary EULA [Cited S31] | none [Cited S28] |

## Open questions and disagreements between sources

1. **GS ORCHESTRA note numbers are derived.** S2's text copy gives sounds row by row; the numbers come from aligning rows with the STANDARD column in GM order. Spot checks agree (Kick Drum2 = 35, Open Triangle = 81, 13 chromatic timpani F-f = 41-53) [Cited S2], but a Roland PDF with explicit numbers was not readable (encrypted). Verify before shipping a GS map.
2. **GS "Slap" (STANDARD 28)** may or may not be a slapstick or whip sound; S2 names it only [Cited S2].
3. **"Gong" versus "tam-tam".** S11 says Western symphonic usage reserves "gong" for the bossed (tuned) type, but also that players read "gong" as tam-tam and that modern orchestras use the words interchangeably [Cited S11]; Forsyth and Widor head the tam-tam chapter "The Gong" [Cited S4] [Cited S5]. Recommendation: `tam_tam` with the alias "gong"; a separate pitched id (for example `gong_tuned`) once a source for tuned-gong pitches is read [Estimate: design judgement from S4, S5 and S11].
4. **Tam-tam dynamics.** Forsyth: hardly more than a single p or mp stroke; a heavy blow sounds dull [Cited S4]. Widor: "terrifying when struck forte" [Cited S5]. S11 explains the difference with priming [Cited S11]. Recommendation: allow the full dynamic range and model the slow "bloom" of an unprimed stroke [Estimate: design judgement from S4, S5 and S11].
5. **Gossec's first orchestral tam-tam**: 1790 in one passage of S11, 1791 (Mirabeau funeral march) in another [Cited S11].
6. **Snare drum in symphonies.** Widor (1906): never used in a symphony [Cited S5]; S6 lists Nielsen's Fifth Symphony and Ravel's Bolero [Cited S6]. Historical, not a data conflict.
7. **Tabor snare.** Forsyth: a single hemp snare (and says the best authorities agree); Widor: "without timbre" [Cited S4] [Cited S5].
8. **Names.** Italian snare: "tamburo militare" (S4), "tamburo piccolo" (S5), "rullante" (S23). Italian castanets: "castagnette" (S5) vs "nacchere" (S23). German tambourine: "Schellentrommel" (S4, S5) vs "Tamburin" (S23). Czech: the Wikipedia titles "Vířivý buben" and "Basový buben" (S23) differ from the common score terms "malý buben" and "velký buben" (Estimate); a Czech orchestration source is needed. Missing cs, de or fr titles (suspended cymbal, clash cymbals, wood block, temple blocks, shaker, whip) are Estimates.
9. **Number of percussionists.** No source read states it (S21 lists instruments per era; S22 gives no count). Every `typical_players` count in this file is an Estimate.
10. **Sonara `GM_MAP` lacks GM1 triangles (80, 81)** and 11 other GM1 notes [Cited S1] [Cited S31]; the Drum Kit's 29 sounds have golden vectors (GLD-01) [Cited S31]. Adding orchestral names to `GM_MAP` is an owner decision; an orchestral map beside it avoids touching golden data [Estimate: reading of the project rules, not an owner decision].
11. **The word "shaker".** Sonara's `shaker` = GM 69/70 (Cabasa, Maracas); GS 82 = Shaker; this file's `shaker` id = the shaker class [Cited S1] [Cited S2] [Cited S31]. A rename needs a decision because `DRUM_NAMES` is generator API [Estimate: reading of the Sonara code (S31) and the contract rules, not an owner decision].
12. **Clash cymbals fallback.** GM has only kit crashes (49, 57); GS ORCHESTRA puts Concert Cymbal1 on 59, which GM and Sonara treat as ride [Cited S1] [Cited S2] [Cited S31].
13. **Sonara sampler opcodes.** `docs/sample-libraries.md` lists `trigger=release` but not `trigger=release_key`, and says filters and CC-driven crossfades are unsupported [Cited S31]. SSO's percussion uses `release_key` (snare and triangle rolls), `xfin`/`xfout` velocity crossfades (bass drum) and `fil_type=hpf_2p` (triangle roll, finger cymbal) [Cited S27]. Whether `release_key` and velocity crossfades play correctly in Sonara is unverified.
14. **VSCO `GM-StylePerc.sfz` is not GM.** Despite its name it puts snare rolls on 39 and 41 (GM Hand Clap, Low Floor Tom), gong on 42 and 46 (GM hi-hats), log drums on 76/77 (GM wood blocks) and sleigh bells on 82 (GS Shaker) [Cited S24] [Cited S1] [Cited S2]. It also puts ratchets on 69-71 (GM Cabasa, Maracas, Short Whistle), suspended-cymbal crescendos on 47, 48 and 50 (GM toms), suspended-cymbal hits on 51 and 59 (GM ride cymbals), an anvil on 67 and a brake drum on 68 (GM agogos), and bell-tree strokes on 83-86 (GS Jingle Bell, Bell Tree, Castanets, Mute Surdo) [Cited S24] [Cited S1] [Cited S2]. A GM file played through it gives wrong sounds; a remap is needed.
15. **Undocumented sample names**: VSCO `HitFM` (triangle), `bassdrum_rub`, `gong_smack`, "SN"/"NS"; whether VSCO's and VCSL's "Gong" folders hold tam-tams [Cited S24] [Cited S26]. Listening or the authors' notes would settle them.
16. **Rimsky's "little bells"** in the high group: sleigh bells or a glockenspiel-like set? [Cited S3].
17. **Beaters as a separate dimension.** Sources distinguish hard stick, soft (timpani) stick, mallet, brush and rute on the same instrument [Cited S4] [Cited S6] [Cited S15]. A `beater` field (stick, soft_mallet, hard_mallet, brush, rute, hand, bow) would avoid multiplying articulation ids; `rute` and `bowed` are proposed as ids here only because the vocabulary has no beater field [Estimate: modelling recommendation].
18. **Snares off versus muffled.** Widor says "sans timbre" is obtained by loosening the snares or by muffling, at the player's choice [Cited S5]; this file proposes `snares_off` and keeps `con_sordino` for a covered head. The owner may prefer one id.
19. **Temple blocks: pitched or not.** Usually five blocks in a pentatonic tuning, yet "not commonly treated as pitched percussion" [Cited S16]. Recommendation: unpitched with five ordered keys [Estimate: design judgement from S16].
20. **Vendor note names are not MIDI numbers here.** The BBC SO Discover key list (S28) and the London Orchestral Percussion manual (S29: hits from C3, timpani key switches C0-F0, scene triggers C6-G6) give note names without stating an octave convention [Cited S28] [Cited S29]. This file quotes them verbatim and never converts them; they must not be read as C4 = 60 names. Many Kontakt and Spitfire products call middle C "C3" [Estimate: vendor practice, not stated on the pages read]. SSO note names, by contrast, are converted with the SFZ default c4 = 60 [Estimate: the file does not state its octave offset]; VSCO keys are plain integers [Cited S24].
21. **Clefs of the tam-tam and castanets.** Forsyth's text says the Gong is written "on one line without clef" and Widor says the same of the Castanets, while Forsyth's instrument table gives the Gong "Conventional bass-clef" and the Castanets "Conventional treble-clef", each "or a single line" [Cited S4] [Cited S5]. The JSON lists both; the table's column alignment comes from OCR text.

## Fact-check log

_Adversarial check, 2026-10-30. I reopened every source below in this session. Methods are in the "How" column. All 14 cards are unpitched, so there are no sounding or written ranges or transpositions to verify. The numbers that matter are `pitched`, `transposition`, the ranges (all null), the clefs, `gm_program` and `gm_percussion_notes`._

### What was checked and how

| Source | How it was reopened | Result |
|---|---|---|
| S1 GM1 sound set | `curl` of the same Wayback URL, because WebFetch refuses web.archive.org. I read the whole percussion key map (35-81), programs 113-120, and the sentence "only these sounds are supported by General MIDI Level 1 devices". | Confirmed: every `gm_percussion_notes` value (38/40, 35/36, 49/57, 49, 81/80, 54, 76/77, 70), `gm_program` 116 "Woodblock", and the cross-references to 113 Tinkle Bell, 117 Taiko Drum, 120 Reverse Cymbal. |
| S2 Roland SC-55 drum-set table | `curl` of the manualsdir page; I re-aligned the rows against STANDARD. | Confirmed: 35/36 Concert BD2/BD1, 37 Side Stick ←, 38/40 Concert SD, 39 Castanets, 41-53 Timpani F to f (F2-F3 at C4 = 60), 57/59 Concert Cymbal2/1, 28 Slap → Pedal Hi-hat, EXC5 on 80/81, 82 Shaker, 83 Jingle Bell, 85 Castanets, 88 Applause. One error found (change 1). |
| S24 VSCO 2 CE | Re-downloaded and parsed `GM-StylePerc.sfz` (every region's key, velocity split, `seq_position`); re-listed the GitHub tree (3,399 entries). | Confirmed: all key numbers quoted in the cards, `ampeg_release=3` in `<global>`, folder counts (bass 39, susp 29, clash 8, four triangle folders, snare folders drum1/drum2/drum3_marching/OldSnare), the rimshot, nosnare, bell, scrape and bow files. One error (change 6). |
| S25 / S26 licences and VCSL folders | GitHub API and raw `LICENSE`/`README.md`; VCSL tree re-listed. | Confirmed: VSCO 2 CE is CC0 1.0. VCSL is "Creative Commons 0", and every VCSL folder named in the cards exists. VCSL has no castanets, maracas or temple blocks. |
| S27 SSO | Re-downloaded the SFZ (path corrected, change 4) and converted every note name with C4 = 60. | Confirmed: all keys (bass drum 38, snare 39-40, cymbal rolls 41-44, piatti 45, tambourine 52-54, triangle 55-58, finger cymbals 59-60, castanets 61, shakers 64-66, tam-tam 67-69, sleigh bells 70, bell tree 71, wood blocks 74/75). Also the `xfin`/`xfout` 70-90 crossfade, the `seq_length=2` hands, `group`/`off_by` on the triangle, `release_key` on the snare and triangle rolls, `hpf_2p` at 300 Hz, the extra shaker-hi/lo files, and the CC Sampling Plus 1.0 licence. |
| S28 Spitfire BBC SO Discover | WebFetch. | Confirmed: all 16 key names (C2 Bass Drum ... D4 Woodblock). The page states no octave convention (new open question 20). |
| S29 London Orchestral Percussion manual | Text extracted from the PDF streams; some lines are garbled. | Confirmed: basic hits from C3, rolls at the top of the mapped keys with gaps between articulation types, timpani key switches C0/C#0/D0/D#0/F0, Vel Sens, Crush on bass drums/toms/snares/timpani, swell in seconds or beats, sequencer, Kontakt 7, single end-user licence. No CC is named. |
| S30 Logic Pro Articulation Set Editor | `curl` of the page (WebFetch got only the navigation). | Confirmed: "compatibility with third-party sample libraries", Permanent/Momentary/Toggle modes (plus Trigger variants), and the Note On, Controller and Program message types. |
| S3, S4, S5 handbooks | Full texts re-downloaded from Gutenberg and archive.org and searched. | Confirmed (examples): Forsyth's table "Pitch indeterminate" with conventional treble or bass clef; "S.D.", "B.D.", "Cym.", "G.C."; Ruthe in Il Seraglio and Mahler 2; the "32 ft" organ pipes; three bass drums in the Battle of Vittoria; "Batterie"/"Kitchen"; "inverse proportion to their use"; Schallbecken/Schale/Teller; the hard/soft-stick and two-stick roll; finger-tip diminuendo; a single p or mp gong stroke; "dull unpleasant sound"; "one line without clef"; four tuned tam-tams plus a fifth rolled at Bayreuth; macho/hembra; the handle "for use in the orchestra and military bands"; the tambourine laid on a kettle-drum; Liszt's E-flat concerto; the Meistersinger triangle. Widor: "never been employed in a symphony"; "sans timbre"; mailloche simple/double; "not worth the trouble"; the Tannhäuser clash roll; "perfectly even and continuous roll"; "terrifying when struck forte"; "borrow the tonality"; "Pianoforte without dampers"; "sonorous altitudes"; the triangle's "inch or so"; "double sound"; red-heat into white-heat; castanets on "a single line, without any clef". Rimsky: the high/medium/deep groups, the rhythm-and-colour combinations, "overpower any orchestral tutti", the gong as church bell (Ivan the Terrible), the bass drum as cannon (Tsar Saltan), "too characteristic". A full-text search of S3, S4 and S5 finds no shaker, maracas, whip, wood block or sleigh bells. Forsyth's one "temple" hit is "temple-gongs". |
| S6-S22 Wikipedia | Raw wikitext re-downloaded; WebFetch also used for S12, S16 and S20. | Confirmed: the 14 in snare and 6 in shell; "floor tom-tom"; "even with a wire brush"; the pressed-roll sentence; Bolero and Nielsen 5; the bass drum's 40 x 20 in and "rolls, repetitions and unison strokes"; the three clash-cymbal weights, 14-22 in, the flam crash, "C.C.", the Russian "+"; chau gongs 7-80 in (20-200 cm); nipple gongs 15-50 cm; Gossec 1790 and 1791; priming; the Rite of Spring; Crumb's bow; the water gong; "used now interchangeably"; Brahms 4 "the only appearance of non-timpani percussion"; the triangle roll with three lines; beeswax/rosin; the wrist roll; Mozart; zills; handle castanets; rolls "in any of the three ways"; temple blocks pentatonic in fives, "not commonly treated as pitched"; Mahler 4 (1901), Kijé (1934); frusta/Peitsche/fouet; Mahler 5 and 6; S21's sentence on section percussionists. |
| S23 langlinks | API query re-run. | Confirmed: every cs/it/de/fr title quoted, and the missing articles. |
| S31 Sonara repository | Read locally. | Confirmed: `data.rs` `snare_drum` = `Unpitched`, `[Marcato, Roll]`, `ARTICULATION_ESTIMATE` ("general orchestration practice; no source was read for the articulation list"). `GM_MAP` has 34 entries; the 13 missing GM notes are as listed; `DRUM_NAMES` has 20. `CINEMATIC` rows are 41, 45, 36. `crash_on_first` adds 49. The fill has four snare strokes. The kit's `from_note` has `_ => return None`. `golden/numpy/drums` holds 58 files, so 29 sounds. The opcode list in `docs/sample-libraries.md` matches. The "Verification of data.rs" table for snare_drum is correct. |
| JSON structure | All 14 blocks parsed with a script. | Confirmed: exact keys in order, assigned ids in order, family `percussion`, existing-vocabulary ids only in `articulations_existing_vocab`. Each invented id (`snares_off`, `rute`, `bowed`, `scrape`, `thumb_roll`, `shake_stroke`) is defined once in prose. |

### Changed (was -> now, why)

1. **Mapping table, suspended_cymbal row, GS column.** Was "none (49, 51, 55 stay as STANDARD)". Now "in this kit 49 and 51 are Timpani c# and d#, 57/59 Concert Cymbal2/1, only 55 Splash stays". Why: in S2's ORCHESTRA column, rows 41-53 are all timpani. The old row contradicted the file's own "41-53 Timpani" bullet. Consequences added: the collision bullet now covers Sonara's `CRASH` 49, `RIDE` 51 and the hi-hats 42/44/46. clash_cymbals and suspended_cymbal section 9 note that 49 is a timpani note on that kit.
2. **suspended_cymbal section 4, Swell.** Was "from nearly inaudible to a loud climax" [S10]. Now S10's own wording, "from almost inaudible to an overwhelming climax" (shortened here; the consistency review turned the card's full quotation into a paraphrase). Why: the old quotation was not in S10.
3. **suspended_cymbal section 1, Italian.** Was [Cited S9] [Cited S23]. Now [Cited S23] only. Why: S9 contains no "piatto sospeso".
4. **suspended_cymbal, bowing.** The JSON `articulations_provenance` was "S10 (roll swell, bowing, bell)". Now "S10 (roll swell, coin or triangle-beater scrape, bell)". In section 5 the `bowed` row was [S9] [S10]; it is now [S9] only. In section 2 the S10 scrape is quoted. Why: S10 never mentions bowing; it does describe a coin or triangle-beater "zing".
5. **S27 source row.** Was `Percussion/All Unpitched Percussion.sfz`. Now `Sonatina Symphonic Orchestra/Percussion/All Unpitched Percussion.sfz`, with the C4 = 60 note-name convention stated. Why: the old path returns 404, and the repository tree shows the real path.
6. **maracas sections 5 and 10.** Was "VSCO 1 raw `maraca1-3.wav`" and "three raw maraca samples". Now `maraca1` to `maraca5` plus `maraca_shake` (six files). Why: the VSCO tree lists six files.
7. **maracas section 9, shaker section 9, open question 14.** Added that `GM-StylePerc.sfz` plays a ratchet on 69/70/71, so GM Cabasa and Maracas sound as a ratchet. Question 14 also gains the other non-GM placements: 47/48/50 crescendos, 51/59 suspended cymbal, 67 anvil, 68 brake drum, 83-86 bell tree. Why: these are in S24 and bear on a GM import.
8. **tam_tam `clefs`.** Was ["single_line", "percussion"]. Now ["single_line", "bass", "percussion"]. Section 3 cites the table. Why: Forsyth's table gives the Gong "Conventional bass-clef or a single line".
9. **castanets `clefs`.** Was ["single_line", "percussion"]. Now ["single_line", "treble", "percussion"]. Section 3 cites the table. Why: Forsyth's table gives the Castanets "Conventional treble-clef or a single line".
10. **castanets, GS 85.** Was "every GS kit" / "all kits". Now "all seven kits of the SC-55 table". Why: S2 shows only those seven sets, and other GS sets were not read.
11. **snare_drum section 5 and section 10, SSO roll.** Was "key 40 holds a looping roll sample and a `trigger=release_key` region plays the roll's end stroke". Now the opcodes are stated as read (`ampeg_release=0.1`, `release_key` from `offset=150243`, `transpose=-1 tune=-50`). "Looping" and "end stroke" are Estimates. Why: the SFZ has no `loop_mode`, and it does not say what the release region contains.
12. **castanets section 5.** Added that SSO's second round robin has `pitch_keycenter=c4` on the c#4 key, so it sounds a semitone sharp. tam_tam section 10 now says "29 WAV files", plus a subfolder of MP3s and a zip.
13. **Open questions.** Added 20: Spitfire and LOP note names are not MIDI numbers under this file's C4 = 60 header. Added 21: the clef disagreement behind changes 8 and 9.

### Downgraded to Estimate

- The SSO snare roll's "looping" sample and its release region being the "end stroke" (change 11). Changes 3 and 4 removed a wrong second citation; each claim keeps a valid citation, so neither is a downgrade.

### Remaining concerns

- `gm_percussion_notes` for the cymbals is a substitution policy, not a GM fact. Suspended cymbal [49] has some support from S9 (a kit crash is a suspended cymbal), but VSCO maps its suspended cymbal to 51/59. On a GS ORCHESTRA kit, 49 is a timpani note. snare_drum keeps 40 "Electric Snare", which GS ORCHESTRA also maps to Concert SD. The owner should decide whether the field holds exact GM names only.
- Forsyth's clef table was read from OCR text. The column alignment is consistent with the Wind-Machine row that the text confirms, but the printed page was not seen.
- No GS sets beyond the seven SC-55 kits were read. Other GS sets, such as an SFX set, may map 85 differently [Estimate: general knowledge of GS, not read].
- Czech, German and French names marked Estimate, every `typical_players` count, and every score abbreviation except "S.D.", "B.D.", "Cym." (S4) and "sus. cym." (S9) remain Estimates. No Czech orchestration source was read.
- Whether the Sonara sampler honours `trigger=release_key` and `xfin`/`xfout` velocity crossfades (open question 13) is still unverified. So is whether the VSCO and VCSL "Gong" folders hold tam-tams.

## Consistency review

Review of 2026-10-30 against the JSON blocks, section 3, section 9, the data.rs table, the fact-check log, tag hygiene and quote licence. Every json correction in the fact-check log (tam_tam and castanets clefs, suspended_cymbal provenance, the mapping-table GS column) was found applied in both json and prose; every [Cited Sn] points to an existing source row.

- S31 source row: the absolute local path in the URL column was replaced by "local checkout of the Sonara repository, branch `research/orchestra-instruments`".
- New convention paragraph under the Sources table: `percussion` in every `clefs` list is the modern engraving default [Estimate: modern engraving practice; no source read states it]; before, the json listed it on all 14 cards while only snare_drum section 3 mentioned it.
- snare_drum json: `con_sordino` added to `articulations_existing_vocab`, matching section 5 (muffled = `con_sordino`), section 6 (muffled side drums, S4), Open question 18 ("keeps `con_sordino` for a covered head") and the json's own provenance ("S4 ... muffled"); the data.rs verification row and the section 5 row now say so.
- suspended_cymbal section 4, Swell: a 16-word verbatim Wikipedia quotation (S10) paraphrased; fact-check change 2 now quotes only a short part of it.
- temple_blocks section 2: a 16-word verbatim Wikipedia quotation (S16) on the pentatonic set of five paraphrased.
- sleigh_bells section 2: the S17 sentence on how the simplest bells are made reworded as a precaution against close paraphrase (S17 was not reopened).
- tam_tam section 1: added the score abbreviation "T.-t." [Estimate: common abbreviation, no source read], which the json carried but no prose mentioned.
- Fact-check log, remaining concerns: "every score abbreviation remain Estimates" corrected; "S.D.", "B.D.", "Cym." (S4) and "sus. cym." (S9) are cited.
- Gave every bare Estimate tag its reason (61 tags); the parenthesised "(Estimate)" markers inside the json strings were left unchanged.
