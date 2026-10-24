# Plugin hosting

## Current state (honest summary)

| Capability | Status |
|---|---|
| Built-in instruments and effects through the common plugin contract | **done** (12 plugins) |
| Parameter schemas, presets, state serialization, latency reporting, automation | **done** for built-ins |
| VST3 hosting | **not implemented** |
| Audio Unit hosting (macOS) | **not implemented** |
| CLAP hosting | **not implemented** |
| Plugin scanning, discovery, sandboxing | **not implemented** |

Sonara does not list, scan or claim support for third-party plugins. The agent tool
`list_plugins` and `get_capabilities` both report that third-party hosting is unsupported.

## The plugin contract

Every plugin, built-in or hosted in future, is described by a `PluginDescriptor`
(`sonara/plugins/descriptor.py`):

- **Identity:** `id` (for example `sonara.dynamics.compressor`), name, vendor, version,
  `format` (`builtin`, `vst3`, `au` or `clap`), kind (`instrument` or `effect`), category,
  tags.
- **Capabilities:** `sidechain` input. Audio and MIDI I/O are implied by the kind:
  instruments take events and produce stereo audio; effects take stereo audio.
- **Parameters:** `ParamSpec` (`id`, name, `float`/`int`/`bool`/`enum`, default, range,
  unit, linear or log scale, `automatable`). These validate every value an edit or the
  agent submits.
- **Presets:** named parameter sets, validated against the schema at registration.
- **A factory** that creates a `Processor` (`sonara/dsp/base.py`). The processor's
  lifecycle:
  - `prepare(spec)`: sample rate and block size changes;
  - `set_params(full_set)`: smoothed internally;
  - `set_state(state)`: non-parameter state, such as an impulse response;
  - `process(block, ctx)`: fixed-size float64 `(frames, 2)` blocks, with MIDI events and a
    sidechain buffer in `ctx`;
  - `reset()`;
  - `latency`, which the engine compensates;
  - `tail_seconds`;
  - `report()`: live readouts such as gain reduction.

Projects store only `plugin_id`, parameter **values** and `state`. On load, values are
overlaid on the descriptor's defaults. An unknown `plugin_id` is kept in the project, and
the device is bypassed with a warning ("plugin unavailable"), so opening a project on a
machine that lacks a plugin never destroys its settings.

**Failure isolation.** A processor that produces NaN or infinity is sanitized at the node
boundary (`ensure_finite`). Exceptions inside the render thread stop playback and are
reported in transport status, instead of crashing the application.

## Plan for third-party hosting

The licensing landscape was checked on 2026-10-20:

- The **VST3 SDK** has been MIT-licensed since version 3.8 (October 2025), so a host can be
  written without copyleft obligations.
- **Audio Units** use Apple's AudioToolbox and AVFoundation system frameworks
  (`AVAudioUnit`, `AUAudioUnit`).
- **CLAP** is MIT-licensed.
- JUCE 8 (AGPLv3 or commercial) and Pedalboard (GPLv3) can host plugins, but they would
  impose copyleft. See [ADR 0001](decisions/0001-audio-engine-architecture.md).

Proposed design (not implemented):

1. **Out-of-process host** (`sonara-plughost`, C++). Each plugin instance, or each plugin
   bundle, runs in a helper process. A plugin crash or hang then kills only the helper; the
   track is bypassed and the error reported.
2. **Transport.** Shared-memory audio buffers plus a lock-free command queue per instance,
   with block-synchronous processing driven by the render thread. The added latency is one
   block, reported through `latency` and compensated by PDC.
3. **Scanning** runs in a separate process with a timeout and writes a scan cache (path,
   hash, identity, parameters) under the config directory. Plugins that crash during scanning
   are blocklisted.
4. **State** is saved as the plugin's opaque chunk (base64 in `device.state`) plus the
   parameter values Sonara knows about.
5. **GUI embedding** shows plugin editors in helper-owned native windows. The macOS AU
   `requestViewController` path and VST3 `IPlugView` are both possible. This is the hardest
   cross-platform part and is scheduled after audio processing is proven.
6. **Compatibility testing** runs a matrix of free reference plugins (for example
   Steinberg's sample plugins and Apple's AU samples) on each platform in CI. Support claims
   only for what that matrix verifies.
