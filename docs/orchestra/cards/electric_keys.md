# Electric and electro-mechanical keyboards: instrument cards

_Researched 2026-10-30. MIDI note numbers, C4 = 60 = middle C. Ranges are sounding unless marked written.
Transposition = sounding minus written, in semitones._

## Sources

| # | Source | URL | Kind (handbook, manufacturer, library maker, spec, encyclopedia, other) | Opened in this session |
|---|---|---|---|---|
| S1 | Rhodes Service Manual (HTML transcription on fenderrhodes.com): table of contents, Ch. 1 "The Rhodes Tone Source", Ch. 4 "Dimensional Standards and Adjustments", Ch. 6 "Repair Procedures and Techniques" (key numbering) | http://www.fenderrhodes.com/org/manual/toc.html (ch1.html, ch4.html, ch6.html) | manufacturer | Yes: TOC, Ch. 1, Ch. 4, Ch. 6. Ch. 7-11 were not opened |
| S2 | fenderrhodes.com, "Fender Rhodes Mark I" model page | https://fenderrhodes.com/pianos/mark1a.html | other (reference site) | Yes |
| S3 | Apple, *Logic Pro Instruments for Mac* (PDF of the Logic Pro 12.3 guide, 752 pages): Vintage Clav pp. 679-695, Vintage Electric Piano pp. 696-706, Legacy Electric Clav(inet) and Electric Piano pp. 711-712. Text extracted locally with PDFKit | https://help.apple.com/pdf/logicpromac-instruments/en_US/logic-pro-mac-instruments-user-guide.pdf | library maker | Yes: the pages named |
| S4 | Apple, "Vintage Electric Piano overview in Logic Pro for Mac" (HTML topic, Logic Pro 12.3) | https://support.apple.com/guide/logicpro/vintage-electric-piano-overview-lgsifc1ba323/mac | library maker | Yes (downloaded with curl; WebFetch returned 404) |
| S5 | Apple, Logic Pro 9.1.6 help, "Electric Piano Models Emulated by the EVP88" | https://help.apple.com/logicpro/mac/9.1.6/en/logicpro/instruments/chapter_9_section_9.html | library maker | Yes |
| S6 | The Wurlitzer Company, *Wurlitzer Electronic Pianos Service Manual, Models 200, 203, 203W, 206, 207, 207V, 214 & 214V* (scan by Tropical Fish; includes a Model 200A schematic) | https://static1.squarespace.com/static/581b462f5016e14ae76bd275/t/5ebb550ed544905f3db1d03a/1589335321798/wurlitzer-200-200a-service-manual.pdf | manufacturer | Yes: specifications, amplifier description, regulation and trouble-shooting pages (text extracted locally) |
| S7 | Arturia, *Wurlitzer V User's Manual* 1.1.0 (includes the "Wurlitzer EP 200-A specifications (from the original manual)") | https://downloads.arturia.com/products/wurlitzer-v/manual/WurlitzerV_Manual_1_1_0_EN.pdf | library maker | Yes (text extracted locally) |
| S8 | Arturia, *Stage-73 V2 User Manual* (2021; retailer-hosted copy) | https://r2.gear4music.com/media/94/942952/download_942952.pdf | library maker | Yes (text extracted locally) |
| S9 | Arturia, *Clavinet V User Manual* (retailer-hosted copy) | https://r2.gear4music.com/media/94/943343/download_943343.pdf | library maker | Yes (text extracted locally) |
| S10 | Hohner, *Clavinet D6 Owner's Manual* (transcription on clavinet.com) | https://clavinet.com/d6man.php | manufacturer | Yes (full page text, downloaded with curl) |
| S11 | till.com, "Clavinet" (electronic-music article on the D6) | https://till.com/articles/Clavinet/ | other | Yes |
| S12 | Sound On Sound, review "Scarbee RSP '73" | https://www.soundonsound.com/reviews/scarbee-rsp-73 | other (magazine review of a sample library) | Yes |
| S13 | Wikipedia, "Rhodes piano" | https://en.wikipedia.org/wiki/Rhodes_piano | encyclopedia | Yes |
| S14 | Wikipedia, "Wurlitzer electronic piano" | https://en.wikipedia.org/wiki/Wurlitzer_electronic_piano | encyclopedia | Yes |
| S15 | Wikipedia, "Clavinet" | https://en.wikipedia.org/wiki/Clavinet | encyclopedia | Yes |
| S16 | Wikipedia, "General MIDI" (GM1 program table, 1-based) | https://en.wikipedia.org/wiki/General_MIDI | encyclopedia | Yes |
| S17 | Wikipedia, "General MIDI Level 2" (variation banks, controllers) | https://en.wikipedia.org/wiki/General_MIDI_Level_2 | encyclopedia | Yes |
| S18 | The MIDI Association, "GM 1 Sound Set" (Wayback Machine copy of 2021-10-18; downloaded with curl because WebFetch cannot reach web.archive.org) | https://web.archive.org/web/20211018235158/https://www.midi.org/specifications-old/item/gm-level-1-sound-set | spec | Yes |
| S19 | Wikipedia interlanguage links (MediaWiki API) for "Rhodes piano", "Wurlitzer electronic piano", "Clavinet", "Electric piano" | https://en.wikipedia.org/w/api.php?action=query&prop=langlinks&lllimit=500&titles=Rhodes_piano%7CWurlitzer_electronic_piano%7CClavinet%7CElectric_piano&format=json | encyclopedia | Yes |
| S20 | jRhodes3d (Jeff Learman), sfzinstruments: `README.md`, `LICENSE`, `jRhodes3d-mono.sfz`, file listing | https://github.com/sfzinstruments/jlearman.jRhodes3d | library maker | Yes |
| S21 | jRhodes3c (Jeff Learman), sfzinstruments: `README.md`, `LICENSE` | https://github.com/sfzinstruments/jlearman.jRhodes3c | library maker | Yes |
| S22 | VCSL (Versilian Community Sample Library): folder listings of `Electrophones`, `Electrophones/TX81Z`, `Chordophones`, `Chordophones/Zithers`, `Chordophones/Composite Chordophones`, `Idiophones`, and `LICENSE` | https://github.com/sgossner/VCSL | library maker | Yes (listings only) |
| S23 | VSCO 2 Community Edition, `SFZ` branch: root listing and `Keys` folder listing | https://github.com/sgossner/VSCO-2-CE/tree/SFZ | library maker | Yes (listings only) |
| S24 | FreePats project home page (instrument index) | https://freepats.zenvoid.org/ | library maker | Yes |
| S25 | Sonara repository: `docs/sample-libraries.md`, `docs/orchestral-audio-engine-research.md` section 5, `docs/orchestration-metadata.md`, `crates/sonara-midi/src/orchestration/{data.rs, articulation.rs, instrument.rs}`, `crates/sonara-midi/src/gm.rs`, `crates/sonara-media/src/{sfz.rs, format.rs, stream.rs}`, `crates/sonara-media/Cargo.toml`, `crates/sonara-app/src/instruments/family.rs` | local checkout of the Sonara repository (branch `research/orchestra-instruments`); paths are repository-relative | other | Yes |
| S26 | sfzinstruments GitHub organisation page (shows 10 of 76 repositories) | https://github.com/sfzinstruments | library maker | Yes (first page only) |
| S27 | Apple support topic pages for Vintage Electric Piano / Vintage Clav (lgsifc1ba040, lgsi84074fad, lgsifbfa467c) | https://support.apple.com/guide/logicpro/lgsifc1ba040/mac | library maker | No: HTTP 404 / 429. The same text was read in S3. Not cited |
| S28 | manuals.plus Wurlitzer manuals category | https://manuals.plus/category/wurlitzer | other | No: HTTP 403. Not cited |
| S29 | Native Instruments Scarbee Mark I / A-200 / Clavinet-Pianet manuals (guessed fileadmin URLs) | https://www.native-instruments.com/fileadmin/ni_media/downloads/manuals/ | library maker | No: HTTP 404. Not cited; Scarbee and Soniccouture facts seen only in search snippets are not used |
| S30 | GitHub REST API (repository and licence metadata) | https://api.github.com/orgs/sfzinstruments/repos | other | No: rate limit. The repositories were read through their web pages and raw files instead (S20, S21, S22, S23) |
| S31 | Greg Sullivan E-Pianos, sfzinstruments: `README.md`, `LICENSE`, file tree, `Wurlitzer EP200/Wurlitzer EP200.sfz`, GitHub REST repository record (licence field) | https://github.com/sfzinstruments/GregSullivan.E-Pianos | library maker | Yes (raw files and the API record; added in the consistency review) |

## electric_piano_rhodes: Rhodes piano

