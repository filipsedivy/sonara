# Audio engine

This document covers the real-time path, offline rendering, threading, latency, timing
guarantees, the DSP design and the measured performance. The decision behind the architecture
is [ADR 0001](decisions/0001-audio-engine-architecture.md); the rules that real-time code
follows are in `.claude/rules/rust/realtime.md` (RT).

## Overview

```text
 sonara-app / sonara-engine                                     sonara-rt + sonara-io
 ──────────────────────────────────────────────────────────    ──────────────────────────────────
 ProjectSession ── snapshot ──► GraphBuilder ── hand-off ──► render thread ── push ──► playback ring ──► device callback ──► device
   (commands)      (sonara-player thread:       (RenderGraph,  keeps the ring at       (K blocks × 512,    (miniaudio)
                    rebuild, adopt state)        sonara_rt)    the lookahead           tag + generation)
                                                                take writer ◄── capture ring ◄──┘ (input, monitoring)
 OfflineRenderer ──► RenderGraph ──► export / previews (faster than real time, no device)
```

Offline rendering and real-time playback execute **the same** `RenderGraph`, so exports
sound exactly like playback (except for the metronome, which is added only during playback).

## Hard rules for the device callback

These rules hold for every real-time module (RT-01 to RT-12) and are enforced by lints,
`assert_no_alloc`, RTSan and loom.

1. The callback and the render loop perform no memory allocation or deallocation, take no
   locks, do no logging and no file or network I/O, and make no system calls beyond reading a
   monotonic clock.
2. They communicate only through preallocated single-producer/single-consumer rings and
   atomics (`sonara-rt`).
3. They never panic: no `unwrap`, no indexing, no unchecked arithmetic.
4. Graphs and snapshots are built and dropped off the real-time threads.
5. The software position (the timeline frame handed to the device) and the backend's latency
   estimate are reported as separate numbers. Sonara never presents the software position as
   the hardware position.
6. Underruns, capture overruns, device errors and the callback load are counted.

## Device engine (`sonara-io`)

**Device I/O.** The engine uses miniaudio 0.11.25 (vendored at `third_party/miniaudio`, public
domain / MIT-0) behind a narrow C shim (`crates/sonara-io/csrc`), so Rust never sees
miniaudio's structs. The backend is Core Audio. There is also a null backend, a simulated
real-time device used by the tests. Streams are float32, low-latency profile, playback-only or
duplex.

**Playback ring** (`sonara_rt::playback`). The ring holds K fixed-size blocks of 512 frames on
`rtrb`. Each block is tagged with the timeline frame of its first sample and a generation
number.

- The producer is the render thread. It pushes whole blocks.
- The consumer is the callback, which reads frame-accurately.
- `flush(generation)` marks older blocks as stale. The callback fades the current output out
  over 128 frames, discards the stale blocks, and fades new material in. Seek and stop are
  therefore click-free.
- An empty ring produces silence. It counts as an underrun only while the transport is
  playing.

**Capture ring.** The callback copies device input into a ring holding 4 s of interleaved
float frames, which the `sonara-takes` thread drains to disk. When capture starts, the engine
publishes the timeline frame at which it began.

**Direct monitoring.** Dry input is added to the output inside the callback with a gain.
This gives the lowest possible latency, but no effects.

**Meters.** Per-channel output and input peaks are kept as atomics. They are maxima since the
previous read.

**Stats.** `callback_frames`, `callbacks`, `underruns`, `overruns`, `queued_frames`,
`timeline_frame`, `latency_frames` (an estimate: period size × periods),
`input_latency_frames`, `callback_load` (callback time / period), and device errors,
interruptions and reroutes reported by miniaudio notifications.

**MIDI input.** midir calls the real-time `midi_callback` module on CoreMIDI's thread; it
pushes each message as a fixed-size event into the input's ring, which the render thread
drains.

## Player and transport (`sonara-engine`, `sonara-app`)

**`sonara_engine::player::Player`.** The `sonara-render` thread fills the ring up to
`lookahead_ms` ahead of the device (default 80 ms; 25 ms while live MIDI input is active).
Before its loop it joins the output device's audio workgroup (see "Measured performance").
The `sonara-player` thread rebuilds the `RenderGraph` whenever the session version changes and
hands it over; the new graph adopts the playing graph's state, matched by device ID, so filter
states, delay lines and voices survive edits during playback. The `sonara-prefetch` thread
reads audio clips ahead of the render thread.

