# Mapping instruments to sound

_Synthesized 2026-10-30 from the research cards in [cards/](cards/); code claims re-checked against
[sfz-conformance.md](sfz-conformance.md). Status: research; nothing here is implemented._

## Sources

| # | Source | URL | Kind | Opened in this session |
|---|---|---|---|---|
| S1 | VSCO 2 Community Edition, GitHub REST tree of branch `SFZ` (75 `.sfz`, all at the branch root) | https://api.github.com/repos/sgossner/VSCO-2-CE/git/trees/SFZ?recursive=1 | library maker | yes |
| S2 | VSCO 2 CE patches read in full: `ViolinEns-KS`, `SViolin-KS`, `ViolaEns-KS`, `CelloEns-KS`, `Contrabass-KS`, `Clarinet-KS`, `Flute-KS`, `Tuba-KS`, `GM-StylePerc`, `Timpani`, `TimpaniRolls`, `UprightPiano`, `VSUpright1`, `Harp`, `OrganLoud`, `FHornMute`, `TrumpetSusVib`, `Glockenspiel`, `Marimba`, `ViolinEnsSusVib`, `CelloEnsPizz`, `OboeSusVib` (22 files) | https://raw.githubusercontent.com/sgossner/VSCO-2-CE/SFZ/ + file name | library maker | yes |
| S3 | Sonatina Symphonic Orchestra, GitHub REST tree of `master` (747 `.sfz`) | https://api.github.com/repos/peastman/sso/git/trees/master?recursive=1 | library maker | yes |
| S4 | Sonatina patches read with their `#include` files: Strings - Notation `1st Violins KS`, `1st Violins Legato`, `1st Violins Sustain`, `Celli Pizzicato`, `Basses KS`, `Violin Solo 1 KS`; Strings - Performance `1st Violins Sustain`; Brass - Notation `Horn Solo KS`, `Trumpets KS`, `Trumpets Marcato`; Woodwinds - Notation `Flutes KS`, `Flutes Staccato`, `Clarinet Solo KS`, `Oboe Solo Sustain (looped)`; Percussion `Timpani`, `All Unpitched Percussion`, `Vibraphone`, `Celeste`; `Organ/Organ Combinations`; `Harpsichord/Harpsichord KS`; `Grand Piano/Grand Piano`; `Concert Harp`; Chorus - Notation and - Performance `Mixed Chorus` (23 files) | https://raw.githubusercontent.com/peastman/sso/master/Sonatina%20Symphonic%20Orchestra/ + path | library maker | yes |
| S5 | VCSL releases (one release, tag `v1.2.2-RC`, "SFZ Build v1.2.2", no uploaded assets; re-queried 2026-10-30) | https://api.github.com/repos/sgossner/VCSL/releases | library maker | yes |
| S6 | VCSL, GitHub REST tree of tag `v1.2.2-RC` (183 `.sfz`) | https://api.github.com/repos/sgossner/VCSL/git/trees/v1.2.2-RC?recursive=1 | library maker | yes |
| S7 | VCSL patches read: `Grand Piano, Kawai`, `Upright Piano, Knight`, `Harpsichord, Flemish - Keyswitch`, `Concert Harp`, `Pipe Organ - Keyswitch`, `Tenor Saxophone - Keyswitch`, `Timpani 1 - Keyswitch`, `Xylophone - Keyswitch`, `Triangles`, `Snare Drum, Modern 1`, `Suspended Cymbal 1`, `Tubular Bells 1`, `Glockenspiel`, `Marimba`, `Woodblock`, `Clash Cymbals 1`, `Gong 1`, `Slapstick`, `Bass Drum 1`, `Vibraphone - Soft Mallets`, `Tambourine 1`, `Sleigh Bells`, `Shaker, Small` (23 files) | https://raw.githubusercontent.com/sgossner/VCSL/v1.2.2-RC/ + path | library maker | yes |
| S8 | Salamander Grand Piano V3 (sfzinstruments remap by kinwie): tree, `Salamander Grand Piano V3.sfz`, `Data/notes.txt`, `Data/region.txt`, `Data/pedal.txt`, `Data/vel_01.txt`, `Data/hammer.txt` | https://github.com/sfzinstruments/SalamanderGrandPiano | library maker | yes |
| S9 | sfzinstruments organisation: repository list with GitHub licence metadata | https://api.github.com/orgs/sfzinstruments/repos?per_page=100 | other (catalogue) | yes |
| S10 | Discord SFZ GM Bank: README, tree, `Discord GM Bank - Melodic.sfz`, `Melodic/041-Violin`, `048-Timpani`, `049-String Ensemble 1`, `053-Choir Aahs`, `061-French Horn`, `Drums/049-Orchestra Kit` | https://github.com/sfzinstruments/Discord-SFZ-GM-Bank | library maker | yes |
| S11 | MTG Solo Saxophones: tree, `MTG Baritone Sax.sfz`, `MTG Baritone Sax (NL).sfz`, `Data/group_bar.txt`; HTTP status of `Data/bar_f_rr1.txt` (200) and `Data/Data/bar_f_rr1.txt` (404) | https://github.com/sfzinstruments/MTG.SoloSax | library maker | yes |
| S12 | Karoryfer and Bigcat open source cello: tree, `Programs/01- Bowed (velocity layer).sfz`, `02- Bowed (mod wheel).sfz`, `03- Plucked.sfz` | https://github.com/sfzinstruments/karoryfer-bigcat.cello | library maker | yes |
| S13 | D. Smolken double bass: tree, `d_smolken_rubner_bass_switched.sfz`, `d_smolken_rubner_bass_arco.sfz` | https://github.com/sfzinstruments/dsmolken.double-bass | library maker | yes |
| S14 | Splendid Grand Piano: tree, `Splendid Grand Piano.sfz` | https://github.com/sfzinstruments/SplendidGrandPiano | library maker | yes |
| S15 | jRhodes3d: tree, `jRhodes3d-st-no-xfade.sfz`, `jRhodes3d-keyswitch.sfz` | https://github.com/sfzinstruments/jlearman.jRhodes3d | library maker | yes |
| S16 | Greg Sullivan E-Pianos: tree, `Wurlitzer EP200/Wurlitzer EP200.sfz` | https://github.com/sfzinstruments/GregSullivan.E-Pianos | library maker | yes |
| S17 | Ixox Flute: tree, `Ixox Flute.sfz` | https://github.com/sfzinstruments/Ixox.Flute | library maker | yes |
| S18 | Karoryfer War Tuba: tree, `Programs/2-solo-poly.sfz` | https://github.com/sfzinstruments/karoryfer.war-tuba | library maker | yes |
| S19 | Terkelsen Mandolin: tree, `Terkelsen's Mandolin.sfz` | https://github.com/sfzinstruments/Terkelsen.Mandolin | library maker | yes |
| S20 | sfzformat.com: `#include` | https://sfzformat.com/opcodes/include | spec | yes |
| S21 | sfzformat.com: `trigger` | https://sfzformat.com/opcodes/trigger | spec | yes |
| S22 | sfzformat.com: `sw_default` | https://sfzformat.com/opcodes/sw_default | spec | yes |
| S23 | sfzformat.com: `sw_last` | https://sfzformat.com/opcodes/sw_last | spec | yes |
| S24 | sfzformat.com: `xfin_lovel` | https://sfzformat.com/opcodes/xfin_lovel | spec | yes |
| S25 | sfzformat.com: `loccN` | https://sfzformat.com/opcodes/loccN | spec | yes |
| S26 | sfzformat.com: `on_loccN` | https://sfzformat.com/opcodes/on_loccN | spec | yes |
| S27 | sfzformat.com: `loprog` | https://sfzformat.com/opcodes/loprog | spec | yes |
| S28 | Spitfire Audio support: "Switching articulations in Spitfire libraries" | https://support.spitfireaudio.com/en/articles/11816120-switching-articulations-in-spitfire-libraries | library maker | yes |
| S29 | Wikipedia: General MIDI (program list, percussion key map, GS extension notes) | https://en.wikipedia.org/wiki/General_MIDI | encyclopedia | yes |
| S30 | rustysynth README | https://github.com/sinshu/rustysynth | other (library) | yes |
| S31 | Sonara repository, read only (branch `research/orchestra-instruments` @ dc96c29): `crates/sonara-media/src/sfz.rs`, `crates/sonara-dsp/src/bank.rs`, `crates/sonara-dsp/src/processors/sampler.rs`, `crates/sonara-engine/src/schedule.rs`, `crates/sonara-core/src/plugins/builtin.rs`, `crates/sonara-midi/src/gm.rs`, `crates/sonara-midi/src/orchestration/articulation.rs`, `crates/sonara-app/src/samples.rs`, `crates/sonara-app/src/instruments/model.rs`, `crates/sonara-agent/src/contract/args.rs`, `crates/sonara-project/schemas/project-v1.schema.json`, `docs/agent-api.md`, `docs/decisions/0010-sample-based-orchestral-instruments.md`, `docs/instrument-catalog.md`, `docs/sample-libraries.md`, `docs/orchestration.md`, `.claude/rules/project.md` | this repository, branch `research/orchestra-instruments` @ dc96c29 | other | yes |
| S32 | sfzformat.com: `<control>` header | https://sfzformat.com/headers/control | spec | yes |
| S33 | Sonatina key-switch files re-read for header order, with the first-level includes of `1st Violins KS`, `Basses KS`, `Violin Solo 1 KS`, `Horn Solo KS`, `Trumpets KS`, `Flutes KS`, `Clarinet Solo KS`, `Harpsichord KS`, `Organ Combinations` | https://raw.githubusercontent.com/peastman/sso/master/Sonatina%20Symphonic%20Orchestra/ + path | library maker | yes |
| S34 | Verification of this file's code claims against Sonara's source, with spec pages and the sfizz reference player: [sfz-conformance.md](sfz-conformance.md) (Q1-Q11 confirmed; C1-C24; new behaviours N1-N7, of which N7 is minor and not listed here) | [sfz-conformance.md](sfz-conformance.md) | other (verification) | yes |

Tags: `[Cited Sn]` is a row above. `[Card cards/<file>.md#<id>]` is an instrument or library card
in [cards/](cards/), read for this research (its own sources are listed in that card).
`[Estimate: reason]` is this author's inference. Statements marked "code reading" were derived
by reading Sonara's source; a second, independent reading confirmed them [Cited S34]. No library
was loaded and nothing was rendered or listened to. Where sfizz is mentioned, it is a reference
player, not the specification.
Opened but not used as a source of fact: Apple's Articulation Set Editor page (the fetch returned
only the table of contents), Steinberg's Cubase and Dorico manuals (HTTP 403, a redirect to a PDF
and a truncated page) and the MIDI Association's GM1 page (no tables on the page). Apple's
articulation facts below come from the Apple card instead.

## Summary

- **Key everything by catalog preset id.** The catalog already has an id,
  `<library id>/<patch path without .sfz>`, which stays stable when the library declares its `id`
  in a manifest. The sampler and the project still store the patch's file path, so a mapping
  keyed by id needs a resolver from id to path (section 1).
