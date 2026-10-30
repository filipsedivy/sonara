# Orchestral audio engine: research and decision

_Date: 2026-10-27 · Decision: [ADR 0010](decisions/0010-sample-based-orchestral-instruments.md)
· User guide: [sample-libraries.md](sample-libraries.md)_

**Reading guide.** Every claim is tagged **VERIFIED** (read in a primary source, linked),
**INFERRED** (my deduction, basis stated) or **UNKNOWN**. **Nobody listened to any audio for
this report**, and no recording was measured. Sound fidelity is therefore not scored: where the
matrix needs it, the cell says *not assessed*. A technically working integration (section 12) is
not evidence that anything sounds realistic.

## 1. Sonara's audio architecture today

Verified from the source and `docs/`:

- Native Rust workspace, Tauri 2 desktop GUI, CLI and MCP on one project session. macOS 26+,
  Apple Silicon. C is used only for miniaudio and the run-time codec libraries.
- Music is MIDI-like clips (notes, CC, pitch bend) on instrument tracks, played by instrument
  *processors* behind one `Processor` trait in `sonara-dsp` (a real-time crate: no allocation,
  locks, panics or `unsafe`; RT rules in `.claude/rules/rust/realtime.md`). Offline rendering and
  live playback share the graph (`sonara-engine`).
- Before this work the instruments were two synthesizers: Poly Synth (13 presets) and Drum Kit
  (5 kits). No sampler existed (milestone M4); plugin hosting (VST3/AU/CLAP) is milestone M5 and
  not implemented.
- Hard constraints that shape the choice: **RT-10** (`unsafe` only in three FFI/ring modules),
  **LIC-01** (no GPL/AGPL; no LGPL Rust crates; LGPL C only as replaceable dylibs; `LICENSE` is 0BSD), **QG-13** (library first), **GLD-01/CTR-01** (the built-in catalog of
  twelve plugins is pinned by `golden/plugins/catalog.json`; 57 agent tools; the `.sonara` schema).
- Integration seams that exist: `DeviceState` carries non-parameter state to a processor (an
  impulse response for the convolver); the engine builder loads media off the real-time thread;
  `ProjectSession` takes its plugin registry from the application; the `.sonara` device stores
  `plugin_id` and string/number/bool `params`.

## 2. Requirements

Functional: play orchestral instruments from recorded samples; velocity layers, round robin,
articulation switching, release samples; polyphony; MIDI note/CC/bend; live and offline;
selection of instruments through the existing agent tools and GUI; clear errors for missing
libraries; no change to existing behaviour.
Non-functional: RT rules; deterministic offline renders; bounded memory; licence-clean
distribution (no samples in the repository); no breaking change to the contracts.

## 3. Method and sources

Primary sources only: crates.io metadata, GitHub repository metadata and files, official
documentation and licence texts, linked inline. Crate source of `rustysynth` 1.3.8 was read
locally. Prices of commercial products come from retailers and press, not first-party price
lists (INFERRED, indicative). Searches ran 2026-10-27.

## 4. Engines

