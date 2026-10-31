# Orchestral parts

Sonara knows the orchestral instruments below by id. For an orchestral part:

- One instrument track per part: `create_track` with `"instrument": "sonara.sampler"` and a
  `part` naming the id (`{"instrument": "horn_f", "name": "Horn 1"}`), then `set_instrument`
  with `"plugin_id": "sonara.sampler"` and `"params": {"instrument": "<preset id>"}`.
  `list_plugins {"plugin_id": "sonara.sampler"}` lists the installed presets; without a sampler
  or a preset, use a built-in instrument and say so.
- Keep each part in its range, mostly in the comfortable one.
- A note's `articulation` plays the preset's recording of it, or the nearest one it has; do not
  write switch keys. A preset with one recording plays it for every note: for another
  articulation, use another track with a preset that plays it.
- Velocity sets a note's sample layer and level. Swells and fades on held notes: controller 11
  (expression) with `write_midi_controllers`, which changes the level, not the timbre.
  Controller 64 is the sustain pedal.
- No recorded legato: overlapping notes sound as separate attacks.

## Instruments

The 85 instrument ids in score order: `id | English / Czech name | range | articulations`. Pitches are MIDI numbers at sounding pitch (C4 = 60); write notes at sounding pitch. Range: playable, [comfortable], (transposition: sounding minus written in semitones, for notation only). A note's `articulation` takes an id of its instrument's list, or none for the ordinary sound; `as viola` is the viola's list. Mute and vibrato ids hold over a passage and a note refuses them: con_sordino non_vibrato palm_mute stopped vibrato_on xylophonic mute_bucket mute_cup mute_harmon mute_plunger.