- **Loops** are sample-accurate. The rendered block that crosses the loop end is cut at the
  exact frame, and rendering continues from the loop start. The remainder is carried over, so
  the graph still sees consecutive blocks.
- **The metronome** is added to playback only and never exported.
- **Recording** records armed audio tracks into WAV files (the project's record subtype),
  written by the `sonara-takes` thread, never from the callback. On stop, each take becomes an
  asset plus an audio clip in one undo step.

**`sonara_app::transport`.** Selects the backend:

- `auto`: the system's audio if it can be used, otherwise none.
- `native`: the system's audio, and an error if unavailable.
- `native-null`: miniaudio's simulated device.
- `null`: no real-time audio; offline rendering only.

Without a backend, real-time calls fail with `ENGINE_UNAVAILABLE`.

## Timing and latency

- **Control latency.** Parameter edits, mutes and similar changes reach the speakers after
  the queued lookahead (default 80 ms). Seek and stop take effect immediately with a 128-frame
  fade.
- **Output latency estimate.** The device period times the number of periods, as reported by
  miniaudio. On the development machine (Core Audio, MacBook Pro speakers, 48 kHz, 256-frame
  period) this is 16 ms. It is labelled an *estimate*: it excludes converter and driver safety
  offsets that the OS does not expose through miniaudio.
- **Position.** `timeline_frame` is the frame of the most recent sample handed to the device.
  The GUI playhead uses it. Recording alignment subtracts the output and input latency
  estimates.
- **Recording alignment.** Sample *i* of a take is placed at
  `capture_start + i - (output_latency + input_latency)`. The estimate can be off by the
  unreported driver offsets. A per-device manual offset is planned.

## Transport and event scheduling

The transport is `sonara_engine::player::Player`; the application's `TransportService`
(`sonara-app`) adds the project and the takes. The MIDI schedule of an instrument track is
`sonara_engine::schedule::NoteSchedule`. Neither depends on a concrete device: the player takes
an `AudioDevice`, and the tests drive a manual device and `Player::manual` step by step, with no
audio hardware and no sleeping.

- **Time base.** Everything is stored in beats. A note's frames are
  `TempoMap::beat_to_sample(beat, sample_rate)`, a closed form over cumulative seconds per tempo
  point (a ramp is logarithmic), so a long timeline accumulates no drift and a tempo change moves
  every later note. Frames are `i64`; the generation and frame counters wrap on purpose.
- **Per block.** The render thread asks the schedule for the events of `[frame, frame + n)`:
  note-offs, then note-ons, then controller events, each with its offset in the block. Events
  with the same offset keep that order. The buffer is sized at build, so no event is dropped and
  nothing allocates on the render thread.
- **Play** builds a fresh graph at the start frame; **pause** halts the stream at the device's
  software position and keeps it as the resting position; **resume** is a play from there;
  **seek** keeps the graph, moves the position and starts a new generation; **stop** halts the
  stream and keeps the device position as the resting position. Each call flushes the stream
  (128-frame fade) and drops stale blocks by generation, so nothing of an old position plays.
- **Active notes.** A block that does not continue the last one (start, seek, loop wrap) chases:
  an all-notes-off (a fast release), then a note-on for every note that is sounding at that
  frame. A seek past a note's end therefore releases its voice, and a seek into a note
  retriggers it. A stop leaves no voice: the next play builds a fresh graph.
- **Tests.** `crates/sonara-engine/tests/player/transport.rs` (position, restart, end of range,
  concurrency), `hanging_notes.rs` (a held voice after seek, resume and stop),
  `schedule.rs` (order, chase, tempo change) and `crates/sonara-app/tests/transport*.rs`.

## Offline renderer (`sonara_engine::renderer`)

- **`RenderGraph`** compiles a project snapshot into nodes in the topological order of the
  routing graph. Each node runs: source (audio clips or instrument) → inserts → pre-fader taps
  (pre-fader sends, sidechain keys) → fader and pan (sample-accurate automation) → post-fader
  sends → output.
- **Plugin delay compensation.** Each node knows its input latency (the maximum over incoming
  edges) and its insert latency. Early inputs to a bus are delayed to match. Sources render
  `latency` frames ahead, and the renderer discards the first `latency` output frames, so
  every path is sample-aligned at the master.
- **Mute and solo.** Without solo, unmuted nodes are audible. With solo, the audible nodes
  are the soloed and solo-safe nodes, everything that feeds them, and their downstream path to
  the master. Changes ramp over 256 samples.
- **Pan law.** Stereo balance: unity at centre, with a constant-power taper of the attenuated
  side. Mono sources are dual-mono before panning, so a centred mono source plays at 0 dB in
  each channel.
- **MIDI.** Note on/off and controller events are scheduled sample-accurately, with tempo
  ramps honoured. On a discontinuity (start, seek, loop), sounding notes are chased:
  all-notes-off, then the notes already sounding are retriggered.
- **Tails.** The `auto` tail renders until the output stays below −100 dBFS for 250 ms,
  bounded by the summed processor tails (at most 30 s).
- **Media.** Uncompressed media at the project rate is read directly with seeks. Other media
  is decoded and resampled once (libsoxr VHQ), streaming, into a float32 cache in the
  workspace (`cache/decoded`), read back with positional reads. Long media never needs to fit
  in RAM.

## DSP

All processing is float64 with fixed 512-frame blocks, in `sonara-dsp`. The per-sample
recursive kernels (attack/release smoother, TPT state-variable filter, gate state machine) and
every processor match the golden data recorded from the original prototype, most of them bit
for bit. Parameter changes glide (gain-type parameters per sample; filter frequencies
geometrically per block), so automation does not click. No fast-math is used (RT-11).

### Sample player (`sonara.sampler`)

Not part of the prototype and not covered by the golden data (ADR 0010). It runs 64 preallocated
voices with linear interpolation and linear attack/release ramps; the render thread allocates
nothing (checked with `assert_no_alloc` in `crates/sonara-dsp/tests/sampler.rs`). An SFZ patch is
parsed and its samples decoded on the builder thread (`sonara-media::sfz`), cached in the
`MediaProvider`, and handed to the processor through `DeviceState::sample_bank`. A graph rebuild
recreates sampler devices, so sounding notes restart. There are no measured load or memory
numbers for it yet (HW-01: they are measured locally).

## Measured performance

These are measurements, not guarantees, and never CI gates (HW-01). The requirements
(PERF-10): 0 underruns, render load at or below the baselines below, and at most 5 % for
Ascension.

**Current result:** with the render thread in the output device's audio workgroup, Ascension
renders at 2.5–2.8 % and Night Drive at 1.5–1.8 %, with 0 underruns and a 16 ms latency
estimate (details below).

**Baselines.** The original prototype, with its GUI running (cocoa platform, MacBook Pro
speakers, 48 kHz, 256-frame period, 80 ms lookahead, follow-playhead on, mixer meters visible,
master muted):

| Project | Underruns | Render load | Callback load |
|---|---|---|---|
| `examples/…/Night Drive.sonara` (electronic, 6 tracks, reverb + delay buses) | 0 | 14.5 % | < 0.1 % |
| `examples/…/Ascension.sonara` (orchestral, 7 s Cathedral reverb) | 0 | 24.7 % | < 0.1 % |

### The headless engine (2026-10-22)

Measured on the same machine and output as the baselines:

- **Machine:** Apple M5 Pro (6 Super and 12 Performance cores), 24 GB, macOS 27.0, on mains
  power. Another agent worked on the machine meanwhile: the load average (`uptime`) before
  the runs was 2.8–4.2, and 10.1 before the third Ascension run.
- **Output:** Core Audio, the MacBook Pro speakers (the default output), 48 kHz, a 256-frame
  period, 80 ms lookahead. The render thread ran at normal priority (for its
  `QoS` class and its audio workgroup since then, see "The render thread's `QoS` class" and "The
  render thread's audio workgroup" below).
- **Method:** the release build of `crates/sonara-app/examples/smoke_play.rs`, on a copy of each
  project:

  ```text
  cargo run --release -p sonara-app --example smoke_play -- --seconds 15 <project>
  ```

  It does what `--smoke-play` does without a GUI: the master at −144 dB through
  `SetTrackMixer`, play from beat 0 for 15 s, `status()`, `stop()`, the same keys and the same
  `render_load` (graph and carry time over the audio time rendered, cumulative). The baselines
  include the prototype's GUI, so they are an upper bound only.

| Project | Underruns | Render load (4 runs each) | Callback load | Latency estimate | Prototype with GUI |
|---|---|---|---|---|---|
| Night Drive | 0 in every run | 8.8 %, 8.6 %, 7.5 %, 8.3 % | ≤ 0.02 % | 16 ms | 0 underruns, 14.5 % |
| Ascension | 0 in every run | 14.8 %, 14.7 %, 14.3 %, 14.2 % | ≤ 0.02 % | 16 ms | 0 underruns, 24.7 % |

The latency estimate is `output_latency_estimate_ms`: 256 × 3 frames at 48 kHz. A fifth Ascension run gave
13.5 %, with 0 underruns.

Further runs, one each, all with 0 underruns unless stated:

- **25 ms lookahead** (the live-input target): Night Drive 8.7 %, Ascension 14.5 %; 0
  underruns.
- **Ascension without its reverb insert** (the 7 s hall on the Reverb bus, disabled in the
  copy): 3.8 %. The reverb's convolution is about 10 of Ascension's 14 points.
- **All 18 cores saturated** by 18 `yes` processes at normal priority: Night Drive 2.8 %,
  Ascension 4.7 %; 0 underruns. The render load is wall-clock time, and it fell to a third
  under full load. A likely cause, not verified: on a quiet machine the light, periodic render
  thread runs at a lower clock or on the slower core tier. The `QoS` runs below rule out the
  thread's own scheduling class: at the user-interactive class it renders no faster.
- **The process clamped to background QoS** (`taskpolicy -c background`): Ascension 1427
  underruns, render load 14.8 %, and the position reached only 7.4 s in 15 s. The scheduling
  class, not the CPU time, decides whether the render thread keeps up.

**Against the requirements:** the fixed part holds: 0 underruns, and render load below the
prototype's 14.5 % (Night Drive) and 24.7 % (Ascension) in every run; the latency estimate is
16 ms. The target of 5 % for Ascension was not met on a quiet machine: 13.5–14.8 % at first,
8.9–9.0 % after the convolution speed-up below, and 8.9–9.1 % with the render thread's `QoS`
class after that. With the render thread in the output device's audio workgroup and on a
time-constraint policy, Ascension renders at 2.5–2.8 % and meets it (5.9–9.2 % for the build
just before, alternated; see "The render thread's audio workgroup" below).

**The convolution speed-up** (2026-10-22, owner decision of the same day). The partitioned
convolver (`sonara_dsp::convolution`) keeps its arithmetic and changes its memory layout and
loop order: bins in tiles of 8 lanes with real and imaginary parts apart, a bin-major layout so
that each tile of each channel reads one long run of spectra and of the delay line with its sum
in registers, a conjugated delay line (no negation per term), and one shared delay-line row for
the reverb's mono input. Every output is bit-identical: the SHA-256 digests of the float64
renders of all six golden batches, of the 66 reverb and convolver plugin renders and of the six
convolution cases are unchanged, and a unit test compares the sum with the plain einsum loop bit
for bit.

- **Microbenchmark** (`cargo bench -p sonara-dsp --bench convolution`; the player's 512-frame
  block; before and after in one process, alternating; load average 3.8). Ascension's reverb
  (998 partitions): 280–284 µs → 122–124 µs a block with its caches warm, and 315–316 µs →
  188–191 µs with the caches evicted before every block, which is closer to playback, where
  the rest of the graph runs between two reverb blocks. The convolver's double-precision path
  with a 2 s response (188 partitions): 103 µs → 73 µs. The cost per partition with warm caches
  fell from 0.270 µs to 0.118 µs. A first attempt kept the partition-major layout and applied
  groups of 4 partitions per pass: 1.8× faster with warm caches, but no faster with cold ones
  (its many short streams defeat the prefetcher), and Ascension rendered at 12.5 %; hence the
  bin-major layout.
- **Render load** (the method above: release builds of `smoke_play` before and after, run
  alternately on copies of the projects, 15 s each; load average 3.0–3.6 for Ascension and
  4.7–5.7 for Night Drive; 0 underruns in every run):

  | Project | Before | After |
  |---|---|---|
  | Ascension | 14.3 %, 14.7 %, 14.9 %, 14.5 % | 8.9 %, 9.0 %, 9.0 %, 8.9 % |
  | Night Drive | 9.3 %, 9.1 %, 9.0 % | 6.7 %, 6.3 %, 6.6 % |
  | Ascension, all 18 cores saturated | 4.5 %, 4.6 % | 3.1 %, 3.1 % |

- **Offline render** (`cargo run --release -p sonara-agent --example bench`, medians of 3
  alternating runs, `render_seconds` before → after): 4 tracks 0.389 → 0.312 s, 8 tracks
  0.582 → 0.510 s, 16 tracks 0.972 → 0.905 s, 44.1 kHz 0.527 → 0.466 s, 96 kHz 1.450 →
  1.145 s; WAV + FLAC + MP3 export 0.95 → 0.87 s.
- **What remains of the 5 % target.** Without its reverb Ascension renders at 3.8 %, so the
  reverb would have to cost at most about 1.2 points on a quiet machine; it costs about 5. With
  the caches cold, a block streams 12.5 MB of spectra and delay line at about 66 GB/s, so exact
  arithmetic is now limited by memory, not by instructions. Further work changes either the
  arithmetic or the scheduling: non-uniform partitions (fewer, longer FFTs for the tail) cut
  the bytes and operations several times over but are not bit-identical, so they move the
  golden residuals; the render load with every core busy (3.1 %) meets the target, which points
  at the clock and core tier of a quiet machine. Both are owner decisions. The owner chose the
  scheduling first; it did not reach the target (below).

**The render thread's `QoS` class** (2026-10-22, owner decision). Before its loop, the `sonara-render` thread requests `QOS_CLASS_USER_INTERACTIVE`:
`sonara_io::thread::raise_render_qos` calls `pthread_set_qos_class_self_np` through the C shim
(RT-10), and the transport hands it to the player as the render thread's start hook
(`Player::with_render_start`) on a real device, not on the null backend. A refused request is
logged, and playback goes on.

- **What macOS grants.** The request succeeds everywhere, but macOS honours the class only in a
  process with an application role. The base priority of `sonara-render`, read with
  `proc_pidinfo` 6 s and 12 s into every run below:
  - **46**, user-interactive, when `smoke_play` runs as an app bundle that Launch Services opens
    (`open -g`, `LSUIElement`; role `TASK_DEFAULT_APPLICATION`), as the GUI app will run;
  - **31**, the default class, when it runs from a terminal, as in the method above. The process
    has no role (`TASK_UNSPECIFIED`), and the kernel squashes the user-interactive and
    user-initiated classes to the default class for it. A probe from the same terminal read 31
    for every class it requested; after it set its own role to `TASK_FOREGROUND_APPLICATION`,
    user-interactive read 47.

  So the terminal method cannot show the class, and the headless CLI and MCP server do not get
  it. Giving them a role (`task_policy_set` with `TASK_CATEGORY_POLICY`) would be a further
  scheduling decision.
- **Render load.** Release builds of `smoke_play` with the hook and from just before it,
  alternated, 15 s each, on copies of the projects, in both contexts. Every run had 0 underruns
  and no `last_error`, and reached 14.97–15.00 s. The quiet runs started only below a load
  average of 3.5 (2.9–3.2 where recorded); during the other runs, other agents' builds and
  containers held it at 6.0–9.4 (Ascension) and 4.1–6.8 (Night Drive).

  | Project, machine | App bundle, hook (46) | App bundle, before (31) | Terminal, hook (31) | Terminal, before (31) |
  |---|---|---|---|---|
  | Ascension, quiet | 8.9 %, 9.0 %, 8.9 %, 9.1 % | 8.9 %, 9.0 %, 8.9 %, 8.9 % | 7.8 %, 8.2 %, 8.2 %, 8.6 % | — |
  | Night Drive, quiet | 6.5 %, 6.6 % | 6.6 %, 6.6 % | — | — |
  | Ascension, load 6.0–9.4 | 5.8 %, 6.7 %, 8.5 % | 6.2 %, 6.5 %, 8.2 % | 5.9 %, 7.3 %, 7.8 % | 6.1 %, 7.9 %, 7.8 % |
  | Night Drive, load 4.1–6.8 | 6.0 %, 6.2 %, 6.4 % | 6.3 %, 6.1 %, 6.3 % | 6.1 %, 5.7 %, 6.1 % | 6.0 %, 5.8 %, 5.1 % |
  | Ascension, 18 `yes` | 3.8 % | 3.5 % | 3.4 % | — |

  The base priority matched its column at both readings of every run whose log was kept: all
  but the first 13 quiet runs, whose log was lost; the 3 quiet runs repeated to check read 46,
  31 and 31.
- **Result.** Where the class is in effect, it changes nothing measurable: with and without the
  hook, the runs of each condition agree within 0.2 points on a quiet machine. Ascension stays at
  8.9–9.1 % on a quiet machine (8.9–9.0 % before), and Night Drive at 6.5–6.6 % (6.3–6.7 %), so
  **the 5 % target is still not met**; Ascension meets it only while every
  core is busy (3.4–3.8 %, with or without the class). The fall under load is therefore not the
  render thread's class. The remaining suspect is the clock that a busy machine gives every core;
  that is not verified (it needs `powermetrics`, which needs root). The options left are the
  arithmetic (non-uniform partitions), a real-time thread policy (`audio_thread_priority`), or
  an application role for headless processes: all owner decisions. The owner chose the audio
  workgroup next (below).

**The render thread's audio workgroup** (2026-10-23, owner decision after the `QoS`
attempt). Before its loop, `sonara-render` joins the output device's audio workgroup, as Apple
advises for an auxiliary audio thread on Apple Silicon, so that the system schedules it as part of
the device's audio deadline. `sonara_io::thread::join_audio_workgroup` calls
`sonara_thread_join_audio_workgroup` in the C shim (RT-10), which:

1. resolves the output device: the saved `output_device` (its UID, through
   `kAudioHardwarePropertyTranslateUIDToDevice`) or the system's default output device;
2. reads the device's nominal sample rate and its workgroup
   (`kAudioDevicePropertyIOThreadOSWorkgroup`), and checks that the workgroup is not cancelled;
3. gives the thread a time-constraint policy (`THREAD_TIME_CONSTRAINT_POLICY`): a period of
   `buffer_frames` at the nominal rate (256 frames, 5.33 ms at 48 kHz), half of it as computation
   (the ratio of Apple's example in TN2169), the whole period as constraint, preemptible. The
   policy comes before the join, as Apple's developer support advises; macOS 27 also joined
   without it;
4. joins the workgroup (`os_workgroup_join`).

The membership is the hook's guard: `RenderThreadStart` now returns an optional guard, which the
render thread holds through its loop and drops after the last step, on itself; dropping it leaves
the workgroup (`os_workgroup_leave`). Where no workgroup can be joined (off macOS, without a valid
device id or period, no device, a cancelled workgroup, a refused policy), the hook falls back to
the `QoS` class above, and a failure is logged while playback goes on (ERR-06, RT-12). Every
check comes before the policy is set, because macOS refuses a `QoS` class (EPERM) to a thread that
has had a time-constraint policy, even after it returns to the standard policy (probed). No crate
was added: the shim calls Core Audio and libSystem. `audio_thread_priority` (MPL-2.0) sets a
time-constraint policy but joins no workgroup.

- **Why at the thread's start.** The render thread starts with the player, before any stream:
  the transport opens the device at the first play. The workgroup belongs to the device, not to a
  stream. A probe read the same workgroup object (the same Mach port) before miniaudio opened
  the device, while it ran, after it closed and after it reopened, and joined it at each point,
  so the thread joins it once, at its start. **Limitation:** a later reroute to another device
  (the default output changes while the stream follows it) leaves the thread in the first
  device's workgroup until the player restarts. Following it would need a stream-change signal
  from the player to the render thread's pacing loop.
- **Proof that it takes effect.** `proc_pidinfo` (`PROC_PIDTHREADINFO`) read `sonara-render` 6 s
  and 12 s into every run below. With the workgroup it reads policy 2 (the time-constraint
  policy; `thread_info` reports it as `POLICY_RR`) at priority 97, from a terminal and as an app
  bundle alike; before, policy 1 (timeshare) at 31 (terminal) or 46 (app bundle, the `QoS`
  class). The time-constraint policy needs no application role, so the CLI and the MCP server
  get it as well as the GUI. The unit tests read the policy back with `thread_policy_get`.
- **Render load.** Release builds of `smoke_play` from just before the change (the `QoS` hook)
  and with it, alternated, 15 s each, on copies of the projects, from a terminal and as an app
  bundle that Launch Services opens (`open -g`, `LSUIElement`), with the method of the headless
  section. Every run had 0 underruns and no `last_error`, and reached 14.98–15.00 s. Two
  orphaned processes (100 % CPU each for days) held the load average above 2.7 throughout;
  the first series ran while Spotlight added to them (4.1–5.2), and the second started below 3.5
  and ran at 2.7–4.3 (the load average at the start of each run).

  | Project, machine | Terminal, workgroup (97) | Terminal, before (31) | App bundle, workgroup (97) | App bundle, before (46) |
  |---|---|---|---|---|
  | Ascension, load 2.7–3.7 | 2.6 %, 2.6 %, 2.5 % | 7.7 %, 8.3 %, 7.7 % | 2.6 %, 2.5 %, 2.7 % | 8.6 %, 8.6 %, 8.1 % |
  | Night Drive, load 2.9–4.3 | 1.6 %, 1.5 %, 1.8 % | 5.7 %, 6.2 %, 1.9 % | 1.6 %, 1.5 %, 1.7 % | 6.2 %, 6.3 %, 6.0 % |
  | Ascension, load 4.1–5.1 | 2.7 %, 2.8 %, 2.7 % | 8.6 %, 6.6 %, 7.7 % | 2.6 %, 2.7 %, 2.8 % | 8.5 %, 9.2 %, 5.9 % |
  | Night Drive, load 4.1–5.2 | 1.8 %, 1.7 %, 1.7 % | 5.9 %, 5.9 %, 6.0 % | 1.7 %, 1.7 %, 1.8 % | 6.3 %, 6.3 %, 6.4 % |

  A 60 s Ascension run from a terminal (load 3.4) read policy 2 at priority 97 at 30 s and 55 s
  too, so the start of a play, which renders about 8 blocks back to back and so overruns the
  policy's computation, does not get the thread demoted: 0 underruns, 2.8 %, 60.0 s.
- **Result.** The render load falls to about a third, from a terminal and as an app bundle
  alike: Ascension renders at 2.5–2.8 % in all 12 runs with the workgroup against 5.9–9.2 %
  before, Night Drive at 1.5–1.8 % against 5.7–6.4 %, 0 underruns; the `QoS` step's quiet runs
  gave 8.9–9.1 % and 6.5–6.6 %. **The ≤ 5 % target is met.** One run before the change (Night Drive, terminal, base priority 31)
  read 1.9 %: a timeshare thread can render that fast at times. That fits the suspicion above
  that the render load follows the clock and core tier the thread gets, which the policy and
  the workgroup keep high, but it is not verified (`powermetrics` needs root). Not separated:
  how much of the gain is the time-constraint policy and how much the workgroup. Measured on
  macOS 27 only; the product minimum is macOS 26.

### Open hardware checks

Each needs hardware, a permission or audible output, and its numbers belong here. The
recordings of 2 and 4 have run only on the null device so far.

1. **Unplugging and rerouting a device.** With wired headphones or a USB interface as the
   default output, run the smoke tool for 60 s (inaudible, as above) and, during the run,
   switch the output in Control Center (a reroute), or unplug the device (a device loss):

   ```text
   cargo run --release -p sonara-app --example smoke_play -- --seconds 60 <copy of Night Drive>
   ```

   Record `underruns`, `device_status`, `last_error`, `position_seconds` and, from the stderr
   line, `device_errors`.
2. **Real-input take alignment.** Give the terminal the microphone permission and keep the
   speakers audible: a recording forces the metronome on, and its clicks, played by the
   speakers into the built-in microphone, are the test signal. Through `sonara mcp`:
   `create_project` at 120 BPM, `create_track` (audio, "Mic"), `set_track_state`
   (`armed: true`), `start_recording` at `start: 4`, about 6 s later `stop_playback`, then
   `play_preview` from 0 and `stop_playback`, a second `start_recording` at 4 (the same duplex
   stream after a play) and `stop_playback`, and `save_project`. For each take clip, sample `i`
   of its asset sits at timeline frame `start × 24000 − source_offset + i`; the clicks should
   land on multiples of 24000 frames, early or late by the same few milliseconds (the driver
   offsets that the latency estimate leaves out) in both takes.
3. **A MIDI keyboard.** With the keyboard plugged in, list midir's port names:

   ```text
   cargo test -p sonara-io --test midi_hardware port_names_match_the_prototype -- --ignored --nocapture
   ```

   The keyboard must appear under its name, with spaces trimmed from both ends as the settings
   store it. Then arm an instrument track in the app and play: the notes must sound, also while
   the transport is stopped.
4. **Capture on a real input device.** With the microphone permission, record 20 s from the
   default input under disk and CPU load (the forced clicks sound on the default output):

   ```text
   SONARA_CAPTURE_CONTEXT=default cargo test --release -p sonara-engine --test player take_writer_stall -- --ignored --nocapture
   ```

   It must report 0 overruns; record the longest gap and drain next to the null device's
   (11.4–28.4 ms and 22.1 ms), against the 4 s ring.

### The GUI's performance sign-off

The owner runs the GUI performance checks of ADR 0009 with one command. Its numbers belong
here, under "Results"; they are never CI gates (HW-01).

**Before you run it:**

- the Mac idle and on mains power, with nothing else running that you can quit;
- the screen unlocked, and no full-screen app on the built-in display (a full-screen Space
  hides the benchmark's window, and a window that is not drawn measures nothing);
- about 30 minutes, during which you leave the Mac alone. Nothing is audible: the playback
  plays copies of the projects with the master at −144 dB.

**The command**, from the repository root:

```text
cd apps/sonara-desktop && pnpm run perf:signoff
```

It builds the release desktop with the `bench` feature and the release `sonara` CLI, places the
window (1440 × 900) on the built-in display at 2×, and runs, each in a sandbox of its own (its
own `SONARA_HOME`; your settings and projects are not touched):

1. the timeline benchmark, 3 runs: draw p95 ≤ 7 ms and no frame over 16.6 ms while scrolling and
   zooming 100 tracks × 40 clips;
2. the meter benchmark, 3 runs: at least 58 fps, less than 1 % dropped frames and a p99 latency
   from the Rust sample to the pixel of at most 50 ms; with the fader drag and the knob drag in
   the Devices tab, once seen at 26 fps (both informational);
3. the piano-roll benchmark, 3 runs: drag, resize and lasso on 2,000 notes within 2 display
   frames, and 10,000 notes painted within one display frame;
4. Night Drive and then Ascension for 60 s each on the system's default output (`native`), with
   the timeline scrolling and zooming and the Mixer tab's meters running: 0 underruns, 0 device
   errors, and the transport rolling throughout (`--suite playback` of
   `scripts/bench-timeline.mjs`, the page's `src/lib/bench/playback.ts`).

It keeps the Mac awake (`caffeinate`), brings the window to the front, waits up to 15 minutes for
a locked screen to be unlocked or a full-screen app to leave the built-in display, repeats a run
whose window was hidden or stalled (up to four attempts), and gives every part a hard timeout.
At the end it writes `summary.md` with every number against its threshold and an overall
verdict, under `target/perf-signoff/<date>/` (each part's reports beside it), and prints the
command that reruns what did not pass, for example:

```text
cd apps/sonara-desktop && pnpm run perf:signoff -- --skip-build --only meters,playback-ascension
```

`-- --quick` runs each part once and briefly, to check the setup in a few minutes; its numbers
do not count. `-- --display main` measures on the main display instead (a check only).

**Results:** not measured yet.

Benchmarks: `cargo run --release -p sonara-agent --example bench` end to end (offline render
and export), `cargo bench` with `criterion` in the crates, and the GUI benchmarks above.

## Known limitations

- Effects on live input are not low-latency: monitoring through effects incurs the lookahead.
  Direct monitoring is dry.
- The output latency estimate excludes driver safety offsets.
- A reroute to another output device leaves the render thread in the first device's audio
  workgroup until the player restarts.
- Tracks cannot route to external MIDI outputs yet.
