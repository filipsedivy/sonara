# ADR 0010: Sample-based orchestral instruments

- **Status:** Accepted (2026-10-27)
- **Research:** [orchestral-audio-engine-research.md](../orchestral-audio-engine-research.md)
- **Amendment proposed:** [ADR 0011](0011-orchestral-instruments-and-performance.md) (proposed) would amend decision 4
  and two consequences (live rebuilds, the memory cap)

## Context

Sonara's instruments were synthesized only (Poly Synth, Drum Kit). Realistic orchestral sound
needs recorded samples with velocity layers, round robin, keyswitched articulations and release
samples. The constraints that decide the design are rules, not taste:

- RT-10 allows `unsafe` in three modules only, so a C or C++ engine (sfizz, FluidSynth) needs an
  owner-approved rule change. LIC-01 allows LGPL only as a replaceable dynamic library.
- GLD-01 and CTR-01 pin the built-in plugin catalog (`golden/plugins/catalog.json`, twelve
  descriptors), the 57 agent tools and the `.sonara` schema.
- No permissively licensed, maintained Rust crate plays multi-file SFZ libraries.

## Decision

1. **Engine: `sonara.sampler`, our own pure-Rust SFZ sample player** in `sonara-dsp` (a real-time
   processor with 64 preallocated voices, no `unsafe`, no new dependency). QG-13 reason: no
   suitable crate exists (`sofiza` is unmaintained since 2022; `oxisynth`, `xsynth-core` and
   `nidhi` are LGPL/GPL and banned; `rustysynth` is SF2-only and has no articulations), and the
   C++ alternative needs an RT-10 amendment.
2. **Patch reading off the real-time thread:** `sonara-media::sfz` parses the SFZ subset and
   decodes the samples into an immutable `SampleBank` (`sonara-dsp::bank`), cached per patch in
   `MediaProvider`. The sampler receives the bank through `DeviceState::sample_bank`, like a
   convolver receives its impulse response.
3. **Libraries are installed by the user; none is bundled.** The settings key
   `sample_libraries` lists directories. `sonara-app::samples` scans them for `.sfz` files.
4. **The sampler is not a built-in.** `builtin_registry()` stays at twelve descriptors, so the
   golden catalog is untouched. The application registers `sonara.sampler` next to them only when
   a configured library holds at least one patch. The instrument is the enum parameter
   `instrument` (the choices are the installed patches' absolute paths), so `list_plugins` and
   `set_instrument` select it and the project stores it with the other parameters: no new tool,
   no schema change.
5. **Failure is soft.** A patch that is missing or unreadable, or an `instrument` that is not
   installed on this machine, renders as silence with a warning; it does not fail the render.
6. **Not built, and why:**
   - sfizz and FluidSynth: need an RT-10 change and, for FluidSynth, an LGPL dylib decision.
     **Owner decision pending.**
   - `rustysynth` as a General MIDI fallback: permissively licensed and plausible, but it adds a
     second engine and a second data model for no orchestral gain. Deferred until a GM fallback
     is wanted.
   - Plugin hosting (VST3/AU/CLAP) for commercial libraries is the only route to the top of the
     quality range. It is milestone M5 ([plugin-hosting.md](../plugin-hosting.md)), a separate
     decision (sandboxing, the VST3 SDK licence, an AU approach).

## Consequences

- A live graph rebuild recreates sampler devices, so sounding notes restart on an edit during
  playback (the loaded bank is cached; no file is read again).
- Supported SFZ: velocity and key ranges, `seq_*` round robin, `lorand`/`hirand`, keyswitches,
  `trigger=release`, `group`/`off_by`, `loop_mode`, `ampeg_attack`/`release`, `tune`, `volume`,
  `offset`/`end`, `#include`/`#define`/`default_path`. CC-driven layers and crossfades, filters,
  per-region pan and streaming are not supported; a patch that uses them plays without them and
  the loader lists the ignored opcodes. Sonatina's performance patches need CC 1 and CC 21, so
  they play only partly.
- Samples are decoded whole: a patch needs RAM for its samples (capped at 3 GiB). Streaming from
  disk is future work.
- The sampler has no prototype, so no golden data covers it. `sonara-dsp/tests/sampler.rs`,
  `sonara-media/tests/sfz.rs` and `sonara-engine/tests/sampler.rs` pin it with generated
  fixtures.
- Nothing here was listened to. Sound quality depends on the library and has not been judged.