piccolo | Piccolo / pikola | 74-108 [80-106] (+12) | sustain legato staccato tenuto marcato trill flutter_tongue staccatissimo multi_tongue sforzando fortepiano crescendo_swell rip fall key_click air_tone
flute | Flute / flétna | 59-98 [60-94] | sustain legato staccato tenuto marcato trill flutter_tongue harmonics staccatissimo portato multi_tongue sforzando crescendo_swell rip fall glissando key_click slap_tongue air_tone multiphonic
alto_flute | Alto flute / altová flétna | 55-91 [55-81] (-5) | as piccolo without fortepiano plus portato
bass_flute | Bass flute / basová flétna | 47-84 [48-81] (-12) | as piccolo without fortepiano key_click air_tone plus portato
oboe | Oboe / hoboj | 58-91 [60-87] | sustain legato staccato tenuto marcato trill flutter_tongue portato sforzando fortepiano crescendo_swell trill_half trill_whole glissando multiphonic key_click slap_tongue
oboe_damore | Oboe d'amore / hoboj d'amore | 56-88 [56-85] (-3) | sustain legato staccato tenuto marcato trill flutter_tongue portato trill_half trill_whole
english_horn | English horn / anglický roh | 52-84 [52-81] (-7) | as oboe without multiphonic key_click slap_tongue plus bisbigliando
clarinet_eb | E♭ clarinet / Es klarinet | 55-96 [63-94] (+3) | sustain legato staccato trill flutter_tongue staccatissimo portato sforzando trill_half trill_whole crescendo_swell
clarinet_bb | Clarinet in B♭ / klarinet in B | 50-94 [50-91] (-2) | sustain legato staccato tenuto marcato trill flutter_tongue staccatissimo portato sforzando trill_half trill_whole multi_tongue glissando growl crescendo_swell
clarinet_a | Clarinet in A / klarinet in A | 49-93 [49-90] (-3) | as clarinet_bb
bass_clarinet | Bass clarinet / basklarinet | 34-86 [37-74] (-14) | as clarinet_bb without tenuto glissando growl crescendo_swell
saxophone_soprano | Soprano saxophone / sopránový saxofon | 56-94 [56-87] (-2) | sustain legato staccato tenuto marcato trill flutter_tongue harmonics staccatissimo fortepiano crescendo_swell trill_half trill_whole glissando scoop fall doit growl slap_tongue multiphonic key_click
saxophone_alto | Alto saxophone / altový saxofon | 49-87 [49-80] (-9) | as saxophone_soprano plus rip
saxophone_tenor | Tenor saxophone / tenorový saxofon | 44-82 [44-75] (-14) | as saxophone_soprano plus rip
saxophone_baritone | Baritone saxophone / barytonový saxofon | 36-75 [37-68] (-21) | as saxophone_soprano plus rip
bassoon | Bassoon / fagot | 34-80 [34-72] | as oboe without fortepiano
contrabassoon | Contrabassoon / kontrafagot | 22-62 [22-57] (-12) | sustain legato staccato tenuto marcato flutter_tongue sforzando fortepiano
horn_f | Horn in F / lesní roh | 35-77 [41-72] (-7) | sustain legato staccato tenuto marcato trill flutter_tongue cuivre bells_up glissando rip fall staccatissimo sforzando fortepiano crescendo_swell trill_whole trill_half multi_tongue multiphonic
wagner_tuba_tenor | Tenor Wagner tuba / tenorová Wagnerova tuba | 27-82 [40-74] (-2) | sustain legato staccato tenuto marcato trill flutter_tongue portato sforzando fortepiano glissando multi_tongue
wagner_tuba_bass | Bass Wagner tuba / basová Wagnerova tuba | 27-77 [34-67] (-7) | as wagner_tuba_tenor
trumpet_piccolo | Piccolo trumpet / piccolo trubka | 59-91 [62-86] (+10) | as wagner_tuba_tenor without glissando
trumpet_c | Trumpet in C / trubka in C | 54-89 [54-84] | as wagner_tuba_tenor plus accent crescendo_swell half_valve cuivre
trumpet_bb | Trumpet in B♭ / trubka in B | 52-91 [52-82] (-2) | sustain legato staccato tenuto marcato trill flutter_tongue accent sforzando fortepiano crescendo_swell portato glissando portamento half_valve fall doit shake scoop rip growl cuivre multi_tongue wah_wah
cornet | Cornet in B♭ / kornet | 52-91 [52-82] (-2) | sustain legato staccato tenuto marcato trill flutter_tongue accent portato multi_tongue glissando half_valve fall shake growl wah_wah
flugelhorn | Flugelhorn / křídlovka | 52-84 [52-82] (-2) | as cornet without accent growl wah_wah
trombone_tenor | Tenor trombone / tenorový pozoun | 40-77 [40-74] | sustain legato staccato tenuto marcato trill flutter_tongue glissando staccatissimo sforzando fortepiano crescendo_swell rip fall doit cuivre growl
trombone_bass | Bass trombone / basový pozoun | 22-77 [31-67] | as trombone_tenor without doit growl
euphonium | Euphonium / eufonium | 23-74 [40-67] | sustain legato staccato tenuto marcato trill flutter_tongue staccatissimo sforzando crescendo_swell half_valve
tuba | Tuba | 21-67 [29-60] | as euphonium plus fortepiano rip fall
cimbasso | Cimbasso | 29-65 [30-60] | as euphonium without trill plus cuivre rip fall
timpani | Timpani / tympány | 36-61 [38-57] | roll marcato let_ring damped glissando crescendo_swell tremolo_measured fortepiano
bass_drum | Bass drum / velký buben | unpitched | roll marcato damped let_ring
snare_drum | Snare drum / malý buben | unpitched | roll marcato flam drag rimshot rim snares_off
clash_cymbals | Clash cymbals / činely | unpitched | as bass_drum plus choke
suspended_cymbal | Suspended cymbal / zavěšený činel | unpitched | as bass_drum plus crescendo_swell choke bowed scrape
tam_tam | Tam-tam | unpitched | as bass_drum plus crescendo_swell scrape bowed
triangle | Triangle / triangl | unpitched | as bass_drum
tambourine | Tambourine / tamburína | unpitched | roll marcato shake_stroke
castanets | Castanets / kastaněty | unpitched | roll marcato
wood_block | Wood block / dřevěný blok | unpitched | marcato roll
temple_blocks | Temple blocks / temple bloky | unpitched | marcato roll glissando
sleigh_bells | Sleigh bells / rolničky | unpitched | as tambourine
maracas | Maracas / rumba koule | unpitched | as tambourine
shaker | Shaker / šejkr | unpitched | as tambourine
whip | Whip / bič | unpitched | marcato
glockenspiel | Glockenspiel / zvonkohra | 72-108 [79-108] (+24) | staccato marcato tremolo trill let_ring damped glissando bowed
xylophone | Xylophone / xylofon | 60-108 [65-108] (+12) | marcato tremolo glissando dead_stroke bowed
vibraphone | Vibraphone / vibrafon | 48-89 [53-89] | as xylophone plus staccato let_ring damped portamento
marimba | Marimba | 36-96 [45-96] | as xylophone plus trill
tubular_bells | Tubular bells / trubicové zvony | 51-79 [60-77] | marcato tremolo let_ring damped glissando
crotales | Crotales / krotaly | 84-108 (+24) | marcato tremolo let_ring damped bowed
carillon | Carillon / zvonohra | 48-96 | tremolo marcato let_ring
harp | Harp / harfa | 23-103 [36-96] | harmonics tremolo trill glissando bisbigliando pres_de_la_table let_ring damped portamento
guitar_classical | Classical guitar / klasická kytara | 40-83 [40-76] (-12) | legato tremolo trill harmonics sul_ponticello harmonics_natural harmonics_artificial sul_tasto bartok_pizzicato let_ring damped glissando
lute | Lute / loutna | 36-79 [43-74] (-12) | trill let_ring
mandolin | Mandolin / mandolína | 55-91 [55-88] | staccato tremolo let_ring
celesta | Celesta | 48-113 [60-108] (+12) | staccato legato trill tremolo
piano_concert_grand | Concert grand piano / koncertní křídlo | 21-108 | legato staccato tenuto marcato trill tremolo staccatissimo portato accent sforzando glissando let_ring
piano_studio_grand | Studio grand piano / křídlo | 21-108 | as piano_concert_grand
piano_upright | Upright piano / pianino | 21-108 | as piano_concert_grand
piano_felt | Felt piano / pianino s moderátorem | 21-108 | legato staccato tenuto portato let_ring
piano_prepared | Prepared piano / preparovaný klavír | 21-108 | staccato marcato tremolo harmonics pizzicato let_ring glissando damped
piano_honky_tonk | Honky-tonk piano | 21-108 | staccato marcato trill tremolo accent glissando let_ring
harpsichord | Harpsichord / cembalo | 29-89 [36-84] | legato staccato trill
clavichord | Clavichord / klavichord | 29-96 [36-86] | as harpsichord plus sustain portato
electric_piano_rhodes | Rhodes piano / Fender Rhodes | 21-108 [28-100] | as harpsichord plus sustain tenuto marcato glissando
electric_piano_wurlitzer | Wurlitzer electronic piano / Wurlitzer | 33-96 | as harpsichord plus sustain tenuto marcato glissando
clavinet | Clavinet | 29-88 | as harpsichord plus sustain marcato portato damped
organ_pipe | Pipe organ / varhany | 36-96 [36-91] | as harpsichord plus sustain tenuto glissando
organ_hammond | Hammond organ / Hammondovy varhany | 36-96 | sustain legato staccato glissando
accordion | Accordion / akordeon | 36-98 [53-93] | sustain legato staccato tremolo crescendo_swell air_tone
voice_soprano | Soprano / Soprán | 60-88 [62-83] | sustain legato staccato tenuto marcato trill vowel text humming whisper crescendo_swell sforzando portamento
voice_mezzo_soprano | Mezzo-soprano / Mezzosoprán | 57-84 [59-79] | as voice_soprano
voice_alto | Contralto / Alt | 53-79 [55-77] | as voice_soprano
voice_tenor | Tenor | 48-74 [50-72] | as voice_soprano plus falsetto
voice_baritone | Baritone / Baryton | 45-69 [47-67] | as voice_soprano plus falsetto
voice_bass | Bass / Bas | 40-65 [43-64] | as voice_soprano plus falsetto
choir_soprano | Sopranos / Soprány | 60-84 [62-81] | as voice_soprano without trill
choir_alto | Altos / Alty | 53-77 [55-74] | as voice_soprano without trill
choir_tenor | Tenors / Tenory | 48-70 [50-69] | as voice_soprano without trill plus falsetto
choir_bass | Basses / Basy | 36-66 [43-62] | as voice_soprano without trill plus falsetto
violin | Violin / housle | 55-105 [55-93] | sustain legato staccato spiccato tenuto marcato pizzicato tremolo trill col_legno harmonics sul_ponticello portato sul_tasto flautando bartok_pizzicato col_legno_battuto col_legno_tratto tremolo_measured harmonics_natural harmonics_artificial trill_half trill_whole portamento glissando fortepiano crescendo_swell su_una_corda
viola | Viola | 48-93 [48-81] | as violin
cello | Cello / violoncello | 36-88 [36-76] | as violin
double_bass | Double bass / kontrabas | 23-67 [28-55] (-12) | as violin without su_una_corda

