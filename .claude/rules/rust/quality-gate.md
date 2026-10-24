# Rust quality gate and rule system

These rules apply to all Rust code in this repository and to the Cargo, lint and CI
configuration around it. The workspace is at the repository root (`crates/`, `apps/`). The
frontend of the Tauri GUI follows its own toolchain (ADR 0009), but the licence and dependency
rules here apply to it too. Every plan, change and
review must follow them. Sources: the [Rust Style Guide](https://doc.rust-lang.org/style-guide/),
the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and the
[Clippy documentation](https://doc.rust-lang.org/clippy/).

## How the rules work

- **MUST / MUST NOT** rules are mandatory. All QG, RT and project rules (CTR, GLD, LIC, HW, VER,
  DOC in `.claude/rules/project.md`) are MUST rules.
  - A MUST rule is never waived inside the code.
  - Changing a MUST rule means editing its rule file first, with the owner's approval.
- **SHOULD** rules are the default. All PRI, ARC, DES, MEM, TYP, ERR, NAM, MOD, TRT, TST and PERF
  rules are SHOULD rules, unless this file or `realtime.md` makes one stricter.
  - A deviation states its reason and the rule ID.
  - Put the reason in a code comment, a lint `reason = "…"`, or the plan or PR description.
- **Precedence**, highest first:
  1. the ADRs in `docs/decisions/` and the golden data in `golden/`;
  2. the project rules (`.claude/rules/project.md`);
  3. RT (`.claude/rules/rust/realtime.md`);
  4. QG and the general rules (`design.md`, `correctness.md`, `testing-performance.md`).
  Where two rules overlap, the stricter one applies.
- **Cite rule IDs.** Plans, reviews and deviation comments name the rule they apply, for example
  `// ERR-01: block_frames was validated in start()`.

## QG: mandatory quality gate

| ID | Rule | Enforced by |
|---|---|---|
| QG-01 | All code is formatted by `cargo fmt`. | `cargo fmt --all -- --check` |
| QG-02 | Clippy reports no unresolved warnings in checked code. | clippy with `-D warnings` and the workspace lints below |
| QG-03 | Production code calls `unwrap()` or `expect()` only with a documented reason. Real-time code never calls them (RT-05). | `clippy::unwrap_used` denied; an `expect` message states why the call cannot fail. Tests are exempt (`allow-unwrap-in-tests`, `allow-expect-in-tests` in `clippy.toml`). |
| QG-04 | `unsafe` is forbidden unless it is necessary and justified by an expert argument. It is confined to the modules listed in RT-10. | `unsafe_code` lint; `clippy::undocumented_unsafe_blocks` denied |
| QG-05 | Every public API documents its purpose and its behaviour on errors. Panics and safety preconditions are documented where relevant. | `missing_docs`; `clippy::missing_errors_doc`, `missing_panics_doc`, `missing_safety_doc`; `RUSTDOCFLAGS="-D warnings" cargo doc` |
| QG-06 | Domain logic is separated from infrastructure details. | the crate boundaries (ARC, RT-01); review |
| QG-07 | Errors are represented explicitly and propagated correctly. | `unused_must_use` (an error under `-D warnings`); review |
| QG-08 | Types exclude invalid domain states wherever that is practical. | review; invariant tests (TST-07) |
| QG-09 | Every significant piece of logic has automated tests. | `cargo test`, including the golden parity tests (GLD-01) |
| QG-10 | A new dependency has a justified benefit, an allowed licence and an acceptable security risk. | `cargo deny check`; an entry in `docs/dependencies.md` (see below) |
| QG-11 | Every optimization is backed by a measurement. | criterion or `cargo run --release -p sonara-agent --example bench` numbers in the change description |
| QG-12 | Every change passes CI and does not break existing public API without need. | CI; the contracts of CTR-01 through golden data |
| QG-13 | **Library first.** When an established, maintained crate with an allowed licence solves the problem, use it instead of writing our own code (owner decision of 2026-10-21). Writing our own code needs a stated reason: no suitable crate exists; its licence is not allowed; it cannot meet the real-time rules (RT-02, RT-03); it cannot reach parity with the golden data (GLD-01); or it is unmaintained. | the change description names the crates considered; review |

The lints in the right-hand column (`clippy::unwrap_used`,
`missing_docs`, the `*_doc` lints) are set in `[workspace.lints]`.

## Commands

The gates run natively on the macOS host (owner decision, 2026-10-24: no Docker, no development
container), through rustup (`/opt/homebrew/opt/rustup/bin`), which the desktop crate and the
hardware tests need. Before you call a Rust change done, run:

```bash
scripts/dev.sh rust          # all Rust gates: rust-check, rust-deny, rust-loom, rust-rtsan
scripts/dev.sh frontend      # the desktop frontend: licences, svelte-check, build, tests
```

`scripts/dev.sh rust-check` is the fast loop. It runs exactly what the macOS CI job runs, from
the workspace root:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
```

The other tasks run these commands:

- `rust-deny` runs `cargo deny --locked check`.
- `rust-loom` runs `RUSTFLAGS="--cfg loom" cargo test --locked -p sonara-rt --release`.
- `rust-rtsan` runs `cargo test` with the `rtsan` features and `RTSAN_ENABLE=1`, on macOS and
  Linux only.

`--locked` makes a stale `Cargo.lock` fail instead of being rewritten.

Two more rules:

- **Never pass `--all-features`.** It turns on `rtsan`, which breaks Windows. RTSan has its own
  CI job.
- **Report every gate.** If a gate could not run (for example, no toolchain or no audio
  hardware), say so explicitly and do not report the change as verified.

## Toolchain and lint policy

- **Edition and toolchain:**
  - edition 2024;
  - `rust-toolchain.toml` pins one exact stable version;
  - `rust-version` in the workspace `Cargo.toml` equals that version, because the workspace is
    an application component and makes no MSRV promise to anyone else.
- **Lints** live in `[workspace.lints]`, and every crate inherits them with
  `[lints] workspace = true`.
  - The baseline is: `clippy::pedantic` as warnings, and `clippy::undocumented_unsafe_blocks`
    denied.
  - `unsafe_code` is denied workspace-wide. Every crate root without an RT-10 module adds
    `#![forbid(unsafe_code)]`. The details are in `realtime.md`.
  - CI's `-D warnings` turns every warning into an error.
- **Exceptions to a lint** go in one of two places:
  - `[workspace.lints]`, with a comment that says why;
  - a single item, with `#[expect(lint, reason = "…")]`. Use `#[expect]`, not `#[allow]`, so
    that a stale exception gets reported.
  - Never relax a lint with `allow` or `expect` on a whole module or crate. The only exception
    is the three RT-10 modules, which carry
    `#[expect(unsafe_code, reason = "RT-10: …")]` on their `mod` declaration.
  - Setup functions in real-time modules may relax the disallowed lists (`realtime.md`).
- **Restriction lints** are enabled one by one (as in QG-03 and RT-05), never as the whole
  `clippy::restriction` group.
- **Release profile:** `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"`.
  Unwinding lets a panic in a worker or job thread be caught at the thread boundary and reported,
  instead of aborting the whole application. Real-time code never panics (RT-05).

## Choosing and adding a dependency (QG-10, QG-13)

1. **Search before you write.** Before implementing any non-trivial mechanism, look for an
   established crate on crates.io, lib.rs or blessed.rs. Prefer crates that meet all of these:
   - widely used;
   - maintained: a recent release, and issues that get answers;
   - documented;
   - an MSRV no newer than the pinned toolchain.

   Typical examples:
   - `rtrb` for SPSC rings, `arc-swap` for snapshot hand-off, `crossbeam-channel` off the audio
     thread;
   - `rustfft`/`realfft` for FFT, `serde`/`serde_json` for project JSON, `thiserror` for error
     types;
   - `hound` for WAV, `midly` for MIDI files, `rubato` for resampling, `ebur128` for loudness.
2. **Check the licence.**
   - It must be on the `deny.toml` allow-list.
   - GPL, AGPL and LGPL are never allowed (LIC-01).
   - A licence that is not on the list (for example MPL-2.0) needs the owner's decision first.
3. **Check advisories** (`cargo deny check advisories`) and the crate's own `unsafe` surface.
4. **Check real-time behaviour.** A crate called from real-time code must be shown not to
   allocate, lock or block on the calls we make (RT-02, RT-03). Show it with documentation, or
   with `assert_no_alloc`/RTSan tests.
5. **Check parity.** Where a crate replaces behaviour that has golden data, it must pass the
   same golden tests. A different algorithm that fails the oracle is not a drop-in replacement
   (GLD-01).
6. **Enable only the features you need.** Use `default-features = false` where that is sensible.
7. **Record it.** Add the crate to `docs/dependencies.md` with its version, licence and use, say
   whether it is needed at runtime or only at build time, and name it in the change
   description.

A crate that passes these checks can be added without asking first. A licence outside the
allow-list, or a crate on the real-time path, is called out explicitly in the plan.

## The goal

Long-lived Rust is not the code with the most traits, generics or advanced constructs. It is
code with clear boundaries of responsibility, which uses the type system to protect invariants,
handles ownership and errors correctly, and whose quality is verified again and again by
automated checks. Three standards come first: idiomatic Rust, an explicit domain model, and
automated quality gates. Detailed architectural rules build on those.