| Engine | Format | Licence | Findings |
|---|---|---|---|
| **rustysynth** 1.3.8 ([crates.io](https://crates.io/crates/rustysynth), [repo](https://github.com/sinshu/rustysynth)) | SF2 only, 16-bit | MIT, no dependencies | VERIFIED active (pushed 2026-10-20). SF3 rejected (a PR was closed unmerged). Render path appears preallocated and lock-free by reading the source (INFERRED, not proven by `assert_no_alloc`); `render` calls `panic!` on mismatched buffers and indexes slices (RT-05 needs a guard). Whole sample set in RAM. No SFZ, no articulations. |
| **oxisynth** 0.1.0 | SF2 | **LGPL-2.1** | VERIFIED. Banned as a Rust crate (LIC-01). |
| **xsynth-core** 0.4.0 / **nidhi** 1.1.0 | SFZ/SF2 | **LGPL-3.0 / GPL-3.0** | VERIFIED. Banned. |
| **FluidSynth** 2.6.1 ([repo](https://github.com/FluidSynth/fluidsynth)) | SF2 (SF3 UNKNOWN) | LGPL-2.1 | VERIFIED active. Only as a replaceable dylib with source in the bundle (LIC-01). Its API mutex is on by default and can be turned off ([settings](https://www.fluidsynth.org/api/settings_synth.html)); allocation on the render thread UNKNOWN. No SFZ (INFERRED). Needs FFI: an RT-10 change. |
| **sfizz** 1.2.3 ([repo](https://github.com/sfztools/sfizz), [opcode status](https://sfztools.github.io/sfizz/development/status/opcodes/?v=2)) | SFZ v1/v2 | BSD-2-Clause | VERIFIED: supports the opcodes orchestral libraries use (`lovel/hivel`, `seq_*`, `sw_*`, `trigger`, `group/off_by`, `loop_mode`, `ampeg_*`, `locc/hicc`, crossfades, `#include`); `lochan/hichan` not supported by design. **Both repositories are archived** (last push 2025-03; last release 2024-01). C++. Real-time safety of `render_block` is not documented: UNKNOWN. No Rust bindings: own FFI, an RT-10 change. Dependency licence question: `faust-libraries` are "a permissive variant of the LGPL". |
| **Pure-Rust SFZ crates** | SFZ | `sofiza` MIT/Apache | VERIFIED: `sofiza` is a parser only, last release 2022. `vibelang-sfz` (young, maturity UNKNOWN), `manguebyte-sampler` (new, licence UNKNOWN). No maintained permissive player. |
| **Plugin hosting** (`vst3` 0.3.0, `vst3-host` 0.10.0, `clack-host` 0.2.0) | VST3 / CLAP | MIT or Apache | VERIFIED metadata only. No AU hosting crate found. VST3 SDK licence UNKNOWN (must be checked before use). Would need out-of-process sandboxing (INFERRED). |

## 5. Sound libraries

Licence of the library is judged independently of the player. "Bundle" means shipping it inside
Sonara; **no library is bundled** (nothing may be committed or shipped without an unambiguous
licence).

| Library | Format | Licence | Bundle? | Notes |
|---|---|---|---|---|
| **VSCO 2 CE** ([repo](https://github.com/sgossner/VSCO-2-CE), [licence](https://raw.githubusercontent.com/sgossner/VSCO-2-CE/master/LICENSE)) | WAV + SFZ (`SFZ` branch: 75 `.sfz`, 3,168 `.wav`) | CC0 | possible (provenance of every sample UNKNOWN) | Strings (solo violin, violin/viola/cello sections, a solo double bass: corrected 2026-10-30, see [orchestra/cards/free_libraries.md](orchestra/cards/free_libraries.md)), woodwinds, brass, harp, timpani, tubular bells, pianos, percussion. SFZ files use a small opcode set, which the Sonara sampler plays. |
| **VCSL** ([repo](https://github.com/sgossner/VCSL)) | WAV (+ SFZ generator) | CC0 | possible | Large, 1–3 velocity layers and about 1 round robin per articulation (VERIFIED from the README). |
| **Sonatina Symphonic Orchestra** ([repo](https://github.com/peastman/sso)) | SFZ, WAV/FLAC | CC Sampling Plus 1.0 (retired) | **unresolved, no** | Full orchestra with keyswitched and performance patches; performance patches use CC 1 and CC 21, which the Sonara sampler does not play. Some sample origins unknown. User-installed use is fine. |
| **Virtual Playing Orchestra 3** ([page](https://virtualplaying.com/virtual-playing-orchestra/)) | WAV + SFZ | author's terms plus CC BY-SA parts | **unresolved, no** | "Not right to repackage and sell"; share-alike parts clash with `LICENSE`. |
| **Philharmonia samples** ([page](https://philharmonia.co.uk/resources/sound-samples/)) | recordings | free incl. commercial work, not "as is" as a sampler | no | |
| **U. Iowa MIS** ([page](https://theremin.music.uiowa.edu/MIS.html)) | recordings | "any projects, without restrictions"; silent on redistribution | unresolved | No SFZ mapping supplied. |
| **FluidR3_GM**, **MuseScore_General** ([licence](https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General_License.md)) | SF2 (SF3) | MIT, notices required | possible | GM sets: 128 programs, no articulations. Not played by the Sonara sampler. |
| **GeneralUser GS** ([licence](https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/main/documentation/LICENSE.txt)) | SF2 | custom; author admits uncertain sample provenance | unresolved | |
| **Salamander Grand Piano** ([FreePats](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html)) | SFZ + FLAC | CC BY 3.0 | possible with attribution | 16 velocity layers (per MuseScore's handbook). Other FreePats entries differ (corrected 2026-10-30: the FreePats Concert Harp sounds are CC0 1.0; only the page photo is CC BY-NC, see [orchestra/cards/free_libraries.md](orchestra/cards/free_libraries.md)). |
| **Arachno** | SF2 | personal use | no | |
| **BBCSO Discover, Spitfire LABS, Berlin Free Orchestra** | proprietary | EULA | no | Run only in their own players (INFERRED for the last two). |

**Commercial libraries** (BBC Symphony Orchestra, VSL Synchron, EastWest Opus, Orchestral Tools,
Kontakt libraries): proprietary formats played by their own plugin (VST3/AU/standalone). Indicative
prices from retailers: BBCSO Core about £399 list; Synchron Strings $330–$595; Opus about $499+;
Berlin Strings €420–€840. They fall in your third category: usable only through an external
player, hence plugin hosting. Their articulation depth and quality is the reason the industry
uses them (INFERRED from product descriptions; not verified by listening or by their EULAs).

## 6. Licence analysis

- `sonara.sampler` is our code, no new dependency: no licence impact (`docs/dependencies.md`).
- Libraries are user-installed; Sonara redistributes none. Each library's licence binds the user.
- Bundling later needs a per-library decision. Candidates with clean licences: VSCO 2 CE and
  VCSL (CC0, provenance caveat), MuseScore_General and FluidR3_GM (MIT, notices), Salamander
  (CC BY, attribution). Not bundlable without legal review: Sonatina, VPO, Philharmonia, Iowa,
  GeneralUser GS.
- LGPL/GPL crates are excluded outright. FluidSynth and sfizz would each be an owner decision
  (section 9).

## 7. Scoring

Weights from the task. The performance criterion (10 %) is **UNKNOWN** for every candidate (no
benchmark of any engine in this setting was run) and is dropped; fidelity (30 %) is **not
assessed** (nobody listened). Scores below cover the remaining 60 % of the weight and are
renormalised to a 1–10 scale. They rank engines on what the evidence supports and **must not be
read as a quality ranking**.

| Candidate (engine + library) | Articulation & dynamics (20) | Coverage (10) | Architecture fit (15) | Licence & distribution (10) | Maintenance (5) | Score (of 60 %) | Blocker |
|---|---|---|---|---|---|---|---|
| **A. Sonara sampler + VSCO 2 CE** | 5 | 7 | 9 | 10 | 6 | **7.3** | none |
| B. Sonara sampler + SSO (user-installed) | 4 | 8 | 9 | 7 | 6 | 6.6 | CC-driven patches only partly play |
| C. rustysynth + GM SF2 | 3 | 7 | 7 | 9 | 8 | 6.1 | no articulations; needs a panic-proof wrapper |
| F. Plugin hosting + commercial | 9 | 9 | 2 | 3 | 4 | 5.8 | M5; sandbox; SDK licence UNKNOWN |
| D. sfizz (FFI) + SSO | 7 | 8 | 3 | 6 | 3 | 5.7 | RT-10 amendment; archived upstream |
| E. FluidSynth (dylib) + SF2 | 3 | 7 | 3 | 6 | 9 | 4.7 | RT-10 amendment; LGPL decision |

Rationale for the main cells (evidence, not listening):

- *Articulation, A:* VSCO's SFZ files use velocity layers, `seq_*` round robin and keyswitches,
  which the sampler plays; they do not use CC layers. No CC crossfades: a 5, not higher.
- *Articulation, D:* sfizz plays CC layers and crossfades (VERIFIED opcode support).
- *Articulation, F:* INFERRED from the products' feature lists; not verified.
- *Architecture, A:* implemented and inside every rule; D/E need an RT-10 change; F needs M5.
- *Maintenance, A:* our code with one maintainer (6); sfizz archived (3).

The top-quality ceiling is almost certainly F (commercial libraries), which is exactly what the
matrix cannot show without fidelity scores. It is the path for later (section 9), not for now.

## 8. Recommended architecture (implemented)

See [ADR 0010](decisions/0010-sample-based-orchestral-instruments.md). In short: the pure-Rust
`sonara.sampler` processor (`sonara-dsp`), an SFZ reader and sample decoder in `sonara-media`
that builds an immutable `SampleBank` off the real-time thread, a library scanner and registry
extension in `sonara-app`, instrument selection through the existing tools. Responsibilities:

| Concern | Where |
|---|---|
| Voice engine, layers, round robin, release, polyphony | `sonara-dsp::processors::sampler` |
| `SampleBank`, `Region`, `Sample` data model | `sonara-dsp::bank` |
| SFZ parsing, sample decoding, bank cache | `sonara-media::sfz`, `MediaProvider::load_sample_bank` |
| Bank hand-off to the processor, soft failure | `sonara-engine::graph` (`sampler_state`, `sampler_params`) |
| Libraries, catalog, `instrument` choices | `sonara-app::samples`, `sonara_core::plugins::sampler_descriptor` |

## 9. Rejected or deferred alternatives

| Alternative | Decision | Reason |
|---|---|---|
| sfizz via FFI | deferred, owner | Best SFZ coverage, but needs an RT-10 amendment, C++ in a C-for-codecs-only repository, an archived upstream, an undocumented RT contract and a licence question on `faust-libraries`. |
| FluidSynth dylib | deferred, owner | SF2 only, no articulations; LGPL dylib like libsoxr/LAME would be allowed, but RT-10 would change. |
| rustysynth | deferred | Fits the rules, but only for a GM fallback; adds a second engine. Revisit when a General MIDI default is wanted. |
| oxisynth, xsynth-core, nidhi | rejected | LGPL/GPL crates (LIC-01). |
| `sofiza` as the parser | rejected | Unmaintained since 2022; it parses one file, not `#include` trees or `default_path` semantics. The SFZ subset we need is small. |
| Bundling any library | rejected | No library has an unambiguous, redistributable licence that was verified end to end; CC0 provenance is itself UNKNOWN. |
| Plugin hosting | deferred (M5) | The only route to commercial libraries; separate design (sandbox, SDK licence, AU). |

## 10. Expected sound quality and known limits

- **Not judged.** The quality of a result is set by the library; the sampler adds linear
  interpolation, an attack/release envelope and level scaling. Linear interpolation is not
  transparent for large transpositions (INFERRED).
- No CC-driven dynamics or crossfades, no legato, no filters, no per-region pan, no streaming,
  no velocity-curve opcodes. The velocity law is a fixed `(velocity/127)²` gain chosen by us, not
  taken from sfizz (UNKNOWN whether it matches).
- A graph rebuild during playback restarts sounding sampler notes.
- Memory: whole samples are decoded into RAM (cap 3 GiB per patch).
- One-shot loading of a large patch blocks the graph build, not the audio thread.

## 11. Impact

| Area | Impact |
|---|---|
| Dependencies | None added (`docs/dependencies.md`). |
| Performance | Not measured (HW-01: hardware numbers stay local; see open checks). The processor allocates nothing (`assert_no_alloc` in `tests/sampler.rs`). |
| Distribution | Unchanged: no samples shipped; the catalog is unchanged unless a library is configured. |
| Contracts | 57 tools, project schema and golden catalog unchanged (CTR-01, GLD-01). |

## 12. Result and acceptance

Implemented and tested with generated fixtures (no licensed samples):

- `crates/sonara-dsp/tests/sampler.rs`: velocity layer selection, round-robin alternation,
  release and voice freeing, release-trigger tails, keyswitches, exclusive groups, loop pitch,
  64-voice stealing without clipping, silence without an instrument; all inside
  `assert_no_alloc`.
- `crates/sonara-media/tests/sfz.rs`: opcode inheritance, `#include`/`#define`/`default_path`,
  note names, ignored opcodes, missing patch or sample, empty patch.
- `crates/sonara-engine/tests/sampler.rs`: an offline render through a sampler track, a missing
  patch, a not-installed instrument, an unregistered sampler.
- `crates/sonara-app/src/samples.rs`: library scan and conditional registration.

Not done, and open for the owner: the FFI engines (RT-10), plugin hosting (M5), bundling any
library, CC-driven articulations, a General MIDI fallback, a check of the sampler in the GUI (the catalog is shared, but no GUI session was run; the
device editor shows an enum of absolute paths, which a friendlier picker would improve), and
**a listening test of a
real library** (the first thing to do after installing VSCO 2 CE).