```json
{
  "id": "electric_piano_rhodes",
  "family": "keyboards",
  "names": {"en": "Rhodes piano (Fender Rhodes)", "cs": "Fender Rhodes (elektrické piano)", "it": "Fender Rhodes", "de": "Fender Rhodes", "fr": "piano Rhodes", "score_abbreviation": "Rhodes"},
  "pitched": true,
  "transposition": 0,
  "clefs": ["treble", "bass"],
  "sounding_playable": [21, 108],
  "sounding_comfortable": [28, 100],
  "range_provenance": {
    "playable": "Cited S1, S3, S13: 88-key models exist (S3, S13); S1 Ch. 6 numbers the 88-key pianos' tone bars 1-88 like a piano 'whose keys are identified A1 through C88' (the fetched page renders 'Al', read as A1: the 1-88 numbering and 'E8' for the 73-key piano's lowest key make it unambiguous), i.e. key number with its letter, so key 1 = A0 = MIDI 21 and key 88 = C8 = MIDI 108 (MIDI = key number + 20)",
    "comfortable": "Estimate: modelling choice that 'comfortable' = the span of the 73-key pianos, so a part inside it plays on every full-size Rhodes and on the 73-key sampled and modelled references (S8, S12, S20). The span itself is Cited S1, S12: the 73-key pianos, Rhodes's original model, are keys 'E8 through E80' (S1 Ch. 6) = E1-E7 = MIDI 28-100, and S12 states 'E1 to E7 (middle C = C4)'"
  },
  "written_playable": [21, 108],
  "gm_program": 5,
  "gm_program_name": "Electric Piano 1",
  "gm_percussion_notes": [],
  "existing_sonara_id": null,
  "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
  "articulations_proposed_new": ["glissando"],
  "articulations_provenance": "Cited S3 for glissando (Voices up to 88 'allowing for glissandi over the entire keyboard range when the sustain pedal is depressed'); the others are Estimate: general keyboard practice, no source read lists articulations for the Rhodes. Tremolo ('vibrato'), phaser, chorus and drive are effects, described as controls in sections 4-5",
  "solo_or_section": "solo",
  "typical_players": {"classical": "0 (instrument dates from 1946 on)", "romantic": "0 (instrument dates from 1946 on)", "film": "1 keyboard player", "chamber": "1"},
  "keyboard": {
    "action": "Velocity-sensitive hammer action that works like a grand piano's, hammers striking tines (S3, S5); escapement ideally 1/32 inch (S1 Ch. 4). 88-key models span A0-C8 (21-108); the common Stage/Suitcase 73 spans E1-E7 (28-100) (S1 Ch. 6)",
    "keys": 88,
    "lowest": 21,
    "highest": 108,
    "pedals": ["sustain"],
    "pedal_ccs": {"sustain": 64},
    "sympathetic_resonance": "Estimate: weak; each note is a separate tine and tone bar with no shared soundboard, and no source read describes sympathetic ringing",
    "release_and_mechanical_noise": "Damper felt landing on the tine on key release, a small 'thwong' (S12), modelled as Damper Noise (S3, S8); hammer, tine and pickup noise are modelled separately in S8",
    "decay": "Long, sustained by the tone bar (S1 Ch. 1); longer than the Wurlitzer's (S8, S13); full-length jRhodes3d samples last up to 25 s (S20); the Mk II has less treble sustain (S3)",
    "dynamic_range": "Velocity-sensitive (S3); a smaller tine-pickup gap gives more volume and a more pronounced dynamic response (S1 Ch. 4); hard playing adds 'bark' (S12, S20)"
  }
}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | Rhodes piano; Fender Rhodes; Rhodes Stage / Suitcase piano | [Cited S13, S2] |
| Czech | Fender Rhodes (the cs Wikipedia article title; the generic term is "elektrické piano") | [Cited S19] |
| Italian | Fender Rhodes (generic: "pianoforte elettrico") | [Cited S19] |
| German | Fender Rhodes | [Cited S19] |
| French | Rhodes (piano), i.e. "piano Rhodes" (generic: "piano électrique") | [Cited S19] |
| Score | "Rhodes" or "E. Pno." on a two-stave (grand-staff) system | [Estimate: common lead-sheet and score practice; no opened source prints an abbreviation] |

- Family slug `keyboards`; `Family::Keyboards` already exists in `instrument.rs` [Cited S25].
- The id names the instrument type: all Rhodes models (Suitcase, Stage, 73- and 88-key, Mk I, Mk II, Mk V) share one tone-generation principle [Cited S3: Apple says that how an individual piano is adjusted and maintained shapes its sound more than which model it is].

### 2. Sound production

| Element | Fact | Provenance |
|---|---|---|
| Tone generator | An asymmetrical tuning fork per note: a thin tine (lower leg) bolted to a heavier tone bar (upper leg); the tone bar vibrates at the same frequency and supports the tine | [Cited S1 Ch. 1, S3] |
| Tone bar role | Squeezing the tone bar with the sustain pedal down makes the tone "die immediately" | [Cited S1 Ch. 1] |
| Tuning | A tuning spring moved along the tine changes the pitch | [Cited S1 Ch. 1, S3] |
| Excitation | Hammers strike the tine through a velocity-sensitive action resembling a grand piano's | [Cited S3, S13] |
| Hammer tips | Felt-covered at first, then neoprene (rubber); felt gives a smoother sound | [Cited S2, S3, S13; date disagreement in the last section] |
| Pickup | An electromagnetic pickup opposite each tine, "similar to that of an electric guitar"; permanent magnets around the tine damp its movement | [Cited S3, S13] |
| Tine position | Timbre adjustment: the Timbre Adjustment Screw is turned until the tine end "rests on a plane slightly above dead center of the Pickup"; "Let your ear guide you" | [Cited S1 Ch. 4] |
| Tine-pickup gap | The smaller the gap, the greater the volume and "the more pronounced the DYNAMIC RESPONSE"; in pianos built since March 1972 a gap of 0.020 in (0.508 mm) is possible in the middle and upper ranges | [Cited S1 Ch. 4] |
| "Bell" | Moving the tine closer to the pickup makes the bell more prominent; many Rhodes were set for more bell in the 1980s | [Cited S3, S5, S13] |
| Construction | Some tone bars are rotated 90 degrees; the lower octaves have a 90-degree twist | [Cited S3, S2] |
| Dampers | Felt dampers; clearance 3/8-1/2 in when the key is depressed (the damper is at maximum clearance as the hammer tip strikes the tine); the damper stops the tine after release | [Cited S1 Ch. 4, S3] |
| Signal | Weak, guitar-like output that needs pre-amplification; "not harmonically rich", hence treble boost or overdrive; "sounds best when played through tube amplifiers" | [Cited S3] |
| Stage vs Suitcase | Stage: no amplifier, single output jack, to any external amp (S13, S3); front panel volume and bass EQ only (S2). Suitcase: preamp plus built-in power amplifier and two-channel speaker cabinet, with the "vibrato" (S3, S13). Power amplifier 80 W, 100 W from 1977 | [Cited S2, S3, S13] |
| Suitcase sound | The Suitcase preamp "could create a sound with a very dominant mid-range" | [Cited S3] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

Conversion: S1 identifies keys by their number on an 88-key keyboard with the note letter attached ("A1 through C88"; the fetched page renders "Al"), so MIDI = key number + 20. "A1" there is key 1 = scientific A0, not MIDI 33 [Cited S1 Ch. 6 for the numbering; the MIDI conversion is derived arithmetic].

| Model | Keys | S1 key numbers | Scientific | MIDI | Provenance |
|---|---|---|---|---|---|
| 73-key (Stage 73, Suitcase 73; the original model) | 73 | 8-80 ("E8 through E80") | E1-E7 | 28-100 | [Cited S1 Ch. 6, S12] |
| 88-key models | 88 | 1-88 ("A1 through C88") | A0-C8 | 21-108 | [Cited S1 Ch. 6, S3, S13] |
| 54-key (added with the Mk II) | 54 | not found | not found | not stored | [Cited S13 for the model; range not found] |
| Piano Bass | 32 | n/a | E1-B3 | 28-59 | [Cited S13] |

- Transposition 0: written = sounding [Estimate: it is played and notated as a piano; no source states a transposition. S3's Tune control puts "the central A key" at 440 Hz for the software model].
- Clefs: grand staff, treble and bass [Estimate: keyboard practice].
- Register notes: the lower tone bars are twisted [Cited S2]; S1 gives a striking-line dimension of about 2-1/4 in at the extreme bass tone bar and about 1/8 in at the extreme treble, and says escapement varies from bass to treble [Cited S1 Ch. 4]. S12 hears "the mid-range bark and low-end fart of the real thing" [Cited S12]. jRhodes samples show that higher notes change timbre less with velocity [Cited S20].
- Comfortable range: the 73-key span is recommended so that parts fit the commonest instrument [Estimate: modelling choice, see open questions].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Topic | Fact | Provenance |
|---|---|---|
| Dynamics | Velocity-sensitive; the tine-pickup gap sets both volume and dynamic response | [Cited S3, S1 Ch. 4] |
| Colour | Soft: mellow, bell-like (S8 "a pure, bell-like tone"; S13 "mellower timbre"). Hard: "bark" (S12, S20: jRhodes was recorded "with substantial bark on the higher velocity layers") | [Cited S8, S12, S13, S20] |
| Colour by register | Bass: strong, growling at high velocity; middle: the main bark region; top: bell, short decay | [Estimate: from S12's "mid-range bark and low-end fart" and S3's Mk II treble remark; no source maps colour by register in detail] |
| Sustain | Long, longer than the Wurlitzer | [Cited S8, S13] |
| Agility | Piano-like action; "the ideal Escapement for the most responsive touch is 1/32\"" | [Cited S1 Ch. 4 for escapement; Estimate: agility equals a piano action's] |
| Polyphony | One tine per key, so every key can sound at once; the Apple model allows 88 voices | [Cited S3 for the model; Estimate: the hardware has no voice limit] |
| Weight | About 130 lb (59 kg) for the 73-key model; MK8 (2021) 75 lb (34 kg) | [Cited S13] |
| Physical limits | No pitch bend or vibrato on the hardware; tuning is fixed per tine (spring) | [Cited S1 Ch. 1 for spring tuning; Estimate: no bend mechanism exists] |

**Controls that belong to the instrument (not articulations)**

| Control | Hardware | Software references and MIDI | Provenance |
|---|---|---|---|
| Sustain pedal | Pedal and pushrod lifting the dampers; on the Stage the pushrod is "part of a Rogers hi-hat stand" (S2) | CC64 (GM2 "Damper Pedal On/Off (Sustain)") | [Cited S2, S13, S17] |
| Half pedal | Not documented for the hardware | jRhodes3d implements it with `ampeg_dynamic=1` and `ampeg_releasecc64=100` (needs a continuous CC64) | [Cited S20; Estimate: hardware behaviour not read] |
| Volume | Stage and Suitcase; at high settings the preamp overdrives slightly | CC7 [Estimate: general MIDI practice] | [Cited S8] |
| Tone / Bass Boost | Stage '73: Tone; Stage '74 and later: Bass Boost; Suitcase: Treble and Bass sliders | Apple Bass Boost emulates "the control found on the original Rhodes piano" | [Cited S8, S3] |
| "Vibrato" (really a stereo tremolo, auto-pan) | Suitcase only: on/off, Intensity, Speed; mislabelled "vibrato" | Apple Tremolo: Rate, Intensity, Stereo (180 degrees for Rhodes); CC25-27; MIDI Mode "Mod Wheel to Tremolo". jRhodes3d: CC1 tremolo depth, CC92 rate, CC93 delay, CC94 fade, CC95-96 auto-pan | [Cited S8, S13, S3, S20] |
| Effects chain | External pedals and amps in practice: drive or tube amp, phaser (1970s), chorus | Apple chain: Drive first, then EQ, Phaser, Tremolo, Chorus; "Chorus is the most commonly used effect on electric piano sounds" | [Cited S3] |
| Apple Vintage Electric Piano CCs | n/a | CC1 Volume (as listed), 12 Model, 13 Decay, 14 Release, 15 Tine Bell, 16 Damper Noise, 17 Stereo Width, 18 Treble, 19 Bass, 20 Drive Gain, 21 Drive Tone, 22-24 Phaser Rate/Color/Stereo, 25-27 Tremolo Rate/Intensity/Stereo, 28 Chorus Intensity | [Cited S3 pp. 705-706] |

What a composer writes differently for the Rhodes piano:

- **Pedalling.** The sustain pedal lifts the dampers (CC64 in MIDI) [Cited S2, S13, S17] and is marked as for the piano
  ("Ped." and a release sign) [Estimate: piano notation practice; no opened source prints a Rhodes part]. Notes ring long,
  longer than on a Wurlitzer [Cited S8, S13], so a held pedal blurs changing harmony as on a piano [Estimate: follows from
  the long decay]. A pedalled glissando over the whole keyboard works; Apple's model allows 88 voices for it [Cited S3].
- **No soft pedal and no resonant halo.** Compared with the concert grand there is no una corda, so "una corda / tre corde"
  marks have no meaning [Estimate: no opened source describes a second pedal]; and the pedal adds little sympathetic
  ringing, because each note is a separate tine and tone bar with no shared soundboard [Estimate: no source read describes
  sympathetic ringing on the Rhodes].
- **Dynamics change the colour.** The action is velocity-sensitive [Cited S3]: soft playing is mellow and bell-like, hard
  playing adds "bark" [Cited S8, S12, S13, S20], so a forte asks for a different sound as well as a louder one; high notes
  change timbre less with velocity [Cited S20]. The weak pickup signal needs amplification [Cited S3], so the level against
  an orchestra is set at the amplifier or in the mix [Estimate: follows from the amplified signal; no source discusses
  orchestral balance].
- **Register and voicing.** Keep the part within E1-E7 (28-100), the span of the 73-key pianos [Cited S1, S12], so that it
  plays on every full-size Rhodes; the grand's A0-D#1 and F7-C8 exist only on the 88-key models [Cited S1, S13; the
  comparison, and that the span fits both models, is arithmetic]. S12 hears a "low-end fart" in the bass [Cited S12], so low chords are best kept open
  [Estimate: general keyboard voicing practice; no source states it].
- **Texture and idiom.** Chord comping, ballad accompaniment and solo lines with bell tone are the usual roles [Estimate:
  from the repertoire in section 7]; in a band the Rhodes tends to blend in rather than stand out [Cited S7].
- **Effects are written into the part.** The Suitcase tremolo (labelled "vibrato"), phaser and chorus belong to the sound
  [Cited S3, S8, S13]; write them as text at the start of a passage, for example "Suitcase tremolo, slow", not as
  articulations [Estimate: lead-sheet and session practice; no opened source prints such a part].
- **Notation.** At pitch on a grand staff, headed "Rhodes" or "E. Pno.", often with chord symbols in band parts [Estimate:
  keyboard and lead-sheet practice; no opened source prints a Rhodes part].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Articulation id | Played as | Library implementation | Provenance |
|---|---|---|---|
| `sustain` | Key held, or sustain pedal | All references: note length plus CC64. jRhodes3d: full-length unlooped samples; jRhodes3c: looped | [Cited S20, S21; Estimate for "all references"] |
| `legato` | Finger legato (overlapping keys) | No legato transition samples or scripts in jRhodes or the Apple and Arturia models: overlapping notes simply both sound | [Estimate: no legato mode is listed in S3, S8 or S20] |
| `staccato`, `tenuto`, `marcato` | Note length and velocity | Note length; velocity layers (jRhodes 5 layers with velocity crossfades `xfin_*`/`xfout_*`; Scarbee RSP '73 12 layers) | [Cited S20, S12 for layers; Estimate for the articulation mapping] |
| `trill` | Fingered | Notes | [Estimate: general keyboard practice; no source read lists articulations for the Rhodes] |
| `glissando` (proposed) | White-key glissando, usually with the pedal down | Notes; Apple's 88-voice limit is meant to allow it | [Cited S3] |

- Key-off: the damper's "thwong" (S12) is a release sample in Scarbee RSP '73 ("release samples ... at the same 12 dynamic levels as the sustains") and a Damper Noise level in Apple and Arturia [Cited S12, S3, S8].
- Model parameters exposed by modelled instruments: Hammer Hardness (felt to neoprene), Pickup Distance, Pickup Alignment, Damper Duration, Tine Noise, Damper Noise, Dynamics, Tonebar Resonance (Arturia Stage-73 V2); Decay, Release, Tine Bell, Damper Noise, Stereo Width, Warmth, stretch tuning (Apple) [Cited S8, S3].
- Key switches: jRhodes3d-keyswitch selects mono, stereo or stereo-vibrato variants on G7-C8 [Cited S20]. Note: with C4 = 60 these keys (MIDI 103-108) lie inside an 88-key Rhodes range [Estimate: derived].
- The Sonara sampler plays velocity layers, CC64 sustain, CC7, CC11 and key switches but no CC-driven layers, crossfades, LFOs or filters [Cited S25: `docs/sample-libraries.md`, `sfz.rs` opcode list].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One instrument, one player; never a section [Estimate: band and studio practice].
- In a band the Rhodes "tends to blend in", while the Wurlitzer has the clearer, more distinctive sound; S7 calls the Rhodes "more of a solo instrument, suited to fusion, acid jazz" [Cited S7].
- Layering with a second keyboard (acoustic piano, pad) is common; Apple's stretch-tuning controls exist for playing alongside an acoustic piano [Cited S3 for the reason of the stretch control; Estimate for the layering practice].

### 7. Role in the orchestra and typical doublings

- Styles: jazz, pop, soul and rock in the 1970s (S13); pop, rock, jazz, soul, house and hip-hop (S3); Miles Davis's main keyboard from *In a Silent Way* (1969) to the mid-1970s; Herbie Hancock, Chick Corea, Joe Zawinul (S13) [Cited S3, S13].
- Roles: chord comping, ballad accompaniment, solo lines with bell tone [Estimate: from the cited repertoire, e.g. S8's list "Just the Way You Are", "Spain", "Chameleon"].
- In an orchestral or film score: rhythm-section colour beside bass, drums and guitar; doubles harp, celesta or vibraphone for bell colour [Estimate: no source read].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Date | Provenance |
|---|---|---|
| Pre-Piano | designed 1946, marketed 1948 | [Cited S13; S3 gives "Designed in 1946"] |
| Piano Bass (32 keys, E1-B3) | 1959 (retail from 1962) | [Cited S13] |
| Fender Rhodes Electric Piano, after CBS bought Fender | 1965 | [Cited S13, S3] |
| Mark I Suitcase and Stage | 1970 (S13); the 73-key piano renamed Suitcase in 1969 (S2) | [Cited S13, S2] |
| 88-key models | 1971 (S13), 1972 (S2), 1973 (S3) | [Cited S2, S3, S13] |
| Mark II (flat top; no treble resonance clamps, less treble sustain) | 1978 (S3) or 1979 (S13) | [Cited S3, S13] |
| Mark V (MIDI output) | 1984 | [Cited S3, S13] |
| Rhodes Mark 7; MK8 | 2007; 2021 | [Cited S13] |
| Celeste and bass versions | "less popular" | [Cited S3] |

- Related instruments a keyboard player switches to: Wurlitzer (`electric_piano_wurlitzer`), Hohner Electra Piano (hammers like the Rhodes, stiffer action), Yamaha CP-70 electric grand (GM 3), FM electric piano (GM 6, "usually a Yamaha DX7 e. piano 1 preset") [Cited S3, S16].

### 9. General MIDI and existing Sonara metadata

| Item | Value | Provenance |
|---|---|---|
| GM1 program | 5 "Electric Piano 1" (1-based) | [Cited S18] |
| What GM 5 sounds like | "usually a Rhodes or Wurlitzer piano" | [Cited S16] |
| GM 6 | "Electric Piano 2", "usually a Yamaha DX7 e. piano 1 preset": not a Rhodes | [Cited S18, S16] |
| GM2 variations of 5 | bank 0 Rhodes Electric Piano, 1 Detuned Electric Piano 1, 2 Variation Electric Piano 1, 3 60's Electric Piano (article's wording); variation banks via CC0 = 121 and CC32 | [Cited S17] |
| GM is a name, not a timbre | GM1 "does not define the actual characteristics of any sounds" | [Cited S18] |
| data.rs | no keyboard instrument among the 16 | [Cited S25] |
| gm.rs | GM percussion map only; no melodic program table | [Cited S25] |
| Catalog family inference | `family.rs` maps the words piano, organ, harpsichord, celesta, keyboard to `keyboards`; "rhodes" is not in the list, so a patch path like `jRhodes3d-mono.sfz` gives no family | [Cited S25 for the word list; Estimate: the outcome for this path was derived, not run] |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage | Sonara sampler fit | Provenance |
|---|---|---|---|---|
| jRhodes3d (Jeff Learman) | `LICENSE`: samples CC BY-NC 4.0; control files and example clips CC0; music made with it unrestricted. The `.sfz` header says "CC-SA-BY" | 1977 Mark I Stage 73, recorded from the harp connector with treble boost and low-mid scoop; 5 velocity layers; unlooped FLAC samples up to 25 s; samples F1-C7 (MIDI 29-96) every few keys, mapped A0-C8; mono, stereo and stereo-vibrato patches; half-pedal; mod-wheel tremolo | Supported: `sample`, `lokey`/`hikey` with note names, `pitch_keycenter`, `lovel`/`hivel`, `ampeg_release`, `volume`, FLAC (symphonia `flac` feature). Ignored: `xfin_*`/`xfout_*`, `lfo01_*`, `lfo02_*`, `ampeg_dynamic`, `ampeg_releasecc64`, `<curve>`, `set_hdcc*` | [Cited S20, S25; Estimate: loading was not tested] |
| jRhodes3c | `LICENSE`: CC BY-NC-SA 4.0 (but links the BY-NC deed) | Same recordings, looped, 16 MB; mono and stereo | as above | [Cited S21] |
| VCSL | CC0 | No Rhodes; `Electrophones` holds only TX81Z (Yamaha FM) patches: "Clavisynth", "FM Piano", "Piano 1" | n/a | [Cited S22] |
| VSCO 2 CE | CC0 | No electric piano; `Keys` holds Organ and uprights | n/a | [Cited S23, S25] |
| FreePats | per bank | Only a "Synthesized Piano" in the piano category | n/a | [Cited S24] |
| MuseScore_General, FluidR3_GM | MIT, notices required | GM program 5 among 128 programs; SF2, not played by the Sonara sampler | no | [Cited S25] |
| Apple Vintage Electric Piano (Logic Pro) | Apple software licence (bundled with Logic) | Component-modelled Rhodes models, 88-key range, effects | not a Sonara source | [Cited S3, S4] |
| Arturia Stage-73 V2 | commercial EULA | Modelled Stage and Suitcase, 7 model profiles, FX and amps | external plugin only | [Cited S8] |
| Scarbee RSP '73 | commercial | Sampled Rhodes 73, 12 velocity layers, release samples; GIGA / EXS24 MKII / HALION | external | [Cited S12] |

## electric_piano_wurlitzer: Wurlitzer electronic piano

```json
{
  "id": "electric_piano_wurlitzer",
  "family": "keyboards",
  "names": {"en": "Wurlitzer electronic piano", "cs": "Wurlitzer (elektrické piano)", "it": "pianoforte elettrico Wurlitzer", "de": "Wurlitzer Electronic Piano", "fr": "piano électrique Wurlitzer", "score_abbreviation": "Wurli."},
  "pitched": true,
  "transposition": 0,
  "clefs": ["treble", "bass"],
  "sounding_playable": [33, 96],
  "sounding_comfortable": [33, 96],
  "range_provenance": {
    "playable": "Cited S6, S7, S14, S3: '64 Notes, A-13 thru c-76' (S6; same in S7's copy of the 200A specification), i.e. 88-key positions 13-76 = A1-C7 = MIDI 33-96 (MIDI = key number + 20); S14 and S7: from the A an octave above the lowest note of an 88-key piano to the C an octave below its top; S3: '64 keys (A to C)'",
    "comfortable": "Estimate: every key is equally reachable on a keyboard and no source names a narrower comfortable span, so comfortable = playable"
  },
  "written_playable": [33, 96],
  "gm_program": 5,
  "gm_program_name": "Electric Piano 1",
  "gm_percussion_notes": [],
  "existing_sonara_id": null,
  "articulations_existing_vocab": ["sustain", "legato", "staccato", "tenuto", "marcato", "trill"],
  "articulations_proposed_new": ["glissando"],
  "articulations_provenance": "Cited S3 for glissando (the Vintage Electric Piano plug-in, which includes the Wurlitzer models, allows 88 voices 'allowing for glissandi over the entire keyboard range when the sustain pedal is depressed'); the others are Estimate: general keyboard practice, no source read lists articulations for the Wurlitzer. The 'vibrato' (tremolo) is an amplifier effect, described as a control",
  "solo_or_section": "solo",
  "typical_players": {"classical": "0 (instrument dates from 1954 on)", "romantic": "0 (instrument dates from 1954 on)", "film": "1 keyboard player", "chamber": "1"},
  "keyboard": {
    "action": "Wurlitzer-made action with whips, butts, flys (jacks) and dampers that 'Regulates Like a Conventional Piano Action'; hammer let-off 1/8 inch from the reed (S6); velocity-sensitive (S5)",
    "keys": 64,
    "lowest": 33,
    "highest": 96,
    "pedals": ["sustain"],
    "pedal_ccs": {"sustain": 64},
    "sympathetic_resonance": "Estimate: weak; each note is a separate reed and the pickup is electrostatic, no soundboard; no source read describes it",
    "release_and_mechanical_noise": "Key release noise and an audible sustain-pedal mechanism ('On a Wurlitzer you can hear the mechanical action of the sustain pedal'), modelled as Note Off Noise and Sustain Pedal Noise (S7); hammer noise (S7)",
    "decay": "Shorter than the Rhodes (S8 'Longer sustain than the Wurlitzer'; S13); the sustain pedal lifts the dampers 'as in a conventional piano' (S6)",
    "dynamic_range": "Velocity-sensitive (S5); gentle playing 'sweet and vibraphone-like', hard playing a 'slightly overdriven tone usually described as a bark' (S7, S14)"
  }
}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | Wurlitzer electronic piano; Wurlitzer EP 200 / 200A | [Cited S14, S6, S7] |
| Czech | Wurlitzer (elektrické piano) | [Cited S19] |
| Italian | Pianoforte elettrico Wurlitzer | [Cited S19] |
| German | Wurlitzer Electronic Piano | [Cited S19] |
| French | Piano électrique Wurlitzer | [Cited S19] |
| Score | "Wurlitzer", "Wurli." or "E. Pno." | [Estimate: lead-sheet practice; no opened source prints an abbreviation] |

