# ADR 0001: Audio engine architecture

- **Status:** Accepted (2026-10-20)
- **Deciders:** Sonara core
- **Supersedes:** none
- **Update (2026-10-24):** the decision holds in the Rust application: a native I/O core with
  lock-free rings and an anticipative block renderer. The render thread, the graph and the DSP
  are Rust now (`sonara-engine`, `sonara-dsp`), so the Python and GIL constraints below no
  longer apply. See [audio-engine.md](../audio-engine.md).

## Context

Sonara is a Python application (PySide6 GUI, Python application services, agent interface).
A DAW needs a real-time audio path: an operating-system audio callback that must deliver
buffers on time, every time, typically every 1–20 ms. The following constraints apply:

- CPython code holds the GIL. Any Python executed in the device callback can be delayed by
  garbage collection, by the GUI thread painting, by import locks, or by any C extension
  that holds the GIL. The callback deadline is hard; missing it produces an audible dropout.
- Offline rendering (export, agent previews) must be deterministic and must produce the same
  mix as live playback.
- Plugin hosting (VST3/AU/CLAP) is a later milestone but must not be ruled out.
- Target platforms: macOS, Windows, Linux. Development machine: macOS on Apple Silicon.
- Licensing must keep the project's licence open (the Sonara licence is not yet final; see
  `LICENSE`). Copyleft-only dependencies would decide that question by accident.

## Options considered

### A. Python callback through `sounddevice` (PortAudio)

The device callback is a Python function that pulls blocks from the Python mixer.

- Performance: worst. The callback acquires the GIL every period. GC pauses, GUI paints and
  long-running Python bytecode in other threads directly cause dropouts. Achievable buffer
  sizes are large and unreliable.
- Licensing: MIT (sounddevice, PortAudio). Platform support: excellent.
- Maintenance and integration: trivial.
- Plugin support: none by itself.
- **Rejected as the primary engine.** It is the baseline the PRD explicitly warns against.

### B. Native I/O core with lock-free rings, block renderer in Python ("anticipative" engine)

A small C++ extension (nanobind) owns the device through **miniaudio**. The device callback
runs only native code: it copies pre-rendered audio from a preallocated single-producer /
single-consumer (SPSC) ring buffer, applies click-free gain ramps on flush, computes meters,
and copies input samples into a capture ring. A Python render thread produces audio ahead of
the playhead (configurable lookahead, default ~100 ms) using the same block-based mixer that
offline rendering uses. Hot per-sample recursive DSP kernels (envelope followers, modulated
filters) are implemented natively and exposed to the Python renderer.

- Performance: the device deadline is met by native code alone, so callback-level dropouts
  only happen if the ring runs dry. Python only needs to keep up on average over the
  lookahead window instead of every period. Cost: control latency (parameter changes, seek,
  stop) equals the lookahead; live input processed through Python effects cannot be
  low-latency. Direct (dry) input monitoring is done natively and is low-latency.
- Licensing: miniaudio is public domain / MIT-0; nanobind is BSD-3. No copyleft.
- Platform support: miniaudio supports Core Audio, WASAPI, DirectSound, ALSA, PulseAudio,
  JACK, and a **null backend** that lets CI exercise the callback path without hardware.
- Maintenance: a few thousand lines of C++ owned by the project; one vendored header.
- Integration complexity: moderate. A narrow, versioned `ABI_VERSION`ed interface.
- Plugin support: plugin hosting would live in the native layer or in a separate process;
  this option does not prevent either.

### C. Fully native graph engine (all mixing and DSP in C++)

Same native I/O as B, but the whole render graph (clips, instruments, effects, automation)
runs in C++ inside or next to the callback. Python only edits a graph description.

- Performance: best; enables low-latency monitoring through effects and small control
  latency.
- Licensing: same as B if written in-house.
- Maintenance: high. Every DSP feature must be written in C++, and the Python offline
  renderer must either be removed or kept bit-compatible with the native one.
- Integration complexity: high (graph serialization, parameter queues, state transfer).
- This is the long-term direction for hot paths, not the initial milestone.

### D. JUCE or Tracktion Engine

A mature C++ framework with devices, a graph, and VST3/AU hosting.

- Performance: excellent; proven in commercial products.
- Licensing: JUCE 8 is dual-licensed **AGPLv3 / commercial**; Tracktion Engine is GPLv3 /
  commercial and depends on JUCE. Adopting either forces Sonara to AGPL/GPL or to paid
  licences, and the JUCE 8 EULA adds restrictions. This decides the project's licence by
  accident and is therefore not acceptable without an explicit licensing decision.
- Maintenance: large C++ dependency, CMake/Projucer toolchain, Python bindings to write.
- Plugin support: best of all options.
- **Rejected for now on licensing grounds.** Revisit if the project chooses AGPL/GPL.

### E. PortAudio blocking API (via `sounddevice.Stream.write`)

PortAudio's blocking-write mode keeps a ring inside PortAudio, so the device thread does not
call Python. This is a no-compile variant of option B.

- Performance: similar to B for playback, but without native meters, timeline tagging of
  queued audio, click-free flush, or capture timestamps.
- Licensing: MIT. Kept in mind as a fallback backend for platforms where the native module
  is unavailable.

## Decision

Adopt **Option B** for the initial engine, with a deliberate migration path toward **C**
for hot paths:

1. `sonara_native` (C++17, nanobind, miniaudio) owns device enumeration, the device
   callback, the playback ring, the capture ring, meters, and per-sample DSP kernels.
2. Python owns the project model, graph compilation and the block renderer, shared by
   real-time playback and offline export, so both paths produce the same mix.
3. The Python interface to the native layer is narrow and versioned (`ABI_VERSION`).
4. If the native module is unavailable, Sonara still renders, analyzes and exports offline;
   real-time playback reports itself as unavailable instead of silently degrading.

## Hard rules for the device callback

These rules are also listed in `docs/audio-engine.md` and are reviewed for every change to
`native/src`:

- No Python, no GIL acquisition, no memory allocation, no locks, no logging, no I/O.
- Communication uses only preallocated SPSC rings and atomics.
- Every nanobind entry point that can block or loop releases the GIL.
- The callback-consumed position (software position) and the backend's latency estimate are
  reported as **separate** numbers. Sonara never presents the software position as the
  hardware position.
- Underrun counters exist from day one, so performance is measured rather than asserted.

## Consequences

- Control latency is bounded by the lookahead. It is reported, and it is configurable.
- Monitoring of live input through Python effects is not low-latency in this design. Dry
  direct monitoring is native and low-latency.
- The GUI thread and the Python render thread share the GIL. Heavy painting can starve the
  renderer. Mitigations: incremental and virtualized painting, cached waveforms, and
  lookahead sizing. **Escalation path:** move the renderer into a separate process that
  writes the ring through shared memory, or move graph execution native (option C).
- Plugin hosting will be implemented native-side or out-of-process (see
  `docs/plugin-hosting.md`).