- **Free SFZ libraries cover the core orchestra.** VSCO 2 CE (75 patches), Sonatina
  (557 patches plus 190 include fragments) and VCSL (183 patches) together voice strings,
  woodwinds, brass, timpani, mallets, the orchestral untuned percussion, harp, harpsichord,
  organ and pianos [Cited S1, S3, S6]. Gap fillers: D. Smolken double bass, Karoryfer/Bigcat
  cello, Karoryfer War Tuba, Terkelsen mandolin [Cited S9, S12, S13, S18, S19]. None is bundled
  (ADR 0010); VSCO and VCSL are CC0, while Sonatina's CC Sampling Plus 1.0 requires attribution
  and Sonatina contains Philharmonia samples [Card cards/free_libraries.md#sonatina_symphonic_orchestra].
- **Most patches play only partly in Sonara.** This research classified 90 files across 14
  libraries. Sonara plays the plain single-articulation patches of VSCO and VCSL as is. Eight
  parser and sampler behaviours that differ from the SFZ spec break or degrade the rest (Q1-Q7
  and Q11 in section 2.0). They are:
  - key-switch settings that are read only at the end of the file, so any later header at the
    same or an outer level drops them;
  - a `<control>` header that wipes the switch settings of the articulations that follow;
  - an `#include` written mid-line, or one whose path holds a `$define`;
  - nested includes resolved against the wrong directory;
  - `first`, `legato` and `release_key` regions played as plain attacks, without any warning;
  - velocity crossfades and CC gates ignored.

  A second reading of the source confirmed all eleven Q rows and found one more behaviour with
  hits: release samples never play for a key released while the sustain pedal is down (N1)
  [Cited S34]. Code reading only; nothing was run.
- **Three VSCO key-switch patches are silent until a switch key is played.** These are
  `ViolinEns-KS`, `Flute-KS` and `Clarinet-KS`. This corrects the free-libraries card's
  "mostly as is" for `ViolinEns-KS` [Cited S2, S31, S34; Card cards/free_libraries.md#vsco_2_ce].
- **Most Sonatina key-switch patches stack articulations in Sonara.** Their included
  articulation files open with a `<control>` header. Sonara clears the enclosing `<global>`
  switch settings at that header, so those articulations sound on every note whatever switch is
  played. In `Flutes KS` and `1st Violins KS` the switches stop working entirely. Of the
  key-switch files checked, only `Clarinet Solo KS` escapes this. Use Sonatina's
  single-articulation files [Cited S33, S31, S34; code reading, not run].
- **Articulations are realised in seven ways.** These are separate patch, sticky key switch,
  CC or CC32/UACC, program change, MIDI channel, velocity zone and note overlap, plus Logic's
  per-note articulation ID. Today Sonara can drive only two of them: separate patches (one track
  each) and key switches written as ordinary notes (section 3).
- **The degradation chain works today except its General MIDI step.** No SF2 engine exists,
  because ADR 0010 deferred rustysynth. The one SFZ General MIDI bank found is mostly stubs,
  and Sonara fails each stub patch outright instead of playing its generator (N3)
  [Cited S10, S31, S34]. So today the chain falls from "family neighbour" straight to the
  built-in Poly Synth or Drum Kit (section 4).

## 1. How Sonara reaches sound today, and the key gap

| Fact | Source |
|---|---|
| A sampled track is a `sonara.sampler` device whose enum parameter `instrument` lists the installed patches' `.sfz` paths; the project stores that path. Each path is the configured library root joined with the patch's entry, not canonicalized, so it is absolute only when the configured root is. | [Cited S31: builtin.rs `sampler_descriptor`, samples.rs `scan_patches`, ADR 0010 decision 4; Cited S34: C1] |
| The `instrument` parameter is built with `automatable: false`, so a track cannot change patch during a clip. | [Cited S31: builtin.rs `ep`] |
| A patch that is missing or unreadable renders as silence with a warning; it does not fail the render. | [Cited S31: ADR 0010 decision 5] |
| The catalog id is `<library id>/<patch path below the library root, without the extension>`, for example `vsco2ce/Strings/Violin Solo`. It survives moving the library to another parent folder, but renaming the library folder changes every id unless a manifest declares the `id`. | [Cited S31: instruments/model.rs `InstrumentId::new`, docs/instrument-catalog.md; Cited S34: C4] |
| The library id is the manifest's `id`; without a manifest it is a slug of the directory name (only ASCII letters and digits are kept, lower-cased, other runs become `-`; at most 64 characters). | [Cited S31: model.rs `LibraryId::from_directory_name`; Cited S34: C5] |
| No agent tool, CLI command or GUI uses the catalog yet. | [Cited S31: docs/instrument-catalog.md "Not done"; Cited S34: C6] |
| The scan lists every `.sfz` file below a root (8 levels; hidden entries and symbolic links skipped; extension matched without regard to case). The sampler's cap of 5,000 patches covers all roots together. | [Cited S31: samples.rs; Cited S34: C7] |

**Assumptions in this file.** Library ids are `vsco2ce`, `sso`, `vcsl`, `salamander`, `splendid`,
`dsmolken-bass`, `bigcat-cello`, `war-tuba`, `mtg-sax`, `ixox-flute`, `terkelsen-mandolin`,
`jrhodes3d`, `gs-epianos` and `discord-gm`, each declared in a `sonara-library.json` manifest
[Estimate: chosen names]. Without a manifest, a checkout of VSCO 2 CE in a folder `VSCO-2-CE`
would get the id `vsco-2-ce` [Cited S31: model.rs slug rule], and renaming that folder would
change every preset id [Cited S34: C4]. So a mapping keyed by preset id needs the manifest. The
`sso` root is the inner folder `Sonatina Symphonic Orchestra/`, which holds both the patches and
`Samples/` [Cited S3]. The `vcsl` root is the repository root at tag `v1.2.2-RC` [Cited S6].

**The gap.** These are the steps that keying by preset id still needs.

1. **Resolve the id at load time.** Something must turn the preset id into the current
   `patch_path` and, where the project stores a path, back again. The project and the sampler
   parameter hold only the patch's file path today (the configured root joined with the entry), so a project opened on another machine plays
   silence [Cited S31: docs/sample-libraries.md]. The mapping data in this file is keyed by
   preset id so that it survives that resolver change [Estimate: design].
2. **Exclude include fragments.** Sonatina's `includes/` folders hold 190 `.sfz` fragments. The
   scan lists them as presets because it takes every `.sfz` file [Cited S3, S31]. A fragment has
   no regions or names samples relative to another file. A deep `verify` marks it `invalid`
   (it runs the loader), but plain discovery shows it as `available`, because discovery only
   checks that the file is a readable, non-empty file inside the library. The same scan also
   offers every fragment as a choice of the sampler's `instrument` parameter [Cited S34: C8].
3. **Keep the file name exactly.** Ids keep file names verbatim, typos included. Sonatina has
   `Chorus - Notation/Large Chrous`, so the preset id is `sso/Chorus - Notation/Large Chrous`
   [Cited S3].

**Contract facts that constrain how an articulation can be sent.** All five are CTR-01 or engine
facts.

| Fact | Source |
|---|---|
| A MIDI note has id, pitch, start, length, velocity and channel. It has no articulation field. | [Cited S31: project-v1.schema.json `note`, contract/args.rs NoteInput] |
| No agent tool writes MIDI CC into a clip: neither the tool contract nor the agent API doc mentions controllers. Clip `controls` kinds are cc, pitch bend and channel pressure, with no program change. | [Cited S31: args.rs, agent-api.md, schedule.rs `ControlKind`] |
| The sampler ignores the MIDI channel at note-on and uses it only to match note-offs, so one sampler plays every channel. The key-switch state is shared by all channels too. | [Cited S31: sampler.rs `note_on`, `note_off`; Cited S34: C9] |
| The engine sorts notes stably by start frame. Within one block it sends note-offs, then note-ons, then controller events. Two notes that start on the same frame keep their clip order. | [Cited S31: schedule.rs] |
| After a seek, the engine sends an all-notes-off, then a note-on for every note that started before and ends after the new position (the chase). A short key-switch note that ended earlier is not re-sent. A seek while playing, and a loop wrap, do not reset the sampler, and all-notes-off leaves the switch alone, so the switch last pressed before the jump stays active. The sampler's `rest()`, which restores `sw_default`, runs only when the graph is prepared or reset: on play from stop and in an offline render. | [Cited S31: schedule.rs, sampler.rs `rest`; Cited S34: C11, player `control.rs`, `graph/live.rs`] |

## 2. Real patch inventories and how Sonara plays them

### 2.0 How the classification was made

Each file was downloaded as text and its opcodes were compared with the sampler's supported list.
That list is `sample`, `default_path`, `key`, `lokey`, `hikey`, `pitch_keycenter`, `lovel`,
`hivel`, `lorand`, `hirand`, `seq_length`, `seq_position`, `sw_lokey`, `sw_hikey`, `sw_default`,
`sw_last`, `trigger`, `group`, `off_by`, `loop_mode`, `loop_start`, `loop_end`, `offset`, `end`,
`tune`, `transpose`, `volume`, `ampeg_attack` and `ampeg_release`, plus the aliases `loopmode`,
`loopstart` and `loopend`. The labels `sw_label`, `group_label`, `region_label` and `sw_note` are
harmless [Cited S31: sfz.rs `SUPPORTED`, `HARMLESS`; Cited S34: C12]. Every other opcode "has no
effect" [Cited S31: sfz.rs module doc]. The loader reports the ignored opcodes, but that list
undercounts: unsupported `trigger` and `loop_mode` values are remapped silently, and opcodes under
`<curve>`, `<effect>`, `<midi>` and `<sample>` headers are dropped without being reported (N2)
[Cited S34]. Verdicts:

- **as is**: only harmless or no-effect opcodes whose loss changes nothing audible
  (`ampeg_dynamic`, `label_cc*`) [Estimate: Sonara has no CC-driven envelope, so recalculating
  one has nothing to act on];
- **partly**: the patch loads and sounds, but some layers, controls or behaviours are lost or wrong;
- **no**: the patch fails to load, or its main regions never sound.

Sonara behaviours that decide the verdicts (code reading of S31, not run). A second reading of
the source confirmed every Q row [Cited S34]. It re-read these hits in the library files: VSCO
`ViolinEns-KS`, `SViolin-KS`, `Flute-KS`, `Clarinet-KS`, `GM-StylePerc` and `ViolinEnsSusVib`; Sonatina `1st Violins KS`,
`Flutes KS`, `Timpani`, `1st Violins Legato` and `Grand Piano`; Salamander V3; MTG Baritone Sax.
The other hits rest on the first reading listed in the Sources table. Rows N1-N6 are behaviours
the second reading found. N1 has audible hits; N2 affects only the loader's warnings; N3 is latent (it would hit the Discord stubs,
which were not re-read); N4 and N6 have no hit in the files read, and N5 one harmless hit (Sonatina `Grand Piano`,
`<curve>curve_index=11`) [Cited S34; [sfz-conformance.md](sfz-conformance.md)].

| # | Sonara behaviour | SFZ meaning | Effect | Hits |
|---|---|---|---|---|
| Q1 | `sw_lokey`/`sw_hikey`/`sw_default` are read once, from the header scopes still open at the end of the file. Any later header at the same scope level or an outer one clears them: `<control>` clears all four scopes, `<global>` the inner three, `<master>` the inner two, and a later `<group>` without `sw_default` drops a group-level one. Values written at `<region>` level are never read (N4). The reference player sfizz instead takes the last `sw_default` parsed in any scope [Cited S34]. | `sw_default` preselects an articulation so a patch is not silent; without it SFZ 1/2 instruments stay silent until a key switch is pressed [Cited S22, S23] | default lost: silent until a switch key; or switches lost entirely: all `sw_last` regions silent | VSCO `ViolinEns-KS`, `Flute-KS`, `Clarinet-KS` (sw_default only in the first `<group>`, each articulation starts a new `<control>`) [Cited S2, S34]; Salamander V3 (`sw_*` in the first of three `<global>` blocks; moot while Q2 holds) [Cited S8, S34] |
| Q2 | `#include` is honoured only at the start of a line. | the spec page is silent on placement; the reference player sfizz reads an include wherever a token starts [Cited S20, S34] | the included regions are never loaded. In Salamander the stray include is even appended to the preceding opcode's value (`hivel` falls back to 127); the patch loads and plays pedal noises but no piano notes | Salamander `Data/notes.txt` (every note group) [Cited S8, S34]; Discord master; Ixox Flute [Cited S10, S17] |
| Q3 | `$NAME` defines are substituted in opcode lines, not in `#include` paths. | the spec's `#define` page shows variables only in opcode values; sfizz expands them in include paths [Cited S34] | file `$DIR/...` not found: load error, silence plus warning | Sonatina `Grand Piano` (`#include "$DIR/$DYN.txt"`) [Cited S4, S34]; Splendid Grand Piano [Cited S14] |
| Q4 | A nested `#include` resolves against the including file's directory. | the SFZ "is interpreted as if it was in the main SFZ file's path" [Cited S20]; sfizz does so [Cited S34] | load error: silence plus a warning for the whole patch | MTG Solo Sax: `Data/group_bar.txt` includes `Data/bar_p_rr1.txt`, which exists only relative to the main file [Cited S11, S34] |
| Q5 | Every `trigger` other than `release` becomes an attack. The remap is silent: `trigger` is a supported opcode, so `first`, `legato` and `release_key` never appear in the loader's ignored-opcode warning (likewise an unknown `loop_mode` value becomes no loop) [Cited S34]. | `first` = only when no note sounds; `legato` = only while another note sounds; `release_key` = on note-off, ignoring the pedal [Cited S21] | legato pairs both fire; `release_key` one-shots sound at note-on | Sonatina legato and KS files, Timpani rolls, All Unpitched Percussion, Organ Combinations [Cited S4]; Bigcat cello, MTG sax, Ixox flute [Cited S12, S11, S17] |
| Q6 | `xfin_*`/`xfout_*` velocity crossfades are ignored. | a region fades in across a velocity span [Cited S24] | where layers rely on crossfades instead of `lovel`/`hivel`, both layers sound on every note, each at its normal velocity gain without the crossfade attenuation | Sonatina Timpani (f and p layers), Celeste [Cited S4, S34]; VCSL Concert Harp, Glockenspiel, Marimba [Cited S7]; jRhodes keyswitch file [Cited S15] |
| Q7 | `locc`/`hicc` gates are ignored, so gated regions always play. A key value of `-1` fails to parse and falls back to the full 0-127 range. | a region plays only while CC N is in range [Cited S25]; `on_locc` regions are triggered by a CC, not by keys [Cited S26]; SFZ 2 uses `lokey=-1` to keep a region off the keys [Cited S34] | CC-switched layers stack; Salamander's pedal-noise regions (`lokey=-1 hikey=-1`) fire on every key: each note-on starts one pedal-down and one pedal-up noise, transposed by key − 60 because `pitch_keycenter` defaults to 60 | Bigcat cello CC107 "Voices" layers [Cited S12]; Salamander `Data/pedal.txt` [Cited S8, S34]; Sonatina and Splendid piano pedal layers [Cited S4, S14] |
| Q8 | Gain is `volume × velocity²` for every region; `amp_veltrack`, `amp_velcurve_*` and the other `*_veltrack` opcodes are ignored. This fixed curve equals the spec's default (`amp_veltrack=100`) [Cited S34]. | per-region velocity response | only regions that set a non-default `amp_veltrack` or `amp_velcurve_N` diverge: organ and noise regions that set `amp_veltrack=0` still follow velocity | Sonatina Organ Combinations, Harpsichord KS, Performance strings; VCSL Pipe Organ KS, Knight upright, Flemish harpsichord [Cited S4, S7] |
| Q9 | `ampeg_decay`/`ampeg_sustain`/`ampeg_hold`, filters (`fil_type`, `cutoff`, `fil_veltrack`), LFOs, `pan`/`width`, `off_mode`/`off_time`, `note_polyphony`, `pitch_keytrack`, `rt_decay` and `offset_random` are ignored. Every choke uses a fixed 10 ms release. | envelope shape, velocity brightness, vibrato, overlap time; the spec's default `off_mode` is a fast (6 ms) release [Cited S34] | "(looped, decay)" variants hold like plain loops; soft notes are as bright as loud ones; Sonatina legato's 1 s overlap (`off_mode=time off_time=1`) becomes a 10 ms choke | most Sonatina and Karoryfer files [Cited S4, S12, S18, S34] |
| Q10 | A region with an empty key range (`lokey` above `hikey`) never matches. | n/a | harmless for key triggering; the CC-triggered sound of those regions is lost anyway (Q7) | VCSL Knight upright pedal regions (`lokey=127 hikey=0`, `lokey=22 hikey=0`) [Cited S7] |
| Q11 | A `<control>` header clears all four scopes: `<control>`, `<global>`, `<master>` and `<group>`. | `<control>` holds `default_path`, `set_cc`, `label_cc` and the like; in ARIA a new one "resets default_path only"; the page says nothing of clearing `<global>` [Cited S32]. sfizz resets only the control settings on a new `<control>` [Cited S34] | a `<global>` that carries `sw_last` and then includes a file starting with `<control>` loses its `sw_last`, so those regions play under every switch. If the file ends after such a `<control>`, the switch range is lost too, and the switched regions never play. | Sonatina KS files (table in 2.2) [Cited S33, S34] |
| N1 | A key released while the sustain pedal (CC64) is down never starts its `trigger=release` regions: note-off returns early, and pedal-up only releases the held voices. | release regions play "on note-off or sustain pedal off" [Cited S21]; sfizz starts them when the pedal lifts [Cited S34] | pedalled piano passages lose their release samples (damper and hammer noise, string-resonance tails). Only CC64 from `import_midi` or live input triggers it, because no tool writes CC | VCSL Kawai grand and the Knight upright (the Steinway B SFZ was not opened: unconfirmed); the Salamander V3 remap's release sections, which load today because they are included at the start of a line; Sonatina Grand Piano unconfirmed (it fails under Q3 anyway) [Cited S34; [sfz-conformance.md](sfz-conformance.md), N1] |
| N2 | The ignored-opcode list undercounts: remapped `trigger`/`loop_mode` values and opcodes under `<curve>`, `<effect>`, `<midi>`, `<sample>` are not reported. | n/a | the render warning understates the losses; a verdict cannot rest on the list alone | Salamander and jRhodes (`<curve>`); Sonatina Vibraphone and Celeste (`<curve>`, per the first reading); every Q5 file [Cited S34] |
| N3 | One sample that cannot be opened fails the whole patch, including the spec's generator samples (`*sine`, `*silence`). | `*silence` is a valid sample in the spec's examples [Cited S34] | one missing file in a large patch silences the instrument; a `sample=*sine` stub gives silence plus a warning, not a sine | Discord GM stubs [Cited S10, S34]; any patch with a missing file |
| N4 | `sw_lokey`/`sw_hikey`/`sw_default` written at `<region>` level are never read (a variant of Q1). | n/a | the switch range is lost | none found [Cited S34] |
| N5 | A header glued to its first opcode (`<region>sample=a.wav`) is not recognised, so the region is lost. | sfzformat.com's own `#define` example is written that way [Cited S34] | the region never loads | one harmless hit: Sonatina `Grand Piano` writes `<curve>curve_index=11`, and the unsupported `<curve>` is skipped anyway [[sfz-conformance.md](sfz-conformance.md), N5] |
| N6 | Loop points embedded in a sample file (WAV `smpl` chunk) are ignored: without `loop_mode`, a region does not loop, and `loop_continuous` without `loop_start`/`loop_end` loops the whole region from its first frame. | the default `loop_mode` is `loop_continuous` for samples with defined loops [Cited S34] | a sustain sample whose loop lives only in the file stops at its end | none found: the VSCO sustain WAV re-read has no `smpl` chunk, and Sonatina's looped includes give explicit points [Cited S34] |

Velocity gain, channel handling and the release scale come from the sampler [Cited S31:
sampler.rs `start_voice`, `note_on`]. Legato in Sonatina under Q5 has a known outcome. Both groups
match every note-on, and both start at `offset=20000`. The first group's region chokes the
previous note's voices over 10 ms and starts its own voice at level 0. The second group's region
then chokes that new voice, which is dropped before it makes any sound, and starts the legato
group's voice. So every note restarts a monophonic sustain with the `trigger=legato` group's
0.2 s attack instead of the `trigger=first` group's 0.30 s [Cited S4: groups
`trigger=first`/`legato`, `group=1 off_by=1`, `offset=20000`; Cited S31: sampler.rs `choke`;
Cited S34: trace of one note-on]. How bad that sounds is a render question.

