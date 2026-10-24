# Real-time rules (RT)

All RT rules are MUST rules. They implement the hard callback rules in `docs/audio-engine.md`. They
take precedence over the general rules (`quality-gate.md`).

**Real-time code** is code that can run on a real-time thread: the miniaudio device callback, the
midir input callback, the graph executor's processing path and the player loop. Lints do not
propagate through calls, so the real-time modules are named explicitly:

| Crate | Real-time modules |
|---|---|
| `sonara-dsp` | The whole crate. Kernels and processors run on the render thread, so they are written that way from the start. |
| `sonara-rt` | The whole crate. |
| `sonara-io` | The device callback module and the midir input callback module. The FFI module is an RT module too, because it holds the `extern "C"` callback trampoline. That trampoline turns the raw pointers into safe references and calls the callback module. |
| `sonara-engine` | The processing path and the player loop. |

Four rules apply to these modules:

- **An RT module calls only other RT modules.** It may also call `core` and the parts of `std`
  or of other crates that are verified not to allocate, lock or block. Verification means
  documentation or `assert_no_alloc`/RTSan tests.
- **Setup code may allocate.** Constructors, `start`, `prepare` and buffer sizing run off the
  real-time thread and are documented as such.
  - RT-02 and RT-03 apply to everything reachable from a real-time thread.
  - Such a setup function may carry
    `#[expect(clippy::disallowed_methods, clippy::disallowed_macros, reason = "setup: runs before the stream starts, off the real-time thread")]`.
  - It is never called from a real-time path.
- **Placement.** Code that runs on a real-time thread lives only in real-time modules.
- **Header.** Each real-time module starts with a module doc comment that says so, followed by
  this lint block:

```rust
//! Real-time: runs on the audio device thread (see docs/audio-engine.md).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]
```

## Rules

- **RT-01 Nothing but audio on the audio path.** Real-time code lives in crates that never
  depend on application, agent, GUI or scripting crates: `sonara-dsp`, `sonara-rt`, `sonara-io`
  and `sonara-engine`. Never add such a dependency, not even an optional one. The compiler then
  guarantees that the callback cannot reach GUI, agent or I/O-heavy code.
- **RT-02 No allocation and no deallocation.** Real-time code allocates no heap memory and
  frees none.
  - This rules out, for example: growing a `Vec`, `String`, `format!`, `Box::new`, `collect()`,
    and dropping the last owner of heap data.
  - Every buffer is preallocated before the stream starts (`start`, `prepare`).
  - Enforced by `assert_no_alloc` as the global allocator in debug and test builds, and by
    RTSan (`rtsan-standalone`, `#[nonblocking]`) in CI.
- **RT-03 No blocking.** Real-time code does none of the following:
  - takes locks: `Mutex`, `RwLock`, `Condvar`, lazy or `Once` initialization, `parking_lot`;
  - logs or prints: `println!`, `eprintln!`, `dbg!`, `log` or `tracing` macros;
  - does file or network I/O;
  - spawns, joins or sleeps threads;
  - makes any system call other than reading a monotonic clock.

  Enforced by RTSan and by the `disallowed-methods` and `disallowed-macros` lists in
  `clippy.toml`.
- **RT-04 Only SPSC rings and atomics.** The only channels between real-time and other threads
  are the preallocated single-producer/single-consumer rings and the atomics in `sonara-rt`.
  - Each atomic access uses a deliberately chosen memory ordering, with a comment naming its
    pair, for example `Release` publish ↔ `Acquire` consume.
  - Every ring protocol has `loom` tests over all producer/consumer interleavings.
- **RT-05 No panics.** A panic cannot unwind out of the `extern "C"` callback, so it aborts the
  process (Rust ≥ 1.81).
  - Real-time modules carry the lint block above. The panic lints in it (`unwrap_used`,
    `expect_used`, `panic`, `indexing_slicing`, `unreachable`, `todo`, `unimplemented`) have no
    `#[allow]` or `#[expect]` exceptions. Only setup functions may relax the two disallowed
    lists, as described above.
  - No `assert!` or `debug_assert!`. Invariants are established before the stream starts.
  - Integer arithmetic that can overflow uses explicit `wrapping_*`, `saturating_*` or
    `checked_*` operations, because debug builds panic on overflow. Generation and frame
    counters wrap on purpose.
  - Slices are accessed through iterators, `chunks_exact`, `zip` or `get()`, not by indexing.
  - **Calls the lints do not catch can also panic:**
    - `copy_from_slice` and `clone_from_slice` with different lengths;
    - `split_at` past the end;
    - `chunks`, `chunks_exact` and `windows` with a size of 0;
    - integer `/` or `%` by zero.

    Prove the lengths and divisors before the stream starts, or use `get()` and the checked
    forms.