- Family `keyboards` [Cited S25 for the slug]. Reference model: the 200A, the model the software references emulate (S7, S3) [Cited S7, S3].

### 2. Sound production

| Element | Fact | Provenance |
|---|---|---|
| Hammers | Three-ply maple covered with mothproofed felt | [Cited S6] |
| Tone source | "Hammers strike Swedish Sandvik steel reeds, causing them to vibrate in a polarized electrostatic field"; the hammers strike the flat reeds at around their middle | [Cited S6, S7] |
| Pickup | Capacitive: the pickups on the reed bar carry a polarizing voltage of 160-170 V DC above ground; the reeds move between the teeth of a comb-like pickup | [Cited S6, S3] |
| Reed position | A reed slightly off centre in its pickup slot makes the note "Extra Loud or Harsh"; centre the reed, or bend the pickup ends up 1/32-1/16 in | [Cited S6] |
| Pickup axis | When the pickup is exactly in front of the reed at rest, "the note jumps one octave higher" (modelled as Pickup Axis) | [Cited S7] |
| Tuning | Solder weight on the reed tip: removing solder raises the pitch, adding lowers it; retuning by about 2 semitones keeps the timbre | [Cited S7, S14, S3] |
| Timbre | "a number of odd harmonics" (S3, S5); "sharper and closer to a sawtooth wave" than the Rhodes (S14); "brighter, more hollow" than the Rhodes Mark II (S7) | [Cited S3, S5, S7, S14] |
| Amplifier | Built-in amplifier and speakers: Model 200 two 4 x 8 in oval speakers; 200A two front-mounted speakers; power requirement 40 W (Models 200, 203, 203W, 206, 207, 214) | [Cited S6, S7] |
| Output | Audio output to a guitar amp or PA; headphones optional on Models 200 and 203 | [Cited S7, S6] |
| Controls on the 200A | Volume and "Vibrato" knobs only | [Cited S7] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