### 2.1 VSCO 2 Community Edition (`vsco2ce`, CC0, branch `SFZ`)

Licence: the `LICENSE` file is CC0 1.0 on both branches. The in-repository `Readme.txt` asks
users to credit Versilian Studios and Ivy Audio, not to sell the samples directly, and to keep
improvements open; these are requests, so CC0 governs [Estimate: legal reading, not legal advice].
The organ and upright piano were sampled by Simon Dalzell of Ivy Audio under a Versilian
redistribution grant [Card cards/free_libraries.md#vsco_2_ce].

All 75 patches sit at the branch root, so the id is `vsco2ce/<file stem>` [Cited S1]. Key switches
are given as MIDI numbers with the SFZ convention c4 = 60, so `c2` = 36 and `c6` = 84
[Cited S31: sfz.rs `note`]. Whether the winds and brass are solo is not stated. Treat them as solo:
single-instrument names in a chamber library [Estimate].

| Preset id(s) | Sonara id | Solo/section | Articulations and how | Verdict | Source |
|---|---|---|---|---|---|
| `vsco2ce/ViolinEns-KS` | violin | section | KS 36 sustain (vibrato), 37 tremolo, 38 spiccato, 39 pizzicato; default 36 | partly: Q1, silent until a switch key; `ampeg_dynamic` | [Cited S2, S34] |
| `vsco2ce/ViolinEnsSusVib`, `-SusVib-Quiet`, `-Trem`, `-Spic`, `-Pizz` | violin | section | separate patches: sustain (and a quiet sustain), tremolo, spiccato, pizzicato | as is (`SusVib` read: only `ampeg_dynamic`) | [Cited S1, S2] |
| `vsco2ce/SViolin-KS` | violin | solo | KS 36-39 as `ViolinEns-KS`; `sw_default` repeated in every group | as is | [Cited S2, S34] |
| `vsco2ce/SViolinVib`, `-Vib-Quiet`, `-Trem`, `-Spic`, `-Pizz` | violin | solo | separate patches | as is [Estimate: same generator as the files read] | [Cited S1] |
| `vsco2ce/ViolaEns-KS` | viola | section | KS 36 sustain, 37 tremolo, 38 spiccato, 39 pizzicato | as is | [Cited S2] |
| `vsco2ce/ViolaEnsSusVib`, `-SusVib-Quiet`, `-Trem`, `-Spic`, `-Pizz` | viola | section | separate patches | as is [Estimate] | [Cited S1] |
| `vsco2ce/CelloEns-KS` | cello | section | KS 84 sustain (vibrato), 85 tremolo, 86 spiccato, 87 pizzicato | as is; but the switch keys 84-87 lie inside the cello's playable range (36-88 in [model.md](model.md), above its comfortable top of 76), so a part that reaches C6 needs the separate patches | [Cited S2] |
| `vsco2ce/CelloEnsSusVib`, `-SusVib-Quiet`, `-Trem`, `-Spic`, `-Pizz` | cello | section | separate patches | as is (`Pizz` read) | [Cited S1, S2] |
| `vsco2ce/Contrabass-KS` | double_bass | solo | KS 84 sustain non-vibrato, 85 sustain vibrato, 86 tremolo, 87 spiccato, 88 pizzicato | as is | [Cited S2; Card cards/free_libraries.md#vsco_2_ce for "solo"] |
| `vsco2ce/ContrabassSusNV`, `-SusVB`, `-SusVB-Quiet`, `-Trem`, `-Spic`, `-Pizz` | double_bass | solo | separate patches | as is [Estimate] | [Cited S1] |
| `vsco2ce/PiccoloSus`, `PiccoloStac` | piccolo | solo | separate patches: sustain, staccato | as is [Estimate] | [Cited S1] |
| `vsco2ce/Flute-KS` | flute | solo | KS 36 sustain non-vibrato, 37 sustain vibrato, 38 "expression vibrato", 39 staccato | partly: Q1 | [Cited S2] |
| `vsco2ce/FluteSusNV`, `FluteSusVib`, `FluteExpVib`, `FluteStac` | flute | solo | separate patches | as is [Estimate] | [Cited S1] |
| `vsco2ce/OboeSusVib`, `OboeSusNV`, `OboeStac` | oboe | solo | separate patches | as is (`SusVib` read) | [Cited S1, S2] |
| `vsco2ce/Clarinet-KS` | clarinet_bb | solo | KS 36 sustain long, 37 staccato | partly: Q1 | [Cited S2]; B♭ key [Estimate: not stated] |
| `vsco2ce/ClarinetSus`, `ClarinetStac` | clarinet_bb | solo | separate patches | as is [Estimate] | [Cited S1] |
| `vsco2ce/BassoonSus`, `BassoonVib`, `BassoonStac` | bassoon | solo | separate patches: sustain, sustain with vibrato, staccato | as is [Estimate] | [Cited S1] |
| `vsco2ce/FHornSus`, `FHornStac`, `FHornMute` | horn_f | solo | separate patches; `FHornMute` = con_sordino sustain | as is (`FHornMute` read) | [Cited S1, S2] |
| `vsco2ce/TrumpetSus`, `TrumpetSusVib`, `TrumpetStac`, `TrumpetStraightMuteSus`, `TrumpetHarmonMuteSus` | trumpet_bb | solo | separate patches; two mute patches = con_sordino (straight, harmon) | as is (`SusVib` read) | [Cited S1, S2]; B♭ key [Estimate] |
| `vsco2ce/TromboneSus`, `TromboneVib`, `TromboneStac` | trombone_tenor | solo | separate patches | as is [Estimate] | [Cited S1]; tenor [Estimate] |
| `vsco2ce/Tuba-KS` | tuba | solo | KS 84 sustain, 85 staccato | as is | [Cited S2] |
| `vsco2ce/TubaSus`, `TubaStac` | tuba | solo | separate patches | as is [Estimate] | [Cited S1] |
| `vsco2ce/Timpani` | timpani | solo | hits by pitch, 2 round robins | as is | [Cited S2] |
| `vsco2ce/TimpaniRolls` | timpani | solo | separate patch: roll | as is | [Cited S2] |
| `vsco2ce/Xylophone`, `Glockenspiel`, `Marimba`, `TubularBells` | xylophone, glockenspiel, marimba, tubular_bells | solo | one articulation each (struck) | as is (`Glockenspiel`, `Marimba` read) | [Cited S1, S2] |
| `vsco2ce/Harp` | harp | solo | one articulation | as is | [Cited S2] |
| `vsco2ce/OrganLoud`, `OrganQuiet`, `OrganLoudPedal`, `OrganQuietPedal` | organ_pipe | solo | separate patches per registration and manual or pedal | as is (`OrganLoud` read) | [Cited S1, S2] |
| `vsco2ce/UprightPiano`, `VSUpright1` | piano_upright | solo | one articulation each | as is | [Cited S2] |
| `vsco2ce/GM-StylePerc` | snare_drum, bass_drum, clash_cymbals, suspended_cymbal, tam_tam, triangle, tambourine, sleigh_bells (one patch, sound by key) | solo | per-key map, see below | as is (`ampeg_dynamic` only; all 220 regions are single-key) | [Cited S2, S34] |

`GM-StylePerc` per-key map, read region by region [Cited S2]:

| Keys | Sound |
|---|---|
| 32-35 | bass-drum rubs |
| 36 | bass drum hit, 7 layers × 2 round robins |
| 37 | snare taps |
| 38 | snare hit, snares on |
| 39 | snare roll, snares on |
| 40 | snare hit, snares off |
| 41 | snare roll, snares off |
| 42 | gong scrape |
| 46 | gong hit |
| 47, 48, 50 | suspended-cymbal crescendos, short, medium and long |
| 49 | crash cymbal |
| 51 | suspended-cymbal hit |
| 53 | tambourine shake |
| 54 | tambourine hit |
| 55 | tambourine roll |
| 56 | cowbell |
| 59 | suspended cymbal, stick hit |
| 60-65 | congas |
| 67 | anvil |
| 68 | brake drum |
| 69-71 | ratchet |
| 72 | guiro |
| 75 | claves |
| 76, 77 | log drums |
| 78-81 | triangles |
| 82 | sleigh bells |
| 83-86 | bell tree |
| 88 | "alien" effect |
| 94 | vibraphone ring |
| 101-104 | bowed suspended cymbal |

GM puts other instruments on several of these keys [Cited S29]:

| Keys | `GM-StylePerc` | GM1 |
|---|---|---|
| 35 | bass-drum rub | acoustic bass drum |
| 42, 46 | gong | hi-hats |
| 47, 48, 50 | cymbal crescendos | toms |
| 76, 77 | log drums | woodblocks |

So a GM drum part must be re-mapped through this patch's own table, never sent unchanged
[Estimate: consequence].

The raw folders `Brass/OldTrombone` and `VSCO 1 Percussion` have no `.sfz`
[Card cards/free_libraries.md#vsco_2_ce].

### 2.2 Sonatina Symphonic Orchestra (`sso`, CC Sampling Plus 1.0)

Licence: `LICENSE` and the README name CC Sampling Plus 1.0 (GitHub reports `NOASSERTION`).
That licence allows sampling and highly transformative derivatives for any purpose, allows
verbatim copies of the whole work only noncommercially, requires attribution (credit the
original author, keep the notices), and was retired by Creative Commons in 2011. The README lists
Philharmonia Orchestra samples among the sources, and the Philharmonia's own terms exclude making
its samples available as a sampler instrument; the README also admits a few old soundfonts of
unknown origin. So a Sonara user may install Sonatina, but its provenance is not clean, and any
preset map that names it must carry the attribution [Card
cards/free_libraries.md#sonatina_symphonic_orchestra].

The tree holds 747 `.sfz` files:

| Folder | Patches | Include fragments |
|---|---|---|
| Strings - Notation, Strings - Performance | 96 each | 30 each |
| Woodwinds - Notation, Woodwinds - Performance | 96 each | 39 and 40 |
| Brass - Notation, Brass - Performance | 66 each | 23 and 26 |
| Percussion | 14 | |
| Organ | 17 | |
| Harpsichord | 4 | |
| Chorus - Notation, Chorus - Performance | 2 each | Performance: 2 |
| Grand Piano | 1 | |
| `Concert Harp` (at the root) | 1 | |
| **Total** | **557** | **190** |

[Cited S3]

Preset ids follow `sso/<Folder>/<Section or Solo> <Articulation>[ (looped)| (looped, decay)| Non-Vibrato]`
[Cited S3]. The Notation set uses velocity for volume; the Performance set uses CC1 for volume on
long notes [Card cards/free_libraries.md#sonatina_symphonic_orchestra]. In Sonara, CC1 does nothing:
`gain_cc1` and `xfin_locc1` are ignored [Cited S4, S31]. **Use the Notation set** [Estimate:
consequence].

Section and soloist names map to Sonara ids [Cited S3]:

| Name in file | Sonara id | Solo/section |
|---|---|---|
| `1st Violins`, `2nd Violins` | violin | section (parts Vn I, Vn II) |
| `Violin Solo 1`, `Violin Solo 2` | violin | solo |
| `Violas` / `Viola Solo` | viola | section / solo |
| `Celli` / `Cello Solo` | cello | section / solo |
| `Basses` / `Bass Solo` | double_bass | section / solo |
| `Flutes` / `Flute Solo 1`, `Flute Solo 2` | flute | section / solo |
| `Piccolo Solo` | piccolo | solo |
| `Alto Flute Solo` | alto_flute | solo |
| `Oboes` / `Oboe Solo` | oboe | section / solo |
| `Cor Anglais Solo` | english_horn | solo |
| `Clarinets` / `Clarinet Solo` | clarinet_bb [Estimate: key not stated] | section / solo |
| `Bass Clarinet Solo` | bass_clarinet | solo |
| `Bassoons` / `Bassoon Solo` | bassoon | section / solo |
| `Contrabassoon Solo` | contrabassoon | solo |
| `Horns` / `Horn Solo` | horn_f | section / solo |
| `Trumpets` / `Trumpet Solo` | trumpet_bb [Estimate: key not stated] | section / solo |
| `Trombones` / `Tenor Trombone Solo` | trombone_tenor | section / solo |
| `Bass Trombone Solo` | trombone_bass | solo |
| `Tuba` | tuba | solo |
| `All Strings`, `All Woodwinds`, `All Brass` | several ids (ensemble layer) | do not map to one id [Estimate] |

Articulation words map to Sonara articulation ids (the vocabulary of `articulation.rs`)
[Cited S3, S31]:

| Word in file name | Articulation id |
|---|---|
| `Sustain` | sustain |
| `Legato` | legato |
| `Marcato` | marcato |
| `Staccato` | staccato |
| `Spiccato` | spiccato (solo violins only) |
| `Pizzicato` | pizzicato |
| `Col Legno` | col_legno |
| `Tremolo` | tremolo |
| `Harmonics` | harmonics |
| `Non-Vibrato` | sustain, plus the proposed `non_vibrato` |
| `(looped)`, `(looped, decay)` | sustain variants. Under Q9 the "decay" variant holds like the looped one. |

Key-switch files read (c4 = 60). Unused keys inside a switch range select nothing
[Cited S4; Estimate for the unused keys].

Each key-switch file is a list of `<global>` blocks. Each block holds `sw_lokey`, `sw_hikey`,
`sw_default` and one `sw_last`, then `#include`s a single-articulation file. Many of those
included files begin with their own `<control>` header [Cited S33]. Under Q11, Sonara drops
the `sw_last` of every articulation whose file contains a `<control>`. Columns "[C]" and
"Sonara effect" are code reading, not run. The second reading re-read the includes of
`1st Violins KS` (all but `Col Legno` and `Tremolo`) and two of the six `Flutes KS` includes,
and confirmed the effects for those files; the other rows rest on the first reading [Cited S33,
S34].

| Preset id | Switch keys → articulation (included file contains `<control>` = [C]) | Default | Sonara effect | Verdict |
|---|---|---|---|---|
| `sso/Strings - Notation/1st Violins KS` | 36 sustain [C], 37 legato [C], 38 marcato [C], 40 staccato, 41 pizzicato, 42 col legno, 43 tremolo, 45 harmonics [C] | 36 | the last include has [C], so the switch range is lost. Sustain, legato, marcato and harmonics sound together on every note; the other four never sound. | no |
| `sso/Strings - Notation/Basses KS` | 84 sustain [C], 85 legato [C], 86 marcato [C], 88 staccato, 89 pizzicato, 90 col legno, 91 tremolo | 84 | sustain, legato and marcato always sound; a switch adds staccato, pizzicato, col legno or tremolo on top | no |
| `sso/Strings - Notation/Violin Solo 1 KS` | 36 sustain, 37 sustain looped, 38 legato, 39 marcato [C], 40 marcato looped [C], 41 spiccato, 42 staccato, 43 pizzicato, 45 harmonics | 36 | both marcatos always sound under the chosen switch | no |
| `sso/Brass - Notation/Horn Solo KS` | 84 sustain, 85 sustain looped, 86 sustain looped decay, 87 legato, 88 marcato [C], 89 marcato looped [C], 90 marcato looped decay [C], 91 staccato | 84 | three marcatos always sound | no |
| `sso/Brass - Notation/Trumpets KS` | 36 sustain looped, 37 sustain, 38 marcato looped [C], 39 marcato [C], 40 staccato, 41 legato | 36 | two marcatos always sound | no |
| `sso/Woodwinds - Notation/Flutes KS` | 36 sustain looped [C], 37 sustain [C], 38 marcato looped [C], 39 marcato [C], 40 staccato [C], 41 legato [C] | 36 | all six articulations sound on every note; switches do nothing | no |
| `sso/Woodwinds - Notation/Clarinet Solo KS` | 36 sustain, 37 sustain looped, 38 sustain looped decay, 39 staccato, 40 legato | 36 | switches work | partly: Q5 legato, Q9 |
| `sso/Harpsichord/Harpsichord KS` | 96 4' [C], 98 8' [C], 100 full | 100 | 4' and 8' always sound; nested includes of "full" not checked | no |
| `sso/Organ/Organ Combinations` | 12-28: 17 registrations (Great mp to intense, Swell mp to f, solo and pedal combinations); several includes [C] | 14 (Great forte) | registrations with [C] always sound | no; also Q5 `release_key`, Q8, `gain_cc1` swell ignored; 1,590 regions |

The order of the switches also differs between files: legato is the second key for the 1st
violins and the last for the trumpets. **An articulation map must be per preset, not per
library** [Cited S4]. The 2nd-violin, viola and celli key-switch files and the other woodwind and
brass key-switch files were not opened. The single-articulation files they include play under
the verdicts in the next table.

Special Sonatina patches:

| Preset id | Sonara id | How articulations are laid out | Verdict | Source |
|---|---|---|---|---|
| `sso/Percussion/Timpani` | timpani | key zones in one patch: hits at the played pitch from `c2`, rolls in a zone from `c4` (looped roll plus a `release_key` roll ending), roll crescendos in a zone from `c6` | partly: Q6 (f and p hit layers both sound), Q5 (the `release_key` roll ending fires at note-on, together with the looped roll) | [Cited S4, S34] |
| `sso/Percussion/All Unpitched Percussion` | several untuned ids | sound by key | partly: Q5, Q6, Q9 | [Cited S4] |
| `sso/Percussion/Vibraphone`, `Celeste` | vibraphone, celesta | one articulation; pedal and vibrato on CC64 curves and CC21 | partly: `<curve>`, CC64 decay curves and LFO ignored; Celeste Q6 | [Cited S4] |
| `sso/Percussion/Chimes`, `Crotales`, `Glockenspiel`, `Xylophone`, `Marimba Hits`, `Marimba Rolls`, `Triangle`, `Cymbals & Tamtam`, `Bass Drum & Snare`, `Conga` | tubular_bells, crotales, glockenspiel, xylophone, marimba (rolls = tremolo), triangle, clash_cymbals/suspended_cymbal/tam_tam, bass_drum/snare_drum, (none) | separate patches | not opened | [Cited S3] |
| `sso/Grand Piano/Grand Piano` | piano_concert_grand [Card cards/free_libraries.md#coverage: model not stated] | velocity layers, pedal resonance | no: Q3 (`#include "$DIR/Res.txt"`); once Q3 is fixed, N1 applies | [Cited S4, S34] |
| `sso/Concert Harp` | harp | one articulation | partly: `cutoff`, `fil_veltrack`, `pitch_veltrack` ignored | [Cited S4] |
| `sso/Chorus - Notation/Mixed Chorus`, `Large Chrous` | choir_soprano/alto/tenor/bass (mixed, no separate parts) | one vowel, velocity | partly: filter and `pan` ignored | [Cited S3, S4] |
| `sso/Chorus - Performance/Mixed Chorus`, `Large Chorus` | as above | CC1 crossfades dynamics | partly: Q6 on CC1 (`xfin_locc1` ignored: both dynamic layers sound) | [Cited S4] |
| `sso/Strings - Notation/1st Violins Legato` (and every `* Legato`) | as section | note overlap via `trigger=first`/`legato` | partly: Q5, retriggered monophonic sustain; Q9, the 1 s overlap becomes 10 ms | [Cited S4, S34] |
| `sso/Strings - Notation/Celli Pizzicato`, `sso/Woodwinds - Notation/Flutes Staccato` | cello, flute | separate patch | partly: `ampeg_decay/sustain/vel2decay` ignored; short samples, so the loss is likely small [Estimate] | [Cited S4] |

### 2.3 VCSL (`vcsl`, CC0, tag `v1.2.2-RC`)

Licence: CC0 1.0; the README says no credit is required. The pipe organ is an Ivy Audio
recording covered by a Versilian redistribution note, and the Knight upright is also an Ivy Audio
recording, with no such note [Card cards/free_libraries.md#vcsl].

The SFZ files exist only in the tree of tag `v1.2.2-RC`, not on `master` [Cited S6; Card
cards/free_libraries.md#vcsl]. The release "SFZ Build v1.2.2" has no uploaded assets
[Cited S5, re-queried 2026-10-30]; the two assets the free-libraries card counts on the release
page are presumably GitHub's automatic source archives of the tag [Estimate: GitHub lists those
on every release page]. So the SFZ set is obtained by checking out or downloading the tag. VCSL
has no bowed strings, orchestral woodwinds or brass [Card cards/free_libraries.md#vcsl].

VCSL key-switch files repeat `sw_lokey`, `sw_hikey` and `sw_default` in every `<group>`, and the
files read use only `<group>` and `<region>` headers, so neither Q1 nor Q11 applies [Cited S7].
The switch keys of the files read lie outside the instruments' playable ranges in
[model.md](model.md), except the timpani's. Untuned percussion uses one key per sound or articulation from 60 upward
[Cited S7].

| Preset id (prefix `vcsl/`) | Sonara id | Articulations and how | Verdict | Source |
|---|---|---|---|---|
| `Chordophones/Zithers/Grand Piano, Kawai` (also `- Legacy`), `Grand Piano, Steinway B` | piano_studio_grand [Card cards/free_libraries.md#coverage] | velocity layers, release samples | partly: `ampeg_decay/sustain`, `rt_decay` ignored (Kawai read); N1, release samples lost under the sustain pedal | [Cited S6, S7, S34] |
| `Chordophones/Zithers/Upright Piano, Knight`, `Upright Piano, Yamaha` | piano_upright | release samples; pedal noises on CC64 | partly: `amp_veltrack`, `pitch_keytrack`, `rt_decay` ignored; pedal regions silent (Q10); N1, release samples lost under the sustain pedal (Knight read) | [Cited S7, S34] |
| `Chordophones/Zithers/Harpsichord, Flemish - Keyswitch` | harpsichord | KS 94 4', 95 8', 96 full; separate `- 4'`, `- 8'`, `- Full` | partly: Q8, `ampeg_decay/sustain` | [Cited S7] |
| `Chordophones/Zithers/Harpsichord, English - Normal`, `- Lute`, `- Keyswitch`, `Harpsichord, French`, `Italian`, `Unk` | harpsichord | lute stop as a separate patch | not opened | [Cited S6] |
| `Chordophones/Composite Chordophones/Concert Harp` | harp | one articulation | partly: Q6 (mp and f layers sound together) | [Cited S7] |
| `Chordophones/Composite Chordophones/Folk Harp` | harp (folk instrument, substitute only) [Estimate] | | not opened | [Cited S6] |
| `Aerophones/Edge-blown Aerophones/Pipe Organ - Keyswitch` | organ_pipe | KS 100 loud, 101 quiet, 102 loud pedal, 103 quiet pedal; also separate patches `- Loud`, `- Quiet`, `- Loud Pedal`, `- Quiet Pedal` | partly: Q8 (organ follows velocity) | [Cited S6, S7] |
| `Aerophones/Edge-blown Aerophones/Renaissance Organ - 4'`, `- 8'`, `- 4'+8'`, `- Full`, `- Keyswitch` | organ_pipe (historical) | registrations as patches or KS | not opened | [Cited S6] |
| `Aerophones/Reed Aerophones/Tenor Saxophone - Keyswitch` | saxophone_tenor | KS 36 vibrato, 37 non-vibrato, 38 staccato; also separate `- Vibrato`, `- Non-Vibrato`, `- Staccato` | as is | [Cited S7] |
| `Aerophones/Reed Aerophones/Saxello - *` | saxophone_soprano substitute [Estimate: a saxello is a curved soprano saxophone] | as tenor | not opened | [Cited S6] |
| `Membranophones/Struck Membranophones/Timpani 1 - Keyswitch` | timpani | KS 36 hit, 37 roll; regions sampled from key 41 | as is. The switch keys fall inside the timpani's playable range per the card (36-61), so notes 36 and 37 would be taken as switches and change the articulation. Use the separate `Timpani 1 - Hit` and `- Roll` patches for low parts. | [Cited S7; Card cards/tuned_percussion.md#timpani] |
| `Membranophones/Struck Membranophones/Timpani 2 - All Samples`, `- Scale`, `- Keyswitch` | timpani | | not opened | [Cited S6] |
| `Idiophones/Struck Idiophones/Xylophone - Keyswitch` | xylophone | KS 48 hard, 49 soft, 50 medium mallets; separate `- Hard`, `- Medium`, `- Soft Mallets` | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Glockenspiel` | glockenspiel | one articulation | partly: Q6 | [Cited S7] |
| `Idiophones/Struck Idiophones/Marimba` | marimba | one articulation | partly: Q6 | [Cited S7] |
| `Idiophones/Struck Idiophones/Vibraphone - Soft Mallets` (also `- Bowed`, `- Hard Mallets`) | vibraphone | mallet type or bow as separate patches | as is (Soft read) | [Cited S6, S7] |
| `Idiophones/Struck Idiophones/Tubular Bells 1`, `2`, `3 - Legacy` | tubular_bells | | as is (1 read) | [Cited S7] |
| `Idiophones/Struck Idiophones/Triangles` | triangle | three triangles × 60/65/69 hit, 61/66/70 "HitFM", 62/67/71 "HitM", 63/68/72 roll (sample names as given) | as is | [Cited S7] |
| `Membranophones/Struck Membranophones/Snare Drum, Modern 1` (also `2`, `3`, `Rope Tension`, `Legacy Snares - *`) | snare_drum | 60 hit "NS", 61 hit "SN", 62 roll "NS", 63 roll "SN", 64 stick, 65 taps | as is | [Cited S7]; VSCO labels "SN" snares on and "NS" snares off [Estimate: same recordings, same naming] |
| `Membranophones/Struck Membranophones/Bass Drum 1` (also `2`, `3 - Legacy`) | bass_drum | 60 hit | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Suspended Cymbal 1` (also `2`) | suspended_cymbal | 60-63 bowed, 64-67 crescendo rolls (1.5, 2, 4, 7.5 s), 68 hit, 69 bell, 70 stick hit, 71 roll, 72 scrape | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Clash Cymbals 1` (also `2`) | clash_cymbals | 60 crash, 61-62 short (choked) crashes | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Gong 1` (also `2`) | tam_tam [Card cards/untuned_percussion.md#tam_tam: gong and tam-tam are used interchangeably] | 60 hit, 61 second hit, 62 scrape | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Tambourine 1` (also `2`, `3-5 - Legacy`) | tambourine | 60 hit, 61 roll, 62 shake | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Woodblock` | wood_block | 60-62 clicks | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Sleigh Bells` | sleigh_bells | 60-64 | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Shaker, Small` (also `Shaker, Large`, `Shaker - Legacy`) | shaker | 60-67 | as is | [Cited S7] |
| `Idiophones/Struck Idiophones/Slapstick` | whip | 60-61 | as is | [Cited S7] |
| `Electrophones/TX81Z - Clavisynth` | none (an FM patch, not a clavinet) | | | [Card cards/free_libraries.md#electric_pianos] |

### 2.4 Pianos with ARIA-style patches

| Preset id | Sonara id | What the file does | Verdict | Source |
|---|---|---|---|---|
| `salamander/Salamander Grand Piano V3` | piano_studio_grand [Card cards/free_libraries.md#coverage] | KS 12 natural / 13 retuned via `$NATURAL C0`; 16 velocity groups through inline `#include`; release hammer and string-resonance layers; pedal noise via `on_locc64` | no: Q2 (note regions never loaded; the patch plays only pedal noises), Q1 (switches cleared by a later `<global>`, which matters once Q2 is fixed), Q7 (a pedal-down and a pedal-up noise on every key, transposed), N1 (its release sections already load) | [Cited S8, S34] |
| `splendid/Splendid Grand Piano` | piano_concert_grand [Card cards/free_libraries.md#splendid_grand_piano] | layers selected through `$DIR` includes; `locc64` pedal layers | no: Q3 | [Cited S14] |
| `sso/Grand Piano/Grand Piano` | piano_concert_grand | as Splendid | no: Q3 | [Cited S4] |

Licences: Salamander is CC BY 3.0 Unported, so a preset map that names it carries the
attribution [Card cards/free_libraries.md#salamander_grand_piano]. Splendid is labelled public
domain; that rests on AKAI's release as relayed by sfzinstruments and MuseScore, not on a document
from AKAI [Card cards/free_libraries.md#splendid_grand_piano]. Sonatina's piano falls under
Sonatina's licence (section 2.2).

The acoustic-piano card expects Salamander's CC64 pedal-noise samples "may never fire"; the code
reading here shows the opposite, they fire on every key (Q7) [Cited S34; Card
cards/acoustic_pianos.md#piano_studio_grand].

The FreePats SFZ release of Salamander (CC BY 3.0) was not inspected. It is the candidate to try
first for piano_studio_grand [Card cards/free_libraries.md#salamander_grand_piano].

### 2.5 Other SFZ libraries (gap fillers and keyboards)

| Preset id | Sonara id | Solo/section | Articulations and how | Verdict | Licence | Source |
|---|---|---|---|---|---|---|
| `dsmolken-bass/d_smolken_rubner_bass_switched` | double_bass | solo | KS 70 arco, 71 pizzicato, 72 noises; the switches sit above the sampled notes | partly: `amp_velcurve_*`, `bend_up/down` ignored (Sonara bends ±2 st) | CC0-1.0 per GitHub's licence metadata; header: "Royalty-free for all commercial and non-commercial use." The free-libraries card found no `LICENSE` file and rests on the catalogue label | [Cited S9, S13; Card cards/free_libraries.md#karoryfer_samples] |
| `dsmolken-bass/d_smolken_rubner_bass_arco`, `..._pizz` | double_bass | solo | separate patches | partly (as above; arco read) | as above | [Cited S13] |
| `bigcat-cello/Programs/01- Bowed (velocity layer)`, `02- Bowed (mod wheel)`, `03- Plucked` | cello | solo, with "four" and "eight" voice layers chosen by CC107 | arco or pizzicato per patch; legato via `trigger=legato`; CC107 "Voices" | partly: Q7 (all voice layers sound together), Q5, Q9 | CC0-1.0 per GitHub's licence metadata (the card: catalogue label only) | [Cited S9, S12; Card cards/free_libraries.md#karoryfer_samples] |
| `bigcat-cello/Programs/vc_arco_*_map`, `vc_pizz_*` (36 more) | cello | solo, four or eight voices by name | sustain, staccato, marcato, legato, pizzicato as separate programs | not opened | as above | [Cited S12] |
| `war-tuba/Programs/2-solo-poly` | tuba [Estimate: an orchestral tuba was not confirmed] | solo; also duo and trio programs | KS 24 staccatissimo, 25 staccato, 26 sustain; default 25 | partly: CC dynamics, EQ, LFO, filters ignored. The switch keys 24-26 lie inside the tuba's playable range (21-67 in [model.md](model.md), below its comfortable bottom of 29), so notes there would switch instead of sound | CC0 1.0 (`LICENSE`) | [Cited S9, S18; Card cards/free_libraries.md#other_sources] |
| `mtg-sax/MTG Solo Saxophones/MTG Soprano Sax`, `Alto`, `Tenor`, `Baritone`, each also `(NL)` | saxophone_soprano, saxophone_alto, saxophone_tenor, saxophone_baritone | solo | legato groups, CC dynamics | no: Q4 nested includes (the whole patch fails: silence plus a warning) | CC BY 4.0 (`LICENSE`): attribution required | [Cited S9, S11, S34; Card cards/free_libraries.md#other_sources] |
| `ixox-flute/Ixox Flute` | flute | solo | legato; switch range 24-48 | no or partly: Q2 and Q5 (not resolved further) | CC BY 4.0 (GitHub metadata and catalogue): attribution required | [Cited S9, S17; Card cards/free_libraries.md#other_sources] |
| `terkelsen-mandolin/Terkelsen's Mandolin` | mandolin | solo | one articulation | as is (`ampeg_release_oncc*` only) | none in GitHub metadata: unknown | [Cited S9, S19] |
| `jrhodes3d/jRhodes3d-st-no-xfade` (also `-mono-no-xfade`, `-sv-no-xfade`) | electric_piano_rhodes | solo | one articulation | partly: `width_oncc*`, `ampeg_releasecc*`, `<curve>` ignored | samples CC BY-NC 4.0 per `LICENSE` (control files CC0; music made with it unrestricted); the `.sfz` header's "CC-SA-BY" disagrees, and the dedicated `LICENSE` file is taken as authoritative [Estimate: the stricter reading]. Non-commercial: user install only | [Cited S15; Card cards/free_libraries.md#electric_pianos, cards/electric_keys.md#electric_piano_rhodes] |
| `jrhodes3d/jRhodes3d-keyswitch` | electric_piano_rhodes | solo | KS G7 to C8 select mono, stereo or vibrato, with or without crossfade | partly: Q6 on the crossfade variants. The switches (103-108) lie inside the 88-key playable range (21-108 in [model.md](model.md)), though above the 73-key Stage's top of 100; use the single files | as above | [Cited S15] |
| `gs-epianos/Wurlitzer EP200/Wurlitzer EP200` | electric_piano_wurlitzer | solo | one articulation | partly: CC amplitude and pan, `ampeg_decay/sustain/hold`, `note_polyphony` ignored | CC BY 3.0 Unported per the `LICENSE` legal code and README, with the author's request for attribution; GitHub's `NOASSERTION` only means its detector did not classify the file [Estimate] | [Cited S9, S16; Card cards/free_libraries.md#electric_pianos] |
| `discord-gm/Discord GM/Melodic/<nnn-Name>` | GM fallback (section 5) | | one program per file | no as a bank. Programs 41, 48, 49, 53, 61 and the Orchestra Kit are one-line stubs `sample=*sine`, and Sonara fails each stub patch (silence plus a warning, not a sine; N3). Only 10 programs name real instruments. The master selects programs with `loprog`/`hiprog`, which Sonara ignores. | per instrument file; no repository licence | [Cited S10, S27, S34] |

Not inspected, with the licences the free-libraries card states:

- VPO3 (zip downloads): custom web terms; it contains CC BY-SA and Sampling Plus parts and
  Philharmonia samples [Card cards/free_libraries.md#virtual_playing_orchestra_3].
- The FreePats SFZ sets (timpani, xylophone, tubular bells, clarinet, nylon guitar, accordion,
  setBfree organs, Aeolus organ, tenor saxophone, Old Piano FB, Upright Piano KW): each entry is
  CC0 1.0, except the FreePats Salamander (CC BY 3.0) and the FreePats General MIDI sets, which are
  GPL 3.0 or later with an exception, so LIC-01 keeps them out of anything Sonara ships; a user may
  install them [Card cards/free_libraries.md#freepats]. Two of the CC0 sets say their samples are
  looped (Tenor Saxophone "infinite sustain loops", Upright Piano KW "bass notes looped"); when
  they are inspected, check that the loop points are written in the `.sfz`, because Sonara ignores
  loops embedded in the sample file (N6) [Estimate: the files were not opened].
- Keppy's Steinway: sources disagree between CC BY 4.0, CC BY-SA 4.0 and a mis-detected CC0; the
  strictest reading, CC BY-SA 4.0, applies [Card cards/free_libraries.md#keppys_steinway_piano].
- Karoryfer Scarypiano: CC0 1.0 (`LICENSE`) [Card cards/free_libraries.md#karoryfer_samples].

### 2.6 What the classification means

- **Plays as is (36 of the files read):**
  - the VSCO single-articulation files, plus `SViolin-KS`, `ViolaEns-KS`, `CelloEns-KS`,
    `Contrabass-KS` and `Tuba-KS`;
  - the VCSL tenor saxophone, timpani, xylophone, vibraphone, tubular bells and untuned
    percussion;
  - the Terkelsen mandolin.

  [Cited S2, S7, S19]
- **Plays partly, but usable with an articulation map that emits switches:** VSCO
  `ViolinEns-KS`, `Flute-KS` and `Clarinet-KS` (Q1) [Cited S2, S34]; Sonatina `Clarinet Solo KS`
  [Cited S33]. Every key-switch patch also keeps the last switch across a seek or loop wrap while
  playing, so the map holds each switch note for its whole passage (section 3.3) [Cited S34: C11].
- **Plays partly, with audible defects:**
  - Sonatina timpani, celesta, harp and organ;
  - the VCSL harp, glockenspiel and marimba (doubled layers);
  - the Bigcat cello (stacked voices);
  - every Sonatina legato;
  - the VCSL grand and upright pianos, whenever the sustain pedal comes from imported MIDI or
    live input (release samples lost, N1).

  [Cited S4, S7, S12, S34]
- **Does not play:** Salamander V3 remap (it loads, but plays only pedal noises), Splendid,
  Sonatina Grand Piano, MTG saxophones and the Discord GM bank [Cited S4, S8, S10, S11, S14, S34].
- **Plays the wrong thing:** eight of the nine Sonatina key-switch files read stack
  articulations (Q11) [Cited S33; Cited S34 for `1st Violins KS` and `Flutes KS`].

## 3. Articulation realization model

### 3.1 How libraries switch articulations

| Mechanism | How it works | Examples (evidence) | Sonara today |
|---|---|---|---|
| Separate patch | one file or program per articulation; the user changes patch | VSCO `ViolinEnsPizz` vs `ViolinEnsSusVib` [Cited S1]; Sonatina `1st Violins Pizzicato` [Cited S3]; Spitfire: "each articulation is a patch" [Card cards/strings.md#violin] | yes, but only as separate tracks: `instrument` is not automatable [Cited S31] |
| Key switch, sticky (`sw_last`) | a note in a reserved range selects the articulation for following notes; `sw_default` preselects one | VSCO 36-39 or 84-88, Sonatina 36-45 or 84-91, VCSL 36-38, 48-50, 94-96, 100-103, D. Smolken 70-72, War Tuba 24-26 [Cited S2, S4, S7, S13, S18]; spec [Cited S22, S23]; Logic Studio Strings switches from MIDI 24 [Card cards/apple.md#2.2] | yes: the sampler implements `sw_last`, with the Q1 and Q11 caveats (most Sonatina key-switch files do not switch). An agent can write switch notes with the existing note tools, since a switch is an ordinary note: `NoteInput.pitch` takes any MIDI number. The same key also triggers any region mapped to it, and the last switch survives a seek or loop wrap while playing [Cited S34: C11, C17] |
| Key switch, momentary or layered | held switch keys layer articulations; `sw_down`/`sw_up`/`sw_previous` | Spitfire: holding several switches layers them [Cited S28] | no (`sw_down` etc. not in the supported list) [Cited S31] |
| CC value (incl. UACC) | a controller value selects the articulation | Spitfire UACC on CC32 by default, described as "a specification that aims to standardise articulation control" [Cited S28]; SFZ `locc`/`hicc` gates [Cited S25]; Bigcat CC107 voices [Cited S12] | no: gates ignored (Q7) and no tool writes CC [Cited S31] |
| CC crossfade (dynamics, vibrato, voices) | CC1/CC11/CC21 crossfades layers or sets vibrato | Sonatina Performance CC1 volume, CC21 vibrato [Cited S4]; Spitfire Dynamics on CC1, vibrato on CC21 [Card cards/strings.md#violin] | CC7 and CC11 act as linear volume (value/127); CC1/CC21 do nothing [Cited S31, S34: C15] |
| Program change | program number selects a region set | SFZ `loprog`/`hiprog` (SFZ v2) [Cited S27]; GM Tremolo/Pizzicato Strings and Muted Trumpet are programs [Cited S29]; Logic Articulation Set switch types include Program [Card cards/apple.md#3.2] | no: `loprog` ignored; clips cannot hold a program change (control kinds are CC, pitch bend and channel pressure, and the sampler ignores channel pressure too) [Cited S31, S34: C16] |
| MIDI channel | each articulation or section listens on its own channel | Logic Studio Strings section channels 1-6 (full section, Vn 1, Vn 2, Va, Vc, Cb) [Card cards/apple.md#2.2]; Logic articulations can be chosen by channel [Card cards/apple.md#3.2]; Cubase expression maps can change the channel on output [Estimate: search-result summary, manual not opened] | notes carry a channel, but one sampler ignores it at note-on and keeps one switch state for all channels [Cited S31, S34: C9] |
| Velocity zone | velocity range selects the articulation | VPO Performance Orchestra: velocity selects the articulation [Card cards/free_libraries.md#virtual_playing_orchestra_3]; Spitfire Performance Legato: velocity picks the transition type [Card cards/strings.md#violin] | yes in principle (lovel/hivel), but it takes velocity away from dynamics [Estimate] |
| Key zone (offset) | the same pitches repeated in another key region play another articulation | Sonatina Timpani: hits from `c2`, rolls from `c4`, crescendos from `c6` [Cited S4] | yes (plain key ranges) |
| Note overlap (legato) | a note started while another sounds plays a transition | SFZ `trigger=legato`/`first` [Cited S21]; Spitfire overlap legato [Card cards/strings.md#violin] | no: both triggers fire as attacks (Q5) |
| Second note, same pitch | a following note of the same pitch triggers a fall or doit | Logic Studio Strings and Horns falls [Card cards/apple.md#2.2] | no |
| Per-note articulation ID | each note carries an ID (Logic: 1-255 or off); the articulation set translates it into switches | Logic Articulation Sets [Card cards/apple.md#3] | no: notes have no articulation field (a CTR-01 project-format change) [Cited S31] |
| Percussion key map | one patch, a different sound or stroke per key | VSCO `GM-StylePerc`, VCSL untuned percussion [Cited S2, S7] | yes |

### 3.2 What a Sonara articulation map must store per preset

| Field | Meaning | Why (evidence) |
|---|---|---|
| `preset` | catalog preset id | stable across machines and library moves only while the library has a manifest `id` or keeps its folder name [Cited S31, S34: C4] |
| `instrument` | Sonara instrument id; `[]` plus a role for an ensemble patch (`All Strings`) | Sonatina ensemble patches [Cited S3] |
| `part` | optional score part, using the part labels of [ensembles.md](ensembles.md), for example `violins_1`, `violins_2` | Sonatina 1st/2nd Violins [Cited S3] |
| `ensemble` | `solo`, `section`, or `section_emulated` (voices layered from a solo) with a player count when stated | VSCO and Sonatina naming [Cited S1, S3]; Bigcat four or eight voices [Cited S12] |
| `sampled_keys` | lowest and highest key that has an attack region, derived from the regions at verify time | VCSL Timpani 1 is sampled from 41 while its switch keys are 36 and 37 [Cited S7] |
| `switch_keys` | `sw_lokey`, `sw_hikey`, the effective default, and whether the default survives Sonara's parser | Q1 [Cited S2, S31] |
| `articulations[]` | per Sonara articulation id: `via` = `patch` / `keyswitch {key}` / `cc {number, value}` / `program {n}` / `channel {n}` / `velocity {lo, hi}` / `key_offset {semitones}` / `keymap {sound → key}`; plus `label` (the library's own name) and `variant` (looped, decay, non-vibrato, mallet) | section 3.1 |
| `dynamics` | `velocity` or `cc1`/`cc11`, and whether Sonara honours it | Sonatina Notation vs Performance [Cited S4]; Sonara ignores CC1 [Cited S31] |
| `legato` | `none`, `overlap` (SFZ first/legato), with Sonara's actual behaviour | Q5 |
| `velocity_policy` | for patches whose regions should ignore velocity (organs), the fixed velocity to send | Q8 |
| `sonara_verdict` | `as_is` / `partly {lost: [...]}` / `no {reason}`, from a deep load plus structural checks for Q1-Q7, Q11 and N1-N6; the loader's ignored-opcode list is one input, not the verdict | ADR 0010: the loader lists ignored opcodes [Cited S31]; the list undercounts and never reflects the structural Q1-Q4 and Q11, the silent Q5 remap, N1 or N3 [Cited S34: C24, N2] |
| `licence` | from the manifest; `null` = unknown; plus the attribution text where the licence requires one (Sonatina, Salamander, MTG Solo Sax, Ixox Flute, Greg Sullivan) | docs/instrument-catalog.md [Cited S31]; [Card cards/free_libraries.md] |
| `provenance` | `read_from_file` / `manifest` / `guessed`, with the date | free-libraries card practice [Card cards/free_libraries.md] |

An illustrative entry. It is not a proposed file format; a manifest v2 or a new store is a
decision for the owner [Estimate: design]:

```json
{ "preset": "vsco2ce/ViolinEns-KS", "instrument": ["violin"], "ensemble": "section",
  "switch_keys": { "lo": 36, "hi": 39, "default": 36, "default_survives_parser": false },
  "articulations": [
    { "id": "sustain",   "via": { "keyswitch": 36 }, "label": "Sustain Vibrato" },
    { "id": "tremolo",   "via": { "keyswitch": 37 }, "label": "Tremolo" },
    { "id": "spiccato",  "via": { "keyswitch": 38 }, "label": "Spiccato" },
    { "id": "pizzicato", "via": { "keyswitch": 39 }, "label": "Pizzicato" } ],
  "dynamics": "velocity", "legato": "none",
  "sonara_verdict": { "partly": ["silent until a switch key (sw_default lost)", "ampeg_dynamic"] },
  "licence": "CC0 1.0", "provenance": "read_from_file 2026-10-30" }
```

### 3.3 Sending articulations under today's contracts

1. **Separate patch.** Use one sampler track per articulation, and put each note on the track
   whose patch plays its articulation [Cited S31: `instrument` not automatable; Estimate:
   approach].
2. **Key switch.** Write the switch as an ordinary note. Start it slightly before the first note
   of the passage. If it starts on the same frame, the order depends on the clip's note order.
   Hold it for the whole passage. A seek or a loop wrap while playing does not reset the sampler,
   so without a held switch the articulation last pressed before the jump would continue, not
   `sw_default`. A held switch note is re-sent by the engine's chase when the jump lands inside it
   [Cited S31: schedule.rs ordering and chase; Cited S34: C10, C11; Estimate: approach].
   - Keep switch keys outside the instrument's playable range per the card: a switch note also
     plays any region mapped to its key [Cited S34: C17]. Where they overlap (VCSL Timpani 1),
     use separate patches instead [Cited S7; Card cards/tuned_percussion.md#timpani].
3. **Velocity or key zone.** Rewrite the velocity or transpose the note into the zone. A velocity
   zone costs dynamic control [Estimate].
4. **CC, program change, channel, per-note articulation ID.** None of these is possible without
   a contract change (CTR-01): a CC-writing tool, a program-change control kind, a note
   articulation field, or channel routing in the sampler [Cited S31; .claude/rules/project.md
   CTR-01].

## 4. Degradation chain

### 4.1 The steps and what the user is told

Each step is tried only when every earlier step failed. A preset counts as available only when the
catalog says `available` **and** a deep `verify` has loaded it **and** its verdict is not `no`
[Estimate: design; Cited S31 for availability states]. Plain discovery does not load the file,
so it calls include fragments and broken patches `available`, and one missing sample fails a
whole patch (C8, N3) [Cited S34].

| Step | Condition and action | Message to the user (and to the agent's result) | Ask first? |
|---|---|---|---|
| 1. Exact | A preset maps the instrument id, the articulation and the requested solo or section, with verdict `as_is` or `partly`. | None for `as_is`. For `partly`: "Timpani (sso/Percussion/Timpani) plays both dynamic layers on every hit: velocity crossfades are not supported." List the losses from the verdict: the loader's ignored-opcode list plus the structural checks of section 2.0, because the list alone undercounts (N2) [Cited S34: C24]. | no |
| 2. Nearest articulation | Same instrument and ensemble; follow the articulation table in 4.2, which also allows emulation by note rewriting. | "Spiccato is not available for Violin section; playing Staccato." / "Tremolo is emulated by repeated 32nd notes." | no; report it |
| 3. Section or solo swap | Same instrument, other ensemble size, then step 2 again. | "No solo violin is installed; the solo is played by a violin section (fuller and less agile)." | no; report it |
| 4. Family neighbour | The instrument in the 4.3 table, only if every note lies inside the neighbour's playable range per its card. Never transpose by octaves silently. | "No cimbasso is installed; the part is played by Tuba (vsco2ce/Tuba-KS)." If notes fall outside: "3 notes are outside the Tuba's range; choose: transpose them an octave, or skip step 4." | yes when notes fall outside the range, or the timbre changes family role (for example a solo voice played by a choir) |
| 5. General MIDI | The program or percussion notes of section 5, through a user-installed GM SoundFont (MuseScore_General or FluidR3_GM, both MIT; MuseScore_General's notices must travel with any derivative) played by an SF2 engine. **Not available today:** ADR 0010 deferred rustysynth, and Sonara plays no SF2 [Cited S31; Card cards/free_libraries.md#musescore_general_and_ms_basic, #fluidr3_gm]. | "No sampled Horn in F; using General MIDI program 61 'French Horn' from <SoundFont>." | no; report it |
| 6. Built-in | Poly Synth preset (Strings, Brass, Choir Pad, Electric Keys, Glass Bells, Warm Pad, Pluck; also Square Lead) or a Drum Kit note (section 4.3). | "No sampled or GM sound for Oboe; using the built-in Poly Synth 'Warm Pad', a synthetic approximation." | no; report it |
| 7. Silence | ADR 0010 soft failure: the track renders silent with a warning. | "No sound source for Whip; the track is silent." Also listed in the render warnings. | no |

Steps 2 and 3 can be swapped by a user preference. Logic offers no such fallback; it only keeps
each articulation set with the patch [Card cards/apple.md#1; Estimate: no Apple fallback found].

### 4.2 Articulation substitution (step 2)

Order of preference, then emulation by rewriting notes, then the message. All rows are a musical
judgement drawn from the technique definitions in the cards [Estimate: musical judgement;
Card cards/strings.md#violin section 5 for the string terms].

| Requested | Nearest available, in order | Emulation if none | Tell the user |
|---|---|---|---|
| sustain | tenuto → legato patch played detached → longest marcato | none needed | only if marcato is used ("accents added") |
| legato | sustain (monophonic, notes overlapped) → tenuto | overlap notes by a few ms. Sonara has no legato transitions today, so every sampled legato already degrades to retriggered sustains (Q5). | "legato transitions not available; played as sustain" |
| staccato | spiccato (strings) → marcato short → staccatissimo (War Tuba) → sustain | shorten notes to about a quarter of their value | "staccato emulated by short sustains" |
| spiccato | staccato → col_legno at low velocity → sustain shortened | as staccato | "spiccato played as staccato" |
| tenuto | sustain → legato patch | none | none |
| marcato | sustain with velocity raised → staccato for short notes | raise velocity by a step | "marcato emulated by accents" |
| pizzicato | the pizzicato of the other ensemble size (step 3) → harp for single lines in an orchestral context (family change, so it needs confirmation) → staccato at low velocity | none convincing | "pizzicato not available; bowed staccato used" |
| tremolo | sustain → staccato | repeated 32nd notes with small velocity variation | "tremolo emulated by repeated notes" |
| trill | sustain + emulation | alternate the main and auxiliary note at about 12-16 notes per second | "trill written out" |
| col_legno | spiccato soft → staccato soft | none | "col legno played as soft spiccato" |
| harmonics | sustain non-vibrato at low velocity | none | "harmonics played as soft sustain (different timbre)" |
| sul_ponticello | tremolo (if the part is tremolo) → sustain | none | "sul ponticello not available; ordinary bowing" |
| con_sordino | the library's mute patch (VSCO `FHornMute`, `TrumpetStraightMuteSus`) → sustain at lower velocity | lower velocity | "mute not available; played softer" |
| flutter_tongue | tremolo (if any) → sustain | none | "flutter-tongue not available" |
| roll | the library's roll (VSCO `TimpaniRolls`, VCSL `Timpani 1 - Roll`, snare roll keys) → tremolo | single strokes at 32nd notes with velocity variation | "roll emulated by repeated strokes" |

### 4.3 Instrument substitution (steps 1, 3, 4 and 6)

Step 1 lists presets found and classified in section 2. For Sonatina, `*` means the
single-articulation files; its key-switch files are excluded except `Clarinet Solo KS` (Q11).
"Neighbour" means within the neighbour's range per its card. The relationships cited come from the
cards' substitution notes and "Variants and related instruments" sections. In the step 3 column,
"n/a" means that no preset of the other ensemble size was found or that the model lists the id as
solo only. Built-in choices are judgements [Estimate: timbre match]. Step 5 is in section 5.

| Id | Step 1: presets (best first) | Step 3: solo ↔ section | Step 4: neighbour | Step 6: built-in |
|---|---|---|---|---|
| violin | `vsco2ce/SViolin-KS` (solo), `vsco2ce/ViolinEns*` (section), `sso/Strings - Notation/1st Violins *`, `2nd Violins *`, `Violin Solo 1/2 *` | either | viola (low notes) [Estimate] | Poly Synth "Strings" |
| viola | `vsco2ce/ViolaEns-KS`, `ViolaEns*`; `sso/... Violas *`, `Viola Solo *` | either | violin (high) or cello (low) [Estimate] | "Strings" |
| cello | `vsco2ce/CelloEns-KS`, `CelloEns*`; `sso/... Celli *`, `Cello Solo *`; `bigcat-cello/*` (partly) | either | viola (high) or double_bass (low) [Estimate] | "Strings" |
| double_bass | `vsco2ce/Contrabass-KS` (solo); `dsmolken-bass/*` (solo); `sso/... Basses *`, `Bass Solo *` | either | cello [Estimate] | "Strings" |
| piccolo | `vsco2ce/PiccoloSus`, `PiccoloStac`; `sso/... Piccolo Solo *` | n/a (solo) | flute [Card cards/flutes.md#piccolo] | Poly Synth "Square Lead" or "Warm Pad" (no woodwind preset exists) |
| flute | `vsco2ce/FluteSus*`, `FluteStac`, `Flute-KS` (Q1); `sso/... Flutes *`, `Flute Solo 1/2 *` | either | alto_flute (low), piccolo (high) [Card cards/flutes.md#flute] | "Warm Pad" |
| alto_flute | `sso/Woodwinds - Notation/Alto Flute Solo *` | n/a | flute [Card cards/flutes.md#alto_flute] | "Warm Pad" |
| bass_flute | none found | n/a | alto_flute, then flute; low notes need confirmation [Card cards/flutes.md#bass_flute] | "Warm Pad" |
| oboe | `vsco2ce/OboeSus*`, `OboeStac`; `sso/... Oboes *`, `Oboe Solo *` | either | english_horn (low) [Card cards/double_reeds.md#oboe] | "Square Lead" |
| english_horn | `sso/... Cor Anglais Solo *` | n/a | oboe (high), bassoon (low) [Card cards/double_reeds.md#english_horn] | "Square Lead" |
| oboe_damore | none | n/a | oboe, then english_horn [Card cards/double_reeds.md#oboe_damore] | "Square Lead" |
| bassoon | `vsco2ce/BassoonSus`, `BassoonVib`, `BassoonStac`; `sso/... Bassoons *`, `Bassoon Solo *` | either | contrabassoon (low), bass_clarinet [Estimate] | "Square Lead" |
| contrabassoon | `sso/... Contrabassoon Solo *` | n/a | bassoon [Card cards/double_reeds.md#contrabassoon] | "Warm Pad" |
| clarinet_bb | `vsco2ce/ClarinetSus`, `ClarinetStac`, `Clarinet-KS` (Q1); `sso/... Clarinets *`, `Clarinet Solo *` | either | clarinet_a presets if any; oboe [Estimate] | "Square Lead" |
| clarinet_a | none | n/a | clarinet_bb (samples are sounding pitch, so only the range matters) [Card cards/clarinets.md#clarinet_a] | "Square Lead" |
| clarinet_eb | none | n/a | clarinet_bb (top notes need confirmation) [Card cards/clarinets.md#clarinet_eb] | "Square Lead" |
| bass_clarinet | `sso/... Bass Clarinet Solo *` | n/a | bassoon, clarinet_bb [Card cards/clarinets.md#bass_clarinet] | "Square Lead" |
| saxophone_soprano | (`mtg-sax/... Soprano Sax`: does not load); `vcsl/... Saxello - *` (not opened) | n/a | saxophone_alto, then oboe [Estimate] | "Square Lead" |
| saxophone_alto | (`mtg-sax/... Alto Sax`: does not load) | n/a | saxophone_tenor [Estimate] | "Square Lead" |
| saxophone_tenor | `vcsl/Aerophones/Reed Aerophones/Tenor Saxophone - *` | n/a | saxophone_alto, saxophone_baritone [Estimate] | "Square Lead" |
| saxophone_baritone | (`mtg-sax/... Baritone Sax`: does not load) | n/a | saxophone_tenor, then bass_clarinet [Estimate] | "Square Lead" |
| horn_f | `vsco2ce/FHornSus`, `FHornStac`, `FHornMute`; `sso/... Horns *`, `Horn Solo *` | either | trombone_tenor (low), euphonium [Estimate] | Poly Synth "Brass" |
| wagner_tuba_tenor | none | n/a | horn_f, then euphonium, then trombone_tenor [Card cards/horns.md#wagner_tuba_tenor] | "Brass" |
| wagner_tuba_bass | none | n/a | horn_f, then tuba [Card cards/horns.md#wagner_tuba_bass] | "Brass" |
| trumpet_bb | `vsco2ce/TrumpetSus*`, `TrumpetStac`, mute patches; `sso/... Trumpets *`, `Trumpet Solo *` | either | cornet, flugelhorn presets if any [Card cards/high_brass.md#cornet] | "Brass" |
| trumpet_c | none | n/a | trumpet_bb [Card cards/high_brass.md#trumpet_c] | "Brass" |
| trumpet_piccolo | none | n/a | trumpet_bb (top notes need confirmation) [Card cards/high_brass.md#trumpet_piccolo] | "Brass" |
| cornet | none | n/a | trumpet_bb [Card cards/high_brass.md#cornet] | "Brass" |
| flugelhorn | none | n/a | trumpet_bb at lower velocity, then horn_f [Card cards/high_brass.md#flugelhorn] | "Brass" |
| trombone_tenor | `vsco2ce/TromboneSus`, `TromboneVib`, `TromboneStac`; `sso/... Trombones *`, `Tenor Trombone Solo *` | either | trombone_bass, euphonium, horn_f [Estimate] | "Brass" |
| trombone_bass | `sso/... Bass Trombone Solo *` | n/a | trombone_tenor (high), tuba (low) [Estimate] | "Brass" |
| tuba | `vsco2ce/Tuba-KS`, `TubaSus`, `TubaStac`; `sso/... Tuba *`; `war-tuba/*` (partly) | n/a | trombone_bass, cimbasso [Card cards/low_brass.md#cimbasso] | "Brass" |
| cimbasso | none | n/a | tuba, then trombone_bass [Card cards/low_brass.md#cimbasso] | "Brass" |
| euphonium | none | n/a | trombone_tenor, then tuba, then horn_f [Card cards/low_brass.md#euphonium] | "Brass" |
| harp | `vsco2ce/Harp`; `vcsl/... Concert Harp` (partly); `sso/Concert Harp` (partly) | n/a | piano_concert_grand at low velocity for arpeggios; the piano has served as an orchestral substitute for the harp [Card cards/acoustic_pianos.md#piano_concert_grand] | Poly Synth "Pluck" |
| guitar_classical | none read (FreePats nylon guitar not inspected) | n/a | harp, lute [Estimate] | "Pluck" |
| lute | none | n/a | guitar_classical, then harp [Card cards/plucked.md#lute] | "Pluck" |
| mandolin | `terkelsen-mandolin/Terkelsen's Mandolin` | n/a | guitar_classical [Card cards/plucked.md#mandolin] | "Pluck" |
| piano_concert_grand | none plays (`sso/Grand Piano/Grand Piano`, `splendid/*`: no) | n/a | piano_studio_grand, then piano_upright [Estimate] | Poly Synth "Electric Keys" |
| piano_studio_grand | `vcsl/Chordophones/Zithers/Grand Piano, Kawai`, `Grand Piano, Steinway B` (partly); `salamander/*` (no) | n/a | piano_concert_grand, piano_upright [Estimate] | "Electric Keys" |
| piano_upright | `vsco2ce/UprightPiano`, `VSUpright1`; `vcsl/... Upright Piano, Knight`, `Yamaha` | n/a | piano_studio_grand [Estimate] | "Electric Keys" |
| piano_felt | none | n/a | piano_upright or piano_studio_grand at low velocity [Card cards/acoustic_pianos.md#piano_felt] | "Electric Keys" |
| piano_prepared | none | n/a | no reasonable neighbour: ask the user [Card cards/acoustic_pianos.md#piano_prepared] | "Pluck", only with confirmation |
| piano_honky_tonk | none read (FreePats Old Piano FB not inspected) | n/a | piano_upright, the same instrument in tune [Card cards/acoustic_pianos.md#piano_honky_tonk] | "Electric Keys" |
| electric_piano_rhodes | `jrhodes3d/jRhodes3d-st-no-xfade` (partly; licence unresolved) | n/a | electric_piano_wurlitzer [Estimate] | "Electric Keys" |
| electric_piano_wurlitzer | `gs-epianos/Wurlitzer EP200/Wurlitzer EP200` (partly) | n/a | electric_piano_rhodes [Estimate] | "Electric Keys" |
| clavinet | none | n/a | harpsichord [Card cards/electric_keys.md#clavinet] | "Pluck" |
| harpsichord | `vcsl/... Harpsichord, *` (partly), `sso/Harpsichord/*` (single-articulation files, partly; `Harpsichord KS` stacks under Q11) | n/a | clavichord, then piano_upright at low velocity [Estimate] | "Pluck" |
| clavichord | none | n/a | harpsichord, quieter [Card cards/historical_keys_organs.md#clavichord] | "Pluck" |
| celesta | `sso/Percussion/Celeste` (partly) | n/a | piano_upright, as Rimsky-Korsakov advises; glockenspiel only with confirmation, since the same source advises against it [Card cards/historical_keys_organs.md#celesta, section 6] | Poly Synth "Glass Bells" |
| organ_pipe | `vsco2ce/Organ*`; `vcsl/... Pipe Organ - *`; `sso/Organ/*` | n/a | organ_hammond only for chords [Estimate] | "Warm Pad" |
| organ_hammond | none read (FreePats setBfree not inspected) | n/a | organ_pipe [Estimate] | "Warm Pad" |
| accordion | none read (FreePats Button Accordion not inspected) | n/a | organ_pipe reed stop (Sonatina `Swell - Oboe 8ft`) [Estimate] | "Warm Pad" |
| timpani | `vsco2ce/Timpani`, `TimpaniRolls`; `vcsl/... Timpani 1 - Hit`, `- Roll`, `- Keyswitch`; `sso/Percussion/Timpani` (partly) | n/a | none pitched; bass_drum for accents only, with confirmation [Estimate] | Drum Kit "Cinematic Toms", toms on notes 41, 43, 45, 47, 48 and 50 only (pitch lost) [Cited S34: C22] |
| xylophone | `vsco2ce/Xylophone`; `vcsl/... Xylophone - *`; `sso/Percussion/Xylophone` | n/a | marimba (low), glockenspiel (high) [Estimate] | "Pluck" |
| marimba | `vsco2ce/Marimba`; `vcsl/... Marimba` (partly); `sso/Percussion/Marimba Hits`, `Marimba Rolls` | n/a | xylophone, vibraphone [Estimate] | "Pluck" |
| vibraphone | `vcsl/... Vibraphone - Soft Mallets`, `- Hard Mallets`, `- Bowed`; `sso/Percussion/Vibraphone` (partly) | n/a | glockenspiel, celesta [Estimate] | "Glass Bells" |
| glockenspiel | `vsco2ce/Glockenspiel`; `vcsl/... Glockenspiel` (partly); `sso/Percussion/Glockenspiel` | n/a | celesta, crotales [Card cards/tuned_percussion.md#crotales] | "Glass Bells" |
| tubular_bells | `vsco2ce/TubularBells`; `vcsl/... Tubular Bells 1`, `2`; `sso/Percussion/Chimes` | n/a | carillon (none), glockenspiel [Estimate] | "Glass Bells" |
| crotales | `sso/Percussion/Crotales` (not opened) | n/a | glockenspiel [Card cards/tuned_percussion.md#crotales] | "Glass Bells" |
| carillon | none | n/a | tubular_bells [Card cards/tuned_percussion.md#carillon] | "Glass Bells" |
| snare_drum | `vcsl/... Snare Drum, Modern 1-3`; `vsco2ce/GM-StylePerc` keys 37-41; `sso/Percussion/Bass Drum & Snare` | n/a | none | Drum Kit note 38 (snare) |
| bass_drum | `vcsl/... Bass Drum 1-2`; `vsco2ce/GM-StylePerc` 36 (rubs 32-35) | n/a | timpani low note for accents [Estimate] | Drum Kit 36 (kick), "Cinematic Toms" kit |
| clash_cymbals | `vcsl/... Clash Cymbals 1-2`; `vsco2ce/GM-StylePerc` 49 | n/a | suspended_cymbal hit [Estimate] | Drum Kit 49 (crash) |
| suspended_cymbal | `vcsl/... Suspended Cymbal 1-2`; `vsco2ce/GM-StylePerc` 47-51, 59, 101-104 | n/a | clash_cymbals for single hits [Estimate] | Drum Kit 51 (ride) for hits, 49 (crash) for swells |
| tam_tam | `vcsl/... Gong 1-2`; `vsco2ce/GM-StylePerc` 42, 46; `sso/Percussion/Cymbals & Tamtam` | n/a | suspended_cymbal roll [Estimate] | Drum Kit 49 with "Cinematic Toms" long cymbal decay |
| triangle | `vcsl/... Triangles`; `vsco2ce/GM-StylePerc` 78-81; `sso/Percussion/Triangle` | n/a | crotales, glockenspiel top note [Estimate] | none in the Drum Kit (GM 80/81 are not in `GM_MAP`) → "Glass Bells" high note, or silence |
| tambourine | `vcsl/... Tambourine 1-2`; `vsco2ce/GM-StylePerc` 53-55 | n/a | sleigh_bells [Estimate] | Drum Kit 54 |
| castanets | Sonatina castanets per the card's coverage matrix (file not identified in the tree) | n/a | wood_block [Estimate] | Drum Kit 75 (claves) |
| wood_block | `vcsl/... Woodblock`; Sonatina wood blocks per the card | n/a | temple_blocks [Card cards/untuned_percussion.md#temple_blocks] | Drum Kit 76/77 |
| temple_blocks | none | n/a | wood_block [Card cards/untuned_percussion.md#temple_blocks] | Drum Kit 76/77 |
| sleigh_bells | `vcsl/... Sleigh Bells`; `vsco2ce/GM-StylePerc` 82 | n/a | tambourine shake [Estimate] | Drum Kit 54 |
| maracas | none with an `.sfz` (VSCO raw folder only) | n/a | shaker [Estimate] | Drum Kit 70 (plays the kit's shaker) |
| shaker | `vcsl/... Shaker, Small`, `Shaker, Large` | n/a | maracas [Estimate] | Drum Kit 70 |
| whip | `vcsl/... Slapstick` | n/a | none [Card cards/untuned_percussion.md#whip] | Drum Kit 39 (clap), only with confirmation; else silence |
| choir_soprano, choir_alto, choir_tenor, choir_bass | `sso/Chorus - Notation/Mixed Chorus`, `Large Chrous` (mixed, partly) for every part | n/a | the same mixed choir for each part [Estimate] | Poly Synth "Choir Pad" |
| voice_soprano, voice_mezzo_soprano, voice_alto, voice_tenor, voice_baritone, voice_bass | none | step 3: choir section of the same voice type (`sso/... Mixed Chorus`), told as "choir instead of a soloist" | none | "Choir Pad" |

Sonara's Drum Kit plays the 34 notes of `GM_MAP`, which include 36, 38, 39, 49, 51, 54, 70, 75, 76 and
77 but not 80/81 (triangle) or any note above 77 [Cited S31: gm.rs; Cited S34: C20]. Its "Cinematic Toms" kit
holds "big low toms and long cymbals" [Cited S31: builtin.rs]. In notes 41-50 only 41, 43, 45,
47, 48 and 50 are toms; 42, 44 and 46 are hi-hats and 49 is the crash. The kit's own description
says "41-50 toms", which has the same gap [Cited S34: C22].

## 5. General MIDI fallback table

GM programs are shown 1-128 and sent as 0-127; percussion is on channel 10 [Cited S29]. The value
"sent" below is the program number minus one. Notes 82 Shaker, 83 Jingle Bell, 84 Belltree and 85
Castanets are Roland GS extensions, not GM1 [Cited S29; Card cards/free_libraries.md, open question 13].

Every row takes its program number from the card for that id (`gm_program` in the card's JSON,
[Card cards/<group>.md#<id>]; the same values as the GM column of [model.md](model.md)),
cross-checked against the GM list [Cited S29]. Where the card has no GM1 program, the fallback
comes from the card's GM section when it names one (marked "card"); the other fallbacks and the
flags are this author's. A GM program plays at sounding pitch, so no fallback transposes.

| Id | GM program (1-based) | Sent | Flag |
|---|---|---|---|
| violin | 41 Violin | 40 | ensemble use: 49 String Ensemble 1; tremolo: 45; pizzicato: 46 |
| viola | 42 Viola | 41 | as violin |
| cello | 43 Cello | 42 | as violin |
| double_bass | 44 Contrabass | 43 | as violin |
| piccolo | 73 Piccolo | 72 | |
| flute | 74 Flute | 73 | |
| alto_flute | none → 74 Flute (card) | 73 | approximate [Card cards/flutes.md#alto_flute] |
| bass_flute | none → 74 Flute (card) | 73 | approximate; a GM flute may not reach the low notes [Card cards/flutes.md#bass_flute; Estimate for the low notes] |
| oboe | 69 Oboe | 68 | |
| english_horn | 70 English Horn | 69 | |
| oboe_damore | none → 69 Oboe (card) | 68 | approximate [Card cards/double_reeds.md#oboe_damore] |
| bassoon | 71 Bassoon | 70 | |
| contrabassoon | none → 71 Bassoon (card) | 70 | approximate; a GM bassoon's low limit is not specified [Card cards/double_reeds.md#contrabassoon] |
| clarinet_bb, clarinet_a, clarinet_eb, bass_clarinet | 72 Clarinet | 71 | bass_clarinet and clarinet_eb are approximate; 71 Bassoon is the card's alternative for the bass clarinet's register [Card cards/clarinets.md#bass_clarinet] |
| saxophone_soprano | 65 Soprano Sax | 64 | |
| saxophone_alto | 66 Alto Sax | 65 | |
| saxophone_tenor | 67 Tenor Sax | 66 | |
| saxophone_baritone | 68 Baritone Sax | 67 | |
| horn_f | 61 French Horn | 60 | section: 62 Brass Section [Estimate] |
| wagner_tuba_tenor, wagner_tuba_bass | none → 61 French Horn (card) | 60 | **weak**: horn mouthpiece and players, but a different instrument [Card cards/horns.md#wagner_tuba_tenor, #wagner_tuba_bass] |
| trumpet_bb, trumpet_c, trumpet_piccolo, cornet, flugelhorn | 57 Trumpet | 56 | con_sordino: 60 Muted Trumpet; flugelhorn **weak**, with 61 French Horn as the card's alternative for its soft colour [Card cards/high_brass.md#flugelhorn] |
| trombone_tenor, trombone_bass | 58 Trombone | 57 | |
| tuba | 59 Tuba | 58 | |
| cimbasso | none → 58 Trombone (card: the sound belongs to the trombone bass); 59 Tuba also possible | 57 | approximate [Card cards/low_brass.md#cimbasso] |
| euphonium | none → 59 Tuba (card; 58 Trombone matches the register better) | 58 | approximate [Card cards/low_brass.md#euphonium] |
| harp | 47 Orchestral Harp | 46 | |
| guitar_classical | 25 Acoustic Guitar (nylon) | 24 | |
| lute | none → 25 Acoustic Guitar (nylon) (card) | 24 | **weak** [Card cards/plucked.md#lute] |
| mandolin | none → 26 Acoustic Guitar (steel) (card: what a GM2 device plays when its Mandolin variation is missing) | 25 | **weak** [Card cards/plucked.md#mandolin] |
| piano_concert_grand | 1 Acoustic Grand Piano | 0 | |
| piano_studio_grand | 2 Bright Acoustic Piano | 1 | the card's choice; 1 is equally defensible [Estimate] |
| piano_upright | none → 1 (card) | 0 | approximate: GM1 has no upright program [Card cards/acoustic_pianos.md#piano_upright] |
| piano_felt | none → 1 (card), at low velocity | 0 | **weak** [Card cards/acoustic_pianos.md#piano_felt; Estimate for the velocity] |
| piano_prepared | none → 1 only as a last resort (card) | 0 | **no reasonable GM equivalent**: program 1 misrepresents the sound; ask the user [Card cards/acoustic_pianos.md#piano_prepared] |
| piano_honky_tonk | 4 Honky-tonk Piano | 3 | |
| electric_piano_rhodes | 5 Electric Piano 1 | 4 | |
| electric_piano_wurlitzer | 5 Electric Piano 1 | 4 | GM 5 covers both [Card cards/free_libraries.md#coverage] |
| clavinet | 8 Clavi (Wikipedia lists it as "Clavinet") | 7 | |
| harpsichord | 7 Harpsichord | 6 | |
| clavichord | none → 8 Clavi | 7 | **weak**; the card names 7 Harpsichord or 8 Clavi [Card cards/historical_keys_organs.md#clavichord] |
| celesta | 9 Celesta | 8 | |
| organ_pipe | 20 Church Organ | 19 | |
| organ_hammond | 17 Drawbar Organ | 16 | |
| accordion | 22 Accordion | 21 | |
| timpani | 48 Timpani | 47 | |
| xylophone | 14 Xylophone | 13 | |
| marimba | 13 Marimba | 12 | |
| vibraphone | 12 Vibraphone | 11 | |
| glockenspiel | 10 Glockenspiel | 9 | |
| tubular_bells | 15 Tubular Bells | 14 | |
| crotales | none → 10 Glockenspiel (card; 113 Tinkle Bell possible) | 9 | approximate [Card cards/tuned_percussion.md#crotales] |
| carillon | none → 15 Tubular Bells | 14 | approximate; tubular bells are the orchestral substitute [Card cards/tuned_percussion.md#carillon] |
| choir_soprano, choir_alto, choir_tenor, choir_bass | 53 Choir Aahs | 52 | |
| voice_soprano, voice_mezzo_soprano, voice_alto, voice_tenor, voice_baritone, voice_bass | 54 Voice Oohs | 53 | approximate: GM has no solo-voice program [Card cards/free_libraries.md#coverage] |

Untuned percussion, on channel 10. The notes come from the cards (`gm_percussion_notes`),
cross-checked against the GM key map [Cited S29].

| Id | GM1 notes | Flag |
|---|---|---|
| snare_drum | 38 Acoustic Snare, 40 Electric Snare | rolls: none in GM; repeat strokes |
| bass_drum | 35 Acoustic Bass Drum, 36 Bass Drum 1 | |
| clash_cymbals | 49 Crash Cymbal 1, 57 Crash Cymbal 2 | |
| suspended_cymbal | 49 (card); 51/59 ride as the nearest stick sound [Card cards/free_libraries.md#coverage] | swells and bowing: none |
| tam_tam | none | **no GM equivalent**; 52 Chinese Cymbal is the nearest metal crash [Estimate] |
| triangle | 81 Open Triangle, 80 Mute Triangle | |
| tambourine | 54 Tambourine | |
| castanets | none in GM1 (85 is GS) | **flag**; 75 Claves as substitute [Estimate] |
| wood_block | 76 Hi Wood Block, 77 Low Wood Block; also melodic program 116 Woodblock (sent 115) | |
| temple_blocks | none | **flag**; 76/77 [Estimate] |
| sleigh_bells | none in GM1 (83 Jingle Bell is GS) | **flag**; 54 Tambourine [Estimate] |
| maracas | 70 Maracas | |
| shaker | none in GM1 (82 is GS) | **flag**; 70 Maracas or 69 Cabasa [Estimate] |
| whip | none | **no GM equivalent** |

Engine options for step 5:

| Option | Facts | Source |
|---|---|---|
| rustysynth | MIT; "pure Rust"; no dependencies beyond std; bank selection and program change done; SF2 named, SF3 not mentioned; reverb and chorus not implemented | [Cited S30] |
| MuseScore_General | MIT; its acknowledgements and copyright notices must be included in any derivative work; SF2 and SF3; MS Basic is the same bank renamed, per its identical documentation (sample data not compared) | [Card cards/free_libraries.md#musescore_general_and_ms_basic] |
| FluidR3_GM | MIT with the copyright notice; Debian's packaging files alone are GPL-2+, so they are never shipped (LIC-01) | [Card cards/free_libraries.md#fluidr3_gm] |
| SFZ General MIDI bank (Discord) | Would avoid a second engine, but today it is mostly stubs, which Sonara fails to load (N3), and it depends on `loprog` | [Cited S10, S34] |
| Real-time fitness of rustysynth | Not checked (QG-13 step 4) | [Estimate] |

## Open questions

1. **Fix the parser, or work around it?** Q1-Q7, Q11 and N1 are parser and sampler gaps
   measured against the SFZ spec; for Q1-Q4, Q11 and N1 the reference player sfizz also
   behaves differently from Sonara. Fixing them would make these usable:
   - the Sonatina key-switch files and timpani;
   - the Salamander remap and the MTG saxophones;
   - the VSCO key-switch defaults;
   - the Bigcat cello;
   - pedalled release samples on every piano with release regions.

   [Cited S20-S26, S32, S31, S34]. Each fix needs a test with a generated fixture that copies the
   real library's header layout (TST-06); [sfz-conformance.md](sfz-conformance.md) names a
   suggested fix and a pinning test for every row. ADR 0010 says the subset was taken from VSCO
   and Sonatina, so the VSCO `sw_default` case and the Sonatina `<control>` case come first
   [Cited S31].
2. **Confirm by ear.** Every Q-row verdict comes from reading code, now confirmed by a second,
   independent reading [Cited S34]; nothing was rendered. How audible the losses are is a render
   question. One render per case would settle it:
   - `vsco2ce/ViolinEns-KS` with no switch note;
   - `sso/Woodwinds - Notation/Flutes KS` with each switch;
   - Sonatina Timpani at velocity 60 and 100;
   - the Salamander remap;
   - a VCSL grand with CC64 held across note-offs (N1);
   - a VSCO patch with and without `ampeg_dynamic`.
3. **Where does the articulation map live?** The options are a `sonara-library.json` v2 field, a
   separate per-library file, or data shipped in Sonara for known libraries. The manifest is not
   on the CTR-01 list, but it is a published v1 format [Cited S31: project.md CTR-01,
   docs/instrument-catalog.md]. Owner decision.
4. **Who writes the switch notes?** An agent or planner writing key-switch notes into clips works
   with today's tools. It does pollute the user's MIDI with technical notes. A per-note
   articulation field would avoid that, but it is a CTR-01 change to `project-v1.schema.json`
   [Cited S31]. Which does the owner prefer?
5. **Preset id to path.** The sampler `instrument` parameter stores file paths (the configured root joined with the entry). Should it
   store preset ids instead? That would change how a parameter value is interpreted, though not
   the schema, and ADR 0010 decision 4 would need amending [Cited S31].
6. **Exclude include fragments.** Should discovery leave out `.sfz` files below a folder named
   `includes`, or files that contain no `<region>`? Sonatina adds 190 such files [Cited S3].
7. **Licences.** Still unresolved: the Terkelsen mandolin (no licence metadata on GitHub)
   [Cited S9, S19], and the Bigcat cello and D. Smolken bass, which rest on GitHub's licence
   metadata and the catalogue label, with no `LICENSE` file found [Cited S9; Card
   cards/free_libraries.md#karoryfer_samples]. Settled here from the card's reading of the
   `LICENSE` files: jRhodes3d samples are CC BY-NC 4.0 (the `.sfz` header's "CC-SA-BY" is the
   weaker source), and the Greg Sullivan E-Pianos are CC BY 3.0 [Card
   cards/free_libraries.md#electric_pianos]. Open beyond Sonara's control: whether Sonatina and
   VPO3 may lawfully redistribute their Philharmonia and Iowa material [Card
   cards/free_libraries.md, open question 14].
8. **Not inspected:** VPO3, every FreePats SFZ set, Keppy's Steinway, the rest of the Sonatina
   and VCSL key-switch files, the Bigcat single-articulation programs and War Tuba's other
   programs. Their verdicts are unknown; their stated licences are listed in section 2.5.
9. **Which keys do VSCO's woodwinds and brass sound in?** VSCO and Sonatina do not state the key
   of their clarinet and trumpet. Samples are at sounding pitch, so only the range check
   depends on it [Estimate].
10. **Step 5 without a second engine?** Is a GM fallback worth it before an SF2 engine exists? A
    user could convert MuseScore_General to SFZ, but that was not tested and its licence notices
    would have to travel with it [Card cards/free_libraries.md#musescore_general_and_ms_basic].
11. **Organ velocity:** organ patches respond to velocity in Sonara (Q8). Should a map's
    `velocity_policy` pin them to one velocity, or should the sampler learn `amp_veltrack`?

## Review notes

Review of 2026-10-30 against the instrument cards, [model.md](model.md), the fact-checked
[cards/free_libraries.md](cards/free_libraries.md) and [sfz-conformance.md](sfz-conformance.md).
Section numbers are unchanged, because other files cite them.

**Repository readiness.** Removed the local checkout path from S31 and the work-folder path from
the tag legend; card tags now name `cards/<file>.md`; added S34 for the verification of the code
claims; dropped internal labels.

**Code claims (sfz-conformance.md).**

1. Q1: any later header at the same or an outer scope level clears the switch settings, a later
   `<group>` included; region-level `sw_*` are never read (N4); sfizz differs.
2. Q2-Q4 and Q11: the spec column now says what the reference player sfizz does; the Salamander
   trace (the stray include lands in `hivel`; only pedal noises play) was added.
3. Q5: the `trigger` remap is silent and never appears in the ignored-opcode warning.
4. Q6: "full level" corrected to "each layer at its normal velocity gain".
5. Q7: Salamander's pedal noises are one down and one up noise per note-on, transposed.
6. Q8: the fixed velocity curve equals the spec default; only non-default `amp_veltrack` diverges.
7. Q9: the spec's default `off_mode` (6 ms) added; Q10: the lost CC-triggered sound noted.
8. Added rows N1-N6. N1 (release samples lost under the sustain pedal) now appears in the VCSL
   piano, Salamander and Sonatina Grand verdicts, in 2.6 and in open questions 1 and 2. N3 (one
   bad sample fails the patch) changes the Discord verdict, the 4.1 availability rule and the
   step 5 engine table. N2 feeds `sonara_verdict` and the step 1 message. N6 is flagged for the
   looped FreePats sets.
9. C1 (paths are root-joined, not canonicalized), C4 (ids survive a move only with a manifest `id`
   or an unchanged folder name), C7 (the 5,000 cap spans all roots), C8 (fragments are also
   choices of the sampler's `instrument` parameter), C9, C12 (loop aliases; opcodes under unknown
   headers dropped silently), C15, C16, C17 (a switch key also plays regions mapped to it).
10. C11: a seek or loop wrap while playing keeps the last switch; `rest()` and `sw_default` apply
    only on play from stop and offline render. Section 1, 2.6, 3.1 and 3.3 corrected; the advice to
    hold switch notes stands with the corrected reason.
11. C22: Drum Kit toms are 41, 43, 45, 47, 48 and 50 only (timpani row and the note after 4.3).
12. C24: `sonara_verdict` and the step 1 loss list now combine a deep load with structural checks.
13. The legato trace now says the `first` voice is dropped silently and gives both attack times.
14. C4: the 3.2 `preset` row now says ids stay stable only with a manifest `id` or an unchanged
    folder name, not with a manifest alone.
15. C24: the 3.2 `sonara_verdict` source cell now lists what the ignored-opcode list misses as
    sfz-conformance.md does: Q1-Q4 and Q11, the silent Q5 remap, N1 and N3.
16. Check, no change: Q1 (row names `<master>` and `<group>`), C11 (sections 1, 2.6, 3.1, 3.3)
    and C22 (timpani row, note after 4.3) re-read against section D; already corrected above.

**Licences (cards/free_libraries.md).** VSCO: CC0 governs; the Readme asks for credit and no
direct sale. Sonatina: Sampling Plus 1.0 terms (attribution, noncommercial whole-work copies,
retired) and its Philharmonia samples. VCSL: CC0; SFZ only in tag `v1.2.2-RC`; release assets
re-queried (none uploaded). Salamander CC BY 3.0; Splendid public domain as a relayed claim.
jRhodes3d settled as CC BY-NC 4.0 for the samples; Greg Sullivan as CC BY 3.0; MTG and Ixox
attribution noted; D. Smolken and Bigcat licence footing noted; War Tuba and Scarypiano CC0;
Keppy's Steinway CC BY-SA 4.0 (strictest reading); FreePats per entry, with the GPL General MIDI
sets kept out by LIC-01; MuseScore_General notices; FluidR3's Debian packaging. Added a `licence`
attribution note to 3.2. Open question 7 shrunk accordingly.

**Contradictions with the cards, fixed here.**

- celesta neighbour: glockenspiel → `piano_upright` (the celesta card cites Rimsky-Korsakov
  advising an upright, not the glockenspiel).
- GM fallbacks: cimbasso 59 Tuba → 58 Trombone; mandolin 25 → 26 Acoustic Guitar (steel);
  prepared piano "none" → program 1 only as a last resort; clavinet named "8 Clavi" as in the
  model; fallbacks that the cards name are now cited to them instead of marked as estimates.
- `piano` (not an id) → `piano_concert_grand` (harp neighbour) and `piano_upright` (harpsichord
  neighbour); honky-tonk neighbour cited to its own card instead of the prepared-piano card.
- 3.2 `part` example: `violin_1` → `violins_1`, as in [ensembles.md](ensembles.md).
- Step 1 example message: `ViolinEns-KS` does not lose velocity crossfades; replaced by Sonatina
  Timpani, which does.
- Switch keys checked against the playable ranges in [model.md](model.md), as section 3.3
  requires: besides VCSL Timpani 1, VSCO `CelloEns-KS` (84-87), War Tuba (24-26) and the jRhodes3d
  keyswitch patch (103-108) overlap their instrument's range; each row now says so.

**Contradictions in other files, not editable here.**

- [cards/free_libraries.md](cards/free_libraries.md) counts "two assets" on the VCSL release; the
  API lists none uploaded, so those are presumably GitHub's automatic source archives.
- [cards/free_libraries.md](cards/free_libraries.md) says VSCO plays "mostly" as is, without the
  Q1 silence of `ViolinEns-KS`, `Flute-KS` and `Clarinet-KS`; and calls the Salamander remap
  "partly", where Q2 makes it play no notes.
- [cards/acoustic_pianos.md](cards/acoustic_pianos.md) (open question 14) expects Salamander's
  CC64 pedal noises "may never fire"; Q7 shows that they fire on every key.
- [cards/free_libraries.md](cards/free_libraries.md) rests Bigcat and D. Smolken on the catalogue
  label; GitHub's licence metadata (S9) also says CC0-1.0.
- N1, N3 and N5 wording aligned with sfz-conformance.md: N3 is latent, N5 has one harmless hit, the Salamander remap's release sections already load (so N1 applies today), and the VCSL Steinway B is unconfirmed.
