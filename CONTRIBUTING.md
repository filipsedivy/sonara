# Contributing to Sonara

## Setup

Sonara is a native Rust application for macOS 26 or later on Apple Silicon. The workspace is at
the repository root: `Cargo.toml`, `crates/` and `apps/`.

- **Git LFS.** `golden/` stores its binaries with Git LFS, so install it (`brew install git-lfs`
  and `git lfs install`) before you clone or commit.
- **Host toolchain** (for macOS builds, the desktop app and the bundle): rustup with the version
  pinned in `rust-toolchain.toml`, the Xcode command line tools, cmake, Node 26 and pnpm 12.

  ```bash
  scripts/native/build-codecs.sh          # libsoxr, LAME and libFLAC into target/native/lib
  cargo build --release -p sonara         # the sonara CLI
  cd apps/sonara-desktop && pnpm install --frozen-lockfile && pnpm tauri dev
  scripts/bundle-macos.sh                 # Sonara.app and the ad hoc signed DMG
  ```

- **Development container** (Linux, needs only Docker). It runs the same Rust gates as CI,
  plus loom and RTSan. It keeps build output and caches in Docker volumes and does not build the
  desktop app.

  ```bash
  scripts/dev.sh rust          # fmt, clippy -D warnings, test, doc, cargo deny, loom, RTSan
  scripts/dev.sh rust-check    # the fast loop: fmt, clippy, test, doc
  scripts/dev.sh rust-fmt      # format the Rust code
  scripts/dev.sh frontend      # the desktop frontend gates
  scripts/dev.sh help          # all tasks
  ```

  If the linker runs out of memory, limit the jobs:
  `scripts/dev.sh run bash -c 'export CARGO_BUILD_JOBS=4; docker/dev/tasks.sh rust'`.

The rules for all code are in `.claude/rules/`, indexed from `CLAUDE.md`. QG-13 says library
first: use an established crate wherever one fits.

## Quality gates

CI runs these on every change; run them before you propose one:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
```

`scripts/dev.sh rust` adds `cargo deny`, loom and RTSan. The desktop frontend gates, from
`apps/sonara-desktop`:

```bash
pnpm install --frozen-lockfile
pnpm run check:licenses
pnpm run check
pnpm run build
pnpm run test:unit
```

Benchmarks are optional, but please run them for engine or DSP changes:
`cargo run --release -p sonara-agent --example bench`, and `criterion` in the crates.

After changing agent tools, regenerate the API reference with
`cargo run -p sonara-agent --example gen_agent_docs`. A test fails while it is stale.

## Ground rules

- **Respect crate boundaries.** See [docs/architecture.md](docs/architecture.md) and ARC-11.
  `sonara-core` stays pure; the real-time crates never depend on the application, agent or GUI;
  the GUI and agent tools edit only through `sonara-app` services and commands.
  `crates/sonara-testkit/tests/architecture.rs` enforces this.
- **Make every edit a command.** A `Command` in `sonara-core` is pure, deterministic (IDs are
  generated at construction) and validated in `apply`, and it returns a `ChangeSet`. Give it a
  merge key for continuous gestures.
- **Never block the device callback.** Read the rules in
  [docs/audio-engine.md](docs/audio-engine.md) and `.claude/rules/rust/realtime.md` before you
  touch the real-time crates.
- **Golden data is the oracle.** `golden/` holds the recorded behaviour of the original
  prototype. Never re-record it or loosen a tolerance to make a test pass (GLD-01).
- **Test audio with reference signals.** Impulses, sines, sweeps, noise, silence and clipped
  signals, with tolerances suited to floating point. Verify lossy codecs by aligned
  correlation, never by peak values alone.
- **Don't claim a feature until it works.** Update
  [docs/implementation-status.md](docs/implementation-status.md) and `CHANGELOG.md` with every
  user-visible change.
- **Keep format changes versioned.** A change to `project.json` bumps `schema_version` and adds
  a migration in `crates/sonara-project/src/migrations.rs` with a test.

## Layout

```text
crates/       sonara-core, -project, -midi, -dsp, -media, -engine, -rt, -io, -app, -agent, -testkit
apps/         sonara (CLI/MCP); sonara-desktop (Tauri 2 GUI: Svelte 5, TypeScript)
third_party/  vendored miniaudio
golden/       recorded regression data (Git LFS)
resources/    brand masters and renders
docker/       development container (scripts/dev.sh)
docs/         architecture, engine, format, agent API, plugins, ADRs, status
examples/     reproducible agent workflows (sonara batch)
scripts/      codec build, macOS bundle, version check
```
