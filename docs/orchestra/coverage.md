# Sound coverage and gaps

_Synthesized 2026-10-30 from the research cards in [cards/](cards/), the generated model in
[model.md](model.md) and the sound mapping in [sound-mapping.md](sound-mapping.md); code claims
re-checked against [sfz-conformance.md](sfz-conformance.md). Status: research; nothing here is
implemented._

## Sources

| # | Source | URL | Kind | Opened in this session |
|---|---|---|---|---|
| S1 | Sonatina Symphonic Orchestra, GitHub REST tree of `master`, recursive: 747 `.sfz` (557 outside `includes/`), patch names and the folder names under `Samples/` | https://api.github.com/repos/peastman/sso/git/trees/master?recursive=1 | library maker | yes |
| S2 | VSCO 2 Community Edition, GitHub REST tree of branch `SFZ`, recursive: 75 `.sfz` | https://api.github.com/repos/sgossner/VSCO-2-CE/git/trees/SFZ?recursive=1 | library maker | yes |
| S3 | VCSL, GitHub REST tree of tag `v1.2.2-RC`, recursive: 183 `.sfz` | https://api.github.com/repos/sgossner/VCSL/git/trees/v1.2.2-RC?recursive=1 | library maker | yes |
| S4 | N. Rimsky-Korsakov, _Principles of Orchestration_, trans. E. Agate, Project Gutenberg eBook 33900, plain text. Read: the introduction on the string group, Chapter I "Kettle-drums", the comparison of resonance between the groups, and the passage on horns and bassoons in harmony | https://www.gutenberg.org/cache/epub/33900/pg33900.txt | handbook (public domain) | yes |
| S5 | Musical Artifacts JSON API: the `tags=choir` listing, the `tags=choir&formats=sfz` listing, and artifact 387 ("KBH Real and Swelling Choirs") | https://musical-artifacts.com/artifacts.json?tags=choir ; https://musical-artifacts.com/artifacts/387.json | other (aggregator) | yes |
| S6 | Sonara repository, read only, branch `research/orchestra-instruments` @ dc96c29 (its `crates/` and `apps/` are identical to `main` @ a212544): `crates/sonara-midi/src/orchestration/articulation.rs` (the 15 ids), `crates/sonara-media/src/sfz.rs` (`SUPPORTED`, `HARMLESS`), `crates/sonara-dsp/src/processors/sampler.rs` (`control_change`: CC 7, 11, 64), `crates/sonara-core/src/plugins/builtin.rs` (Poly Synth and Drum Kit presets, `sampler_descriptor`), `crates/sonara-agent/src/contract/args.rs` (`NoteInput`, no CC argument), `docs/agent-api.md` (57 tools; `create_automation` targets `<device_id>:<param>`), `docs/decisions/0010-sample-based-orchestral-instruments.md` | this repository, branch `research/orchestra-instruments` @ dc96c29 | other | yes |
| S7 | Mapping instruments to sound, [sound-mapping.md](sound-mapping.md): per-file verdicts, the parser and sampler behaviours Q1-Q11 and N1-N6, the articulation map fields, the degradation chain, the substitution and GM tables | [sound-mapping.md](sound-mapping.md) | other (synthesis in this research) | yes |
| S8 | sfzformat.com, opcode `trigger` | https://sfzformat.com/opcodes/trigger | spec | yes |
| S9 | sfzformat.com, opcode `xfin_loccN` | https://sfzformat.com/opcodes/xfin_loccN | spec | yes |
| S10 | Orchestral instrument model, [model.md](model.md), generated from the cards: the 85 ids, family, sounding ranges, GM program, solo or section per id | [model.md](model.md) | other (research data) | yes |
| S11 | Sonara repository, further paths on the same branch as S6: `crates/sonara-midi/src/orchestration/` (instrument model: no GM program, no solo or section), `crates/sonara-app/src/instruments/` and `docs/instrument-catalog.md` (catalog, manifest v1 fields, "Not done"), `crates/sonara-project/schemas/project-v1.schema.json` (no articulation, part or ensemble field), `apps/sonara-desktop/src/lib/pianoroll/` (velocity lane, no CC lane), `.claude/rules/project.md` (CTR-01, LIC-01) | this repository, branch `research/orchestra-instruments` @ dc96c29 | other | yes |
| S12 | SFZ conformance of Sonara's sampler, [sfz-conformance.md](sfz-conformance.md): a second, independent reading of the source that confirms Q1-Q11, corrects C4, C11, C22 and C24, adds N1-N7, and traces one note-on of a Sonatina legato file | [sfz-conformance.md](sfz-conformance.md) | other (verification) | yes |

Two web searches were also run: one for a free SFZ choir with separate voice parts and one for a free
SFZ clavinet. They returned only forum and shop leads. None of those pages was opened, so none is used
as a fact here.

### How to read this file

- **Tags.** `[Cited Sn]` is a row above. `[Card cards/<file>.md#<id>]` is an instrument or library
  card in [cards/](cards/), read for this research (its own sources are listed in that card).
  `[Estimate: reason]` is this author's inference.
- **Verdict marks come from [sound-mapping.md](sound-mapping.md).** It derived every play verdict by
  reading Sonara's parser and sampler, and a second, independent reading of the source confirmed
  every behaviour those verdicts rest on [Cited S7, S12]. No library was loaded, and nothing was
  rendered or listened to. So every verdict mark below is a code-reading result, not a test result.
  - no mark: the patch plays as is in `sonara.sampler`;
  - `†`: it plays partly (some layers, controls or behaviours are lost or wrong);
  - `‡`: it does not play, or it plays the wrong thing (fails to load, plays only noises, or stacks
    articulations);
  - `?`: the file was not opened by this research, so the verdict is unknown.
- **Q- and N-numbers** are the parser and sampler behaviours that differ from the SFZ spec
  [Cited S7 §2.0, S12]. The ones that matter most here:
  - Q1: key-switch settings are read once, at the end of the file, and any later header at the
    same or an outer scope level clears them, so a patch can stay silent until a switch key is
    played;
  - Q11: a `<control>` header clears all four scopes, `<global>` included, so the articulations
    included after it lose their switch and sound under every switch;
  - Q2, Q3 and Q4: `#include` forms the parser does not resolve (an include in mid-line, a
    `$define` in the include path, a nested include resolved against the wrong directory);
  - Q5: `trigger=first`, `trigger=legato` and `trigger=release_key` all fire as plain attacks,
    silently: they never appear in the loader's ignored-opcode warning;
  - Q6: velocity crossfades (`xfin_*`, `xfout_*`) are ignored, so crossfaded layers both sound,
    each at its normal velocity gain;
  - Q7: CC gates (`locc`, `hicc`) are ignored, and a key of `-1` falls back to the full keyboard;
  - Q8: a non-default `amp_veltrack` is ignored. Sonara's fixed velocity curve equals the spec's
    default, so only regions such as organs with `amp_veltrack=0` diverge;
  - Q9: filters, decay envelopes, LFOs and `off_mode`/`off_time` are ignored, and every choke takes
    a fixed 10 ms;
  - N1: a key released while the sustain pedal is down never starts its release samples;
  - N3: one sample that cannot be opened fails the whole patch, generator samples such as `*sine`
    included;
  - N6: loop points stored only inside a sample file are ignored.
