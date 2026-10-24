# Sonara

Sonara is a digital audio workstation for macOS with a first-class agent interface: MCP, CLI and
local IPC onto one project session. It is a native Rust application with a Tauri 2 desktop GUI
(Svelte 5, TypeScript). C is used only for native libraries: the vendored miniaudio, and the
codec libraries loaded at run time (libsoxr, LAME, libFLAC).

The application was ported from a Python prototype, which is gone. Its recorded behaviour stays
in `golden/` as the regression oracle (GLD-01).

## Rules (mandatory)

The files in `.claude/rules/` load automatically at the start of every session. Follow them in
every plan, change and review, and cite their rule IDs.

| File | Rule IDs | Covers |
|---|---|---|
| [.claude/rules/project.md](.claude/rules/project.md) | CTR, GLD, LIC, HW, VER, DOC | Contracts, golden data, licences, hardware numbers, versions, docs |
| [.claude/rules/rust/quality-gate.md](.claude/rules/rust/quality-gate.md) | QG | Rule levels and precedence, mandatory gate, commands, lints, dependencies |
| [.claude/rules/rust/realtime.md](.claude/rules/rust/realtime.md) | RT | Audio-thread code, `unsafe`, numerics |
| [.claude/rules/rust/design.md](.claude/rules/rust/design.md) | PRI, ARC, DES, MOD, TRT, NAM | Principles, architecture and crate roles, modules, traits, naming |
| [.claude/rules/rust/correctness.md](.claude/rules/rust/correctness.md) | MEM, TYP, ERR | Ownership, types, errors, the agent error boundary |
| [.claude/rules/rust/testing-performance.md](.claude/rules/rust/testing-performance.md) | TST, PERF | Tests, golden parity, performance |

**Precedence:** the ADRs and the golden data first, then the project rules, then RT, then QG
and the general rules. Where two rules overlap, the stricter one applies.

- **MUST rules** (project rules, QG, RT) are never waived in code.
- **SHOULD rules** may be deviated from only with a written reason that cites the rule ID.
- **Subagents:** the built-in Explore and Plan subagents do not load this file or the rules. When
  you delegate to them, pass the paths of the relevant rule files in the prompt.

## Layout

```text
Cargo.toml    Rust workspace (crates/, apps/)
crates/       sonara-core, -project, -midi, -dsp, -media, -engine, -rt, -io, -app, -agent, -testkit
apps/         sonara (CLI and MCP entry point); sonara-desktop (Tauri GUI, src-tauri/ + Svelte)
third_party/  vendored miniaudio
golden/       golden data recorded from the prototype (Git LFS): the regression oracle
resources/    brand masters (SVG, PNG, icns)
docker/       development container (scripts/dev.sh)
scripts/      dev.sh, check-versions.sh, bundle-macos.sh, native/build-codecs.sh
docs/         architecture, audio engine, project format, agent API, ADRs, status, dependencies
examples/     reproducible agent workflows (sonara batch)
```

## Commands

Rust gates in the development container (the desktop crate is excluded there):

```bash
scripts/dev.sh rust                # fmt, clippy -D warnings, test, doc, cargo deny, loom, RTSan
scripts/dev.sh rust-check          # the fast loop: fmt, clippy, test, doc
scripts/dev.sh frontend            # the desktop frontend: licences, svelte-check, build, tests
scripts/dev.sh help                # every task
```

On the macOS host (the toolchain is rustup's, in `/opt/homebrew/opt/rustup/bin`):

```bash
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
scripts/native/build-codecs.sh     # libsoxr, LAME and libFLAC into target/native/lib
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
(cd apps/sonara-desktop && pnpm install --frozen-lockfile && pnpm tauri dev)
scripts/bundle-macos.sh            # the ad-hoc signed Sonara.app and DMG
```

Details are in `.claude/rules/rust/quality-gate.md` (Commands).

## Documentation map

- [docs/architecture.md](docs/architecture.md): the crates, threads and data flow.
- [docs/audio-engine.md](docs/audio-engine.md): the real-time engine and the measured baselines.
- [docs/project-format.md](docs/project-format.md), [docs/agent-api.md](docs/agent-api.md): the
  contracts that must not change (CTR-01).
- [docs/decisions/](docs/decisions/): the ADRs; 0009 is the GUI toolkit (Tauri 2).
- [docs/implementation-status.md](docs/implementation-status.md): what works.
- [docs/dependencies.md](docs/dependencies.md): every dependency with its licence.

## Working agreements

- All repository content is written in English: code, comments and docs.
- Do not commit, push, tag or rename branches. The owner versions the repository.
- Ask before installing toolchains or system packages on the host.
- Library first (QG-13): use an established crate instead of writing our own code whenever one
  fits.
- The bundle identifier is `cz.filipsedivy.sonara`, and the minimum macOS is 26.
- Releases are ad-hoc signed and not notarized (owner decision).