## Presets of the library profiles Sonara ships

Presets of the libraries below by the instrument id they play; only those that `list_plugins` offers as `instrument` choices are installed. Preset id: library id, `/`, the `folder` line above (if any), name. After the name: the articulations it plays (none: the ordinary sound), `+` the mute or vibrato recorded. `keys 36 sustain, 37 staccato`: a note on key 36 selects sustain for the following notes and does not sound. On percussion lines a key alone plays the ordinary stroke.

vsco2ce (VSCO 2 Community Edition):
piccolo: PiccoloSus sustain; PiccoloStac staccato
flute: Flute-KS keys 36 sustain+non_vibrato, 37 sustain, 38 sustain+vibrato_on, 39 staccato; FluteSusNV sustain+non_vibrato; FluteSusVib sustain; FluteExpVib sustain+vibrato_on; FluteStac staccato
oboe: OboeSusVib sustain; OboeSusNV sustain+non_vibrato; OboeStac staccato
clarinet_bb: Clarinet-KS keys 36 sustain, 37 staccato; ClarinetSus sustain; ClarinetStac staccato
bassoon: BassoonSus sustain; BassoonVib sustain+vibrato_on; BassoonStac staccato
horn_f: FHornSus sustain; FHornStac staccato; FHornMute sustain+con_sordino
trumpet_bb: TrumpetSus sustain; TrumpetSusVib sustain+vibrato_on; TrumpetStac staccato; TrumpetStraightMuteSus sustain+con_sordino; TrumpetHarmonMuteSus sustain+mute_harmon
trombone_tenor: TromboneSus sustain; TromboneVib sustain+vibrato_on; TromboneStac staccato
tuba: Tuba-KS keys 84 sustain, 85 staccato; TubaSus sustain; TubaStac staccato
timpani: Timpani; TimpaniRolls roll
glockenspiel: Glockenspiel
xylophone: Xylophone
marimba: Marimba
tubular_bells: TubularBells
harp: Harp
piano_upright: UprightPiano; VSUpright1
organ_pipe: OrganLoud; OrganQuiet; OrganLoudPedal; OrganQuietPedal
violin: SViolin-KS keys 36 sustain, 37 tremolo, 38 spiccato, 39 pizzicato; SViolinVib sustain; SViolinVib-Quiet sustain; SViolinTrem tremolo; SViolinSpic spiccato; SViolinPizz pizzicato; ViolinEns-KS keys 36 sustain, 37 tremolo, 38 spiccato, 39 pizzicato; ViolinEnsSusVib sustain; ViolinEnsSusVib-Quiet sustain; ViolinEnsTrem tremolo; ViolinEnsSpic spiccato; ViolinEnsPizz pizzicato
viola: ViolaEns-KS keys 36 sustain, 37 tremolo, 38 spiccato, 39 pizzicato; ViolaEnsSusVib sustain; ViolaEnsSusVib-Quiet sustain; ViolaEnsTrem tremolo; ViolaEnsSpic spiccato; ViolaEnsPizz pizzicato
cello: CelloEns-KS keys 84 sustain, 85 tremolo, 86 spiccato, 87 pizzicato; CelloEnsSusVib sustain; CelloEnsSusVib-Quiet sustain; CelloEnsTrem tremolo; CelloEnsSpic spiccato; CelloEnsPizz pizzicato
double_bass: Contrabass-KS keys 84 sustain+non_vibrato, 85 sustain, 86 tremolo, 87 spiccato, 88 pizzicato; ContrabassSusNV sustain+non_vibrato; ContrabassSusVB sustain; ContrabassSusVB-Quiet sustain; ContrabassTrem tremolo; ContrabassSpic spiccato; ContrabassPizz pizzicato
kit: GM-StylePerc keys 36 bass_drum, 38 snare_drum, 39 snare_drum roll, 40 snare_drum snares_off, 42 tam_tam scrape, 46 tam_tam, 47/48/50 suspended_cymbal crescendo_swell, 49 clash_cymbals, 51/59 suspended_cymbal, 53 tambourine shake_stroke, 54 tambourine, 55 tambourine roll, 79/81 triangle, 82 sleigh_bells, 101/102/103/104 suspended_cymbal bowed

