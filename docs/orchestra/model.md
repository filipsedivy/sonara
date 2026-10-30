# Orchestral instrument model

_Generated from the json blocks of the instrument cards in [cards/](cards/) on 2026-10-30. Status: research data for the proposal in [../orchestra-design.md](../orchestra-design.md); nothing here is implemented. Edit the cards, not this file._

MIDI note numbers with C4 = 60 = middle C. Ranges are **sounding**; the written range is the sounding range minus the transposition (transposition = sounding − written, in semitones). "Cited" means a source that was opened states the value (the card names it); "Estimate" means it rests on a stated reason. GM is the General MIDI Level 1 program, numbered 1–128; unpitched percussion lists its GM channel-10 notes. Ensemble templates are in [ensembles.md](ensembles.md); articulation ids are explained in [articulation-vocabulary.md](articulation-vocabulary.md).

85 instruments: 17 woodwinds, 13 brass, 22 percussion, 15 keyboards, 4 harp and plucked strings, 10 voices, 4 bowed strings. 16 of them exist in `crates/sonara-midi/src/orchestration/data.rs` today.

## Woodwinds

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `piccolo` | Piccolo / pikola / ottavino | Picc. | D5–C8 (74–108) | G#5–A#7 (80–106) | Cited / Cited | 12 | D4–C7 (62–96) | treble | 73 Piccolo | solo | yes | [flutes](cards/flutes.md#piccolo-piccolo) |
| `flute` | Flute / flétna / flauto | Fl. | B3–D7 (59–98) | C4–A#6 (60–94) | Cited / Cited | 0 | B3–D7 (59–98) | treble | 74 Flute | both | yes | [flutes](cards/flutes.md#flute-flute) |
| `alto_flute` | Alto flute / altová flétna / flauto contralto | Alto Fl. | G3–G6 (55–91) | G3–A5 (55–81) | Cited / Estimate | -5 | C4–C7 (60–96) | treble | – | solo | no | [flutes](cards/flutes.md#alto_flute-alto-flute) |
| `bass_flute` | Bass flute / basová flétna / flauto basso | B. Fl. | B2–C6 (47–84) | C3–A5 (48–81) | Cited / Cited | -12 | B3–C7 (59–96) | treble | – | solo | no | [flutes](cards/flutes.md#bass_flute-bass-flute) |
| `oboe` | Oboe / hoboj / oboe | Ob. | A#3–G6 (58–91) | C4–D#6 (60–87) | Cited / Cited | 0 | A#3–G6 (58–91) | treble | 69 Oboe | both | yes | [double_reeds](cards/double_reeds.md#oboe-oboe) |
| `english_horn` | English horn / anglický roh / corno inglese | Eng. horn | E3–C6 (52–84) | E3–A5 (52–81) | Cited / Cited | -7 | B3–G6 (59–91) | treble | 70 English Horn | solo | yes | [double_reeds](cards/double_reeds.md#english_horn-english-horn) |
| `oboe_damore` | Oboe d'amore / hoboj d'amore / oboe d'amore | Ob. d'am. | G#3–E6 (56–88) | G#3–C#6 (56–85) | Cited / Cited | -3 | B3–G6 (59–91) | treble | – | solo | no | [double_reeds](cards/double_reeds.md#oboe_damore-oboe-damore) |
| `bassoon` | Bassoon / fagot / fagotto | Fag. | A#1–G#5 (34–80) | A#1–C5 (34–72) | Cited / Cited | 0 | A#1–G#5 (34–80) | bass, tenor, treble | 71 Bassoon | both | yes | [double_reeds](cards/double_reeds.md#bassoon-bassoon) |
| `contrabassoon` | Contrabassoon / kontrafagot / contrafagotto | C-fag. | A#0–D4 (22–62) | A#0–A3 (22–57) | Cited / Cited | -12 | A#1–D5 (34–74) | bass, tenor | – | solo | yes | [double_reeds](cards/double_reeds.md#contrabassoon-contrabassoon) |
| `clarinet_bb` | Clarinet in B♭ / klarinet in B (B klarinet) / clarinetto in si♭ | Cl. (B♭) | D3–A#6 (50–94) | D3–G6 (50–91) | Cited / Cited | -2 | E3–C7 (52–96) | treble | 72 Clarinet | both | yes | [clarinets](cards/clarinets.md#clarinet_bb-clarinet-in-b) |
| `clarinet_a` | Clarinet in A / klarinet in A (A klarinet) / clarinetto in la | Cl. (A) | C#3–A6 (49–93) | C#3–F#6 (49–90) | Cited / Cited | -3 | E3–C7 (52–96) | treble | 72 Clarinet | both | no | [clarinets](cards/clarinets.md#clarinet_a-clarinet-in-a) |
| `clarinet_eb` | E♭ clarinet / Es klarinet / clarinetto piccolo in mi♭ | Cl. picc. (E♭) | G3–C7 (55–96) | D#4–A#6 (63–94) | Cited / Cited | 3 | E3–A6 (52–93) | treble | 72 Clarinet | solo | no | [clarinets](cards/clarinets.md#clarinet_eb-e-clarinet) |
| `bass_clarinet` | Bass clarinet / basklarinet / clarinetto basso (clarone) | Cl. b. | A#1–D6 (34–86) | C#2–D5 (37–74) | Cited / Cited | -14 | C3–E7 (48–100) | treble, bass | 72 Clarinet | solo | no | [clarinets](cards/clarinets.md#bass_clarinet-bass-clarinet) |
| `saxophone_soprano` | Soprano saxophone / sopránový saxofon / sassofono soprano | S. Sax. | G#3–A#6 (56–94) | G#3–D#6 (56–87) | Cited / Cited | -2 | A#3–C7 (58–96) | treble | 65 Soprano Sax | solo | no | [saxophones](cards/saxophones.md#saxophone_soprano-soprano-saxophone) |
| `saxophone_alto` | Alto saxophone / altový saxofon / sassofono contralto | A. Sax. | C#3–D#6 (49–87) | C#3–G#5 (49–80) | Cited / Cited | -9 | A#3–C7 (58–96) | treble | 66 Alto Sax | solo | no | [saxophones](cards/saxophones.md#saxophone_alto-alto-saxophone) |
| `saxophone_tenor` | Tenor saxophone / tenorový saxofon / sassofono tenore | T. Sax. | G#2–A#5 (44–82) | G#2–D#5 (44–75) | Cited / Cited | -14 | A#3–C7 (58–96) | treble | 67 Tenor Sax | solo | no | [saxophones](cards/saxophones.md#saxophone_tenor-tenor-saxophone) |
| `saxophone_baritone` | Baritone saxophone / barytonový saxofon / sassofono baritono | Bar. Sax. | C2–D#5 (36–75) | C#2–G#4 (37–68) | Cited / Cited | -21 | A3–C7 (57–96) | treble | 68 Baritone Sax | solo | no | [saxophones](cards/saxophones.md#saxophone_baritone-baritone-saxophone) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `piccolo` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | staccatissimo, multi_tongue, fingered_tremolo, sforzando, fortepiano, crescendo_swell, rip, fall, key_click, air_tone, vibrato_on |
| `flute` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, harmonics | staccatissimo, portato, multi_tongue, fingered_tremolo, sforzando, crescendo_swell, rip, fall, glissando, key_click, slap_tongue, tongue_ram, air_tone, jet_whistle, whistle_tone, multiphonic, vibrato_on, non_vibrato |
| `alto_flute` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | staccatissimo, portato, multi_tongue, sforzando, crescendo_swell, rip, fall, key_click, air_tone, vibrato_on, non_vibrato |
| `bass_flute` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | staccatissimo, portato, multi_tongue, sforzando, crescendo_swell, rip, fall, vibrato_on |
| `oboe` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | portato, sforzando, fortepiano, crescendo_swell, trill_half, trill_whole, fingered_tremolo, glissando, multiphonic, key_click, slap_tongue, vibrato_on, non_vibrato |
| `english_horn` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | portato, sforzando, fortepiano, crescendo_swell, trill_half, trill_whole, fingered_tremolo, glissando, bisbigliando, vibrato_on, non_vibrato |
| `oboe_damore` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | portato, trill_half, trill_whole |
| `bassoon` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | portato, sforzando, crescendo_swell, trill_half, trill_whole, fingered_tremolo, glissando, multiphonic, key_click, slap_tongue, vibrato_on, non_vibrato |
| `contrabassoon` | sustain, legato, staccato, tenuto, marcato, flutter_tongue | sforzando, fortepiano, vibrato_on |
| `clarinet_bb` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | staccatissimo, portato, sforzando, trill_half, trill_whole, fingered_tremolo, multi_tongue, glissando, growl, crescendo_swell |
| `clarinet_a` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | staccatissimo, portato, sforzando, trill_half, trill_whole, fingered_tremolo, multi_tongue, glissando, growl, crescendo_swell |
| `clarinet_eb` | sustain, legato, staccato, trill, flutter_tongue | staccatissimo, portato, sforzando, trill_half, trill_whole, crescendo_swell |
| `bass_clarinet` | sustain, legato, staccato, marcato, trill, flutter_tongue | staccatissimo, portato, sforzando, trill_half, trill_whole, multi_tongue |
| `saxophone_soprano` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, harmonics | staccatissimo, fortepiano, crescendo_swell, vibrato_on, non_vibrato, trill_half, trill_whole, glissando, scoop, fall, doit, growl, subtone, slap_tongue, multiphonic, key_click |
| `saxophone_alto` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, harmonics | staccatissimo, fortepiano, crescendo_swell, vibrato_on, non_vibrato, trill_half, trill_whole, glissando, scoop, fall, doit, rip, growl, subtone, slap_tongue, multiphonic, key_click |
| `saxophone_tenor` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, harmonics | staccatissimo, fortepiano, crescendo_swell, vibrato_on, non_vibrato, trill_half, trill_whole, glissando, scoop, fall, doit, rip, growl, subtone, slap_tongue, multiphonic, key_click |
| `saxophone_baritone` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, harmonics | staccatissimo, fortepiano, crescendo_swell, vibrato_on, non_vibrato, trill_half, trill_whole, glissando, scoop, fall, doit, rip, growl, subtone, slap_tongue, multiphonic, key_click |

## Brass

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `horn_f` | Horn in F / lesní roh / corno | Hn. | B1–F5 (35–77) | F2–C5 (41–72) | Cited / Estimate | -7 | F#2–C6 (42–84) | treble, bass | 61 French Horn | both | yes | [horns](cards/horns.md#horn_f-horn-in-f) |
| `wagner_tuba_tenor` | Tenor Wagner tuba (in B-flat) / tenorová Wagnerova tuba / tuba wagneriana tenore | Ten. Tb. | D#1–A#5 (27–82) | E2–D5 (40–74) | Cited / Estimate | -2 | F1–C6 (29–84) | treble, bass | – | section | no | [horns](cards/horns.md#wagner_tuba_tenor-tenor-wagner-tuba) |
| `wagner_tuba_bass` | Bass Wagner tuba (in F) / basová Wagnerova tuba / tuba wagneriana bassa | B. Tb. | D#1–F5 (27–77) | A#1–G4 (34–67) | Cited / Estimate | -7 | A#1–C6 (34–84) | treble, bass | – | section | no | [horns](cards/horns.md#wagner_tuba_bass-bass-wagner-tuba) |
| `trumpet_bb` | Trumpet in B♭ / trubka in B / tromba in si♭ | B♭ Tpt. | E3–G6 (52–91) | E3–A#5 (52–82) | Cited / Cited | -2 | F#3–A6 (54–93) | treble | 57 Trumpet | both | yes | [high_brass](cards/high_brass.md#trumpet_bb-trumpet-in-b) |
| `trumpet_c` | Trumpet in C / trubka in C / tromba in do | C Tpt. | F#3–F6 (54–89) | F#3–C6 (54–84) | Cited / Cited | 0 | F#3–F6 (54–89) | treble | 57 Trumpet | both | no | [high_brass](cards/high_brass.md#trumpet_c-trumpet-in-c) |
| `trumpet_piccolo` | Piccolo trumpet / piccolo trubka / tromba piccola | P. Tpt. B♭ | B3–G6 (59–91) | D4–D6 (62–86) | Cited / Cited | 10 | C#3–A5 (49–81) | treble | 57 Trumpet | solo | no | [high_brass](cards/high_brass.md#trumpet_piccolo-piccolo-trumpet) |
| `cornet` | Cornet in B♭ / kornet / cornetta | B♭ Cnt. | E3–G6 (52–91) | E3–A#5 (52–82) | Cited / Cited | -2 | F#3–A6 (54–93) | treble | 57 Trumpet | both | no | [high_brass](cards/high_brass.md#cornet-cornet-in-b) |
| `flugelhorn` | Flugelhorn / křídlovka / flicorno soprano | Flghn. | E3–C6 (52–84) | E3–A#5 (52–82) | Cited / Cited | -2 | F#3–D6 (54–86) | treble | 57 Trumpet | both | no | [high_brass](cards/high_brass.md#flugelhorn-flugelhorn) |
| `trombone_tenor` | Tenor trombone / tenorový pozoun (tenorový trombon) / trombone tenore | Tbn. | E2–F5 (40–77) | E2–D5 (40–74) | Cited / Cited | 0 | E2–F5 (40–77) | bass, tenor, alto | 58 Trombone | both | yes | [low_brass](cards/low_brass.md#trombone_tenor-tenor-trombone) |
| `trombone_bass` | Bass trombone / basový pozoun (basový trombon) / trombone basso | B. Tbn. | A#0–F5 (22–77) | G1–G4 (31–67) | Cited / Estimate | 0 | A#0–F5 (22–77) | bass, tenor | 58 Trombone | both | no | [low_brass](cards/low_brass.md#trombone_bass-bass-trombone) |
| `tuba` | Tuba / tuba / tuba (tuba bassa) | Tba. | A0–G4 (21–67) | F1–C4 (29–60) | Cited / Cited | 0 | A0–G4 (21–67) | bass | 59 Tuba | solo | no | [low_brass](cards/low_brass.md#tuba-tuba) |
| `cimbasso` | Cimbasso / cimbasso / cimbasso (Verdi: trombone basso) | Cimb. | F1–F4 (29–65) | F#1–C4 (30–60) | Cited / Estimate | 0 | F1–F4 (29–65) | bass | – | solo | no | [low_brass](cards/low_brass.md#cimbasso-cimbasso) |
| `euphonium` | Euphonium / eufonium / eufonio | Euph. | B0–D5 (23–74) | E2–G4 (40–67) | Cited / Cited | 0 | B0–D5 (23–74) | bass, tenor, treble | – | both | no | [low_brass](cards/low_brass.md#euphonium-euphonium) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `horn_f` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | stopped, cuivre, bells_up, glissando, rip, fall, staccatissimo, sforzando, fortepiano, crescendo_swell, trill_whole, trill_half, multi_tongue, multiphonic |
| `wagner_tuba_tenor` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | portato, sforzando, fortepiano, glissando, multi_tongue, vibrato_on |
| `wagner_tuba_bass` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | portato, sforzando, fortepiano, glissando, multi_tongue, vibrato_on |
| `trumpet_bb` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | accent, sforzando, fortepiano, crescendo_swell, portato, glissando, portamento, half_valve, fall, doit, shake, scoop, rip, growl, cuivre, multi_tongue, vibrato_on, non_vibrato, mute_cup, mute_harmon, mute_plunger, mute_bucket, wah_wah |
| `trumpet_c` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | accent, sforzando, fortepiano, crescendo_swell, portato, glissando, half_valve, cuivre, multi_tongue, vibrato_on, non_vibrato, mute_cup, mute_harmon, mute_bucket |
| `trumpet_piccolo` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | portato, sforzando, fortepiano, multi_tongue, vibrato_on, non_vibrato |
| `cornet` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | accent, portato, multi_tongue, glissando, half_valve, fall, shake, growl, vibrato_on, mute_cup, mute_harmon, mute_plunger, wah_wah |
| `flugelhorn` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue | portato, multi_tongue, vibrato_on, fall, shake, glissando, half_valve |
| `trombone_tenor` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | glissando, staccatissimo, sforzando, fortepiano, crescendo_swell, rip, fall, doit, cuivre, vibrato_on, growl |
| `trombone_bass` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | glissando, staccatissimo, sforzando, fortepiano, crescendo_swell, rip, fall, cuivre, vibrato_on |
| `tuba` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | staccatissimo, sforzando, fortepiano, crescendo_swell, rip, fall, half_valve, vibrato_on |
| `cimbasso` | sustain, legato, staccato, tenuto, marcato, flutter_tongue, con_sordino | staccatissimo, sforzando, crescendo_swell, cuivre, rip, fall, half_valve |
| `euphonium` | sustain, legato, staccato, tenuto, marcato, trill, flutter_tongue, con_sordino | staccatissimo, sforzando, crescendo_swell, vibrato_on, half_valve |

## Percussion

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `timpani` | Timpani (kettledrums) / tympány / timpani | Timp. | C2–C#4 (36–61) | D2–A3 (38–57) | Cited / Cited | 0 | C2–C#4 (36–61) | bass | 48 Timpani | solo | no | [tuned_percussion](cards/tuned_percussion.md#timpani-timpani) |
| `xylophone` | Xylophone / xylofon / xilofono | Xyl. | C4–C8 (60–108) | F4–C8 (65–108) | Cited / Cited | 12 | C3–C7 (48–96) | treble | 14 Xylophone | solo | yes | [tuned_percussion](cards/tuned_percussion.md#xylophone-xylophone) |
| `marimba` | Marimba / marimba / marimba | Mar. | C2–C7 (36–96) | A2–C7 (45–96) | Cited / Cited | 0 | C2–C7 (36–96) | treble, bass | 13 Marimba | solo | no | [tuned_percussion](cards/tuned_percussion.md#marimba-marimba) |
| `vibraphone` | Vibraphone / vibrafon / vibrafono | Vib. | C3–F6 (48–89) | F3–F6 (53–89) | Cited / Cited | 0 | C3–F6 (48–89) | treble | 12 Vibraphone | solo | no | [tuned_percussion](cards/tuned_percussion.md#vibraphone-vibraphone) |
| `glockenspiel` | Glockenspiel (orchestral bells) / zvonkohra / campanelli | Glock. | C5–C8 (72–108) | G5–C8 (79–108) | Cited / Cited | 24 | C3–C6 (48–84) | treble | 10 Glockenspiel | solo | yes | [tuned_percussion](cards/tuned_percussion.md#glockenspiel-glockenspiel) |
| `tubular_bells` | Tubular bells (chimes) / trubicové zvony / campane tubolari | Tub. B. | D#3–G5 (51–79) | C4–F5 (60–77) | Cited / Cited | 0 | D#3–G5 (51–79) | treble | 15 Tubular Bells | solo | no | [tuned_percussion](cards/tuned_percussion.md#tubular_bells-tubular-bells) |
| `crotales` | Crotales (antique cymbals) / krotaly / crotali | Crot. | C6–C8 (84–108) | C6–C8 (84–108) | Cited / Cited | 24 | C4–C6 (60–84) | treble | – | solo | no | [tuned_percussion](cards/tuned_percussion.md#crotales-crotales) |
| `carillon` | Carillon / zvonohra (věžní zvonkohra) / carillon | Car. | C3–C7 (48–96) | – | Estimate | 0 | C3–C7 (48–96) | treble, bass | – | solo | no | [tuned_percussion](cards/tuned_percussion.md#carillon-carillon) |
| `snare_drum` | Snare drum / malý buben / tamburo militare | S.D. | – | – | – | – | – | single_line, treble, percussion | perc 38, 40 | solo | yes | [untuned_percussion](cards/untuned_percussion.md#snare_drum-snare-drum) |
| `bass_drum` | Bass drum / velký buben / gran cassa | B.D. | – | – | – | – | – | single_line, bass, percussion | perc 35, 36 | solo | no | [untuned_percussion](cards/untuned_percussion.md#bass_drum-bass-drum) |
| `clash_cymbals` | Clash cymbals / činely / piatti | Cym. | – | – | – | – | – | single_line, bass, percussion | perc 49, 57 | solo | no | [untuned_percussion](cards/untuned_percussion.md#clash_cymbals-clash-cymbals) |
| `suspended_cymbal` | Suspended cymbal / zavěšený činel / piatto sospeso | sus. cym. | – | – | – | – | – | single_line, bass, percussion | perc 49 | solo | no | [untuned_percussion](cards/untuned_percussion.md#suspended_cymbal-suspended-cymbal) |
| `tam_tam` | Tam-tam / tam-tam / tam-tam | T.-t. | – | – | – | – | – | single_line, bass, percussion | – | solo | no | [untuned_percussion](cards/untuned_percussion.md#tam_tam-tam-tam) |
| `triangle` | Triangle / triangl / triangolo | Trgl. | – | – | – | – | – | single_line, treble, percussion | perc 81, 80 | solo | no | [untuned_percussion](cards/untuned_percussion.md#triangle-triangle) |
| `tambourine` | Tambourine / tamburína / tamburino | Tamb. | – | – | – | – | – | single_line, treble, percussion | perc 54 | solo | no | [untuned_percussion](cards/untuned_percussion.md#tambourine-tambourine) |
| `castanets` | Castanets / kastaněty / castagnette | Cast. | – | – | – | – | – | single_line, treble, percussion | – | solo | no | [untuned_percussion](cards/untuned_percussion.md#castanets-castanets) |
| `wood_block` | Wood block / dřevěný blok / cassa di legno | W.B. | – | – | – | – | – | single_line, percussion | 116 Woodblock | solo | no | [untuned_percussion](cards/untuned_percussion.md#wood_block-wood-block) |
| `temple_blocks` | Temple blocks / temple bloky / temple block | T.Bl. | – | – | – | – | – | percussion | – | solo | no | [untuned_percussion](cards/untuned_percussion.md#temple_blocks-temple-blocks) |
| `sleigh_bells` | Sleigh bells / rolničky / sonagli | Sl. B. | – | – | – | – | – | single_line, percussion | – | solo | no | [untuned_percussion](cards/untuned_percussion.md#sleigh_bells-sleigh-bells) |
| `maracas` | Maracas / rumba koule / maracas | Mar. | – | – | – | – | – | single_line, percussion | perc 70 | solo | no | [untuned_percussion](cards/untuned_percussion.md#maracas-maracas) |
| `shaker` | Shaker / šejkr / shaker | Shk. | – | – | – | – | – | single_line, percussion | – | solo | no | [untuned_percussion](cards/untuned_percussion.md#shaker-shaker) |
| `whip` | Whip / bič / frusta | Whip | – | – | – | – | – | single_line, percussion | – | solo | no | [untuned_percussion](cards/untuned_percussion.md#whip-whip-slapstick) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `timpani` | roll, marcato, con_sordino | let_ring, damped, glissando, crescendo_swell, tremolo_measured, fortepiano |
| `xylophone` | staccato, marcato, tremolo | glissando, dead_stroke, bowed |
| `marimba` | staccato, marcato, tremolo, trill | glissando, dead_stroke, bowed |
| `vibraphone` | staccato, marcato, tremolo | let_ring, damped, dead_stroke, glissando, bowed, vibrato_on, non_vibrato, portamento |
| `glockenspiel` | staccato, marcato, tremolo, trill | let_ring, damped, glissando, bowed |
| `tubular_bells` | marcato, tremolo | let_ring, damped, glissando |
| `crotales` | marcato, tremolo | let_ring, damped, bowed |
| `carillon` | tremolo, marcato | let_ring |
| `snare_drum` | roll, marcato, con_sordino | flam, drag, rimshot, rim, snares_off |
| `bass_drum` | roll, marcato, con_sordino | damped, let_ring, rute |
| `clash_cymbals` | roll, marcato | let_ring, damped, choke |
| `suspended_cymbal` | roll, marcato | crescendo_swell, let_ring, damped, choke, bowed, scrape |
| `tam_tam` | roll, marcato | let_ring, damped, crescendo_swell, scrape, bowed |
| `triangle` | roll, marcato | let_ring, damped |
| `tambourine` | roll, marcato | thumb_roll, shake_stroke |
| `castanets` | roll, marcato | – |
| `wood_block` | marcato, roll | – |
| `temple_blocks` | marcato, roll | glissando |
| `sleigh_bells` | roll, marcato | shake_stroke |
| `maracas` | roll, marcato | shake_stroke |
| `shaker` | roll, marcato | shake_stroke |
| `whip` | marcato | – |

## Keyboards

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `piano_concert_grand` | Concert grand piano / koncertní křídlo / pianoforte a coda da concerto (gran coda) | Pno. | A0–C8 (21–108) | A0–C8 (21–108) | Cited / Estimate | 0 | A0–C8 (21–108) | treble, bass | 1 Acoustic Grand Piano | solo | no | [acoustic_pianos](cards/acoustic_pianos.md#piano_concert_grand-concert-grand-piano) |
| `piano_studio_grand` | Studio grand piano / křídlo (salonní/studiové) / pianoforte a coda (mezza coda / tre quarti di coda) | Pno. | A0–C8 (21–108) | A0–C8 (21–108) | Cited / Estimate | 0 | A0–C8 (21–108) | treble, bass | 2 Bright Acoustic Piano | solo | no | [acoustic_pianos](cards/acoustic_pianos.md#piano_studio_grand-studio-grand-piano) |
| `piano_upright` | Upright piano / pianino / pianoforte verticale | Pno. | A0–C8 (21–108) | A0–C8 (21–108) | Cited / Estimate | 0 | A0–C8 (21–108) | treble, bass | – | solo | no | [acoustic_pianos](cards/acoustic_pianos.md#piano_upright-upright-piano) |
| `piano_felt` | Felt piano / pianino s moderátorem (plstěný klavír) / pianoforte con feltro | Pno. | A0–C8 (21–108) | A0–C8 (21–108) | Cited / Estimate | 0 | A0–C8 (21–108) | treble, bass | – | solo | no | [acoustic_pianos](cards/acoustic_pianos.md#piano_felt-felt-piano) |
| `piano_prepared` | Prepared piano / preparovaný klavír / pianoforte preparato | Prep. Pno. | A0–C8 (21–108) | A0–C8 (21–108) | Cited / Estimate | 0 | A0–C8 (21–108) | treble, bass | – | solo | no | [acoustic_pianos](cards/acoustic_pianos.md#piano_prepared-prepared-piano) |
| `piano_honky_tonk` | Honky-tonk piano / honky-tonk piano (rozladěné pianino) / pianoforte honky-tonk | Pno. | A0–C8 (21–108) | A0–C8 (21–108) | Estimate / Estimate | 0 | A0–C8 (21–108) | treble, bass | 4 Honky-tonk Piano | solo | no | [acoustic_pianos](cards/acoustic_pianos.md#piano_honky_tonk-honky-tonk-piano) |
| `electric_piano_rhodes` | Rhodes piano (Fender Rhodes) / Fender Rhodes (elektrické piano) / Fender Rhodes | Rhodes | A0–C8 (21–108) | E1–E7 (28–100) | Cited / Estimate | 0 | A0–C8 (21–108) | treble, bass | 5 Electric Piano 1 | solo | no | [electric_keys](cards/electric_keys.md#electric_piano_rhodes-rhodes-piano) |
| `electric_piano_wurlitzer` | Wurlitzer electronic piano / Wurlitzer (elektrické piano) / pianoforte elettrico Wurlitzer | Wurli. | A1–C7 (33–96) | A1–C7 (33–96) | Cited / Estimate | 0 | A1–C7 (33–96) | treble, bass | 5 Electric Piano 1 | solo | no | [electric_keys](cards/electric_keys.md#electric_piano_wurlitzer-wurlitzer-electronic-piano) |
| `clavinet` | Clavinet / Clavinet / Clavinet | Clav. | F1–E6 (29–88) | F1–E6 (29–88) | Cited / Estimate | 0 | F1–E6 (29–88) | treble, bass | 8 Clavi | solo | no | [electric_keys](cards/electric_keys.md#clavinet-clavinet) |
| `harpsichord` | Harpsichord / cembalo / clavicembalo | Hpschd. | F1–F6 (29–89) | C2–C6 (36–84) | Cited / Estimate | 0 | F1–F6 (29–89) | treble, bass | 7 Harpsichord | solo | no | [historical_keys_organs](cards/historical_keys_organs.md#harpsichord-harpsichord) |
| `clavichord` | Clavichord / klavichord / clavicordo | Clav. | F1–C7 (29–96) | C2–D6 (36–86) | Cited / Cited | 0 | F1–C7 (29–96) | treble, bass | – | solo | no | [historical_keys_organs](cards/historical_keys_organs.md#clavichord-clavichord) |
| `celesta` | Celesta / celesta / celesta | Cel. | C3–F8 (48–113) | C4–C8 (60–108) | Cited / Cited | 12 | C2–F7 (36–101) | treble, bass | 9 Celesta | solo | no | [historical_keys_organs](cards/historical_keys_organs.md#celesta-celesta) |
| `organ_pipe` | Pipe organ / varhany / organo a canne | Org. | C2–C7 (36–96) | C2–G6 (36–91) | Cited / Cited | 0 | C2–C7 (36–96) | treble, bass | 20 Church Organ | solo | no | [historical_keys_organs](cards/historical_keys_organs.md#organ_pipe-pipe-organ) |
| `organ_hammond` | Hammond organ / Hammondovy varhany / organo Hammond | Hammond Org. | C2–C7 (36–96) | C2–C7 (36–96) | Cited / Estimate | 0 | C2–C7 (36–96) | treble, bass | 17 Drawbar Organ | solo | no | [historical_keys_organs](cards/historical_keys_organs.md#organ_hammond-hammond-organ) |
| `accordion` | Accordion / akordeon / fisarmonica | Acc. | C2–D7 (36–98) | F3–A6 (53–93) | Cited / Cited | 0 | C2–D7 (36–98) | treble, bass | 22 Accordion | solo | no | [historical_keys_organs](cards/historical_keys_organs.md#accordion-accordion) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `piano_concert_grand` | sustain, legato, staccato, tenuto, marcato, trill, tremolo | staccatissimo, portato, accent, sforzando, glissando, let_ring, cluster |
| `piano_studio_grand` | sustain, legato, staccato, tenuto, marcato, trill, tremolo | staccatissimo, portato, accent, sforzando, glissando, let_ring, cluster |
| `piano_upright` | sustain, legato, staccato, tenuto, marcato, trill, tremolo | staccatissimo, portato, accent, sforzando, glissando, let_ring, cluster |
| `piano_felt` | sustain, legato, staccato, tenuto | portato, let_ring |
| `piano_prepared` | sustain, staccato, marcato, tremolo, harmonics, pizzicato | let_ring, glissando, damped, cluster, palm_mute |
| `piano_honky_tonk` | sustain, staccato, marcato, trill, tremolo | accent, glissando, let_ring |
| `electric_piano_rhodes` | sustain, legato, staccato, tenuto, marcato, trill | glissando |
| `electric_piano_wurlitzer` | sustain, legato, staccato, tenuto, marcato, trill | glissando |
| `clavinet` | sustain, legato, staccato, marcato, trill | portato, damped |
| `harpsichord` | sustain, legato, staccato, trill | – |
| `clavichord` | sustain, legato, staccato, trill | vibrato_on, portato |
| `celesta` | sustain, staccato, legato, trill, tremolo | – |
| `organ_pipe` | sustain, legato, staccato, tenuto, trill | glissando |
| `organ_hammond` | sustain, legato, staccato | glissando |
| `accordion` | sustain, legato, staccato, tremolo | crescendo_swell, air_tone |

## Harp and plucked strings

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `harp` | Harp / harfa / arpa | Hp. | B0–G7 (23–103) | C2–C7 (36–96) | Cited / Estimate | 0 | B0–G7 (23–103) | treble, bass | 47 Orchestral Harp | both | no | [plucked](cards/plucked.md#harp-harp) |
| `guitar_classical` | Classical guitar / klasická kytara / chitarra classica | Gtr. | E2–B5 (40–83) | E2–E5 (40–76) | Estimate / Estimate | -12 | E3–B6 (52–95) | treble | 25 Acoustic Guitar (nylon) | solo | no | [plucked](cards/plucked.md#guitar_classical-classical-guitar) |
| `lute` | Lute / loutna / liuto | Lt. | C2–G5 (36–79) | G2–D5 (43–74) | Estimate / Estimate | -12 | C3–G6 (48–91) | treble | – | solo | no | [plucked](cards/plucked.md#lute-lute) |
| `mandolin` | Mandolin / mandolína / mandolino | Mand. | G3–G6 (55–91) | G3–E6 (55–88) | Estimate / Estimate | 0 | G3–G6 (55–91) | treble | – | solo | no | [plucked](cards/plucked.md#mandolin-mandolin) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `harp` | sustain, harmonics, tremolo, trill | glissando, bisbigliando, pres_de_la_table, let_ring, damped, portamento, xylophonic |
| `guitar_classical` | sustain, legato, tremolo, trill, harmonics, sul_ponticello | harmonics_natural, harmonics_artificial, sul_tasto, rasgueado, strum, palm_mute, bartok_pizzicato, golpe, vibrato_on, let_ring, damped, glissando |
| `lute` | sustain, trill | let_ring |
| `mandolin` | sustain, staccato, tremolo | strum, let_ring |

## Voices

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `choir_soprano` | Sopranos (choir) / Soprány (sbor) / Soprani (coro) | S. | C4–C6 (60–84) | D4–A5 (62–81) | Cited / Cited | 0 | C4–C6 (60–84) | treble | 53 Choir Aahs | section | no | [voices](cards/voices.md#choir_soprano-sopranos-choir-section) |
| `choir_alto` | Altos (choir) / Alty (sbor) / Contralti (coro) | A. | F3–F5 (53–77) | G3–D5 (55–74) | Cited / Cited | 0 | F3–F5 (53–77) | treble | 53 Choir Aahs | section | no | [voices](cards/voices.md#choir_alto-altos-choir-section) |
| `choir_tenor` | Tenors (choir) / Tenory (sbor) / Tenori (coro) | T. | C3–A#4 (48–70) | D3–A4 (50–69) | Cited / Cited | -12 | C4–A#5 (60–82) | treble_8vb, bass | 53 Choir Aahs | section | no | [voices](cards/voices.md#choir_tenor-tenors-choir-section) |
| `choir_bass` | Basses (choir) / Basy (sbor) / Bassi (coro) | B. | C2–F#4 (36–66) | G2–D4 (43–62) | Cited / Cited | 0 | C2–F#4 (36–66) | bass | 53 Choir Aahs | section | no | [voices](cards/voices.md#choir_bass-basses-choir-section) |
| `voice_soprano` | Soprano / Soprán / Soprano | Sopr. | C4–E6 (60–88) | D4–B5 (62–83) | Cited / Cited | 0 | C4–E6 (60–88) | treble | 54 Voice Oohs | solo | no | [voices](cards/voices.md#voice_soprano-soprano-solo-voice) |
| `voice_mezzo_soprano` | Mezzo-soprano / Mezzosoprán / Mezzosoprano | M.-sopr. | A3–C6 (57–84) | B3–G5 (59–79) | Cited / Cited | 0 | A3–C6 (57–84) | treble | 54 Voice Oohs | solo | no | [voices](cards/voices.md#voice_mezzo_soprano-mezzo-soprano-solo-voice) |
| `voice_alto` | Contralto (alto) / Alt / Contralto | C.-alto | F3–G5 (53–79) | G3–F5 (55–77) | Cited / Cited | 0 | F3–G5 (53–79) | treble | 54 Voice Oohs | solo | no | [voices](cards/voices.md#voice_alto-contralto-solo-alto-voice) |
| `voice_tenor` | Tenor / Tenor / Tenore | Ten. | C3–D5 (48–74) | D3–C5 (50–72) | Cited / Cited | -12 | C4–D6 (60–86) | treble_8vb | 54 Voice Oohs | solo | no | [voices](cards/voices.md#voice_tenor-tenor-solo-voice) |
| `voice_baritone` | Baritone / Baryton / Baritono | Bar. | A2–A4 (45–69) | B2–G4 (47–67) | Cited / Cited | 0 | A2–A4 (45–69) | bass | 54 Voice Oohs | solo | no | [voices](cards/voices.md#voice_baritone-baritone-solo-voice) |
| `voice_bass` | Bass / Bas / Basso | B. | E2–F4 (40–65) | G2–E4 (43–64) | Cited / Cited | 0 | E2–F4 (40–65) | bass | 54 Voice Oohs | solo | no | [voices](cards/voices.md#voice_bass-bass-solo-voice) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `choir_soprano` | sustain, legato, staccato, tenuto, marcato | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento |
| `choir_alto` | sustain, legato, staccato, tenuto, marcato | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento |
| `choir_tenor` | sustain, legato, staccato, tenuto, marcato | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento, falsetto |
| `choir_bass` | sustain, legato, staccato, tenuto, marcato | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento, falsetto |
| `voice_soprano` | sustain, legato, staccato, tenuto, marcato, trill | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento |
| `voice_mezzo_soprano` | sustain, legato, staccato, tenuto, marcato, trill | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento |
| `voice_alto` | sustain, legato, staccato, tenuto, marcato, trill | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento |
| `voice_tenor` | sustain, legato, staccato, tenuto, marcato, trill | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento, falsetto |
| `voice_baritone` | sustain, legato, staccato, tenuto, marcato, trill | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento, falsetto |
| `voice_bass` | sustain, legato, staccato, tenuto, marcato, trill | vowel, text, humming, whisper, crescendo_swell, sforzando, vibrato_on, non_vibrato, portamento, falsetto |

## Bowed strings

| id | Name (en / cs / it) | Score | Sounding playable | Sounding comfortable | Range | Transp. | Written playable | Clefs | GM | Solo/section | In data.rs | Card |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `violin` | Violin / housle / violino | Vn. | G3–A7 (55–105) | G3–A6 (55–93) | Cited / Cited | 0 | G3–A7 (55–105) | treble | 41 Violin | both | yes | [strings](cards/strings.md#violin-violin) |
| `viola` | Viola / viola / viola | Va. | C3–A6 (48–93) | C3–A5 (48–81) | Cited / Cited | 0 | C3–A6 (48–93) | alto, treble | 42 Viola | both | no | [strings](cards/strings.md#viola-viola) |
| `cello` | Cello (violoncello) / violoncello / violoncello | Vc. | C2–E6 (36–88) | C2–E5 (36–76) | Cited / Cited | 0 | C2–E6 (36–88) | bass, tenor, treble | 43 Cello | both | yes | [strings](cards/strings.md#cello-cello) |
| `double_bass` | Double bass / kontrabas / contrabbasso | Cb. | B0–G4 (23–67) | E1–G3 (28–55) | Cited / Cited | -12 | B1–G5 (35–79) | bass, tenor, treble | 44 Contrabass | both | yes | [strings](cards/strings.md#double_bass-double-bass) |

| id | Articulations (existing vocabulary) | Proposed new articulation ids |
|---|---|---|
| `violin` | sustain, legato, staccato, spiccato, tenuto, marcato, pizzicato, tremolo, trill, col_legno, harmonics, sul_ponticello, con_sordino | detache, martele, sautille, ricochet, loure, portato, sul_tasto, flautando, bartok_pizzicato, left_hand_pizzicato, col_legno_battuto, col_legno_tratto, tremolo_measured, fingered_tremolo, harmonics_natural, harmonics_artificial, trill_half, trill_whole, portamento, glissando, fortepiano, crescendo_swell, non_vibrato, vibrato_on, su_una_corda, behind_the_bridge |
| `viola` | sustain, legato, staccato, spiccato, tenuto, marcato, pizzicato, tremolo, trill, col_legno, harmonics, sul_ponticello, con_sordino | detache, martele, sautille, ricochet, loure, portato, sul_tasto, flautando, bartok_pizzicato, left_hand_pizzicato, col_legno_battuto, col_legno_tratto, tremolo_measured, fingered_tremolo, harmonics_natural, harmonics_artificial, trill_half, trill_whole, portamento, glissando, fortepiano, crescendo_swell, non_vibrato, vibrato_on, su_una_corda, behind_the_bridge |
| `cello` | sustain, legato, staccato, spiccato, tenuto, marcato, pizzicato, tremolo, trill, col_legno, harmonics, sul_ponticello, con_sordino | detache, martele, sautille, ricochet, loure, portato, sul_tasto, flautando, bartok_pizzicato, left_hand_pizzicato, col_legno_battuto, col_legno_tratto, tremolo_measured, fingered_tremolo, harmonics_natural, harmonics_artificial, trill_half, trill_whole, portamento, glissando, fortepiano, crescendo_swell, non_vibrato, vibrato_on, su_una_corda, behind_the_bridge |
| `double_bass` | sustain, legato, staccato, spiccato, tenuto, marcato, pizzicato, tremolo, trill, col_legno, harmonics, sul_ponticello, con_sordino | detache, martele, loure, portato, sul_tasto, flautando, bartok_pizzicato, left_hand_pizzicato, slap_pizzicato, col_legno_battuto, col_legno_tratto, tremolo_measured, fingered_tremolo, harmonics_natural, harmonics_artificial, trill_half, trill_whole, portamento, glissando, fortepiano, crescendo_swell, non_vibrato, vibrato_on |

## Keyboard mechanics

| id | Action | Keys | Pedals (MIDI CC) | Sympathetic resonance | Release and mechanical noise | Decay |
|---|---|---|---|---|---|---|
| `carillon` | mechanical baton clavier: wooden batons played with the closed fist, connected by wires to the clappers; pedalboard of 1-2 octaves for the bass bells | 49 | – | all bells hang free and undamped; struck bells and their minor-third (tierce) partials sustain together | audible clapper and transmission noise from the clavier; no damper release | each bell rings until its vibration dies out naturally; large bells ring for a long time |
| `piano_concert_grand` | grand action: horizontal strings, hammers strike upward and fall back by gravity; double repetition (double escapement) lets a key re-strike after rising about a third of its travel | 88 | sustain (64), una_corda (67), sostenuto (66) | with the dampers raised (CC64) every string can resonate, most strongly at harmonically related pitches; the undamped top octave and a half and the duplex/aliquot string lengths resonate even without the pedal | damper return on key release, key and action noise, pedal down/up thumps, hammer knock audible in the treble from about A4 | two-stage: a loud prompt sound decaying at about 8 dB/s in one measured example, then an aftersound decaying at under a quarter of that rate; long in the bass, short in the top octave |
| `piano_studio_grand` | grand action with double repetition, as on the concert grand | 88 | sustain (64), una_corda (67), sostenuto (66) | as on the concert grand; shorter bass strings give a weaker low resonance | damper return, key noise and pedal thumps; Salamander samples hammer-release noise, string-resonance releases and pedal down/up noise separately | two-stage, shorter overall sustain in the bass than a concert grand |
| `piano_upright` | upright action: vertical strings, hammers move horizontally and return by springs; single escapement, so the key must rise almost fully before a repeat | 88 | sustain (64), soft (67), practice_muffler | present with the dampers raised; smaller soundboard and shorter strings than a grand | damper return, key and action noise, pedal noise (Logic's Vintage Upright exposes Key Noise, Pedal Noise and Release Samples) | two-stage, shorter sustain than a grand, especially in the bass |
| `piano_felt` | normally an upright action (sometimes a grand) with a felt strip lowered between hammers and strings: the practice/muffler rail, moderator or a fitted felt | 88 | sustain (64), soft (67), practice_muffler | present with the dampers raised, as on the underlying upright; NI Una Corda exposes a resonance control | hammer, key and pedal mechanics are prominent because the string tone is muted and the microphones are close; some libraries record a separate hammers signal | softer attack and shorter, darker sustain than the open piano |
| `piano_prepared` | a normal grand (or upright) action; objects (screws, bolts, rubber, plastic, paper) sit between or on the strings | 88 | sustain (64), una_corda (67), sostenuto (66) | partly suppressed on prepared strings; unprepared strings resonate normally | rattles of loose objects can continue after the key; the dampers still stop the strings | per key: from a short drum-like thud to metallic ringing; often shorter than the open string |
| `piano_honky_tonk` | usually an old upright action (single escapement, spring return) | 88 | sustain (64), soft (67) | as an upright | worn actions: louder key and damper noise | the detuned unison strings beat instead of locking, so the aftersound wavers |
| `electric_piano_rhodes` | Velocity-sensitive hammer action that works like a grand piano's, hammers striking tines (S3, S5); escapement ideally 1/32 inch (S1 Ch. 4). 88-key models span A0-C8 (21-108); the common Stage/Suitcase 73 spans E1-E7 (28- | 88 | sustain (64) | Estimate: weak; each note is a separate tine and tone bar with no shared soundboard, and no source read describes sympathetic ringing | Damper felt landing on the tine on key release, a small 'thwong' (S12), modelled as Damper Noise (S3, S8); hammer, tine and pickup noise are modelled separately in S8 | Long, sustained by the tone bar (S1 Ch. 1); longer than the Wurlitzer's (S8, S13); full-length jRhodes3d samples last up to 25 s (S20); the Mk II has less treble sustain (S3) |
| `electric_piano_wurlitzer` | Wurlitzer-made action with whips, butts, flys (jacks) and dampers that 'Regulates Like a Conventional Piano Action'; hammer let-off 1/8 inch from the reed (S6); velocity-sensitive (S5) | 64 | sustain (64) | Estimate: weak; each note is a separate reed and the pickup is electrostatic, no soundboard; no source read describes it | Key release noise and an audible sustain-pedal mechanism ('On a Wurlitzer you can hear the mechanical action of the sustain pedal'), modelled as Note Off Noise and Sustain Pedal Noise (S7); hammer noise (S7) | Shorter than the Rhodes (S8 'Longer sustain than the Wurlitzer'; S13); the sustain pedal lifts the dampers 'as in a conventional piano' (S6) |
| `clavinet` | Single-arm key lever; a plunger (rubber hammer pad) under the key presses the string onto an anvil and stays there while the key is held (S10, S3); no escapement (Estimate: derived from the plunger holding the string on  | 60 | foot_swell (11) | Estimate: little; every string rests in its wool winding when its key is up (S10), so undamped strings do not ring along | Key-off click when strings stick to worn rubber hammers, a 'second trigger when the key is released' (S3); key release noise, hammer noise and pickup noise are modelled (S9); string buzzes and key clicks (S3) | Sounds while the key is held and is muted immediately on release by the wool winding (S10, S3); the mute slider gives a quick decay (S9); no sustain pedal on the hardware (S10 lists only the foot swell) |
| `harpsichord` | plucked: the key lifts one jack per engaged choir; a quill or plastic plectrum plucks the string; on release the plectrum's hinged tongue passes the string and a felt damper on the jack stops it [Cited S5] | 61 | – | undamped strings of disengaged choirs can ring sympathetically [Estimate: not stated by an opened source] | audible release (jack falling, damper touching the string): VCSL, SSO and VSL record separate release samples [Cited S29, S31, S32] | free decay of a plucked string while the key is held; no damper pedal [Estimate: S5 describes dampers on the jacks and no pedal for them] |
| `clavichord` | tangent: a brass blade on the key end strikes the string pair and stays on it while the key is held, acting as the nut; cloth damps the string on release [Cited S6] | 61 | – | the string part beyond the tangent is damped by cloth; unfretted instruments carry many strings on one bridge [Cited S6]; audible sympathetic ringing is not documented in an opened source [Estimate] | tangent contact and release are part of the sound: the tangent both starts the sound and fixes the pitch [Cited S6]; no release-noise sample was examined [Estimate] | short, quiet decay while the key is held; ends when the tangent leaves the string [Cited S6 for the mechanism; Estimate for the length] |
| `celesta` | simplified piano action: felt hammers strike steel plates from above; a tuned wooden resonator sits under each plate [Cited S2, S23] | 61 | sustain (64) | each plate drives its own resonator [Cited S2, S23]; ringing of undamped plates with the pedal down is not described by an opened source [Estimate] | dampers stop the plates on key release unless the pedal lifts them [Cited S23]; action noise not documented [Estimate] | bell-like decay; the instrument lacks the orchestral sostenuto, so melody notes must not be too long; the pedal materially enhances sustain [Cited S2] |
| `organ_pipe` | mechanical (tracker), pneumatic, electro-pneumatic or electric key action; Barker lever on large French organs [Cited S8, S4] | 61 | expression (11), crescendo | the building: Widor asks for time to let the sound travel to and fro under cathedral roofs [Cited S4] | wind and blower noise (VCSL ships a separate blower recording); key and stop action noise on mechanical organs [Cited S31 for the blower; Estimate for action noise] | no decay while a key is held: continuous wind sustains the pipe; after release the room's reverberation [Cited S8, S3; Estimate for the room] |
| `organ_hammond` | light waterfall-style keys; each key closes nine busbar contacts, one per drawbar; volume does not depend on key force [Cited S12] | 61 | expression (11) | tonewheel crosstalk (leakage): pickups also catch wheels that are not selected; now part of the signature sound and emulated by clones [Cited S12, S14, S27] | key click at note-on and note-off from the key contacts, longer when contacts corrode; motor start and run switches [Cited S12, S27] | none: tones sustain until release; only the harmonic percussion decays, fast or slow [Cited S12, S27] |
| `accordion` | piano-style keys (or chromatic buttons) open pallets over free metal reeds in reed chambers; the touch is not expressive [Cited S15, S16] | 41 | – | two or three 8' ranks tuned up to about 25 cents apart beat against each other (violin and musette registers) [Cited S17] | bellows and air noise; the air button moves air without sounding reeds [Cited S15]; key and pallet clatter is not documented [Estimate] | none while the bellows move: the reed sounds as long as air passes it [Cited S15 for the bellows-driven reed; Estimate for the release behaviour] |

## Proposed corrections to data.rs

The 16 instruments of `crates/sonara-midi/src/orchestration/data.rs` against the researched values. A proposal: data.rs is unchanged until the design is accepted.

| id | Transp. today → researched | Playable today → researched | Comfortable today → researched |
|---|---|---|---|
| `violin` | 0 → 0 | G3–C8 (55–108) Cited → G3–A7 (55–105) Cited **changed** | G3–E7 (55–100) Cited → G3–A6 (55–93) Cited **changed** |
| `cello` | 0 → 0 | C2–A6 (36–93) Estimate → C2–E6 (36–88) Cited **changed** | none → C2–E5 (36–76) Cited **changed** |
| `double_bass` | -12 → -12 | C1–G4 (24–67) Estimate → B0–G4 (23–67) Cited **changed** | E1–G3 (28–55) Cited → E1–G3 (28–55) Cited |
| `flute` | 0 → 0 | B3–C7 (59–96) Cited → B3–D7 (59–98) Cited **changed** | none → C4–A#6 (60–94) Cited **changed** |
| `piccolo` | 12 → 12 | D5–C8 (74–108) Cited → D5–C8 (74–108) Cited | none → G#5–A#7 (80–106) Cited **changed** |
| `oboe` | 0 → 0 | A#3–G6 (58–91) Cited → A#3–G6 (58–91) Cited | none → C4–D#6 (60–87) Cited **changed** |
| `english_horn` | -7 → -7 | E3–C6 (52–84) Cited → E3–C6 (52–84) Cited | none → E3–A5 (52–81) Cited **changed** |
| `clarinet_bb` | -2 → -2 | D3–A#6 (50–94) Cited → D3–A#6 (50–94) Cited | none → D3–G6 (50–91) Cited **changed** |
| `bassoon` | 0 → 0 | A#1–G#5 (34–80) Cited → A#1–G#5 (34–80) Cited | A#1–E5 (34–76) Cited → A#1–C5 (34–72) Cited **changed** |
| `contrabassoon` | -12 → -12 | A#0–D4 (22–62) Cited → A#0–D4 (22–62) Cited | none → A#0–A3 (22–57) Cited **changed** |
| `horn_f` | -7 → -7 | B1–F5 (35–77) Estimate → B1–F5 (35–77) Cited | none → F2–C5 (41–72) Estimate **changed** |
| `trumpet_bb` | -2 → -2 | E3–A#5 (52–82) Cited → E3–G6 (52–91) Cited **changed** | none → E3–A#5 (52–82) Cited **changed** |
| `trombone_tenor` | 0 → 0 | E2–F5 (40–77) Cited → E2–F5 (40–77) Cited | E2–D5 (40–74) Cited → E2–D5 (40–74) Cited |
| `glockenspiel` | 24 → 24 | G5–C8 (79–108) Estimate → C5–C8 (72–108) Cited **changed** | none → G5–C8 (79–108) Cited **changed** |
| `xylophone` | 12 → 12 | F4–C8 (65–108) Estimate → C4–C8 (60–108) Cited **changed** | none → F4–C8 (65–108) Cited **changed** |
| `snare_drum` | unpitched | unpitched | – |
