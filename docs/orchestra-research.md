# Orchestra and keyboard instruments: research summary

_Date: 2026-10-30 · Status: research complete · Design: [orchestra-design.md](orchestra-design.md) and
[ADR 0011](decisions/0011-orchestral-instruments-and-performance.md) (both proposed) · Detail:
[docs/orchestra/](orchestra/README.md)_

**Reading guide.** Every fact in the research records is tagged **Cited** (a source that was opened states it; the
record's Sources table names it) or **Estimate** (with the reason). The records hold 7,289 Cited and 993 Estimate tags.
**Nobody listened to any audio** for this research, and nothing was rendered: what Sonara's sampler does with a
library comes from reading its code ([sfz-conformance.md](orchestra/sfz-conformance.md)). Wikipedia was used only as a
secondary source for numbers, and its text is paraphrased (it is CC BY-SA; the repository is 0BSD).

## 1. Question and method

Sonara should not only let you write MIDI but compose: when a user or an agent says "the string section plays a legato
melody, horns and timpani underneath", Sonara should know what those instruments are, their range, how they are played,
how they sound and how they are controlled. GarageBand and Logic Pro are the reference. This research maps the whole
orchestra and the keyboard instruments as data and as practice, and checks what Sonara can voice today.

1. **Instrument cards.** One researcher per group (15 groups) filled the same ten fields for every instrument: names
   (English, Czech, Italian, German, French, score abbreviation), sound production, sounding and written range with
   transposition and clef, dynamics and colour by register, articulations and how sample libraries realize them, solo and
   section, role and doublings, variants, General MIDI and existing Sonara metadata, and free sound sources. Each card
   has a machine-readable json block per instrument.
2. **Adversarial fact-check.** A second agent per card reopened the cited sources, re-did every note-name conversion
   (C4 = 60), corrected wrong values and downgraded unverifiable claims to Estimate. The records of free libraries and of
   Apple's products got the same pass, licences first.
3. **Synthesis.** Score order, seating and ensemble templates; doubling and balance as checkable rules; the mapping of
   instruments and articulations to library presets with a degradation chain; coverage and gaps; a harmonized
   articulation vocabulary; a code-level check of Sonara's SFZ parser and sampler against the libraries.
4. **Two review rounds and a triage.** Cross-file consistency (json against prose, ids, tags, quote length, local paths),
   then every open question deduplicated and classified ([open-questions.md](orchestra/open-questions.md)).

Primary sources were preferred: the public-domain handbooks of Rimsky-Korsakov (*Principles of Orchestration*),
Forsyth (*Orchestration*, 1914), Widor (*The Technique of the Modern Orchestra*, 1906) and Berlioz/Strauss; the Vienna
Symphonic Library's instrument pages; manufacturers; the manuals and file lists of sample libraries; the MIDI
Association's General MIDI tables; Apple's Logic Pro and GarageBand user guides. Copyrighted handbooks (Adler, Blatter,
Piston) were not readable and are never cited.

## 2. What the orchestra contains

85 instrument ids in seven families ([model.md](orchestra/model.md) has every range, transposition, clef, GM program and
articulation list; the cards in [orchestra/cards/](orchestra/cards/) have the detail):

| Family | Ids |
|---|---|
| Woodwinds (17) | piccolo, flute, alto_flute, bass_flute, oboe, english_horn, oboe_damore, bassoon, contrabassoon, clarinet_bb, clarinet_a, clarinet_eb, bass_clarinet, saxophone_soprano, saxophone_alto, saxophone_tenor, saxophone_baritone |
| Brass (13) | horn_f (covers the double horn), wagner_tuba_tenor, wagner_tuba_bass, trumpet_bb, trumpet_c, trumpet_piccolo, cornet, flugelhorn, trombone_tenor, trombone_bass, tuba, cimbasso, euphonium |
| Percussion (22) | timpani, xylophone, marimba, vibraphone, glockenspiel, tubular_bells, crotales, carillon; snare_drum, bass_drum, clash_cymbals, suspended_cymbal, tam_tam (covers the gong), triangle, tambourine, castanets, wood_block, temple_blocks, sleigh_bells, maracas, shaker, whip |
| Keyboards (15) | piano_concert_grand, piano_studio_grand, piano_upright, piano_felt, piano_prepared, piano_honky_tonk, electric_piano_rhodes, electric_piano_wurlitzer, clavinet, harpsichord, clavichord, celesta, organ_pipe, organ_hammond, accordion |
| Harp and plucked (4) | harp, guitar_classical, lute, mandolin |
| Voices (10) | choir_soprano, choir_alto, choir_tenor, choir_bass, voice_soprano, voice_mezzo_soprano, voice_alto, voice_tenor, voice_baritone, voice_bass |
| Bowed strings (4) | violin (Violin I and II are parts of one instrument type), viola, cello, double_bass |

Ids name the acoustic instrument type, never a sample preset or a part: "Violin I", "Flute 3 doubling piccolo" and
"Horns 1-4" are parts in an ensemble ([ensembles.md](orchestra/ensembles.md)). Solo and section are properties of a part,
with the typical numbers of players per era on each card.

**Keyboards are distinct instruments, not presets of one piano.** The cards explain what makes each different: the
action (grand double escapement against upright action), hammers and felt (the felt piano is a moderator felt between
hammers and strings), dampers and the three pedals with their MIDI controllers (sustain CC 64 with half-pedalling, una
corda CC 67, which shifts the action on a grand but moves the hammers closer on an upright, sostenuto CC 66), string
and sympathetic resonance, release, pedal and key noise, the two-stage decay, dynamic range; the tines and tonebars of
the Rhodes, the reeds and electrostatic pickup of the Wurlitzer, the struck strings and tangents of the Clavinet; the
plucked jacks and registers of the harpsichord (no velocity dynamics), the clavichord's bebung, the celesta's octave
transposition, organ registration and swell, Hammond drawbars and the Leslie, and the accordion's bellows as
expression ([acoustic_pianos](orchestra/cards/acoustic_pianos.md), [electric_keys](orchestra/cards/electric_keys.md),
[historical_keys_organs](orchestra/cards/historical_keys_organs.md)). Registrations, drawbars, pedals and the Leslie
speed are instrument **controls**, not articulations.

## 3. The existing metadata, verified

`crates/sonara-midi/src/orchestration/data.rs` holds 16 instruments today. The research confirms every transposition
and changes a range or adds a comfortable range for 14 of them (21 values); the table is in
[model.md, "Proposed corrections to data.rs"](orchestra/model.md#proposed-corrections-to-datars). The largest changes:
violin playable top A7 (105) instead of C8, with a comfortable top of A6 (93); cello C2–E6 instead of the estimated
C2–A6; horn in F now cited (Forsyth: written F#2–C6); trumpet in Bb playable to G6; glockenspiel C5–C8 and xylophone
C4–C8 (both were estimates). The articulation lists, all estimates today, are now sourced on the cards, except a few
ids that stay marked as estimates (for example the trumpet's tenuto). The viola, timpani,
harp and tuba are missing from data.rs today; nothing in data.rs is changed by this research.

## 4. GarageBand and Logic Pro

[apple.md](orchestra/cards/apple.md) records how Apple structures and performs instruments. The patterns that matter for
Sonara: a Library organized by category and instrument with patches that bundle instrument, effects and Smart Controls;
one **articulation ID per note** (or none), defined per track by an Articulation Set whose output sends key switches,
controllers, program changes or a channel, so one input layout drives different libraries; modelled keyboards with
their mechanics exposed as parameters (Studio Piano, Vintage Electric Piano, Vintage Clav, Vintage B3); Studio Strings
and Studio Horns with articulation sets and ensemble sizes; and performance logic that turns a few choices into music
(Drummer, Session Players following the Chord Track).

## 5. Sound sources and licences

[free_libraries.md](orchestra/cards/free_libraries.md) checked every candidate against its licence text. Under ADR 0010
**nothing is bundled**; users install libraries. The core free set: **VSCO 2 Community Edition** (CC0; its readme asks for
credit and no direct sale; some keyboards were sampled by Simon Dalzell under a redistribution grant), **VCSL** (CC0;
the SFZ files exist only in the tag `v1.2.2-RC`), and **Sonatina Symphonic Orchestra** (CC Sampling Plus 1.0, a retired
licence that requires attribution; it includes Philharmonia-derived samples whose terms exclude sampler instruments).
Others: Salamander Grand Piano (CC BY 3.0), Karoryfer's CC0 sets (for example War Tuba and Meatbass), MTG Solo Sax
(CC BY 4.0), FreePats sets with mixed licences, Keppy's Steinway (CC BY-SA 4.0, share-alike), jRhodes3d (CC BY-NC 4.0),
and the MIT-licensed General MIDI SoundFonts MuseScore_General and FluidR3 (SF2, which Sonara does not play).

## 6. Sonara's sampler against real libraries

[sfz-conformance.md](orchestra/sfz-conformance.md) checked, line by line in `crates/sonara-media/src/sfz.rs` and
`crates/sonara-dsp/src/processors/sampler.rs`, how the sampler reads the SFZ files of these libraries. Eleven
divergences from the SFZ specification are confirmed (Q1–Q11) and six more were found (N1–N6). Their consequences:

- key-switch settings are read only from the scopes open at the end of a file, and a `<control>` header clears the
  enclosing `<global>` (Q1, Q11): three VSCO key-switch patches are silent until a switch key is played, and most Sonatina
  key-switch patches stack their articulations;
- `#include` mid-line, `$define`s in include paths and nested include directories are mishandled (Q2–Q4): the
  Salamander remap, Splendid, Sonatina's grand piano and the MTG saxophones do not play, so **no free grand piano plays
  as is**;
- `trigger=first`/`legato` become plain attacks and the old voice is choked after 10 ms (Q5, Q9): **legato patches play
  as a retriggered sustain**;
- velocity and CC crossfades are ignored (Q6), release samples do not play for keys released under the sustain pedal
  (N1), and one missing sample fails a whole patch (N3).

Each item has a suggested fix and the regression test that would pin it (TST-06).

## 7. Coverage and gaps

[coverage.md](orchestra/coverage.md): 49 of the 85 ids have a free file known to play; 36 do not. The product sentence,
part by part:

| Part | Today |
|---|---|
| Horns underneath | VSCO solo horn sustains play as is; Sonatina's horn section plays partly; no swell (no CC crossfades) |
| Timpani underneath | VSCO and VCSL hits and rolls play as is; roll crescendos need dynamics |
| Strings play a legato melody | **ENGINE gap**: legato patches retrigger (Q5, Q9). **SOUND gap**: no free library records legato transitions; Sonatina's legato reuses its sustain samples |

The ranked gaps after legato: dynamics (CC crossfades in the engine, and no agent tool writes MIDI CC), key-switch
parsing, preset-to-instrument metadata, the grand piano, section against solo, articulation delivery, layer fidelity,
choir, electric and historical keyboards, colour techniques, a GM fallback, saxophones, library browsing, and instruments
with no free sound at all (for example oboe d'amore, cimbasso, flugelhorn, clavinet, clavichord, the solo voices).
Recorded legato transitions and the top of the quality range need commercial libraries through plugin hosting (M5).

## 8. The orchestra as a whole

- [ensembles.md](orchestra/ensembles.md): score order and part naming (Rimsky-Korsakov's conventions), German and
  American seating and what they mean for default stereo placement (libraries recorded in their seating positions keep
  pan 0), and ten ensemble templates from the Classical orchestra to the film orchestra, big band, choir and orchestra and
  chamber groups, each part with its id, players and doublings. The existing `orchestral` project template is covered by
  golden data (the "Ascension" example, its golden workflow and a render-load baseline) and must stay unchanged (GLD-01).
- [doubling-and-balance.md](orchestra/doubling-and-balance.md): functions by family and register, doubling tables,
  Rimsky-Korsakov's balance of tone (in forte one trumpet, trombone or tuba equals two horns, and one horn two
  woodwinds) with a derived weight table, low-interval limits, voicing per family, and **67 checkable rules** with
  inputs, severity and source for an arranging layer and the AI Composer.
- [articulation-vocabulary.md](orchestra/articulation-vocabulary.md): the cards use 96 articulation ids (15 existing),
  harmonized with a kind and definition each, and the modelling options (a flat id per note, or states such as mute and
  vibrato).

## 9. Open questions

[open-questions.md](orchestra/open-questions.md) holds 170 items: 25 owner decisions (A1–A25) with options and
recommendations, 8 checks only a person can make (renders, listening, printed scores), 126 disagreements between sources
with the value adopted, and 11 research follow-ups. The owner decisions that shape the design are where an instrument's
identity lives in a project (A1), how articulations and dynamics reach a library (A3, A4), preset ids in the sampler
(A5), library mapping data (A6), template exposure (A7), the SFZ fixes and sampler features (A8, A9), a GM fallback
(A10) and the articulation vocabulary (A12–A16). The design takes a position on each in
[orchestra-design.md](orchestra-design.md).

## 10. Limits

- Nothing was listened to or rendered; sound quality is not judged anywhere.
- The sampler verdicts come from reading the code; a render per divergence would confirm them (open question B1).
- Some sources could not be opened (HTTP errors, paywalled handbooks); claims resting on them are Estimates.
- Numeric balance weights, low-interval limits and default pan values are Estimates derived from cited ratios.
- Several libraries were not inspected file by file (VPO3, most FreePats sets); their cells say so.

## 11. Files

| File | Content |
|---|---|
| [orchestra/model.md](orchestra/model.md) | The 85 ids in one table per family, keyboard mechanics, proposed data.rs corrections (generated from the cards) |
| [orchestra/cards/](orchestra/cards/) | 15 instrument cards, the free-library record and the Apple record |
| [orchestra/ensembles.md](orchestra/ensembles.md) | Score order, seating, ensemble templates |
| [orchestra/doubling-and-balance.md](orchestra/doubling-and-balance.md) | Doubling, balance, voicing and 67 rules |
| [orchestra/sound-mapping.md](orchestra/sound-mapping.md) | Library presets per instrument and articulation, degradation chain, GM table |
| [orchestra/sfz-conformance.md](orchestra/sfz-conformance.md) | Sonara's SFZ parser and sampler against the specification and the libraries |
| [orchestra/coverage.md](orchestra/coverage.md) | Coverage matrices, ranked gaps, starter kit |
| [orchestra/articulation-vocabulary.md](orchestra/articulation-vocabulary.md) | Canonical articulation ids and modelling options |
| [orchestra/open-questions.md](orchestra/open-questions.md) | Triaged open questions |