vcsl (Versilian Community Sample Library):
folder Aerophones/Reed Aerophones/
saxophone_soprano: Saxello - Keyswitch keys 48 sustain, 49 sustain+non_vibrato, 50 staccato; Saxello - Non-Vibrato sustain+non_vibrato; Saxello - Staccato staccato; Saxello - Vibrato sustain
saxophone_tenor: Tenor Saxophone - Keyswitch keys 36 sustain, 37 sustain+non_vibrato, 38 staccato; Tenor Saxophone - Non-Vibrato sustain+non_vibrato; Tenor Saxophone - Staccato staccato; Tenor Saxophone - Vibrato sustain
folder Membranophones/Struck Membranophones/
timpani: Timpani 1 - Hit; Timpani 1 - Keyswitch keys 36 ordinario, 37 roll; Timpani 1 - Roll roll; Timpani 2 - Keyswitch; Timpani 2 - Scale
bass_drum: Bass Drum 1 keys 60; Bass Drum 2 keys 62, 63/64 roll, 60/61 crescendo_swell; Bass Drum 3 - Legacy keys 60, 62/63/64/65/66 roll, 61 dead_stroke
snare_drum: Legacy Snares - Keyswitch keys 60, 65 snares_off, 66 rim, 67 rimshot; Legacy Snares - OldSnare keys 60, 61 rim; Legacy Snares - drum1 keys 60, 65 snares_off, 66 rim, 67 rimshot; Legacy Snares - drum2 keys 61, 63 snares_off, 65 rimshot; Legacy Snares - drum3_marching keys 60, 62 rimshot; Snare Drum, Modern 1 keys 61, 63 roll, 60 snares_off; Snare Drum, Modern 2 keys 61, 64 roll, 60 snares_off, 62 rim; Snare Drum, Modern 3 keys 60, 63 roll, 61 rim, 62 rimshot; Snare Drum, Rope Tension keys 62/64, 63 snares_off, 60 rim
folder Idiophones/Struck Idiophones/
clash_cymbals: Clash Cymbals 1 keys 60, 61/62 choke; Clash Cymbals 2 keys 60
suspended_cymbal: Suspended Cymbal 1 keys 68, 71 roll, 64/65/66/67 crescendo_swell, 60/61/62/63 bowed, 72 scrape; Suspended Cymbal 2 keys 66, 69 roll, 63/64/65 crescendo_swell, 60/61/62 bowed, 70 scrape
tam_tam: Gong 1 keys 60/61, 62 scrape; Gong 2 keys 60/61/62/63/64, 65 scrape
triangle: Triangles keys 60/65/69, 63/68/72 roll
tambourine: Tambourine 1 keys 60, 61 roll, 62 shake_stroke; Tambourine 2 keys 60, 61 roll, 62 shake_stroke; Tambourine 3 - Legacy keys 60/61; Tambourine 4 - Legacy keys 60, 61/62 roll; Tambourine 5 - Legacy keys 60, 61 roll
wood_block: Woodblock keys 60/61/62
sleigh_bells: Sleigh Bells keys 60/62/63, 61/64 shake_stroke
shaker: Shaker - Legacy keys 60/61; Shaker, Large keys 61/62; Shaker, Small keys 62/63/64/65/66/67, 60 roll
whip: Slapstick keys 60/61
glockenspiel: Glockenspiel
xylophone: Xylophone - Hard Mallets; Xylophone - Keyswitch; Xylophone - Medium Mallets; Xylophone - Soft Mallets
vibraphone: Vibraphone - Bowed bowed; Vibraphone - Hard Mallets; Vibraphone - Keyswitch keys 36 bowed, 38 ordinario; Vibraphone - Soft Mallets
marimba: Marimba
tubular_bells: Tubular Bells 1; Tubular Bells 2; Tubular Bells 3 - Legacy
folder Chordophones/Composite Chordophones/
harp: Concert Harp
folder Chordophones/Zithers/
piano_studio_grand: Grand Piano, Kawai - Legacy; Grand Piano, Kawai; Grand Piano, Steinway B
piano_upright: Upright Piano, Knight; Upright Piano, Yamaha
harpsichord: Harpsichord, English - Keyswitch; Harpsichord, English - Lute; Harpsichord, English - Normal; Harpsichord, Flemish - 4'; Harpsichord, Flemish - 8'; Harpsichord, Flemish - Full; Harpsichord, Flemish - Keyswitch; Harpsichord, French; Harpsichord, Italian; Harpsichord, Unk
folder Aerophones/Edge-blown Aerophones/
organ_pipe: Pipe Organ - Keyswitch; Pipe Organ - Loud Pedal; Pipe Organ - Loud; Pipe Organ - Quiet Pedal; Pipe Organ - Quiet; Renaissance Organ - 4'+8'; Renaissance Organ - 4'; Renaissance Organ - 8'; Renaissance Organ - Full; Renaissance Organ - Keyswitch