- **RT-06 Control calls never wait on the audio thread.** Calls from the GUI, agent or
  transport into the engine never block on the real-time thread. Examples are start, stop, flush
  and parameter changes. They post to rings or atomics and return. Anything that must wait, such
  as device start or stop, runs on a non-real-time thread. Each such call has a test that calls
  it concurrently.
- **RT-07 Position is not latency.** These stay separate numbers:
  - the software position: `timeline_frame`, the last frame handed to the device;
  - the backend's latency estimates: `latency_frames` (period × periods) and
    `input_latency_frames`.

  Never present the software position as the hardware position.
- **RT-08 Count everything.** These are counted with the semantics of the recorded counters
  (`docs/audio-engine.md`, "Stats"):
  - underruns (an empty ring counts only while playing);
  - capture overruns;
  - device errors, stops caused by the device, and reroutes;
  - callback load.

  Peaks reset on each read.
- **RT-09 Drop off the real-time thread.**
  - Graphs and snapshots are built off the real-time thread.
  - They reach the real-time thread through a lock-free hand-off and leave it through a return
    queue.
  - They are dropped on a non-real-time thread.
- **RT-10 `unsafe` is confined.** `unsafe` exists only in three modules:
  - the ring module of `sonara-rt`;
  - the FFI module of `sonara-io`;
  - the codec FFI module of `sonara-media`, which loads the codec libraries as replaceable
    dynamic libraries at run time: the LGPL `libmp3lame` and `libsoxr` (LIC-01; owner decision
    of 2026-10-21) and the BSD-licensed `libFLAC` (owner decision of 2026-10-22). It never runs
    on a real-time thread.

  Every other crate and module forbids it.
  - Every `unsafe` block has a `// SAFETY:` comment that states the invariants it relies on and
    why they hold.
  - Every `unsafe fn` and unsafe trait has a `# Safety` doc section.
  - The safe wrapper around ring and FFI code upholds those invariants, so that callers cannot
    break them.
  - The bindgen output is generated, committed, and never edited by hand.
  - Allowing `unsafe` anywhere else means changing this rule first, with the owner's approval.
- **RT-11 Numerics match the oracle.**
  - No fast-math, neither in Rust nor in the flags used to build the vendored C.
  - Denormal handling matches the current C++ behaviour. Flush-to-zero or denormals-are-zero is
    a separate, tested decision.
  - Keep the reference's order of operations wherever reordering would move results beyond the
    documented tolerance.
- **RT-12 Report, do not handle.** The callback does only the minimum: copy, fade, sum the direct
  monitor, meter and count.
  - Errors and unusual events become counters and flags.
  - A non-real-time thread reads those and turns them into errors, logs or user messages.

## Enforcement notes

These enforcement decisions are implemented in the workspace configuration.

- **`forbid` cannot be relaxed by an inner `allow` or `expect`,** so "forbid by default, relaxed
  per module" cannot work as written. The mechanism these rules prescribe (`quality-gate.md`) is:
  1. `unsafe_code = "deny"` in `[workspace.lints]`;
  2. `#![forbid(unsafe_code)]` in every crate root without an RT-10 module;
  3. `#[expect(unsafe_code, reason = "RT-10: …")]` on the three RT-10 `mod` declarations only.

- **`clippy.toml` applies to whole crates, not to single modules.** To make the disallowed lists
  apply where RT-03 needs them, one option is:
  1. set `clippy::disallowed_methods` and `clippy::disallowed_macros` to `allow` in
     `[workspace.lints]`;
  2. deny them in the real-time lint block above.
- **RTSan runs on macOS and Linux only.** Gate the `rtsan` feature so that Windows builds and
  `cargo test --workspace` never enable it.