| Model | Keys | Manual key numbers | Scientific | MIDI | Provenance |
|---|---|---|---|---|---|
| 200, 203, 203W, 206, 207, 207V, 214, 214V; 200A | 64 | A-13 thru c-76 | A1-C7 | 33-96 | [Cited S6, S7] |
| "Most Wurlitzer pianos" | 64 | n/a | A1-C7 | 33-96 | [Cited S14] |
| 106P pupil piano | 44 | n/a | not found | not stored | [Cited S14 for the key count] |

- Transposition 0, grand staff [Estimate: notated as a piano; no source states a transposition].
- Register: the 200A is "sweet and vibraphone-like" when played gently in any register; bark grows with velocity [Cited S7]. A register-by-register account was not found [Estimate: no opened source describes the colour register by register].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Topic | Fact | Provenance |
|---|---|---|
| Dynamics | Velocity-sensitive hammer action like an acoustic piano's | [Cited S3, S5] |
| Bark | Hard playing gives the "bark"; debris between reed and pickup gives "a burst of distortion" | [Cited S7, S14] |
| Colour | Clear and distinctive in a band with guitar, bass and drums, where a Rhodes blends in | [Cited S7] |
| Sustain | Shorter than the Rhodes | [Cited S8, S13] |
| Agility | Piano-like regulation (let-off 1/8 in, check-back) | [Cited S6; Estimate: agility equals a piano action's] |
| Quiet playing | No acoustic noise, so it can be played quietly on headphones | [Cited S14] |
| Weight | 56 lb without legs and pedal (Model 200, 200A) | [Cited S6, S7] |

**Controls that belong to the instrument (not articulations)**

| Control | Hardware | Software references and MIDI | Provenance |
|---|---|---|---|
| Sustain pedal | Mechanical; "Lifts dampers, permitting tone to sustain as in a conventional piano"; detachable on the Model 200 | CC64 | [Cited S6, S14, S17] |
| "Vibrato" = mono tremolo | Amplitude modulation in the preamp; oscillator "preset to a frequency of 5.75 cycles per second"; knob sets depth only (fixed rate). The 140 first had the onboard fixed-rate tremolo, also mislabelled "vibrato" | Apple: mono tremolo, fixed 5.5 Hz, use Stereo 0 degrees; Arturia: "approximately 6.34Hz", mono; Apple CC25-27 | [Cited S6, S14, S3, S7] |
| Volume | Knob | Arturia: volume pedal on CC7, wah pedal on CC11 | [Cited S7] |
| External chain | Guitar amp, PA | Arturia adds a guitar amp simulator and a Leslie, plus wah, auto-wah, phaser, chorus pedals | [Cited S7] |
| Apple Vintage Electric Piano | n/a | Wurlitzer models in the same plug-in as the Rhodes; same CC table as in the Rhodes card | [Cited S3] |

What a composer writes differently for the Wurlitzer electronic piano:

- **Range.** 64 keys, A1-C7 (33-96) [Cited S6, S7, S14]: compared with the concert grand the bottom octave and the top
  octave are missing [Cited S7, S14], so a piano part has to be folded into this span [Estimate: follows from the range].
- **Pedalling and sustain.** The sustain pedal lifts the dampers "as in a conventional piano" [Cited S6] and is marked as
  for the piano [Estimate: piano notation practice; no opened source prints a Wurlitzer part]. The tone dies sooner than a
  Rhodes's [Cited S8, S13], so long held chords fade early [Estimate: follows from the shorter decay], and the pedal
  mechanism itself is audible [Cited S7]. Write no una corda marks [Estimate: no opened source describes a second pedal].
- **Dynamics change the colour.** The action is velocity-sensitive [Cited S5]: gentle playing is "sweet and
  vibraphone-like", hard playing gives the "bark" [Cited S7, S14], so dynamics are also colour marks. The overall level
  comes from the Volume knob or, in Arturia's model, a volume pedal on CC7 [Cited S7].
- **Tremolo has one speed.** The built-in "vibrato" is a tremolo whose oscillator is preset to 5.75 Hz [Cited S6]; the
  knob sets only its depth [Cited S6, S14, S3, S7, as in the controls table]. A part can ask for tremolo on or off and for
  more or less depth, not for a speed [Estimate: follows from the fixed rate].
- **Texture and idiom.** It stays clear and distinctive beside guitar, bass and drums, where a Rhodes blends in, and it can
  take over a rhythm-guitar part [Cited S7]; rhythmic comping and soul and pop ballad accompaniment are the usual roles
  [Estimate: from the repertoire in section 7].
- **Notation.** At pitch on a grand staff, headed "Wurlitzer", "Wurli." or "E. Pno." [Estimate: lead-sheet practice; no
  opened source prints an abbreviation].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Articulation id | Played as | Library implementation | Provenance |
|---|---|---|---|
| `sustain` | Key held or CC64 | Note length plus CC64; pedal noise modelled (Arturia Sustain Pedal Noise) | [Cited S7] |
| `legato` | Overlapping keys | No legato mode in Arturia Wurlitzer V or Apple | [Estimate: none is listed in S7 or S3] |
| `staccato`, `tenuto`, `marcato` | Length and velocity | Velocity curve editing (S7 "Velocity Curve"); Dynamics parameter compresses pp-ff range "without any distortion" | [Cited S7; Estimate for the mapping] |
| `trill` | Fingered | Notes | [Estimate: general keyboard practice; no source read lists articulations for the Wurlitzer] |
| `glissando` (proposed) | Keyboard glissando, usually with the pedal down | Notes; Apple's 88-voice limit, which covers its Wurlitzer models, is meant to allow it | [Cited S3] |

- Modelled parameters of Arturia Wurlitzer V: Vibrato Rate, Pickup Distance (closer = more distortion and a wider soft-loud timbre difference), Pickup Axis, Impedance (sustain length), Dynamics, Octave Stretch, Hammer Hardness, Hammer Noise, Note Off Noise, Sustain Pedal Noise, Velocity Curve [Cited S7].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One instrument and player [Estimate: band and studio practice; no source states a player count].
- In tone and attack the Wurlitzer suits an amplified, guitar-based rock band better and can even take over a rhythm-guitar part, while the Rhodes is more of a solo instrument [Cited S7].

### 7. Role in the orchestra and typical doublings

- Popular with bar bands and amateur musicians; Ray Charles "What'd I Say" (Model 120), Joe Zawinul "Mercy, Mercy, Mercy" (140B), Supertramp, Van Halen, Norah Jones (206) [Cited S14].
- Supertramp *Crime of the Century*, Pink Floyd *The Dark Side of the Moon*, The Beatles "I Am the Walrus"; most success in the 1970s [Cited S3].
- Also MOR ballads and country music [Cited S7].
- Roles: rhythmic comping with guitar, soul and pop ballad accompaniment [Estimate: from the cited repertoire].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Fact | Provenance |
|---|---|---|
| First model | 1954; production until 1983 | [Cited S14] |
| 100, 110, 111, 112 | early portables; the 112 the first mass-produced | [Cited S14] |
| 120 | 1956, first with tremolo (via an optional external amp) | [Cited S14] |
| 140 / 145 / 140B | 1962 transistor amp (140); tube amp (145); 140B continued after 1965 | [Cited S14] |
| 200 | 1968, plastic top, two speakers | [Cited S14] |
| 200A | 1974 to 1983 (S14); "The production started in 1972" (S7) | [Cited S14, S7] |
| 203, 203W (home consoles), 206 (student), 207 (teacher), 214 (classroom), 720 (spinet of the 145), 270 "Butterfly Baby Grand", 106P (44-key pupil piano) | consoles and classroom models; the 200-series consoles share the 64-note range | [Cited S6, S14] |

- Related: Rhodes (`electric_piano_rhodes`), Hohner Electra Piano, Hohner Pianet (plucked reeds), Yamaha CP-70 [Cited S3, S7, S16]. Arturia classifies electric pianos as struck strings (Yamaha, Baldwin, Helpinstill, Kawai), tuning forks (Fender Rhodes, Hohner Electra), plucked reeds (Pianet, Cembalet, Weltmeister Claviset) and struck reeds (Wurlitzer, Denon Elepian) [Cited S7].

### 9. General MIDI and existing Sonara metadata

| Item | Value | Provenance |
|---|---|---|
| GM1 program | 5 "Electric Piano 1" | [Cited S18] |
| Why 5 | GM 5 is "usually a Rhodes or Wurlitzer piano"; GM 6 is usually a DX7-type FM piano, so 6 is not recommended for a Wurlitzer | [Cited S16; the recommendation is an Estimate] |
| GM2 | program 5, bank 3 "60's Electric Piano" exists; no opened source says it is a Wurlitzer | [Cited S17; Estimate for the identity] |
| Sonara | no data.rs entry; gm.rs percussion only; "wurlitzer" is not a `family.rs` keyword | [Cited S25] |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage | Provenance |
|---|---|---|---|
| Greg Sullivan E-Pianos: Wurlitzer EP200 v1.1 (1999), mapped to SFZ by kinwie | CC BY 3.0 Unported per the `LICENSE` legal code and the README, which adds the author's request for attribution; GitHub's repository record reports `NOASSERTION`, a disagreement left open (the likely reason, a licence its detector did not classify, is an Estimate) | `Wurlitzer EP200.sfz`: 48 regions over keys 33-96 (the whole 64-note compass), four velocity groups (1-37, 38-65, 66-89, 90-127), 42 FLAC samples; declared as SFZ v2 with ARIA extensions. Sonara supports `sample`, `default_path`, `lokey`/`hikey`, `pitch_keycenter`, `lovel`/`hivel`, `tune`, `volume`, `ampeg_attack`; it ignores, among others, `amplitude_oncc7`, `pan_oncc10`, `set_hdcc*`, `ampeg_hold`/`ampeg_decay`/`ampeg_sustain`, `amp_veltrack`, `note_polyphony`, `off_time` | [Cited S31, S25; Estimate: loading, including the `#define`d sample extension, was not tested] |
| VCSL, VSCO 2 CE, FreePats | n/a | no Wurlitzer | [Cited S22, S23, S24] |
| MuseScore_General, FluidR3_GM | MIT, notices | GM 5 generic electric piano, SF2 (not played by the Sonara sampler) | [Cited S25] |
| Apple Vintage Electric Piano | Apple software licence | Wurlitzer models, modelled | [Cited S3] |
| Arturia Wurlitzer V | commercial EULA | Modelled 200A with effects, Leslie and guitar amp | [Cited S7] |

## clavinet: Clavinet

```json
{
  "id": "clavinet",
  "family": "keyboards",
  "names": {"en": "Clavinet", "cs": "Clavinet", "it": "Clavinet", "de": "Clavinet", "fr": "Clavinet", "score_abbreviation": "Clav."},
  "pitched": true,
  "transposition": 0,
  "clefs": ["treble", "bass"],
  "sounding_playable": [29, 88],
  "sounding_comfortable": [29, 88],
  "range_provenance": {
    "playable": "Cited S10, S15, S3, S9: D6 owner's manual '60 piano keys with a range from contra F to e'''' and 'keyboard range of 5 octaves (F1-e3)' (German/Helmholtz names: contra F = F1 = MIDI 29, e''' = E6 = MIDI 88); S15 'Most models have 60 keys ranging from F1 to E6'; S3 'The 60-key range (F to E) of the original D6'; S9 '60 notes'",
    "comfortable": "Estimate: every key is equally reachable and no source names a narrower span, so comfortable = playable"
  },
  "written_playable": [29, 88],
  "gm_program": 8,
  "gm_program_name": "Clavi",
  "gm_percussion_notes": [],
  "existing_sonara_id": null,
  "articulations_existing_vocab": ["sustain", "legato", "staccato", "marcato", "trill"],
  "articulations_proposed_new": ["portato", "damped"],
  "articulations_provenance": "Cited S10 for portato ('One has to play more or less Portato to produce Staccato') and for damped (the right-hand slide 'puts a damper on the strings and produces a dull, dry sound'; S9 likens it to a guitarist's palm mute); sustain = the note sounds while the key is held (S10); legato, staccato, marcato and trill are Estimate (keyboard practice)",
  "solo_or_section": "solo",
  "typical_players": {"classical": "0 (instrument dates from 1964 on)", "romantic": "0 (instrument dates from 1964 on)", "film": "1 keyboard player", "chamber": "1"},
  "keyboard": {
    "action": "Single-arm key lever; a plunger (rubber hammer pad) under the key presses the string onto an anvil and stays there while the key is held (S10, S3); no escapement (Estimate: derived from the plunger holding the string on the anvil while the key is down; neither S10 nor S3 uses the word); velocity and pressure set the strength of the strike (S10, S3)",
    "keys": 60,
    "lowest": 29,
    "highest": 88,
    "pedals": ["foot_swell"],
    "pedal_ccs": {"foot_swell": 11},
    "sympathetic_resonance": "Estimate: little; every string rests in its wool winding when its key is up (S10), so undamped strings do not ring along",
    "release_and_mechanical_noise": "Key-off click when strings stick to worn rubber hammers, a 'second trigger when the key is released' (S3); key release noise, hammer noise and pickup noise are modelled (S9); string buzzes and key clicks (S3)",
    "decay": "Sounds while the key is held and is muted immediately on release by the wool winding (S10, S3); the mute slider gives a quick decay (S9); no sustain pedal on the hardware (S10 lists only the foot swell)",
    "dynamic_range": "Fully dynamic: 'the harder you hit, the louder and more vibrant the tone' (S3); the string hits the anvil 'with greater or less strength according to the heaviness of key pressure' (S10)"
  }
}
```

### 1. Identity and names (Czech, English, Italian, German, French; how it appears in a score; family)

| Language | Name | Provenance |
|---|---|---|
| English | Clavinet; Hohner Clavinet D6 | [Cited S15, S10] |
| Czech, Italian, German, French | Clavinet (all four Wikipedia articles use the brand name) | [Cited S19] |
| Score | "Clav." or "Clavinet" on a grand staff | [Estimate: lead-sheet practice; no opened source prints an abbreviation] |

- Family `keyboards` [Cited S25 for the slug]; it is a struck-string instrument, "designed to replicate the sound of a clavichord, but with an altogether fuller sound" [Cited S3]. Reference model: the D6, "the most popular model" [Cited S15, S9].

### 2. Sound production

| Element | Fact | Provenance |
|---|---|---|
| Strings | 60 tensioned steel strings placed diagonally below the keys; one string per note; the strings are "relatively slack" | [Cited S15, S10] |
| Strike | A plunger under the key "touches the string and presses it on to an anvil"; the string hits the anvil harder with harder key pressure | [Cited S10, S3] |
| Hammer pad | Rubber; it ages, and worn pads make strings stick and click on release; the attack resembles a guitarist's hammer-on | [Cited S3, S9] |
| Damping | On release the contact breaks and the wool-wound part of the string, left of the anvil, mutes the vibration at once | [Cited S10, S3] |
| Pickups | Magnetic pickups at the other end of the strings; two pickups, one above and one below the strings, encased in epoxy | [Cited S10, S15, S3] |
| Pickup switches (AB, CD) | Select the pickups "either singly or parallel", with one pickup's polarity reversible. Arturia's table: AC lower only, BC upper only, AD both in phase, BD both out of phase | [Cited S10, S9] |
| Pickup colours | Neck pickup warm, bridge pickup bright, both full, both out of phase thin | [Cited S11] |
| Tone switches | Four on/off tabs: Brilliant, Treble, Medium, Soft; at least one must be down or the instrument is silent. Brilliant and Treble act as high-pass, Medium and Soft as low-pass filters (S15). Apple: Brilliant nasal with strong bass cut, Treble sharper with gentle bass cut, Medium thinner, Soft softer | [Cited S10, S9, S15, S3] |
| Mute slider | Right-hand slide damping all strings for "a dull, dry sound"; best used at its end positions | [Cited S10, S9] |
| Power and output | 9-V battery or a 6-V adapter; foot-swell socket "Output 100mV"; reproduction needs an external amplifier ("Orgaphon amplifiers are specially recommended") | [Cited S10] |
| Timbre jump | Strong key-to-key difference between the highest wound string and the lowest unwound string | [Cited S3] |
| Pitch behaviour | Pressure on a held key raises the pitch slightly; the pitch falls just after release; a slight upward bend right after the strike (tension modulation) | [Cited S3; S15 calls the pressure effect aftertouch] |

### 3. Range, transposition and clef (sounding and written; comfortable and extreme; register breaks)

| Source | Range as printed | Scientific | MIDI | Provenance |
|---|---|---|---|---|
| D6 owner's manual | "contra F to e'''" (the transcription prints `e'"`, read as e''') and "5 octaves (F1-e3)" (German/Helmholtz octave names) | F1-E6 | 29-88 | [Cited S10] |
| Wikipedia | "F1 to E6", 60 keys, most models | F1-E6 | 29-88 | [Cited S15] |
| Apple | "60-key range (F to E)"; its own key-scale centre is "MIDI note number 60 (C3)" (Apple names middle C "C3") | F-E (S3 gives no octaves) | 29-88 only with the octaves of S10 and S15 | [Cited S3 for the letters and key count] |

- Transposition 0; grand staff [Estimate: notated as a keyboard; no source states a transposition]. The tuning pin of each string sits about one octave left of its key, which affects servicing, not pitch [Cited S10].
- Register break: wound bass strings vs unwound treble strings (see section 2) [Cited S3]. Arturia's Bass Guitar profile targets "the lowest 1-1/2 octaves" [Cited S9].

### 4. Dynamics by register, colour by register, agility, breath/bow/physical limits

| Topic | Fact | Provenance |
|---|---|---|
| Dynamics | Fully dynamic by touch, "just as on a piano"; volume set on the instrument and on the amplifier | [Cited S10, S3] |
| Touch | As on a clavichord, each key must be struck decisively and then kept pressed down; careless playing sounds weak | [Cited S10] |
| Colour by register | Bass wound strings fuller, treble thinner, with a timbre jump between them | [Cited S3 for the jump; Estimate for the description] |
| Agility | Very fast repeated and percussive figures are the funk idiom (e.g. "Superstition") | [Cited S3, S15 for the repertoire; Estimate for "very fast"] |
| Sustain | Only while the key is held; no sustain pedal on the D6 | [Cited S10] |
| Ensemble level | Too quiet acoustically; needs amplification ("Rock music has tremendous impact ... provided ample amplification is available") | [Cited S10, S9] |

**Controls that belong to the instrument (not articulations)**

| Control | Hardware | Software references and MIDI | Provenance |
|---|---|---|---|
| Foot swell | Optional volume pedal | Apple Vintage Clav: CC11 scales the output level unless assigned to Wah or Damper | [Cited S10, S3] |
| Tone tabs, AB/CD | see section 2 | Apple "Filter switches" and "Pickup switches"; Arturia Tone/EQ switches and pickup switches | [Cited S3, S9] |
| Mute slider | two useful positions | Apple Damper slider, assignable to a CC, velocity or aftertouch ("Damper Control"); Arturia: map to a button or footswitch | [Cited S3, S9] |
| Key pressure | raises pitch | Apple "Pitch Pressure" (aftertouch) | [Cited S3] |
| Castle Bar | mid-1970s add-on bar that bends the pitch like a guitar tremolo arm | pitch bend | [Cited S15] |
| Effects | Wah or auto-wah, phaser, chorus in practice | Apple: Compressor, Distortion, Modulation (Phaser/Flanger/Chorus), Wah (8 modes), any order; Arturia: stomp-box effects, tube amp with tremolo, spring reverb, wah on a pedal | [Cited S15, S3, S9] |

What a composer writes differently for the Clavinet:

- **No sustain pedal.** A note sounds only while its key is held, and the wool winding mutes it at once on release [Cited
  S10, S3]; the D6 has no sustain pedal [Cited S10]. Written durations are therefore exact and the part carries no pedal
  marks; sustained harmony goes to another instrument [Cited S10 for the sustain limit; Estimate: arranging practice, no
  source states it].
- **Touch and articulation.** Each key is struck decisively and kept down; careless playing sounds weak [Cited S10]. For
  staccato the manual asks for "more or less Portato" [Cited S10]. Pressure on a held key raises its pitch slightly [Cited
  S3], and worn hammer pads can click on release [Cited S3].
- **Dynamics.** Touch sets the level "just as on a piano" [Cited S10, S3]. The optional foot swell is a volume pedal (CC11
  in Apple's model) [Cited S10, S3], which can shape swells and fades on held notes [Estimate: follows from a volume pedal;
  no source describes its use].
- **Registration instead of pedalling.** Colour is set by switches: four tone tabs (Brilliant, Treble, Medium, Soft), the
  two pickup switches, and the mute slider for "a dull, dry sound" [Cited S10, S9, S15, S3]. The part names the setting in
  words at the start of a passage, for example "Treble tab, muted" [Estimate: no opened source prints a Clavinet part].
- **Register.** 60 keys, F1-E6 (29-88) [Cited S10, S15]: the top lies 20 semitones below the concert grand's C8 and the
  bottom 8 semitones above its A0 [Estimate: arithmetic on the card's range and the grand's A0-C8]. Wound bass strings and
  unwound treble strings differ, with an audible timbre jump between them [Cited S3].
- **Texture and idiom.** Short percussive riffs and comping are the funk idiom ("Superstition", *Head Hunters*) [Cited S3,
  S15 for the repertoire; Estimate for the description of the figures], often through a wah or auto-wah [Cited S15]; doubling the bass or guitar riff is common
  [Estimate: no source read describes doublings].
- **Notation.** At pitch on a grand staff, headed "Clav." or "Clavinet" [Estimate: lead-sheet practice; no opened source
  prints an abbreviation].

### 5. Articulations and techniques, and how MIDI libraries implement them (key switch, CC, separate patch, velocity, note overlap for legato; name the library/manual you read)

| Articulation id | Played as | Library implementation | Provenance |
|---|---|---|---|
| `sustain` | Key held | Note length; no CC64 on the hardware (Arturia reserves CC64 and CC67 in software) | [Cited S10, S9] |
| `staccato` | Short notes; the manual: play "more or less Portato to produce Staccato" | Note length; immediate muting on note-off | [Cited S10] |
| `portato` (proposed) | as above | Note length | [Cited S10] |
| `damped` (proposed) | Mute slider down: dull, dry, quick decay | Apple: Damper slider via CC, velocity or aftertouch; Arturia: Mute bar mapped to a button | [Cited S10, S3, S9] |
| `legato` | Overlapping keys | Apple Voices "Legato" mode retriggers only pitch when notes overlap (mono) | [Cited S3] |
| `marcato`, `trill` | Velocity; fingering | Notes and velocity | [Estimate: keyboard practice; no source read lists these for the Clavinet] |

- Key-off click: Apple Click parameters (Intensity, Random, Velocity, release-velocity mode with "Auto" sensing of release velocity); Arturia Key Release Noise [Cited S3, S9].
- Modelled parameters: Apple Brilliance, Decay, String Release, Damping, Stiffness/Inharmonicity, Tension Mod, Pitch Fall, Excite Shape, 14 models (Belltone, Classic I and II, Dulcitone, Funktone, Harpsitone I and II, Mellotone, Plectratone I and II, Sitartone, Vintage I and II, Woodtone); Arturia Harmonic Profile, String Resonance, Release Time, Hammer Hardness, Hammer Noise, Pickup Noise, Dynamics [Cited S3, S9].

### 6. Solo vs section (typical number of players, divisi, unison doubling)

- One instrument and player; often doubled in octave or unison with bass guitar or electric guitar riffs in funk [Estimate: no source read describes doublings].
- Usually a single riff or comping part; a second keyboard covers sustained harmony because the Clavinet cannot sustain beyond the held key [Cited S10 for the sustain limit; Estimate for the arrangement practice].

### 7. Role in the orchestra and typical doublings

- "Synonymous with funk", also 1970s rock, pop and electric jazz (Stevie Wonder, Herbie Hancock, Keith Emerson, Foreigner, the Commodores) [Cited S3].
- "Superstition" (1972) main riff, Herbie Hancock *Head Hunters* (1973), Bill Withers "Use Me" [Cited S15]; "Higher Ground", "Outa-Space", "Still...You Turn Me On", "Trampled Under Foot" [Cited S3, S9].
- Hohner's manual pitches it also for Baroque and earlier keyboard music [Cited S10].
- Often played through a wah-wah pedal or auto-wah, especially in funk [Cited S15].

### 8. Variants and related instruments (when a player changes instrument)

| Variant | Fact | Provenance |
|---|---|---|
| Cembalet, Pianet (CH, N, T) | Hohner predecessors with reeds; Pianet T with suction pads | [Cited S3] |
| Clavinet I | 1964, built-in amplifier | [Cited S15, S3] |
| Clavinet II, L, C | II with tonal filters; L triangular (1968); C (1968) | [Cited S3, S15] |
| D6 | 1971, the most popular model | [Cited S15, S9] |
| Clavinet/Pianet Duo; E7 | 1978; 1979, "more roadworthy, quieter" | [Cited S15, S3] |
| Production span | seven models (S15; S9 says "a half a dozen versions"), 1964 to 1982 (S15; S9: "from 1964 until about 1982") | [Cited S15, S9] |

- Related instruments the player may switch to: harpsichord (GM 7), clavichord, Pianet, Wurlitzer [Cited S3, S18 for GM 7; Estimate for the switching practice].

### 9. General MIDI and existing Sonara metadata

| Item | Value | Provenance |
|---|---|---|
| GM1 program | 8, spelled "Clavi" in the MIDI Association table; Wikipedia writes "Clavinet" | [Cited S18, S16] |
| GM2 | program 8: bank 0 Clavinet, bank 1 Pulsed Clavinet (article's wording) | [Cited S17] |
| Sonara | no data.rs entry; gm.rs percussion only; "clavinet" and "clav" are not `family.rs` keywords | [Cited S25] |

### 10. Free or licensable sound sources and their coverage (library, licence as stated, what of this instrument it covers)

| Library | Licence as stated | Coverage | Provenance |
|---|---|---|---|
| (no free sampled Clavinet found) | n/a | VCSL has only a Yamaha TX81Z "Clavisynth" FM patch, not a Clavinet; VSCO 2 CE and FreePats have none | [Cited S22, S23, S24; Estimate: a wider search may find one] |
| MuseScore_General, FluidR3_GM | MIT, notices | GM 8, SF2, not played by the Sonara sampler | [Cited S25] |
| Apple Vintage Clav | Apple software licence | Modelled D6, full MIDI range (127 notes), effects | [Cited S3] |
| Arturia Clavinet V | commercial EULA | Modelled D6 extended to 88 keys (A to C), effects | [Cited S9] |

## Open questions and disagreements between sources

| Topic | Values found | Recommendation |
|---|---|---|
| Rhodes range model | 73-key E1-E7 (28-100) vs 88-key A0-C8 (21-108) [Cited S1, S12] | Store playable 21-108 and comfortable 28-100; the `keyboard` object follows the playable range (88 keys). Owner to confirm that "comfortable" may encode "fits the 73-key model" [Estimate: modelling choice; no source defines a comfortable Rhodes range] |
| Date of the 88-key Rhodes | 1971 [Cited S13], 1972 [Cited S2], 1973 [Cited S3] | Prefer 1971-72 (two sources, S2 a specialist site) [Estimate: judgement on source weight; two of the three sources put it in 1971-72] |
| Neoprene hammer tips | from 1969 [Cited S8]; early 1971 [Cited S2, S13] | Prefer early 1971 [Estimate: two agreeing sources] |
| Rhodes Mk II | 1978 [Cited S3]; late 1979 [Cited S13] | Unresolved; not stored |
| Suitcase / Stage naming | Suitcase name from 1969 [Cited S2]; Mk I Suitcase and Stage 1970 [Cited S13] | Not stored |
| Fender marketing | "marketed by guitar manufacturer Fender from 1956" [Cited S3]; Piano Bass 1959, Fender Rhodes after CBS 1965 [Cited S13] | Not stored |
| Rhodes tine wording | Apple calls the Rhodes generator "metal reeds" [Cited S3]; the service manual says tine and tone bar [Cited S1] | Use tine and tone bar (primary source) |
| Wurlitzer tremolo rate | 5.75 Hz [Cited S6, service manual]; 5.5 Hz [Cited S3]; about 6.34 Hz [Cited S7] | Use 5.75 Hz (manufacturer's manual); the rate is fixed, only depth is a control |
| Wurlitzer pickup voltage | 160-170 V DC [Cited S6]; 150 V [Cited S3]; 170 V [Cited S14] | Use 160-170 V (primary) |
| Wurlitzer 200A dates | 1974-1983 [Cited S14]; "production started in 1972" [Cited S7] | Unresolved |
| First Wurlitzer electronic piano | 1954 [Cited S14]; "first manufactured in the early 1960s" [Cited S3 p. 705, S5] | Prefer 1954 (S14 lists the 1950s models 100-120 by name; Apple's sentence is a marketing aside) [Estimate: judgement on source weight; no source settles the disagreement]; not stored |
| Apple's tremolo wording | Legacy Electric Piano "Tremolo ... (wobbling pitch)" [Cited S3 p. 712] vs Vintage Electric Piano tremolo = amplitude [Cited S3 p. 700] and S8 | Tremolo is amplitude modulation; the Rhodes and Wurlitzer "vibrato" labels are tremolo [Cited S8, S13, S14] |
| GM for the Wurlitzer | GM 5 "usually a Rhodes or Wurlitzer"; GM 6 "usually a Yamaha DX7 e. piano 1 preset" [Cited S16] | Map the Wurlitzer to GM 5, not 6 [Estimate: inferred from S16's descriptions of GM 5 and GM 6]; a GM synth may still use 6 for a Wurlitzer-like sound |
| GM 8 spelling | "Clavi" [Cited S18] vs "Clavinet" [Cited S16] | Use the spec's "Clavi" |
| D6 manual tab count | AB, CD plus "the other four registers", yet "One of the four tabs must be depressed" [Cited S10] | Four tone tabs plus two pickup tabs, as S9 and S15 describe [Cited S9, S15] |
| jRhodes licence | 3d `LICENSE` CC BY-NC 4.0; 3c `LICENSE` CC BY-NC-SA 4.0 linking the BY-NC deed; 3d `.sfz` header "CC-SA-BY" [Cited S20, S21] | Treat as non-commercial: user-installed only, never bundled (LIC-01 and the bundling rule in S25) |
| Sonara sampler and jRhodes3d | FLAC, note names and velocity layers are supported; crossfades, LFO tremolo, half pedal and curves would be ignored [Cited S25, S20] | Test a load and render before recommending it [Estimate: untested] |
| Catalog family | `family.rs` keywords lack "rhodes", "wurlitzer", "clavinet", "clav" [Cited S25] | Add them (or "epiano") so such patches are classed `keyboards`; a code change outside this research |
| Free Wurlitzer and Clavinet samples | Wurlitzer: Greg Sullivan's EP200 (sfzinstruments), CC BY 3.0 Unported per `LICENSE` and README but `NOASSERTION` in GitHub's repository record [Cited S31]; Clavinet: none found in VCSL, VSCO 2 CE, FreePats or the visible sfzinstruments page [Cited S22, S23, S24, S26] | Wurlitzer: settle the licence disagreement before relying on CC BY 3.0, keep the attribution, test a load [Estimate: design judgement; the licence records disagree and no load was tested]; Clavinet: search further (only 10 of 76 sfzinstruments repositories were visible) |
| Effects as part of the instrument | Tremolo, phaser, chorus, drive, wah and the Clavinet mute are controls, not articulation ids | A control vocabulary (effect state per track) may be needed beside the 15 articulation ids [Estimate: design judgement; no source defines such a control vocabulary] |
| 54-key Rhodes and 44-key Wurlitzer 106P ranges | key counts only [Cited S13, S14] | Not stored |

## Fact-check log

Adversarial fact-check of 2026-10-30, by a second researcher. Sources were reopened in this check
as follows. Raw text read (downloaded with curl and, for PDFs, extracted locally with PDFKit): S3
(Apple PDF, pp. 679-712), S6 (Wurlitzer service manual), S7 (Arturia Wurlitzer V), S8 (Arturia
Stage-73 V2), S9 (Arturia Clavinet V), S10 (clavinet.com D6 manual), S18 (MIDI Association GM1
page via the Wayback Machine), S19 (MediaWiki langlinks API), S20 (jRhodes3d `README.md`,
`LICENSE`, `jRhodes3d-mono.sfz`), S25 (repository files). Read through WebFetch page summaries
(quotes returned by the fetch tool, not the raw page): S1 Ch. 1, Ch. 4 and Ch. 6, S2, S5, S11,
S12, S13, S14, S15, S16, S17, S22 (`Electrophones` listing only).

### Numbers that matter: confirmed

| Instrument | Field | Value in card | Check | Verdict |
|---|---|---|---|---|
| electric_piano_rhodes | sounding_playable | [21, 108] | S1 Ch. 6: 88-key pianos are "Al through C88" (keys 1-88), 73-key "E8 through E80", the seven keys below E8 numbered 0. Key 1 = A0 = 21, key 88 = C8 = 108 (key + 20). S13: 88-note model 1971; S3: 1973, "entire 88-key range" | confirmed |
| electric_piano_rhodes | sounding_comfortable | [28, 100] | S1 Ch. 6 keys 8-80 = E1-E7; S12 "E1 to E7 (middle 'C' = C4)" for the Rhodes 73. 100 - 28 + 1 = 73 | numbers confirmed; provenance re-tagged as Estimate (see below) |
| electric_piano_rhodes | Piano Bass range | E1-B3 = 28-59, 32 keys | S13 "E1 to B3", 32 notes; 59 - 28 + 1 = 32 | confirmed |
| electric_piano_wurlitzer | sounding_playable | [33, 96] | S6 p. 1 and S7 p. 9: "64 Notes, A-13 thru c-76"; key 13 = A1 = 33, key 76 = C7 = 96; S14 and S7 p. 8: from the A an octave above the bottom of an 88-key piano to the C an octave below its top; S3 p. 705 "64 keys (A to C)" | confirmed |
| clavinet | sounding_playable | [29, 88] | S10: "contra F to e'"" (transcription) and "5 octaves (F1-e3)"; German/Helmholtz contra F = F1 = 29, e''' = e3 = E6 = 88; 88 - 29 + 1 = 60 = "60 piano keys"; S15 "60 keys ranging from F1 to E6"; S3 "60-key range (F to E)"; S9 "60 notes" | confirmed |
| all three | transposition, written range, clefs | 0; written = sounding; treble + bass | No source states a transposition; all three are tagged Estimate in the card | confirmed as Estimate |
| electric_piano_rhodes, electric_piano_wurlitzer | gm_program | 5 "Electric Piano 1" | S18 table: "5. Electric Piano 1 6. Electric Piano 2 7. Harpsichord 8. Clavi"; S16 "usually a Rhodes or Wurlitzer piano" | confirmed |
| clavinet | gm_program | 8 "Clavi" | S18 "8. Clavi"; S16 writes "Clavinet" | confirmed |
| all | GM2 banks, CC64 name | 5: banks 0-3; 8: banks 0-1; CC64 "Damper Pedal On/Off (Sustain)"; CC0 = 121 + CC32 | S17 | confirmed |
| all | keyboard keys / lowest / highest | 88/21/108; 64/33/96; 60/29/88 | consistent with the playable ranges | confirmed |

### Structure checks

- All three json blocks parse and have exactly the 20 required top-level keys in the required
  order, the six `names` keys, the two `range_provenance` keys, the four `typical_players` keys and
  the ten `keyboard` keys (checked with a script).
- Ids are the assigned ones (`electric_piano_rhodes`, `electric_piano_wurlitzer`, `clavinet`);
  family slug `keyboards` is valid and exists as `Family::Keyboards` in `instrument.rs`.
- Every id in `articulations_existing_vocab` is in the 15-id vocabulary of `articulation.rs`;
  every proposed id (`glissando`, `portato`, `damped`) is on the preferred-spelling list.
- Verification of data.rs: `data.rs` holds 16 instruments in the families Strings, Woodwinds,
  Brass and Percussion only, and none of the three ids; `existing_sonara_id: null` is correct and
  no "Verification of data.rs" table is required for this group.

### Other Cited claims spot-checked and confirmed

| Claim | Source | Result |
|---|---|---|
| 88 voices "allowing for glissandi over the entire keyboard range when the sustain pedal is depressed" | S3 p. 701 | confirmed |
| Vintage Electric Piano CC table 1, 12-28 (CC1 = "Volume knob") | S3 pp. 705-706 | confirmed |
| Wurlitzer tremolo 5.5 Hz mono, Stereo 0 degrees; Rhodes 180 degrees | S3 | confirmed |
| Wurlitzer "vibrato oscillator is preset to a frequency of 5.75 cycles per second"; pickups "160 to 170 volt DC"; 40 W; let-off 1/8 in; pedal "Lifts dampers ... as in a conventional piano"; 56 lbs less legs and pedal; reed fix 1/32-1/16 in | S6 | confirmed |
| Arturia Wurlitzer V: tremolo "approximately 6.34Hz", mono; volume pedal CC7, wah CC11; Rhodes "more of a solo instrument"; "sit more comfortably with an amplified guitar-based rock band"; retune 2 semitones; pickup axis "jumps one octave higher" | S7 | confirmed |
| Clavinet V: reserved CC64 and CC67; pickup table AC/BC/AD/BD; Mute bar likened to a guitarist's palm mute, two positions; Bass Guitar profile "lowest 1-1/2 octaves"; 88 keys "low A to the sparkly high C" | S9 | confirmed |
| Apple Vintage Clav: "MIDI controller 11 scales the output level" unless assigned to Wah or Damper; Legato voice mode; "second trigger when the key is released"; "Pitch Pressure" | S3 | confirmed |
| D6 manual: "Portato" to produce "Staccato"; slide "puts a damper on the strings and produces a dull, dry sound"; wool-wound part mutes on release; foot swell on "Output 100mV" | S10 | confirmed |
| Scarbee RSP '73: 12 dynamic levels, release samples at the same 12 levels, damper "thwong", "mid-range bark and low-end fart", GIGA / EXS24 MKII / HALION | S12 | confirmed |
| jRhodes3d: 1977 Mark I Stage 73, harp connector, treble boost and low-mid scoop, 5 layers, up to 25 s, keyswitches G7-C8; `.sfz` CC1/CC92-96 LFO controls, `ampeg_dynamic=1`, `ampeg_releasecc64=100`, samples F1-C7 mapped A0-C8, header "CC-SA-BY"; `LICENSE` samples CC BY-NC 4.0 | S20 | confirmed |
| Pickup colours (neck warm, bridge bright, both full, out of phase thin) | S11 | confirmed |
| Names in cs/it/de/fr | S19 | confirmed (all eight titles match) |
| VCSL `Electrophones` holds only `TX81Z` | S22 | confirmed (folder level; patch names not re-checked) |
| Sonara sampler capabilities; `family.rs` keyword list; FLAC in `sonara-media` | S25 | confirmed |

### Changes made (was -> now, why)

| Location | Was | Now | Why |
|---|---|---|---|
| electric_piano_rhodes json `range_provenance.comfortable` | "Cited S1, S12 for the span ... Using the 73-key span as 'comfortable' is an Estimate" | Leads with "Estimate: modelling choice ..."; the span stays Cited S1, S12 | No source states a comfortable range for the Rhodes; only the 73-key span is cited. The field's tag must be the Estimate |
| electric_piano_rhodes json `range_provenance.playable` | quote "A1 through C88" | adds that the fetched page renders "Al", read as A1 | S1 Ch. 6 as fetched renders "Al" (fenderrhodes.com refused curl, so only the fetch tool's rendering was seen) |
| clavinet json `keyboard.action` | "no escapement (S10, S3)" | "no escapement (Estimate: derived ...; neither S10 nor S3 uses the word)" | Neither source states it (grep of both texts) |
| Rhodes section 2, tine-pickup gap | "0.020 in (0.508 mm) is possible in the middle and upper ranges" | adds "in pianos built since March 1972" | S1 Ch. 4 qualifies the value by date |
| Rhodes section 2, dampers | "clearance 3/8-1/2 in at the moment the hammer strikes" | "when the key is depressed (maximum clearance as the hammer tip strikes the tine)" | S1 Ch. 4 wording |
| Rhodes section 4, sustain pedal | "the Stage pedal comes from a Rogers hi-hat stand" | "on the Stage the pushrod is 'part of a Rogers hi-hat stand' (S2)" | S2 says the pushrod, not the pedal |
| Clavinet section 3, Apple row | MIDI 29-88 [Cited S3] | "F-E (S3 gives no octaves)"; 29-88 only with the octaves of S10 and S15 | S3 names letters and a key count, not octaves |
| Clavinet section 8, production span | "seven models, 1964 to 1982 [Cited S15, S9]" | seven models (S15) vs "a half a dozen versions" (S9); 1964 to 1982 (S15) / "until about 1982" (S9) | S9 does not say seven |
| Open questions | (none) | new row: first Wurlitzer 1954 (S14) vs "early 1960s" (S3 p. 705, S5) | Disagreement found in this check |

No json range, transposition, GM number or articulation id was changed: none was wrong.

### Downgraded to Estimate

- Clavinet "no escapement" (json `keyboard.action`).
- Rhodes comfortable range: the field's tag (the cited span is unchanged).

### Not re-checked (left as the first researcher tagged them)

S14 artist and model list (Ray Charles 120, Zawinul 140B, Norah Jones 206) and the Model 120
external-amp tremolo; S7's four-way classification of electric pianos; S22 TX81Z patch names;
S23 VSCO 2 CE `Keys` listing; S24 FreePats "Synthesized Piano"; S21 jRhodes3c licence; S2's and
S13's Suitcase amplifier and model-date details beyond those named above.

### Remaining concerns

- Clavinet `pedals: ["foot_swell"]`, `pedal_ccs: {"foot_swell": 11}` is outside the
  sustain / una_corda / sostenuto set the format names. It is correct for the D6 (no sustain
  pedal; S10, S3) but a parser with a closed pedal list may reject it.
- `damped` for the Clavinet mute slider is a control position more than an articulation; S9's own
  comparison suggests `palm_mute` as an alternative spelling. Decide one for the whole catalogue.
- The Rhodes card stores the 88-key range as playable while most instruments, samples and
  models are 73-key; the "comfortable = 73-key span" convention needs the owner's confirmation.
- The WebFetch-summarised sources (S1, S2, S5, S11-S17) were checked through quotes returned by
  the fetch tool, not the raw page text.

## Consistency review

_Consistency review of 2026-10-30. No source was re-researched; no json value changed._

- Checked all three json blocks against sections 3 and 9 and the fact-check log: ranges, transposition, written range
  (sounding minus 0), clefs, GM programs (5, 5, 8) and every logged correction agree in json and prose; validator clean.
- S25 row: replaced the absolute local path in the URL column with "local checkout of the Sonara repository (branch
  `research/orchestra-instruments`)".
- Rhodes section 1: paraphrased a 22-word Apple (S3) quote on adjustment versus model; tag kept.
- Wurlitzer section 2, Pickup row: paraphrased a 16-word service-manual (S6) quote; the 160-170 V DC figure kept.
- Wurlitzer section 3 table: "A-13 thru C-76" corrected to "A-13 thru c-76", as printed in S6 and quoted in the json
  and the fact-check log.
- Wurlitzer section 6: paraphrased a 16-word Arturia (S7) quote on the rock-band fit.
- Clavinet section 2, Damping row: paraphrased a 20-word Hohner manual (S10) quote.
- Clavinet section 4, Touch row: paraphrased a 22-word Hohner manual (S10) quote.
- Left as is: quotes of 15 words or fewer (S6 "Hammers strike Swedish Sandvik steel reeds ..." at exactly 15; S3
  "designed to replicate the sound of a clavichord ..." and the S3 glissando quote at 14).
- Tags: every Sn cited exists in the Sources table; S27-S30 (not opened) are never cited; no foreign tags, local
  paths, temporary-file references or attribution lines remain.
- Wurlitzer section 10 and the open question on free samples: "no free SFZ found" contradicted the free-libraries
  card, which lists Greg Sullivan's Wurlitzer EP200. New source S31 (opened for this change); the row gives the
  licence caveat (CC BY 3.0 Unported in `LICENSE` and README, `NOASSERTION` in GitHub's record), the patch's
  coverage and the opcodes Sonara supports or ignores (`sfz.rs`, S25). VCSL, VSCO 2 CE and FreePats still hold no
  Wurlitzer; the Clavinet gap is unchanged.
- Gave every bare Estimate tag its reason (11 tags).
- Added a "What a composer writes differently" block at the end of section 4 for the Rhodes piano, the Wurlitzer
  electronic piano and the Clavinet, from the card's existing sources (no new source).
