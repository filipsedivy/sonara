# Rust testing and performance rules

These are SHOULD rules unless stated otherwise (see `quality-gate.md`). QG-09 and QG-11 make
testing and measurement mandatory; these rules say how.

## TST: testing and verifiability

- **TST-01 Unit tests for domain logic.** Business rules get fast tests. Here those are kernels,
  ring arithmetic, the time base and processors.
- **TST-02 Integration tests.** Verify that components work together and that external
  boundaries hold. Examples are `crates/*/tests/`, the null audio backend, and virtual MIDI
  ports.
- **TST-03 Test the failure paths.** Cover unsuccessful paths, not only successful ones. Every
  error code that an agent tool or the CLI can return has a test.
- **TST-04 Deterministic tests.** A result must not depend on random order, the current time or
  the network unless the test controls them.
  - Use fixed seeds.
  - Engine tests use the null backend.
  - Proptest regressions stay in the tree.
- **TST-05 Test the public API.** Verify the contracts that other parts of the system rely on.
  For Sonara those are the contracts of CTR-01, checked against golden data: project files, tool
  schemas and transcripts, IPC and the CLI.
- **TST-06 Regression tests.** Cover every significant fixed defect with a test.
- **TST-07 Test invariants.** Check that invalid states cannot arise or are safely rejected.
  - Example: after `flush(generation)`, no block of an older generation is ever played.
  - Example: an empty ring counts as an underrun only while playing.
- **TST-08 Isolate external dependencies.** Isolate devices, MIDI backends, the file system and
  system services wherever that makes tests more reliable.
- **TST-09 Property-based tests where they pay off.** Use them for parsing, serialization,
  numerics and complex algorithms. Use `proptest` for kernels:
  - processing a signal in one call equals processing it split across several calls (state
    continuity);
  - output stays stable at extreme cutoffs.
- **TST-10 No fake tests.** A test checks the actual result against an oracle: a reference
  implementation, a known value or an invariant. "It ran and did not panic" is not a test.

`cargo test` is the base. A high coverage percentage alone does not guarantee correctness.

**Sonara test layers.** Each one is used for its purpose:

| Layer | Tooling |
|---|---|
| Kernel and ring invariants | `cargo test`, `proptest` |
| Ring protocols under all interleavings | `loom` |
| No allocation, locks or blocking on audio paths | `assert_no_alloc`, RTSan |
| Golden parity | `sonara-testkit` with `golden/` (GLD-01) |
| Numerical equivalence | golden vectors and renders; null tests at −120 dBFS |
| Agent contract | golden tool schemas and transcripts (CTR-01) |
| Hardware | local runs only (HW-01) |
| Performance | `criterion` |

**Audio tests:**

- **Signals.** Use reference signals: impulses, sines, sweeps, noise, silence and clipped
  signals.
- **Tolerances.** Use tolerances suited to floating point.
- **Kernels.** Compare kernels with their golden vectors within the existing test tolerances.

## PERF: performance and memory

- **PERF-01** Measure first, then optimize.
- **PERF-02** Prefer iterators and common Rust idioms where they improve readability. In
  real-time code, prefer `chunks_exact` and `zip` to indexing (RT-05).
- **PERF-03** Avoid needless allocation of strings and collections. Real-time code allocates
  nothing at all (RT-02).
- **PERF-04** When the size of a collection is known, consider `with_capacity()`. Real-time
  buffers are always preallocated before the stream starts.
- **PERF-05** Judge copies of large structures and `clone()` by their real cost.
- **PERF-06** Never introduce `unsafe` only for an expected speed-up (RT-10 still applies).
- **PERF-07** In concurrent code, do not block executors and do not hold locks across `await`
  or across a long operation. Real-time threads never block or wait (RT-03).
- **PERF-08** Use benchmarks and profiling for compute-heavy parts: `criterion` in the crates,
  and `cargo run --release -p sonara-agent --example bench` end to end.
- **PERF-09** Distinguish CPU-bound from I/O-bound work. Rendering is CPU-bound. Device, MIDI
  and file work is I/O-bound and stays off the render path.
- **PERF-10** Optimize within measurable requirements for latency, throughput and memory
  (`docs/audio-engine.md`):
  - **Fixed:** 0 underruns, with render load at or below the measured baselines (Night Drive and
    Ascension).
  - **Targets:**
    - render load of 5 % or less on Ascension;
    - the GUI numbers of ADR 0009: timeline p95 ≤ 7 ms, meters ≥ 58 fps with p99 ≤ 50 ms, the
      piano roll within two display frames.