- **S / E.** S = solo instrument. E = ensemble, meaning a section recording.
- **Articulations** use only the 15 ids of `articulation.rs`: sustain, legato, staccato, spiccato,
  tenuto, marcato, pizzicato, tremolo, trill, col_legno, harmonics, sul_ponticello, con_sordino,
  flutter_tongue, roll [Cited S6]. A library label with no matching id appears in quotes. The cards
  propose further ids (`non_vibrato`, `detache` and others) [Card cards/strings.md#violin], which
  [articulation-vocabulary.md](articulation-vocabulary.md) collects. This file does not use them.
- **Ranges** are the sounding playable ranges of [model.md](model.md) [Cited S10], quoted only
  where an argument needs one.

## Summary

- **The product sentence is two-thirds covered by free sound.** "Strings play a legato melody, horns
  and timpani underneath":
  - Horns: free sustains exist, solo in VSCO 2 CE (as is) and section in Sonatina (`†`)
    [Cited S1, S2, S7].
  - Timpani: free hits and rolls exist in three libraries. VSCO's and VCSL's separate hit and roll
    patches play as is [Cited S1-S3, S7].
  - String legato: free legato files for a string section exist only in Sonatina; VPO3's documented
    articulations name no legato [Card cards/free_libraries.md#virtual_playing_orchestra_3]. The gap
    is both an ENGINE gap and a SOUND gap:
    - **ENGINE.** Sonara cannot play even the overlap legato these files describe. It turns
      `trigger=first` and `trigger=legato` into plain attacks without a warning (Q5), and it cuts
      the files' 1 s overlap to a 10 ms choke (Q9). Every note restarts one monophonic sustain voice
      [Cited S7 §2.0; Cited S12: trace of one note-on].
    - **SOUND.** Both legato groups of a Sonatina legato file include the same sustain fragment
      (`includes/1st-violins-sustain.sfz` in `1st Violins Legato`). A spec-correct engine would
      therefore give an overlap crossfade between sustain samples, not a recorded transition
      [Cited S12; Card cards/free_libraries.md#sonatina_symphonic_orchestra].
      No free library inspected records legato transitions. The Bigcat cello, the MTG saxophones
      and the Ixox flute declare legato regions, but their samples were not traced, and none of
      them is a string section [Cited S7 §2.5].

    This is the one hard blocker.
- **Coverage.** 48 of the 85 ids have a free SFZ source in VSCO 2 CE, VCSL or Sonatina, and 46 of
  them have a file that plays as is or partly [Estimate: count from section 1; the 48 include the
  mixed chorus counted once per choir part, and Sonatina's crotales and castanets, whose files this
  research did not open].
  - Other free SFZ sets add 10 ids: guitar_classical, mandolin, piano_honky_tonk,
    electric_piano_rhodes, electric_piano_wurlitzer, organ_hammond, accordion, saxophone_alto
    (Karoryfer Weresax), saxophone_baritone (Karoryfer Bear Sax) and maracas (FreePats World
    Percussion). Only the mandolin, Rhodes and Wurlitzer files were read; the other seven are not
    inspected. These sets also give second sources for the double bass, cello, tuba, tenor
    saxophone, upright piano and several percussion ids [Card cards/free_libraries.md#freepats,
    #karoryfer_samples; Cited S7 §2.5].
  - saxophone_soprano has only VCSL's saxello as a substitute (not opened) and the MTG soprano,
    which fails to load (Q4).
  - piano_concert_grand has no free file that plays: Sonatina's and Splendid's fail to load (Q3),
    and Keppy's Steinway (CC BY-SA 4.0) was not opened.
  - 25 ids have no free SFZ file in any form (gap rank 15).
  - So 49 ids have a free file known to play, and 36 do not [Estimate: count from section 1].
- **Most gaps are not sound gaps.** The four highest-ranked gaps are:
  - legato: an ENGINE gap (Q5, Q9) and a SOUND gap (no recorded transitions found free);
  - dynamics: CC crossfades, plus no tool that writes CC;
  - the key-switch parser (Q1, Q11);
  - preset-to-instrument metadata, a DATA gap.

  The free samples already exist for all of them except recorded legato transitions. A grand piano
  that plays as is is also missing, because of parser and sampler gaps Q2, Q3, Q7 and N1 (gap
  rank 5).
- **Starter kit.** VSCO 2 CE, the VCSL SFZ build and Sonatina's Notation files cover the core
  orchestra with three user installs (section 4). ADR 0010 means Sonara documents them and bundles
  nothing [Cited S6]. VSCO and VCSL are CC0; Sonatina's licence requires attribution, and Sonatina
  contains Philharmonia samples [Card cards/free_libraries.md#sonatina_symphonic_orchestra].

## 1. Coverage matrix

Column sources, unless a row's Basis cell says otherwise:

- **VSCO 2 CE, VCSL, Sonatina, Salamander.** The patch names are [Cited S1-S3]. The articulation
  content and the verdicts of the files read are [Cited S7 §2.1-2.4]. The verdict marks themselves
  are code reading, confirmed by a second reading [Cited S12].
  - **Sonatina** lists the Notation single-articulation files only. Its Performance set needs CC1
    for the volume of long notes, so it plays only partly [Cited S7 §2.2]. Its key-switch files
    stack articulations (Q11), except `Clarinet Solo KS` [Cited S7 §2.2].
  - Every Sonatina Notation file is `†`: filters, decay envelopes and vibrato are ignored (Q9), and
    `legato` is always a retriggered sustain (Q5, Q9) [Cited S7 §2.0, §2.2].
- **Other free SFZ** [Card cards/free_libraries.md] and [Cited S7 §2.5]. Licences are given as the
  fact-checked free-libraries card states them. VPO3 is listed as "VPO3 ?" where the VPO pages
  list the instrument. Its files were not opened, and its custom terms cover CC BY-SA and Sampling
  Plus parts and Philharmonia samples [Card cards/free_libraries.md#virtual_playing_orchestra_3].
- **GM fallback** is the 1-based GM program, or channel-10 notes [Cited S7 §5, S10]. It is **not
  playable today**: Sonara has no SF2 engine, because ADR 0010 deferred `rustysynth` [Cited S6]. A
  user-installed GM SoundFont would voice it: MuseScore_General or FluidR3_GM, both MIT, and
  MuseScore_General's notices must travel with any derivative
  [Card cards/free_libraries.md#musescore_general_and_ms_basic, #fluidr3_gm]. "≈" marks an
  approximate or weak equivalent, and "(card)" a fallback that the id's card names.
- **Built-in** is the Poly Synth preset, or the Drum Kit note or kit, chosen as the last resort
  [Estimate: timbre judgement, Cited S7 §4.3]. That the presets exist is [Cited S6].
- **Commercial (M5)** is a product named in the row's card, playable only after plugin hosting
  (milestone M5) [Cited S6: ADR 0010 decision 6]. BBCSO-D = BBC Symphony Orchestra Discover. It costs
  nothing but is a proprietary plugin, and its page list is truncated
  [Card cards/free_libraries.md#bbc_so_discover]. A blank cell means the card named no product.
  Apple's Logic instruments are not listed, because they run only inside Logic
  [Card cards/historical_keys_organs.md#organ_hammond].

### 1.1 Strings and plucked

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| violin | S and E: sustain, tremolo, spiccato, pizzicato (single files); `SViolin-KS`; `ViolinEns-KS` † (silent until a switch key, Q1) | | E (parts `1st Violins`, `2nd Violins`): sustain, legato†, marcato, staccato, pizzicato, col_legno, tremolo, harmonics. S (`Violin Solo 1`, `2`): sustain, legato†, marcato, spiccato, staccato, pizzicato, harmonics; tremolo and non-vibrato variants on Solo 2 | | VPO3 ? (S and E) | 41 Violin; E 49; tremolo 45; pizzicato 46 | Poly Synth "Strings" | Spitfire Symphony Orchestra; BBCSO-D | [Card cards/strings.md#violin]; VCSL has no bowed strings [Cited S3] |
| viola | E: sustain, tremolo, spiccato, pizzicato (`ViolaEns-KS` as is) | | E (`Violas`): as violin E. S (`Viola Solo`): sustain, legato†, marcato, staccato, pizzicato, harmonics | | VPO3 ? (S and E) | 42 Viola | "Strings" | Spitfire Symphony Orchestra; BBCSO-D | [Card cards/strings.md#viola] |
| cello | E: sustain, tremolo, spiccato, pizzicato (single files); `CelloEns-KS` plays, but its switch keys 84-87 lie inside the cello's range (36-88), so a part that reaches C6 needs the single files | | E (`Celli`): as violin E. S (`Cello Solo`): sustain, legato†, marcato, staccato, pizzicato, harmonics | | Bigcat Cello (CC0 per GitHub metadata and the catalogue; no `LICENSE` file found), S: sustain, legato, pizzicato † (CC107 voice layers stack, Q7; Q5; Q9); 36 single programs ?. VPO3 ? (S and E) | 43 Cello | "Strings" | Spitfire Symphony Orchestra; BBCSO-D | [Card cards/strings.md#cello]; [Cited S7 §2.1, S10] |
| double_bass | S: sustain (vibrato and non-vibrato), tremolo, spiccato, pizzicato (`Contrabass-KS` as is) | | E (`Basses`): sustain, legato†, marcato, staccato, pizzicato, col_legno, tremolo. S (`Bass Solo`): sustain, legato†, marcato, staccato, pizzicato, harmonics | | D. Smolken (CC0 per GitHub metadata; no `LICENSE` file found), S: arco, pizzicato, key-switch file † (`amp_velcurve_*` and the bend range ignored); Meatbass (CC0) ?; Sneakybass (CC0, quiet pizzicato only) ?. VPO3 ? (S and E) | 44 Contrabass | "Strings" | Spitfire Symphony Orchestra; BBCSO-D | [Card cards/strings.md#double_bass]; [Card cards/free_libraries.md#karoryfer_samples] |
| harp | S: one articulation (ordinary pluck) | S: `Concert Harp` † (Q6: mp and f layers sound together); `Folk Harp` ? (a substitute) | S: `Concert Harp` † (`cutoff`, `fil_veltrack`, `pitch_veltrack` ignored) | | FreePats Concert Harp (VCSL recordings, CC0) ?. VPO3 ? | 47 Orchestral Harp | "Pluck" | VSL Synchron Harp; Spitfire Harp Redux | [Card cards/plucked.md#harp] |
| guitar_classical | | | | | FreePats Nylon-String Guitar (CC0) ? | 25 Acoustic Guitar (nylon) | "Pluck" | | [Card cards/free_libraries.md#freepats] |
| lute | | | | | | ≈ 25 (card) | "Pluck" | | nothing free found [Card cards/free_libraries.md, open question 12] |
| mandolin | | | | | Terkelsen Mandolin, S: one articulation (no licence stated: unknown) | ≈ 26 Acoustic Guitar (steel) (card) | "Pluck" | | [Card cards/plucked.md#mandolin]; Philharmonia mandolin samples may not be made available as a sampler instrument [Card cards/free_libraries.md#philharmonia_sound_samples] |

### 1.2 Woodwinds

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| piccolo | S: sustain, staccato | | S: sustain, legato†, marcato, staccato | | VPO3 ? | 73 Piccolo | "Square Lead" or "Warm Pad" | Spitfire Studio Woodwinds Pro; VSL Synchron Woodwinds; BBCSO-D | [Card cards/flutes.md#piccolo] |
| flute | S: sustain (vibrato, non-vibrato, "expressive vibrato"), staccato; `Flute-KS` † (Q1: silent until a switch key) | | S (`Flute Solo 1`, `2`) and E (`Flutes`): sustain, legato†, marcato, staccato; `Flutes KS` ‡ (Q11: all six articulations sound on every note) | | Ixox Flute (CC BY 4.0, attribution), S: legato ‡ or † (Q2, Q5). VPO3 ? | 74 Flute | "Warm Pad" | Spitfire Studio Woodwinds Pro; VSL Synchron Woodwinds; BBCSO-D | [Card cards/flutes.md#flute] |
| alto_flute | | | S: sustain, legato†, marcato, staccato | | VPO3 ? | ≈ 74 (card) | "Warm Pad" | VSL Studio Alto Flute; Spitfire Studio Woodwinds Pro | [Card cards/flutes.md#alto_flute] |
| bass_flute | | | | | | ≈ 74 (card) | "Warm Pad" | VSL Studio Bass Flute; Spitfire Studio Woodwinds Pro | Iowa recordings only, no SFZ mapping [Card cards/free_libraries.md#university_of_iowa_mis] |
| oboe | S: sustain (vibrato, non-vibrato), staccato | | S and E (`Oboes`): sustain, legato†, staccato | | VPO3 ? | 69 Oboe | "Square Lead" | VSL Synchron Woodwinds; BBCSO-D | [Card cards/double_reeds.md#oboe] |
| english_horn | | | S (`Cor Anglais Solo`): sustain, legato†, staccato | | VPO3 ? | 70 English Horn | "Square Lead" | VSL Synchron Woodwinds | [Card cards/double_reeds.md#english_horn] |
| oboe_damore | | | | | | ≈ 69 (card) | "Square Lead" | VSL Synchron Oboe d'Amore | [Card cards/double_reeds.md#oboe_damore] |
| bassoon | S: sustain (also with vibrato), staccato | | S and E (`Bassoons`): sustain, legato†, staccato | | VPO3 ? | 71 Bassoon | "Square Lead" | VSL Synchron Woodwinds; BBCSO-D | [Card cards/double_reeds.md#bassoon] |
| contrabassoon | | | S: sustain, legato†, staccato | | VPO3 ? | ≈ 71 (card) | "Warm Pad" | VSL Synchron Woodwinds | [Card cards/double_reeds.md#contrabassoon] |
| clarinet_bb | S: sustain, staccato; `Clarinet-KS` † (Q1: silent until a switch key). Key not stated [Estimate: treated as B♭] | | S: sustain, legato†, staccato; `Clarinet Solo KS` † (its switches work). E (`Clarinets`): sustain, legato†, staccato | | FreePats Clarinet (CC0) ?. VPO3 ? | 72 Clarinet | "Square Lead" | BBC SO Professional; Berlin Woodwinds; BBCSO-D | [Card cards/clarinets.md#clarinet_bb] |
| clarinet_a | | | | | | 72 | "Square Lead" | (no A-clarinet patch in BBC SO or Berlin Woodwinds) | samples are at sounding pitch, so B♭ presets substitute within the range [Card cards/clarinets.md#clarinet_a] |
| clarinet_eb | | | | | | ≈ 72 | "Square Lead" | Berlin Woodwinds Additional Instruments; VSL Synchron Clarinet (Eb) | Iowa recordings only [Card cards/clarinets.md#clarinet_eb] |
| bass_clarinet | | | S: sustain, legato†, staccato | | VPO3 ? | ≈ 72 (71 Bassoon is the card's alternative) | "Square Lead" | BBC SO Professional; Berlin Woodwinds Additional Instruments | [Card cards/clarinets.md#bass_clarinet] |
| saxophone_soprano | | `Saxello - *` ? as a substitute (vibrato, non-vibrato, staccato) [Estimate: a saxello is a curved soprano saxophone] | | | MTG Soprano Sax (CC BY 4.0, attribution) ‡ (Q4: the whole patch fails) | 65 Soprano Sax | "Square Lead" | | [Card cards/saxophones.md#saxophone_soprano] |
| saxophone_alto | | | | | Weresax (Karoryfer, alto saxophone, CC0 `LICENSE`) ?; MTG Alto Sax (CC BY 4.0) ‡ (Q4) | 66 Alto Sax | "Square Lead" | NI Session Horns Pro | [Card cards/saxophones.md#saxophone_alto]; [Card cards/free_libraries.md#karoryfer_samples] |
| saxophone_tenor | | S: "vibrato" (sustain), "non-vibrato" (sustain), staccato; key-switch file as is | | | FreePats Tenor Saxophone (edited VCSL recordings with looped sustains, CC0) ?: check that its loop points are written in the `.sfz` (N6); MTG Tenor Sax (CC BY 4.0) ‡ (Q4) | 67 Tenor Sax | "Square Lead" | NI Session Horns Pro | [Card cards/saxophones.md#saxophone_tenor]; [Card cards/free_libraries.md#freepats] |
| saxophone_baritone | | | | | Bear Sax (Karoryfer, 1926 Conn baritone, CC0 `LICENSE`) ?; MTG Baritone Sax (CC BY 4.0) ‡ (Q4) | 68 Baritone Sax | "Square Lead" | NI Session Horns Pro | [Card cards/saxophones.md#saxophone_baritone]; [Card cards/free_libraries.md#karoryfer_samples] |

The Karoryfer sets were not opened. Karoryfer writes for Sforzando, so they may use CC opcodes that
Sonara ignores, as War Tuba does [Card cards/free_libraries.md#karoryfer_samples; Cited S7 §2.5].

### 1.3 Brass

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| horn_f | S: sustain, staccato, con_sordino (`FHornMute`, a sustain) | | S (`Horn Solo`) and E (`Horns`): sustain, legato†, marcato, staccato; `Horn Solo KS` ‡ (Q11: three marcatos always sound) | | VPO3 ? | 61 French Horn | Poly Synth "Brass" | Spitfire Studio Brass; CineBrass; BBCSO-D | [Card cards/horns.md#horn_f] |
| wagner_tuba_tenor | | | | | | ≈ 61 (card; weak) | "Brass" | VSL Synchron Wagner Tuba; VSL Dimension Brass | [Card cards/horns.md#wagner_tuba_tenor] |
| wagner_tuba_bass | | | | | | ≈ 61 (card; weak) | "Brass" | VSL Synchron Wagner Tuba; VSL Dimension Brass | [Card cards/horns.md#wagner_tuba_bass] |
| trumpet_bb | S: sustain (also with vibrato), staccato, con_sordino (straight and harmon mute, sustains). Key not stated [Estimate: treated as B♭] | | S (`Trumpet Solo`) and E (`Trumpets`): sustain, legato†, marcato, staccato; `Trumpets KS` ‡ (Q11: two marcatos always sound) | | VPO3 ? | 57 Trumpet; muted 60 | "Brass" | Samplemodeling The Trumpet 3; VSL Studio Trumpet; BBCSO-D | [Card cards/high_brass.md#trumpet_bb] |
| trumpet_c | | | | | | 57 | "Brass" | VSL Studio Trumpet (C) | trumpet_bb presets substitute [Cited S7 §4.3; Card cards/high_brass.md#trumpet_c] |
| trumpet_piccolo | | | | | | 57 | "Brass" | VSL Studio Piccolo Trumpet; Samplemodeling The Trumpet 3 | [Card cards/high_brass.md#trumpet_piccolo] |
| cornet | | | | | | 57 | "Brass" | VSL Studio Cornet; Samplemodeling The Trumpet 3 | [Card cards/high_brass.md#cornet] |
| flugelhorn | | | | | | ≈ 57 (weak; 61 is the card's alternative) | "Brass" | VSL Studio Flugelhorn; Samplemodeling The Trumpet 3 | [Card cards/high_brass.md#flugelhorn] |
| trombone_tenor | S: sustain (also with vibrato), staccato [Estimate: tenor, not stated] | | S (`Tenor Trombone Solo`) and E (`Trombones`): sustain, legato†, marcato, staccato | | VPO3 ? | 58 Trombone | "Brass" | Spitfire Studio Brass; Berlin Brass; BBCSO-D | [Card cards/low_brass.md#trombone_tenor] |
| trombone_bass | | | S: sustain, legato†, marcato, staccato | | VPO3 ? | 58 | "Brass" | Spitfire Studio Brass; Berlin Brass; BBCSO-D | [Card cards/low_brass.md#trombone_bass] |
| tuba | S: sustain, staccato (`Tuba-KS` as is; its switch keys 84 and 85 lie above the range) | | S: sustain, legato†, marcato, staccato | | War Tuba (Karoryfer, CC0 `LICENSE`), S, program `2-solo-poly`: key switches 24 "staccatissimo", 25 staccato, 26 sustain, default 25 † (CC dynamics, EQ, LFO and filters ignored; the switch keys lie inside the tuba's range, 21-67, so low notes there switch instead of sounding); orchestral tuba not confirmed [Estimate]; duo and trio programs ?. VPO3 ? | 59 Tuba | "Brass" | Spitfire Studio Brass; Berlin Brass; BBCSO-D | [Card cards/low_brass.md#tuba]; [Cited S7 §2.5, S10] |
| cimbasso | | | | | | ≈ 58 Trombone (card) | "Brass" | Spitfire Studio Brass | [Card cards/low_brass.md#cimbasso] |
| euphonium | | | | | | ≈ 59 (card) | "Brass" | Spitfire Studio Brass Professional | [Card cards/low_brass.md#euphonium] |

### 1.4 Keyboards

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| piano_concert_grand | | | `Grand Piano/Grand Piano` ‡ (Q3: fails to load) | | Splendid Grand Piano (public domain as relayed from AKAI) ‡ (Q3); Keppy's Steinway (CC BY-SA 4.0, the strictest reading of conflicting sources) ?; Maestro (all rights reserved: not usable) | 1 Acoustic Grand Piano | Poly Synth "Electric Keys" | VSL Synchron Pianos; BBCSO-D (piano) | no free concert grand plays [Card cards/acoustic_pianos.md#piano_concert_grand]; [Card cards/free_libraries.md#splendid_grand_piano, #keppys_steinway_piano] |
| piano_studio_grand | | `Grand Piano, Kawai` † (decay and `rt_decay` ignored; release samples lost under the sustain pedal, N1); `Grand Piano, Steinway B` † [Estimate: same generator; not opened] | | sfzinstruments V3 remap ‡ (Q2: loads but plays only pedal noises; Q1; Q7; N1 once the notes load); FreePats SFZ (CC BY 3.0) ? | Headroom Piano (CC BY 4.0) ?; Piano in 162 (licence not stated) ? | 2 Bright Acoustic Piano | "Electric Keys" | see piano_concert_grand | [Card cards/acoustic_pianos.md#piano_studio_grand]; the card's "raw WAVs" refers to VCSL `master`; the tag holds the SFZ [Cited S3] |
| piano_upright | `UprightPiano`, `VSUpright1` | `Upright Piano, Knight` † (pedal regions silent, Q10; release samples lost under the sustain pedal, N1); `Upright Piano, Yamaha` ? | | | FreePats Upright Piano KW (CC0; bass notes looped: check the loop points are in the `.sfz`, N6) ?; LivingRoom Upright Micro (CC BY-SA 4.0) ? | ≈ 1 (card) | "Electric Keys" | | [Card cards/acoustic_pianos.md#piano_upright] |
| piano_felt | | | | | | ≈ 1 (card; weak) | "Electric Keys" | Spitfire Originals Felt Piano; Spitfire Cinematic Soft Piano; NI Una Corda | [Card cards/acoustic_pianos.md#piano_felt] |
| piano_prepared | | | | | | 1 only as a last resort (card) | "Pluck", only with confirmation | Big Fish Audio John Cage Prepared Piano (WAV export; an SFZ map would be needed) | [Card cards/acoustic_pianos.md#piano_prepared] |
| piano_honky_tonk | | | | | FreePats Old Piano FB (CC0) ? | 4 Honky-tonk Piano | "Electric Keys" | | [Card cards/free_libraries.md#freepats]; [Card cards/acoustic_pianos.md#piano_honky_tonk] |
| electric_piano_rhodes | | | | | jRhodes3d `-no-xfade` files † (samples CC BY-NC 4.0 per `LICENSE`; the `.sfz` header's wording is the weaker source; user install only); `jRhodes3d-keyswitch` † (Q6; switches 103-108 lie inside the 88-key range, 21-108) | 5 Electric Piano 1 | "Electric Keys" | Arturia Stage-73 V2; Scarbee RSP '73 | [Card cards/electric_keys.md#electric_piano_rhodes]; [Card cards/free_libraries.md#electric_pianos]; [Cited S7 §2.5, S10] |
| electric_piano_wurlitzer | | | | | Greg Sullivan Wurlitzer EP200 † (CC BY 3.0 Unported per its `LICENSE` and README: attribution) | 5 | "Electric Keys" | Arturia Wurlitzer V | the sound mapping read the EP200 file; the electric-keys card found no free Wurlitzer [Cited S7 §2.5; Card cards/electric_keys.md#electric_piano_wurlitzer] |
| clavinet | | (`TX81Z - Clavisynth` is an FM patch, not a clavinet) | | | | 8 Clavi | "Pluck" | Arturia Clavinet V | [Card cards/electric_keys.md#clavinet] |
| harpsichord | | Flemish 4', 8', Full and KS † (Q8, decay ignored); English Normal and Lute, French, Italian, Unk ? | `Harpsichord 4'`, `8'`, `Full` ?; `Harpsichord KS` ‡ (Q11: 4' and 8' always sound) | | | 7 Harpsichord | "Pluck" | VSL Studio Special Keyboards; Pianoteq | [Card cards/historical_keys_organs.md#harpsichord] |
| clavichord | | | | | | ≈ 8 (weak; the card names 7 or 8) | "Pluck" | | [Card cards/historical_keys_organs.md#clavichord] |
| celesta | | | `Percussion/Celeste` † (Q6; pedal curves ignored) | | VPO3 ? | 9 Celesta | "Glass Bells" | | [Card cards/historical_keys_organs.md#celesta] |
| organ_pipe | `OrganLoud`, `OrganQuiet`, `OrganLoudPedal`, `OrganQuietPedal` | `Pipe Organ - *` and KS † (Q8: follows velocity); `Renaissance Organ - *` ? | 14 single stops and `Organ All Stops` ?; `Organ Combinations` ‡ (Q11; also Q5 and Q8) | | FreePats Church Organ Emulation (a recording of the Aeolus organ synthesizer, CC0) ? | 20 Church Organ | "Warm Pad" | | [Card cards/historical_keys_organs.md#organ_pipe] |
| organ_hammond | | | | | FreePats Drawbar, Percussive and Rock Organ Emulations (setBfree recordings, CC0) ?: static registrations, no drawbars and no Leslie [Estimate: samples cannot move drawbars] | 17 Drawbar Organ | "Warm Pad" | | [Card cards/historical_keys_organs.md#organ_hammond] |
| accordion | | | | | FreePats Button Accordion HN (CC0) ? | 22 Accordion | "Warm Pad" | | [Card cards/free_libraries.md#freepats] |

### 1.5 Tuned percussion

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| timpani | S: hits (2 round robins); roll (`TimpaniRolls`) | S: `Timpani 1 - Hit`, `- Roll`; `Timpani 1 - Keyswitch` plays, but its switch keys 36 and 37 lie inside the timpani's range (36-61); `Timpani 2` ? | S: hits, rolls, roll crescendos in key zones † (Q6: both hit layers sound; Q5: the roll ending fires at note-on) | | FreePats Timpani (VCSL recordings, CC0) ?. VPO3 ? | 48 Timpani | Drum Kit "Cinematic Toms", toms on notes 41, 43, 45, 47, 48 and 50 only (pitch lost) | VSL Synchron Timpani | [Card cards/tuned_percussion.md#timpani]; [Cited S12: C22] |
| xylophone | S: one articulation | S: hard, medium and soft mallets, KS file | `Xylophone` ? (the card lists filter and decay opcodes: likely †, Q9) | | FreePats Xylophone (CC0) ?. VPO3 ? | 14 Xylophone | "Pluck" | VSL Synchron Percussion I | [Card cards/tuned_percussion.md#xylophone] |
| marimba | S: one articulation | `Marimba` † (Q6) | `Marimba Hits` ?; `Marimba Rolls` (roll) ? | | | 13 Marimba | "Pluck" | | [Card cards/tuned_percussion.md#marimba]; VPO3 has no marimba [Card cards/free_libraries.md#virtual_playing_orchestra_3] |
| vibraphone | | `Vibraphone - Soft Mallets`; `- Hard Mallets`, `- Bowed` ? | `Vibraphone` † (pedal curves and CC21 vibrato ignored) | | VPO3 ? | 12 Vibraphone | "Glass Bells" | | [Card cards/tuned_percussion.md#vibraphone] |
| glockenspiel | S: one articulation | `Glockenspiel` † (Q6); `Tubular Glockenspiel` ? | `Glockenspiel` ? (the card lists filter and decay opcodes: likely †, Q9) | | VPO3 ? | 10 Glockenspiel | "Glass Bells" | VSL Synchron Percussion I | [Card cards/tuned_percussion.md#glockenspiel] |
| tubular_bells | S: one articulation | `Tubular Bells 1`; `2`, `3 - Legacy` ? | `Chimes` ? | | FreePats Tubular Bells (CC0) ?. VPO3 ? | 15 Tubular Bells | "Glass Bells" | VSL Synchron Percussion I | [Card cards/tuned_percussion.md#tubular_bells] |
| crotales | | | `Crotales` ? (the card lists `xfin`/`xfout` velocity layers: likely †, Q6) | | | ≈ 10 (card) | "Glass Bells" | | [Card cards/tuned_percussion.md#crotales] |
| carillon | | (hand chimes and hand bells are other instruments) | | | | ≈ 15 (card) | "Glass Bells" | | [Card cards/tuned_percussion.md#carillon] |

### 1.6 Untuned percussion

VSCO 2 CE voices these ids from one patch, `GM-StylePerc`, with one sound per key. Its key map differs
from GM on several keys, so a GM drum part must be re-mapped [Cited S7 §2.1]. VCSL uses one patch per
instrument, with a sound per key from 60 upward [Cited S7 §2.3]. FreePats World Percussion (CC0:
cajón, bongos, egg shaker, tambourine, hand clap, claves, castanets, conga, maracas, darbuka) and
VPO3's percussion map were not opened [Card cards/free_libraries.md#freepats,
#virtual_playing_orchestra_3].

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| snare_drum | hits (snares on and off), rolls, taps | `Snare Drum, Modern 1-3`, `Rope Tension`: hits, rolls, stick, taps | `Bass Drum & Snare` ?; `All Unpitched Percussion` † | | VPO3 ? | ch. 10: 38, 40 | Drum Kit 38 | BBCSO-D | [Card cards/untuned_percussion.md#snare_drum] |
| bass_drum | hits (7 layers), rubs | `Bass Drum 1`, `2` | `Bass Drum & Snare` ? | | VPO3 ? | 35, 36 | Drum Kit 36; "Cinematic Toms" | BBCSO-D | [Card cards/untuned_percussion.md#bass_drum] |
| clash_cymbals | crash | `Clash Cymbals 1`, `2` (crash, choked) | `Cymbals & Tamtam` ? | | VPO3 ? (cymbal type not stated) | 49, 57 | Drum Kit 49 | BBCSO-D (piatti, stopped) | [Card cards/untuned_percussion.md#clash_cymbals] |
| suspended_cymbal | hits, stick hit, swells, bowed (bell hits and scrapes exist as WAVs but are not mapped) | `Suspended Cymbal 1`, `2`: bowed, crescendo rolls, hit, bell, roll, scrape | `Cymbals & Tamtam` ? | | VPO3 ? (cymbal type not stated) | 49 (card); ≈ 51, 59 | Drum Kit 51 for hits, 49 for swells | BBCSO-D | [Card cards/untuned_percussion.md#suspended_cymbal]; [Card cards/free_libraries.md#vsco_2_ce] |
| tam_tam | gong hit, scrape | `Gong 1`, `2`: hits, scrape | `Cymbals & Tamtam` ? | | VPO3 ? | none (≈ 52 Chinese Cymbal) | Drum Kit 49 | BBCSO-D | [Card cards/untuned_percussion.md#tam_tam] |
| triangle | hits (`Hit` and `HitFM` on two triangles; the `HitM` and roll WAVs are not mapped) | `Triangles`: three triangles, each with hits ("Hit", "HitFM", "HitM") and a roll | `Triangle` ? | | VPO3 ? | 81, 80 | none (80 and 81 are not in `GM_MAP`): "Glass Bells" | BBCSO-D | [Card cards/untuned_percussion.md#triangle]; [Card cards/free_libraries.md#vsco_2_ce] |
| tambourine | shake, hit, roll | `Tambourine 1`, `2`: hit, roll, shake | samples exist; via `All Unpitched Percussion` † [Estimate] | | FreePats World Percussion (CC0) ?. VPO3 ? | 54 | Drum Kit 54 | BBCSO-D | [Card cards/untuned_percussion.md#tambourine] |
| castanets | | | samples `castanets-rr1`, `-rr2` exist; no dedicated patch; which patch maps them is not identified ? | | FreePats World Percussion (CC0) ?. VPO3 ? | none (85 is a GS note) | Drum Kit 75 (claves) | BBCSO-D | [Cited S1]; [Card cards/free_libraries.md#freepats] |
| wood_block | (raw files only, no `.sfz`) | `Woodblock` | wood blocks per the card ? | | VPO3 ? | 116 Woodblock; ch. 10: 76, 77 | Drum Kit 76, 77 | BBCSO-D | [Card cards/untuned_percussion.md#wood_block]; [Cited S10] |
| temple_blocks | | | | | (MuseScore_General has a Temple Blocks preset: SF2) | none | Drum Kit 76, 77 | | [Card cards/free_libraries.md#coverage] |
| sleigh_bells | hit | `Sleigh Bells` | samples exist; via `All Unpitched Percussion` † [Estimate] | | VPO3 ? | none (83 is a GS note) | Drum Kit 54 | | [Card cards/untuned_percussion.md#sleigh_bells] |
| maracas | (raw files only, no `.sfz`) | | | | FreePats World Percussion (CC0) ? | 70 | Drum Kit 70 (the kit's shaker) | | [Card cards/untuned_percussion.md#maracas]; [Card cards/free_libraries.md#freepats] |
| shaker | | `Shaker, Small`, `Shaker, Large` | samples exist; via `All Unpitched Percussion` † [Estimate] | | FreePats World Percussion egg shaker (CC0) ?. VPO3 ? | none (82 is a GS note); ≈ 70 or 69 | Drum Kit 70 | | [Card cards/untuned_percussion.md#shaker] |
| whip | (raw files only, no `.sfz`) | `Slapstick` | | | | none | Drum Kit 39 (clap), only with confirmation | | [Card cards/untuned_percussion.md#whip] |

Tambourine, sleigh bells and shaker samples are in Sonatina's `Samples/Percussion/` folder
[Cited S1]. That `All Unpitched Percussion` maps them is an Estimate: the sound mapping read that file
but did not list its keys [Cited S7 §2.2].

### 1.7 Voices

| id | VSCO 2 CE | VCSL | Sonatina | Salamander | Other free SFZ | GM fallback | Built-in | Commercial (M5) | Basis |
|---|---|---|---|---|---|---|---|---|---|
| choir_soprano | | | E: `Chorus - Notation/Mixed Chorus` and `Large Chrous` (sic) †, a mixed choir with one vowel and no separate parts; the Performance versions † (CC1 crossfade ignored, Q6) | | VPO3 Choir Female and Choir Mixed ? (no per-voice parts). (KBH Real and Swelling Choirs: SF2, CC BY per the aggregator, but its uploader found it with no sign of an author) | 53 Choir Aahs | Poly Synth "Choir Pad" | Spitfire Eric Whitacre Choir (per-section microphones); EW Hollywood Choirs | [Card cards/voices.md#choir_soprano]; [Cited S5] |
| choir_alto | | | as choir_soprano | | VPO3 Choir Female and Choir Mixed ? | 53 | "Choir Pad" | Spitfire Eric Whitacre Choir | [Card cards/voices.md#choir_alto] |
| choir_tenor | | | as choir_soprano | | VPO3 Choir Male and Choir Mixed ? | 53 | "Choir Pad" | Spitfire Eric Whitacre Choir; EW Hollywood Choirs (men's choir) | [Card cards/voices.md#choir_tenor] |
| choir_bass | | | as choir_soprano | | VPO3 Choir Male and Choir Mixed ? | 53 | "Choir Pad" | Spitfire Eric Whitacre Choir; EW Hollywood Choirs (men's choir) | [Card cards/voices.md#choir_bass] |
| voice_soprano | | | | | | ≈ 54 Voice Oohs | "Choir Pad" | Soundiron Voices of Rapture; EW Symphonic Choirs | [Card cards/voices.md#voice_soprano] |
| voice_mezzo_soprano | | | | | | ≈ 54 | "Choir Pad" | | [Card cards/voices.md#voice_mezzo_soprano] |
| voice_alto | | | | | | ≈ 54 | "Choir Pad" | EW Symphonic Choirs | [Card cards/voices.md#voice_alto] |
| voice_tenor | | | | | | ≈ 54 | "Choir Pad" | | [Card cards/voices.md#voice_tenor] |
| voice_baritone | | | | | | ≈ 54 | "Choir Pad" | | [Card cards/voices.md#voice_baritone] |
| voice_bass | | | | | | ≈ 54 | "Choir Pad" | | [Card cards/voices.md#voice_bass] |

The aggregator's `tags=choir&formats=sfz` query returned only Sonatina entries [Cited S5]. That is one
catalogue, not an exhaustive search [Estimate].

### 1.8 What the matrix says

- **Where free sound is strong.** Strings, the standard double-wind section, standard brass, timpani,
  mallets, harp, organ, upright piano and orchestral untuned percussion all have a free source that
  plays as is or partly [Cited S1-S3, S7].
- **Where free sound is missing or unproven.**
  - Nothing free at all: auxiliary and historical brass, auxiliary clarinets and oboes, the bass
    flute, lute, carillon, temple blocks, felt and prepared piano, clavinet, clavichord and the solo
    voices (gap rank 15).
  - Free files that do not play today: the concert grand (Q3) and the soprano saxophone (MTG, Q4).
  - Free files not inspected: the alto and baritone saxophones (Karoryfer Weresax and Bear Sax,
    CC0), the classical guitar, honky-tonk piano, Hammond-style organ, accordion, maracas, crotales
    and castanets.
  - Choir parts: only a mixed chorus plays; VPO3's female and male choirs were not opened.

  For the first two groups, only GM (not playable today), the built-in synth or a commercial plugin
  (M5) remains [Cited S7 §4.3; Card cards/free_libraries.md#coverage].
- **Section and solo** come from different libraries:
  - VSCO 2 CE gives sections for violin, viola and cello only. The winds and brass are single
    instruments [Estimate: the sound mapping's reading of chamber-library naming, Cited S7 §2.1].
  - Among the inspected libraries, free wind and brass sections exist only in Sonatina [Cited S1].
    VPO3 lists section files, but it was not opened
    [Card cards/free_libraries.md#virtual_playing_orchestra_3].
- **Licences of the free sources** [Card cards/free_libraries.md; Cited S7 §2.1-2.5]:
  - CC0, no credit required: VSCO 2 CE (its Readme asks for credit, as a request only), VCSL,
    War Tuba, Weresax, Bear Sax, Meatbass, Sneakybass and the FreePats entries named above; the
    Bigcat cello and D. Smolken bass by GitHub's licence metadata and the catalogue, with no
    `LICENSE` file found.
  - Attribution required: Sonatina (CC Sampling Plus 1.0, a retired licence), Salamander and Greg
    Sullivan's E-Pianos (CC BY 3.0), MTG Solo Sax, Ixox Flute and the Headroom Piano (CC BY 4.0).
  - Non-commercial samples: jRhodes3d (CC BY-NC 4.0). Share-alike: Keppy's Steinway and the
    LivingRoom Upright (CC BY-SA 4.0). No licence stated: the Terkelsen mandolin.
  - GPL: the FreePats General MIDI sets, which LIC-01 keeps out of anything Sonara ships
    [Cited S11: `.claude/rules/project.md`].

## 2. Articulation matrix for the core orchestra

Each cell gives the best free source for that instrument and articulation, best first. The ranking
rule, in order [Estimate: rule derived from S7 §2.6]:

1. a file that plays as is beats one that plays partly;
2. among partly playing files, one that only loses controls beats one with an audible defect
   (stacked or doubled layers);
3. a single-articulation file beats a key-switch file, because key-switch files carry Q1 and Q11
   and, where their switch keys lie inside the instrument's range, take notes away;
4. at equal standing, CC0 beats CC Sampling Plus.

Codes:
- `V` = a VSCO 2 CE single-articulation file (CC0, plays as is).
- `C` = VCSL (CC0).
- `N` = a Sonatina Notation single-articulation file (CC Sampling Plus 1.0, attribution required;
  `†` throughout).
- `B` = Bigcat cello (voice layers stack, Q7); `D` = D. Smolken double bass; `W` = War Tuba
  program `2-solo-poly`, a key-switch file whose switches lie inside the tuba's range. All three are
  `†` and CC0 (War Tuba by its `LICENSE`, the other two by GitHub's licence metadata).
- After the code, `S` = solo and `E` = section.
- A blank cell means no free source was found.
- `n/i` = not idiomatic for the instrument [Estimate: musical judgement against the card's
  section 5].

The source cells are [Cited S1, S2, S3] for the file names and [Cited S7] for the verdicts. Free
sets that were not opened (Meatbass, Sneakybass, the FreePats sets, VPO3) are left out until they
are inspected.

| id | sustain | legato | staccato | spiccato | tenuto | marcato | pizzicato | tremolo | trill | col_legno | harmonics | sul_ponticello | con_sordino | flutter_tongue | roll |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| violin | V S, E; N S, E | N S, E: a retriggered monophonic sustain (Q5, Q9) | N S, E | V S, E; N S | | N S, E | V S, E; N S, E | V S, E; N E, S (Solo 2) | | N E | N S, E | | | n/i | n/i |
| viola | V E; N S, E | N S, E (Q5, Q9) | N S, E | V E | | N S, E | V E; N S, E | V E; N E | | N E | N S, E | | | n/i | n/i |
| cello | V E; N S, E; B S | N S, E (Q5, Q9); B S (Q5, Q7) | N S, E | V E | | N S, E | V E; N S, E; B S | V E; N E | | N E | N S, E | | | n/i | n/i |
| double_bass | V S; D S; N S, E | N S, E (Q5, Q9) | N S, E | V S | | N S, E | V S; D S; N S, E | V S; N E | | N E | N S | | | n/i | n/i |
| piccolo | V S; N S | N S (Q5, Q9) | V S; N S | n/i | | N S | n/i | n/i | | n/i | | n/i | n/i | | n/i |
| flute | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | N S, E | n/i | n/i | | n/i | | n/i | n/i | | n/i |
| alto_flute | N S | N S (Q5, Q9) | N S | n/i | | N S | n/i | n/i | | n/i | | n/i | n/i | | n/i |
| oboe | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | | n/i | n/i | | n/i | n/i | n/i | n/i | | n/i |
| english_horn | N S | N S (Q5, Q9) | N S | n/i | | | n/i | n/i | | n/i | n/i | n/i | n/i | | n/i |
| clarinet_bb | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | | n/i | n/i | | n/i | n/i | n/i | n/i | | n/i |
| bass_clarinet | N S | N S (Q5, Q9) | N S | n/i | | | n/i | n/i | | n/i | n/i | n/i | n/i | | n/i |
| bassoon | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | | n/i | n/i | | n/i | n/i | n/i | n/i | | n/i |
| contrabassoon | N S | N S (Q5, Q9) | N S | n/i | | | n/i | n/i | | n/i | n/i | n/i | n/i | | n/i |
| horn_f | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | N S, E | n/i | n/i | | n/i | n/i | n/i | V S (sustain only) | | n/i |
| trumpet_bb | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | N S, E | n/i | n/i | | n/i | n/i | n/i | V S (straight and harmon, sustain only) | | n/i |
| trombone_tenor | V S; N S, E | N S, E (Q5, Q9) | V S; N S, E | n/i | | N S, E | n/i | n/i | | n/i | n/i | n/i | | | n/i |
| trombone_bass | N S | N S (Q5, Q9) | N S | n/i | | N S | n/i | n/i | | n/i | n/i | n/i | | | n/i |
| tuba | V S; N S; W S | N S (Q5, Q9) | V S; N S; W S | n/i | | N S | n/i | n/i | | n/i | n/i | n/i | | | n/i |
| timpani | V S; C S; N S (single stroke; Q6) | n/i | | n/i | n/i | | n/i | n/i | n/i | n/i | n/i | n/i | | n/i | V S; C S; N S (Q5, Q6) |
| harp | V S; N S; C S (Q6) | n/i | n/i | n/i | n/i | n/i | n/i | | | n/i | | n/i | n/i | n/i | n/i |
| piano_concert_grand | | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i |
| piano_studio_grand | C S † (`Grand Piano, Kawai`; `Steinway B` by the same generator; N1) | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i |
| piano_upright | V S; C S † (`Upright Piano, Knight`: Q10, N1) | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i | n/i |

Notes on the matrix:

- **Every legato cell is a sound that exists but does not play as legato.**
  - Sonatina's `* Legato` files hold two groups. One is `trigger=first` with a 0.30 s attack, the
    other `trigger=legato` with a 0.2 s attack. Both start at `offset=20000`, both set
    `group=1 off_by=1 off_mode=time off_time=1`, and both include the same sustain fragment,
    `includes/1st-violins-sustain.sfz` [Cited S12: `1st Violins Legato` re-read; Card
    cards/free_libraries.md#sonatina_symphonic_orchestra].
  - The SFZ spec plays a `first` region only when no other note sounds, and a `legato` region only
    while one does [Cited S8]. Sonara fires both as attacks (Q5). The `first` voice is dropped
    before it makes a sound, the previous note is choked in 10 ms instead of overlapping for 1 s
    (Q9), and the legato group's voice remains. So every note restarts a monophonic sustain with a
    0.2 s attack [Cited S12: trace of one note-on; code reading, not rendered].
  - With Q5 and Q9 fixed, the files would play as written: the new note's sustain fades in while
    the old one fades out over 1 s. That is an overlap crossfade between sustain samples, not a
    recorded transition [Estimate: reading of the file against the spec].
  - VSCO 2 CE has no legato patch [Card cards/strings.md#violin; Cited S2].
  - The Bigcat cello's legato regions also hit Q5. Whether they use separate transition samples was
    not checked [Cited S7 §2.5].
- **Blank columns.** tenuto, trill, sul_ponticello and flutter_tongue have no free file anywhere in
  the three trees [Cited S1-S3: no file name contains these techniques]. The sound mapping gives the
  emulations (written-out trills, accents, shortened notes) [Cited S7 §4.2].
- **Mutes are brass-only and sustain-only.** No free string mute was found. Sonatina has no mute
  files at all [Cited S1, S2].
- **Timpani.** A single stroke is listed under sustain, because the 15 ids have no "hit". Use VCSL's
  separate hit and roll patches rather than its key-switch file, whose switch keys 36 and 37 lie
  inside the timpani's range [Cited S7 §2.3, S10]. Muffled timpani (coperti) have no free file
  [Cited S1-S3]. Rimsky-Korsakov describes the effect: "a piece of cloth is generally placed on the
  skin of the drum" [Cited S4].
- **Harp.** Harp harmonics and bisbigliando (the tremolo column) have no free file. VSCO's harp is the
  ordinary pluck only [Card cards/plucked.md#harp].
- **Pianos.** The articulation axis is not where pianos differ. Their axes are the sustain pedal, the
  release samples and the resonance layers.
  - The sampler honours CC 64 and `trigger=release` [Cited S6]. But a key released while the pedal
    is down never starts its release samples, so pedalled passages lose their damper noise and
    resonance tails (N1). Only imported MIDI or live input carries CC 64, because no tool writes CC
    [Cited S12].
  - It ignores the CC gates of pedal-resonance and pedal-noise layers (Q7). In Salamander those
    noises therefore fire on every key [Cited S7 §2.0, S12].
  - Free sources for the other three piano ids are in section 1.4.

## 3. Gap list, ranked

**Ranking rule.** The first question is whether the gap blocks the product sentence: "strings play a
legato melody, horns and timpani underneath". The second is how many ids or everyday requests it
unlocks. The third is parity with GarageBand and Logic.

What the sentence needs musically:

- Rimsky-Korsakov calls the string group "the richest and most expressive of all". He lists among
  its means "_legato_ … _crescendo_, _diminuendo_, _sforzando_, _morendo_" [Cited S4].
- For chords he advises "to entrust the stationary parts to the brass" [Cited S4].
- Kettle-drums "occupy the most important place in the group of percussion instruments". In
  _tremolando_ they "can execute the most gradual _crescendo_, _diminuendo_" [Cited S4].

So the sentence needs three things: a connected string line with phrasing, held brass chords that can
swell, and timpani rolls that can grow [Estimate: musical reading of S4].

Gap types used here:
- **SOUND**: no free library has the sound.
- **ENGINE**: the sampler cannot play what exists.
- **DATA**: metadata or mapping is missing.
- **UI**: the interface has no control for it.

Where closing a gap also needs a tool-schema or project-format change, the "What would close it"
column says "(CTR-01)".

| Rank | Gap | Type | Evidence | What it blocks | What would close it |
|---|---|---|---|---|---|
| 1 | **True legato** | ENGINE and SOUND: two separate gaps | **Engine:** Q5 turns `first` and `legato` into attacks, silently, and the previous note is choked; Q9 turns the files' 1 s overlap into a fixed 10 ms choke [Cited S7 §2.0, S12; S8 for the spec]. **Sound:** Sonatina's legato groups reuse its sustain samples [Cited S12; Card cards/free_libraries.md#sonatina_symphonic_orchestra]. VSCO has no legato patch. The Bigcat cello, MTG saxophones and Ixox flute declare legato regions whose samples were not traced, and MTG and Ixox do not load today (Q4, Q2) [Cited S7 §2.5]. No free string-section legato transitions were found. Commercial transitions exist, for example Spitfire Performance Legato [Card cards/strings.md#violin] | "Strings play a legato melody"; every wind and brass melody | **Engine:** add `first`, `legato` and `release_key` to the sampler's triggers, gated on whether another key is held, and honour `off_mode`/`off_time` instead of the fixed 10 ms choke [Cited S12: suggested fixes for Q5 and Q9]. Until then, report unsupported `trigger=` values in the loader's warning (N2). RT-02..RT-05 apply. Pin it with the tests [sfz-conformance.md](sfz-conformance.md) names (`legato_region_plays_only_while_another_key_is_held`, `off_mode_normal_chokes_over_the_region_release`) and fixtures that copy Sonatina's group layout (TST-06). This gives the overlap legato the free files describe, not recorded transitions. **Data:** a map field per preset, `legato: overlap`, with Sonara's actual behaviour [Cited S7 §3.2]. **Sound:** recorded transitions only through M5 [Estimate: none found free; open question 7]. |
| 2 | **Dynamics: CC crossfades and CC writing** | ENGINE, UI | The sampler acts on CC 7, 11 and 64 only; 7 and 11 are a linear gain [Cited S6; Cited S12: C15]. `xfin_loccN` crossfades by CC, an SFZ v1 opcode [Cited S9], and is ignored, as are `gain_cc` and `locc` gates (Q6, Q7) [Cited S7 §2.0]. No agent tool writes CC into a clip [Cited S6: `NoteInput`, 57 tools]. The piano roll has no CC lane [Cited S11: `pianoroll/`]. Free CC-driven sets exist and go unused: Sonatina Performance, the Bigcat mod-wheel program, the jRhodes crossfade files [Cited S7 §2.2, §2.5], VPO3 Performance [Card cards/free_libraries.md#virtual_playing_orchestra_3] | Swells on held horn chords; string phrasing; timpani roll crescendos; any `crescendo` instruction | **Engine:** support `xfin_/xfout_loccN` and `gain_ccN` per voice (a gain read from the current CC value, with no allocation). **Contract:** either a CC-writing tool (CTR-01), or, without a schema change, an automatable `dynamics` parameter on `sonara.sampler` that `create_automation` can target as `<device_id>:<param>`. The sampler is not a built-in, so its parameters are not pinned by golden data [Cited S6: ADR 0010 decision 4]. That second route is an Estimate (design idea). **UI:** a CC or dynamics lane. **Today's workaround:** velocity on Notation patches, plus track-volume automation, which changes level but not timbre [Estimate]. |
| 3 | **Key-switch parsing (Q1, Q11)** | ENGINE | Three VSCO key-switch files (`ViolinEns-KS`, `Flute-KS`, `Clarinet-KS`) are silent until a switch key. Eight of the nine Sonatina key-switch files read stack articulations; in `1st Violins KS` four articulations always sound and four never do. Salamander's switches are cleared by a later `<global>`, which matters once Q2 is fixed [Cited S7 §2.1, §2.2, §2.4; S12]. Two related behaviours: the last switch survives a seek or loop wrap while playing (C11), and a switch note also plays any region mapped to its key (C17). VCSL Timpani 1, VSCO `CelloEns-KS`, War Tuba and the jRhodes keyswitch patch put their switch keys inside the instrument's range [Cited S7 §2.1, §2.3, §2.5, §3.3; S10 for the ranges] | One-track articulation changes (the only route that today's tools can drive, by writing switch notes) | Take the switch range from the regions' merged opcodes and the last `sw_default` seen in any scope, as the reference player sfizz does; let a new `<control>` reset only the control scope [Cited S12: suggested fixes for Q1 and Q11]. Pin it with TST-06 tests built from the real header layouts (`sw_default_in_the_first_group_survives_later_groups`, `control_header_keeps_the_enclosing_global_switch`) [Cited S12]. Cheap, and it concerns up to about 75 existing files: 71 Sonatina `* KS` files (35 per set plus the harpsichord), `Organ Combinations`, three VSCO files and the Salamander remap [Cited S1, S2, S7]. How many of them then play correctly is an Estimate. C11 stays: the planner holds each switch note for its whole passage [Cited S7 §3.3]. |
| 4 | **Preset-to-instrument metadata** | DATA | The manifest v1 holds `version`, `id`, `name`, `license` and, per instrument, `path`, `name`, `family` and `articulations`. No field maps a preset to an orchestration id, solo or section, part, key switch, CC or GM program, and no tool, CLI or GUI uses the catalog [Cited S11: `docs/instrument-catalog.md`; Cited S7 §1]. Preset ids survive moving a library only with a manifest `id` or an unchanged folder name (C4). Sonatina's 190 include fragments show up as available presets and as choices of the sampler's `instrument` parameter (C8). Plain discovery also calls a patch with one missing sample available, although it fails to load (N3) [Cited S7 §1, S12] | Everything an agent needs to "know the instruments": choosing a preset for `horn_f` section, `legato` and so on; the whole degradation chain | An articulation map per preset (fields in [Cited S7 §3.2]): instrument id, `ensemble`, `part`, `switch_keys`, articulation `via`, `dynamics`, `legato`, `sonara_verdict`, licence and attribution text. The verdict comes from a deep load plus structural checks, because the loader's ignored-opcode list undercounts (C24, N2) [Cited S12]. Shipped for VSCO 2 CE, VCSL and Sonatina as data. Where it lives is an owner decision [Cited S7, open question 3]. Also exclude include fragments from discovery, add a preset-id-to-path resolver, and give each mapped library a manifest `id`, so projects play on another machine [Cited S7 §1]. |
| 5 | **No grand piano plays as is** | ENGINE (then DATA) | Concert grand: Sonatina and Splendid ‡ (Q3, `$define` inside include paths). Studio grand: the Salamander remap ‡ (Q2: the note regions never load, so it plays only pedal noises; Q7: a pedal-down and a pedal-up noise on every key, transposed; Q1); VCSL Kawai and Steinway B only †, and they lose their release samples under the sustain pedal (N1) [Cited S7 §2.3-2.4, S12]. Logic ships a modelled-mechanics Studio Piano [Card cards/apple.md#2.4] | The most common instrument request outside the orchestra [Estimate]; piano sketches of the orchestration | Fix Q2 (an include after a header on the same line), Q3 (defines in include paths), Q7 (`locc` gates; keys of `-1`) and N1 (start the release regions when the pedal lifts, from a fixed per-key table, RT-02-safe) [Cited S12]. Then re-check Splendid and Salamander. Inspect the FreePats Salamander SFZ (CC BY 3.0), which may already play, and Keppy's Steinway (CC BY-SA 4.0) and the Headroom Piano (CC BY 4.0) [Card cards/free_libraries.md#salamander_grand_piano, #keppys_steinway_piano, #other_acoustic_pianos]. |
| 6 | **Section vs solo, and parts** | DATA, then SOUND | No field records solo or section, or Violin I vs II: neither the orchestration model nor the project schema has one [Cited S11]. Among the inspected libraries, free wind and brass sections exist only in Sonatina, and VSCO's are single instruments [Cited S1, S2, S7 §2.1]. VPO3 lists solo and section files but was not opened [Card cards/free_libraries.md#virtual_playing_orchestra_3]. Logic Studio Strings has a channel per section part [Card cards/apple.md#2.2] | "Horns underneath" means a section, and a solo horn played four times does not sound like one [Estimate]; Vn I and Vn II split | **Data:** `ensemble` and `part` in the map, with the part labels of [ensembles.md](ensembles.md) such as `violins_1` and `violins_2` [Cited S7 §3.2]; the planner's step 3 (solo and section swap) with a message [Cited S7 §4.1]. **Sound:** a free solo-to-section emulation is not attempted here [Estimate: layering one solo recording does not make a section]. |
| 7 | **Articulation delivery** | DATA, UI (CTR-01) | Notes have no articulation field. Changing patch needs a new track, because `instrument` is not automatable. Key switches must be written as ordinary notes and held for the whole passage, because a seek or loop wrap while playing keeps the last switch (C11) [Cited S7 §3.3, S12]. Logic stores an articulation ID per note [Card cards/apple.md#3.1] | Mixed articulations in one part (legato phrase ending staccato, pizzicato bar) without extra tracks | Short term: the planner writes and holds switch notes; the map knows the keys (rank 4). Long term: a per-note articulation field (CTR-01, versioned) and an articulation lane in the piano roll (UI). Owner decision [Cited S7, open question 4]. |
| 8 | **Layer and timbre fidelity (Q6, Q8, Q9)** | ENGINE | Velocity crossfades are ignored: VCSL harp, glockenspiel and marimba, and Sonatina timpani and celesta, sound both layers on every note, each at its normal velocity gain (Q6). Organ regions that set `amp_veltrack=0` still follow velocity (Q8); elsewhere the fixed curve equals the spec's default. Velocity-to-brightness filters are ignored, so soft notes are as bright as loud ones (Q9) [Cited S7 §2.0, S12] | Timpani dynamics (the first instrument for "underneath"); harp; soft string entries | A velocity crossfade gain per voice, set at note-on; `amp_veltrack` as `1 − t + t·(v/127)²` [Cited S12: suggested fixes for Q6 and Q8]; then one filter type with `fil_veltrack` [Estimate]. Each needs an allocation-free implementation and tests (RT-02, RT-05). |
| 9 | **Choir** | SOUND, then ENGINE | The only free SFZ choir inspected is Sonatina's mixed chorus: one vowel, no separate parts, and its Performance version needs CC1 [Cited S7 §2.2; S5]. VPO3 lists female, male and mixed choirs, still without separate parts; not opened [Card cards/free_libraries.md#virtual_playing_orchestra_3]. No free solo voice found [Card cards/voices.md#voice_soprano] | Choir parts in orchestral writing; the `orchestral` template's choir | Use the mixed chorus for every part, limited to each part's range in [model.md](model.md) [Cited S7 §4.3, S10]; inspect VPO3's female and male choirs for a two-part split, under its custom terms; built-in "Choir Pad" otherwise. Real SATB parts and solo voices only through M5 [Card cards/voices.md#choir_soprano]. |
| 10 | **Electric and historical keyboards** | SOUND (clavinet, clavichord, felt and prepared piano); ENGINE (Hammond behaviour); DATA (licence and attribution flags) | Rhodes: jRhodes3d †; its samples are CC BY-NC 4.0, so it is a user install only. Wurlitzer: Greg Sullivan EP200 †, CC BY 3.0 with attribution [Cited S7 §2.5; Card cards/free_libraries.md#electric_pianos]. Clavinet: none free [Card cards/electric_keys.md#clavinet]. Hammond: only static CC0 recordings of a software organ [Card cards/free_libraries.md#freepats]. Logic models the electric piano, the clavinet and the tonewheel organ [Card cards/apple.md#2.5] | Pop and jazz requests (GarageBand parity) more than orchestral ones | Carry the non-commercial limit of jRhodes3d and the attribution of the Wurlitzer in the map's licence field [Cited S7 §3.2]. Inspect the FreePats organ and accordion SFZ. A modelled drawbar organ would have to be registered outside `builtin_registry()`, as the sampler is, because the built-in catalog is pinned by golden data (GLD-01) [Cited S6: ADR 0010 decision 4; Estimate: design]. |
| 11 | **Colour techniques** | SOUND | No free con_sordino for strings, sul_ponticello, trill, flutter_tongue, or harp harmonics; brass mutes are sustain-only [section 2; Cited S1-S3] | "muted strings", "trills in the flutes", "flutter-tongue" | Emulate as the sound mapping's articulation table says, and tell the user [Cited S7 §4.2]. Real techniques through M5 (for example VSL and Spitfire woodwinds list flutter and trills) [Card cards/flutes.md#alto_flute]. |
| 12 | **General MIDI fallback** | ENGINE | No SF2 engine; `rustysynth` was deferred [Cited S6]. The one SFZ GM bank found (Discord) is mostly one-line `sample=*sine` stubs, and Sonara fails each stub patch outright, with silence and a warning instead of a sine (N3); its master file also selects programs with `loprog`, which Sonara ignores [Cited S7 §2.5, §5; Cited S12] | A low-quality voice for the 36 ids with no free SFZ known to play (section 1); a uniform fallback before the built-in synth | An owner decision to add an SF2 player (QG-13 check of `rustysynth`, including a real-time check) and a user-installed MIT SoundFont whose notices travel with it [Cited S7 §5; Card cards/free_libraries.md#musescore_general_and_ms_basic]. |
| 13 | **Saxophones** | ENGINE (Q4), then unverified SOUND | MTG Solo Sax (CC BY 4.0) is ‡ for all four saxophones, because a nested include resolves against the including file's directory (Q4; the baritone was re-read, the other patches share the layout) [Cited S7 §2.5, S12]. Only the tenor is known to play (VCSL, as is) [Cited S7 §2.3]. Karoryfer's Weresax (alto) and Bear Sax (baritone), both CC0, and the FreePats Tenor Saxophone (CC0) were not inspected [Card cards/free_libraries.md#karoryfer_samples, #freepats] | Jazz and pop horn sections (Logic Studio Horns parity [Card cards/apple.md#2.3]) | Resolve nested includes against the main file's directory, as the spec requires [Cited S7 Q4; Cited S12]. That makes the four MTG patches load [Estimate: if nothing else blocks them; their legato groups then hit Q5]. Inspect Weresax and Bear Sax, which may already play partly [Estimate]. The soprano would still have only MTG and the VCSL saxello substitute. |
| 14 | **Library install and browsing** | UI, DATA | The sampler's `instrument` parameter is a select of patch paths: the configured library root joined with the patch's entry, absolute only when the root is [Cited S12: C1]. The catalog has no UI [Cited S11: `docs/instrument-catalog.md`]. VCSL's SFZ files exist only in a tag, and its one release has no uploaded assets, so the user downloads the tag's source archive [Cited S3, S7 §2.3] | First-run experience: a user cannot find "Horns" | A catalog browser by family and instrument id (from rank 4 data), and per-library install steps in the docs, with the attribution each licence requires. |
| 15 | **Instruments with nothing free** | SOUND | No free SFZ source (section 1): bass_flute, oboe_damore, clarinet_eb, wagner_tuba_tenor, wagner_tuba_bass, trumpet_piccolo, cornet, flugelhorn, cimbasso, euphonium, lute, carillon, piano_felt, piano_prepared, clavinet, clavichord, temple_blocks and the six solo voices; also clarinet_a and trumpet_c, which substitute well. That is 25 ids [Card cards/free_libraries.md, open question 12; Estimate: not exhaustive]. maracas has left this list: FreePats World Percussion (CC0) includes maracas [Card cards/free_libraries.md#freepats] | Rare requests | Neighbour substitution with a range check and a message (step 4) [Cited S7 §4.3]. clarinet_a and trumpet_c substitute well, because the samples are at sounding pitch [Cited S7 §4.3]. |

### 3.1 The product sentence, gap by gap

| Part of the sentence | Free sound today | What stands in the way | Ranks |
|---|---|---|---|
| "strings play a legato melody" | Sonatina `1st Violins Legato` and the other legato files; VSCO sustains [Cited S1, S2] | Q5 and Q9 make legato a retriggered monophonic sustain; the legato files reuse sustain samples, so even a fixed engine gives an overlap crossfade, not recorded transitions; no CC phrasing [Cited S12] | 1, 2, 4 |
| "horns … underneath" | VSCO `FHornSus` (solo, as is); Sonatina `Horns Sustain` (section, †) [Cited S1, S2, S7] | No swell (rank 2); section only in Sonatina (rank 6); the agent cannot find the preset by instrument id (rank 4) | 2, 4, 6 |
| "timpani underneath" | VSCO `Timpani`, `TimpaniRolls`; VCSL `Timpani 1 - Hit`, `- Roll` (as is) [Cited S2, S3, S7] | Roll crescendos need dynamics (rank 2). The VCSL key-switch file puts its switch keys 36 and 37 inside the timpani's range (36-61), so use the separate files. Sonatina's timpani doubles its layers (Q6) and fires the roll ending at note-on (Q5) [Cited S7 §2.2, §2.3, S10] | 2, 4, 8 |

## 4. Starter kit recommendation

Constraints:
- ADR 0010 decision 3: users install libraries, and Sonara bundles none [Cited S6].
- LIC-01 bans GPL and AGPL anywhere. It allows LGPL only for replaceable C libraries, and MPL only
  unmodified [Cited S11: `.claude/rules/project.md`].

So the starter kit is documentation: links, install steps, the attribution each licence requires, and
a library manifest or mapping data that Sonara owns. It ships no samples. This file assumes that
recommending a user-installed library does not make it a dependency under LIC-01 [Estimate: to
confirm with the owner, open question 6].

**Core: three installs.**

| # | Library | Licence (per the card) | What it adds | Caveats |
|---|---|---|---|---|
| 1 | VSCO 2 CE, `SFZ` branch | CC0 1.0. The in-repository Readme asks for credit to Versilian Studios and Ivy Audio and for no direct sale of the samples; these are requests, so CC0 governs [Card cards/free_libraries.md#vsco_2_ce; Estimate: legal reading, not legal advice] | Violin (solo and section), viola, cello and bass; solo winds and brass, with horn and trumpet mutes; timpani hits and rolls; mallets; harp; organ; two uprights; orchestral percussion [Cited S2, S7 §2.1] | Mostly plays as is. Three key-switch files are silent until a switch key (Q1), and `CelloEns-KS` puts its switch keys inside the cello's range [Cited S7 §2.1]. The organ and upright piano were sampled by Ivy Audio under a Versilian redistribution grant. Dormant since 2021 [Card cards/free_libraries.md#vsco_2_ce] |
| 2 | VCSL, tag `v1.2.2-RC` (the SFZ build) | CC0 1.0; no credit required | Grand pianos (†), uprights, five harpsichords, pipe and Renaissance organs, concert harp, tenor sax and saxello, timpani, all the mallets, a full set of untuned percussion [Cited S3, S7 §2.3] | The `.sfz` files exist only in the tag's tree, and the release has no uploaded assets [Cited S7 §2.3], so the user downloads the tag's source archive [Estimate]. The pianos lose their release samples under the sustain pedal (N1), and `Timpani 1 - Keyswitch` puts its switches inside the range [Cited S7 §2.3]. The pipe organ is an Ivy Audio recording redistributed under a Versilian permission note; the Knight upright is also Ivy Audio's, with no such note [Card cards/free_libraries.md#vcsl] |
| 3 | Sonatina Symphonic Orchestra, Notation single-articulation files only | CC Sampling Plus 1.0: attribution required; verbatim copies of the whole work only for noncommercial use; retired by Creative Commons in 2011 [Card cards/free_libraries.md#sonatina_symphonic_orchestra] | Wind and brass sections; Violin I and II; solo strings; legato (as retriggered sustain until rank 1 is closed), marcato, col_legno, harmonics; alto flute, English horn, bass clarinet, contrabassoon, bass trombone; mixed chorus; celesta; crotales; castanet samples [Cited S1, S7 §2.2] | All `†`. Avoid its key-switch files (Q11), its Performance set (CC1) and its `Grand Piano` (Q3). It contains Philharmonia samples, whose owner's terms exclude making them available as a sampler instrument, and a few old soundfonts of unknown origin [Card cards/free_libraries.md#sonatina_symphonic_orchestra]. Acceptable for a user install with attribution, never for bundling; a preset map that names it carries the attribution [Cited S7 §2.2] |

Together these give a free source for 48 of the 85 ids, 46 of them with a file that plays as is or
partly [Estimate: section 1 count]. That includes the whole product sentence, apart from true legato.
Keep each switch note held for its passage with every key-switch file, because a seek or loop wrap
while playing keeps the last switch (C11) [Cited S7 §3.3, S12].

**Optional single-instrument add-ons**, each to be inspected before it is recommended:
- FreePats CC0 sets: Upright Piano KW, Old Piano FB (honky-tonk), Nylon-String Guitar, Button
  Accordion HN, Drawbar Organ Emulation, World Percussion (maracas, castanets, tambourine, egg
  shaker) and Tenor Saxophone [Card cards/free_libraries.md#freepats]. Upright Piano KW and Tenor
  Saxophone say their samples are looped: check that the loop points are written in the `.sfz`,
  because Sonara ignores loops stored only in the sample file (N6) [Cited S7 §2.5, S12].
- Karoryfer CC0 sets: Weresax (alto saxophone), Bear Sax (baritone saxophone) and Meatbass (double
  bass) [Card cards/free_libraries.md#karoryfer_samples]; and the CC0 gap fillers already read:
  D. Smolken bass, Bigcat cello and War Tuba [Cited S7 §2.5].
- The FreePats Salamander SFZ (CC BY 3.0, attribution), the best candidate for a playable studio
  grand [Card cards/free_libraries.md#salamander_grand_piano].
- Greg Sullivan's Wurlitzer EP200 (CC BY 3.0, attribution; †) [Cited S7 §2.5].
- jRhodes3d, its `-no-xfade` files (†): the samples are CC BY-NC 4.0, so Sonara may point to it but
  never ships or redistributes it; per its `LICENSE`, music made with it is unrestricted
  [Card cards/free_libraries.md#electric_pianos].

These add 9 ids (guitar_classical, piano_honky_tonk, accordion, organ_hammond, maracas,
saxophone_alto, saxophone_baritone, electric_piano_wurlitzer, electric_piano_rhodes) and a second
source for several more; only the Wurlitzer and Rhodes files were read [Estimate: count from
section 1]. The tenth id from other sets, the mandolin, waits for a licence (below).

**Not recommended for the kit:**
- The FreePats General MIDI sets: GPL 3.0 or later with an exception, which LIC-01 keeps out of
  everything Sonara ships; a user may still install them.
- VPO3: custom web terms over CC BY-SA and Sampling Plus parts and Philharmonia samples; its
  Performance set needs CC1; not inspected.
- Philharmonia samples: their terms forbid making them available as a sampler instrument.
- Iowa MIS: recordings only, with no mapping, and silent on redistribution.
- Keppy's Steinway: CC BY-SA 4.0 under the strictest reading of conflicting sources.
- Maestro Concert Grand: all rights reserved.
- KBH choir: SF2, with unknown provenance.
- The Terkelsen mandolin: it plays as is, but no licence is stated [Cited S7 §2.5].
- MTG Solo Sax and Ixox Flute (CC BY 4.0): they do not load or play correctly until Q4 and Q2 are
  fixed [Cited S7 §2.5].

[Card cards/free_libraries.md; Cited S5]

**After M5.** BBC SO Discover is the cheapest step to a full proprietary orchestra. It costs nothing,
but it runs only in its own plugin and may not be used to build another sample library
[Card cards/free_libraries.md#bbc_so_discover].

## Open questions

1. **Confirm the verdicts by rendering.** Every `†` and `‡` comes from code reading, confirmed by a
   second reading [Cited S7, S12]. One render per Q row, as the sound mapping proposes, would confirm
   them. Add Sonatina `Horns Sustain` and `1st Violins Legato` (the product sentence), VCSL
   `Timpani 1 - Roll`, and a VCSL grand with CC 64 held across note-offs (N1).
2. **Inspect the files no one opened (`?` cells):**
   - the FreePats SFZ sets (Salamander, Upright KW, Old Piano FB, nylon guitar, accordion, organ
     emulations, timpani, xylophone, tubular bells, clarinet, tenor sax, World Percussion);
   - Karoryfer Weresax, Bear Sax, Meatbass and Sneakybass, and War Tuba's other programs;
   - VPO3, Keppy's Steinway and the Headroom Piano;
   - Sonatina's single organ stops, harpsichord registers, xylophone, glockenspiel, chimes, crotales,
     marimba and cymbal files;
   - the Bigcat single programs, and whether any free legato regions (Bigcat, MTG, Ixox) use
     recorded transition samples;
   - VCSL Timpani 2, Renaissance Organ, `Grand Piano, Steinway B` and the other harpsichords.
3. **Sonatina castanets, tambourine, sleigh bells and shaker.** The samples exist [Cited S1]. Which
   keys of `All Unpitched Percussion` play them?
4. **Disagreements between the cards and the sound mapping**, still open in the cards:
   - The keyboard cards say VCSL pianos, harpsichords and organs are WAV only. They read `master`;
     the `v1.2.2-RC` tag holds the `.sfz` [Cited S3].
   - The electric-keys card found no free Wurlitzer; the free-libraries card lists Greg Sullivan's
     EP200 (CC BY 3.0), and the sound mapping read it [Cited S7 §2.5].
   - The historical-keys card says Sonatina's harpsichord patches play, `Harpsichord KS` included;
     the sound mapping finds that file stacks its 4' and 8' registers (Q11) [Cited S7 §2.2].
   - The free-libraries card says VSCO plays "mostly" as is, without the Q1 silence of three
     key-switch files, and calls the Salamander remap "partly", where Q2 makes it play no notes
     [Cited S12].
   - Settled by code reading: the acoustic-pianos card expected Salamander's pedal noises "may never
     fire"; Q7 shows that they fire on every key [Cited S12].
5. **Dynamics without a contract change?** Is an automatable `dynamics` parameter on
   `sonara.sampler`, driven by `create_automation`, acceptable instead of a CC-writing tool? Both
   need sampler crossfade support first (gap rank 2). This is an owner decision.
6. **Does LIC-01 apply to a recommendation?** This file assumes that recommending a user-installed
   library is not a dependency. Is that right? Could Sonara ship a downloader or mapping data for
   Sonatina, given its retired licence and its Philharmonia samples, without that counting as
   distribution under ADR 0010?
7. **Fake legato.** Is an overlap crossfade built on sustain samples, which is what Sonatina's legato
   files give once Q5 and Q9 are fixed, good enough for "legato melody"? Or is true legato a reason to
   bring M5 forward? No free legato-transition library was found; the search was not exhaustive, and
   the Bigcat, MTG and Ixox legato samples were not traced.
8. **A wider search for sound gaps.** A forum lead (not opened) names a free SFZ clavinet on
   bandshed.net, with no licence seen. Choir and clavinet searches covered one aggregator and one
   general search engine only.
9. **GM fallback priority.** 36 ids have no free SFZ known to play. Is a second engine (SF2) worth it
   for them? ADR 0010 deferred it.
10. **Coverage count.** "48 of 85" counts files not opened (crotales, castanets) and substitutes such
    as the mixed chorus for each choir part; "49 known to play" counts partly playing patches. Should a
    coverage metric count only `as is` and `†` patches with the right solo or section?
11. **Licences still open.** The Terkelsen mandolin states none. The Bigcat cello and D. Smolken bass
    rest on GitHub's licence metadata and the catalogue label, with no `LICENSE` file found
    [Cited S7, open question 7; Card cards/free_libraries.md#karoryfer_samples].

## Review notes

Review of 2026-10-30 against the fixed [sound-mapping.md](sound-mapping.md), the fact-checked
[cards/free_libraries.md](cards/free_libraries.md), the instrument cards, [model.md](model.md) and
[sfz-conformance.md](sfz-conformance.md). Section numbers, gap ranks and the numbers of open questions
1-10 are unchanged, because other files cite them; question 11 is new.

**Repository readiness.**
- S6 no longer names a local checkout; it names the branch and says its code equals `main` @ a212544.
- S7 now links [sound-mapping.md](sound-mapping.md); every internal label for that file was replaced
  by "the sound mapping" or a section reference.
- S10 now cites [model.md](model.md) instead of the former id index.
- S11 now cites repository paths instead of a notes file; each fact was re-checked in the code
  (manifest v1 fields, no articulation, part or ensemble field in the project schema, no CC lane in
  `pianoroll/`, LIC-01).
- Added S12, [sfz-conformance.md](sfz-conformance.md).
- Card tags now read `[Card cards/<file>.md#<id>]`; the tag legend no longer names a work folder;
  the gap-type list no longer points to an internal instruction.

**Legato (Summary, section 2 notes, rank 1, 3.1, open question 7).** Kept as both an ENGINE and a
SOUND gap, now stated separately. ENGINE: Q5 (silent remap of `first` and `legato`) and Q9 (the 1 s
`off_time` overlap becomes a 10 ms choke), with the verified trace: the `first` voice is dropped
before it sounds and one monophonic voice with a 0.2 s attack remains. SOUND: Sonatina's legato groups
include the sustain samples, so a fixed engine gives an overlap crossfade, not transitions; the
Bigcat, MTG and Ixox legato samples were not traced, which the earlier wording overstated, and VPO3's
documented articulations name no legato. Rank 1's
fix now names the Q5 and Q9 fixes and the pinning tests from sfz-conformance.md.

**Code claims.** Q legend rewritten (Q1 any later header, Q5 silent, Q6 normal velocity gain, Q8 only
non-default `amp_veltrack`, Q9 `off_mode`), and N1, N3 and N6 added. N1 now appears in the VCSL
piano cells, the matrix, the piano note, rank 5 and the kit; N3 in rank 4 and rank 12 (Discord stubs
fail outright); N6 on the looped FreePats sets. C22: the timpani Drum Kit fallback is toms 41, 43,
45, 47, 48 and 50 only. C1: rank 14 no longer says the `instrument` choices are absolute paths. C4,
C8 and C24 in rank 4; C11 and C17 in ranks 3 and 7 and the kit. Switch keys inside the playable
range, checked against [model.md](model.md): VSCO `CelloEns-KS` (84-87), War Tuba (24-26) and the
jRhodes keyswitch patch (103-108), besides VCSL Timpani 1. The Salamander cell now says it plays only
pedal noises (Q2), and the flute, horn, trumpet and harpsichord key-switch cells say what stacks.

**New and moved sources (free-libraries fact-check).** Weresax moved to saxophone_alto and Bear Sax to
saxophone_baritone (they had sat under the tenor); Meatbass and Sneakybass added to the double bass;
FreePats World Percussion added to tambourine, castanets, maracas and shaker; FreePats Tenor
Saxophone flagged for N6; LivingRoom Upright added; VPO3 cells added wherever the card's VPO3 column
lists the instrument (marimba noted as absent).

**Licences.** VSCO: CC0 governs, the Readme's credit and no-sale wording are requests. VCSL: Ivy Audio
organ and Knight upright noted; release has no uploaded assets. Sonatina: attribution required,
noncommercial whole-work copies, retired licence, Philharmonia samples. jRhodes3d settled as CC BY-NC
4.0 (user install only) and Greg Sullivan as CC BY 3.0, so both moved from "wait" to the add-ons, with
their limits. Bigcat and D. Smolken: CC0 by GitHub metadata, no `LICENSE` found. Keppy's Steinway:
CC BY-SA 4.0. Splendid: public domain as a relayed claim. Terkelsen: no licence, so not recommended.
MTG and Ixox: CC BY 4.0. A licence summary was added to 1.8.

**Counts.** Recounted from section 1: 48 ids with a core-library source (unchanged), 46 of them known
to play; other sets now add 10 ids (was about 7: alto and baritone saxophones and maracas joined);
25 ids have nothing free (was about 26: maracas left); 36 ids have nothing known to play (rank 12 and
open question 9 said "about 30"). The add-ons now add 9 ids (was about 4). The ‡-only statement for
the alto and baritone saxophones was withdrawn, since CC0 sets exist for both.

**Contradictions with the cards, fixed here.** GM fallbacks: cimbasso ≈ 58 Trombone, mandolin ≈ 26,
piano_prepared "1 only as a last resort", wood_block adds program 116 (model.md), suspended cymbal
leads with the card's 49; card-named fallbacks are marked "(card)". The VSCO triangle and suspended
cymbal cells now say which WAVs are not mapped. The Sonatina crotales, xylophone and glockenspiel
cells cite the tuned-percussion card's opcode reading. The tuba's War Tuba "staccatissimo" stays a
quoted library label, since it is not one of the 15 ids.

**Not editable here.** [open-questions.md](open-questions.md) A10 still says "about 30" ids; the
count is now 36. The card disagreements in open question 4 remain in the cards.
