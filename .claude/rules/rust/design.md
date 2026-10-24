# Rust design rules: principles, architecture, modules, traits, naming

These are SHOULD rules unless stated otherwise (see `quality-gate.md` for levels and precedence).

## PRI: principles

- **PRI-01 Correctness over cleverness.** Code must above all be correct. Prefer a simple,
  verifiable solution to a clever abstraction that is hard to understand.
- **PRI-02 Explicitness over magic.** Dependencies, errors, mutation and side effects must be
  visible in the code. Avoid hidden global state and implicit behaviour.
- **PRI-03 Composition over inheritance.** Build functionality from smaller types, functions,
  traits and components. Rust has no implementation inheritance, so do not emulate it, for
  example through `Deref`.
- **PRI-04 Make invalid states unrepresentable.** Use the type system to rule out invalid
  states wherever that is reasonable. Do not rely on comments and runtime checks alone.
- **PRI-05 Maintainability over premature optimization.** Optimize only after measuring. Choose
  an architecture that can be extended safely, without large rewrites.

## ARC: architecture

- **ARC-01 Single responsibility.** Every module and type has one clearly defined role.
- **ARC-02 High cohesion.** Group functions and types by a shared domain responsibility.
- **ARC-03 Low coupling.** Changing how one component works must not require edits in
  unrelated modules.
- **ARC-04 Explicit dependencies.** Pass dependencies through constructors, parameters or
  clearly defined context structs.
- **ARC-05 No premature abstraction.** Add a trait or a generic mechanism only when it brings a
  real benefit.
- **ARC-06 No catch-all modules.** Do not create `utils`, `helpers`, `common` or `misc` modules,
  which gradually collect unrelated functionality.
- **ARC-07 Separate layers.** Domain logic must not depend without need on device APIs, the
  file system or any concrete storage.
- **ARC-08 Dependencies point inward.** Infrastructure adapts to what the domain needs, not the
  other way round.
- **ARC-09 Small public API.** Make public only the types and functions that are really part of
  the public contract.
- **ARC-10 Stable module boundaries.** Change the internal implementation without needless
  changes to public interfaces.

Architecture is not a contest in the number of layers. Directory names matter less than the real
dependencies between components.

**The Sonara workspace.** The crates below and their dependency rules (ARC-11) are fixed. Do
not introduce a different layout, such as a generic `domain/application/infrastructure` tree. Each
crate's role:

| Crate | Role |
|---|---|
| `sonara-core` | Domain layer: time base, domain model, commands, undo, session. Pure: no I/O. |
| `sonara-project` | The `.sonara` format and store. |
| `sonara-midi` | Theory, generators, Standard MIDI Files. |
| `sonara-dsp` | Kernels, processors, instruments, analysis. A real-time crate with no I/O and no `unsafe`. |
| `sonara-media` | Decoding, resampling, the media cache, encoders. |
| `sonara-engine` | Render graph, offline renderer, player loop. |
| `sonara-rt` | Real-time primitives: rings, flush and fade, meters, stats atomics. |
| `sonara-io` | Infrastructure: miniaudio FFI and the device engine, MIDI through midir. |
| `sonara-app` | Application services and composition root. |
| `sonara-agent` | Tool registry, handlers, permissions, audit, IPC, MCP. |
| `sonara-testkit` | Golden-data loaders and comparisons; a dev-dependency only. |
| `apps/sonara`, `apps/sonara-desktop` | Binaries: CLI and entry points; GUI (ADR 0009). |

**ARC-11 Crate dependency rules.** These constraints hold between the crates, and
`crates/sonara-testkit/tests/architecture.rs` checks them:

- `sonara-core` is pure.
- Real-time crates never depend on `sonara-app`, `sonara-agent` or the GUI (RT-01).
- Interfaces edit only through `sonara-app` services and commands.
- There are no cycles.
- The desktop app reaches the core only through `sonara-app` and `sonara-agent`; no crate depends
  on it.

Any other dependency between crates is a deliberate decision, recorded in the architecture test.

## DES: design principles

- **DES-01 DRY.** Share one source of truth for a rule, not necessarily every similar line of
  code.
- **DES-02 KISS.** Prefer simple functions, explicit branching and common Rust idioms.
- **DES-03 YAGNI.** Do not implement hypothetical future needs.
- **DES-04 SOLID.** Use it as orientation, not as an obligation to create types and traits for
  everything.
- **DES-05 Composition.** Combine smaller components instead of building large hierarchies.
- **DES-06 Dependency inversion.** Abstract infrastructure where that enables testing or a
  change of implementation. One example is the device layer behind the null backend.
- **DES-07 Separation of concerns.** Keep validation, domain rules, I/O and presentation of data
  apart.
- **DES-08 Law of Demeter.** Do not reach needlessly into the internal structure of other
  components.

Do not create a generic trait just because two functions share a signature. If their meaning
and expected evolution differ, joining them can turn out worse than a little duplication.

## MOD: modules, files and public API

- **MOD-01** Every module has a clear purpose.
- **MOD-02** Visibility stays private by default.
- **MOD-03** Use `pub` only for API meant for other crates. In this workspace, those crates are
  the external users.
- **MOD-04** Use `pub(crate)` for interfaces needed inside one crate.
- **MOD-05** Document public types and functions with `///` (QG-05).
- **MOD-06** Do not export internal implementation details without a reason.
- **MOD-07** Avoid circular dependencies between layers, modules and crates.
- **MOD-08** Keep library APIs stable and plan changes by semantic versioning. Here the external
  contracts are those of CTR-01 (the project format, tool schemas, IPC and CLI).
- **MOD-09** Split into more crates only when the split brings a concrete benefit: separate
  compilation, distribution, or a safety boundary. The workspace's crates are justified:
  - keeping application, agent and GUI code out of the real-time crates is a compile-time
    safety boundary (RT-01);
  - `unsafe` is confined to two crates (RT-10);
  - loom and RTSan run per crate.

  Do not merge these crates, and do not add a crate without a reason of the same kind.
- **MOD-10** Public APIs document the meaning of their errors and panics, and any safety
  preconditions.

A module is a tool for organizing and encapsulating code. It is not an architectural layer, and
it is not an end in itself.

## TRT: traits and generics

- **TRT-01** Define a trait by behaviour and contract, not by the name of an implementing type.
- **TRT-02** Use generics and static dispatch when you need abstraction without dynamic
  dispatch.
- **TRT-03** Use `dyn Trait` for heterogeneous collections and runtime polymorphism. An example
  is the graph nodes holding different processors.
- **TRT-04** Do not create a trait for a single implementation without a real reason. A test
  seam that is actually needed counts as a real reason (DES-06).
- **TRT-05** Add a default method implementation only when it has meaningful and correct
  general semantics.
- **TRT-06** Keep generic parameters and trait bounds to the necessary minimum.
- **TRT-07** Use newtypes to separate values that mean different things (TYP-01, TYP-07).
- **TRT-08** Prefer small, cohesive traits to universal interfaces with dozens of methods.

The `Processor` trait is predetermined: `prepare`, `set_params` with smoothing, `set_state`,
`process`, `reset`, `latency`, `tail_seconds` and `report`, as the golden plugin data records
them.

## NAM: naming and readability

| Construct | Convention | Example |
|---|---|---|
| Functions | `snake_case` | `queued_frames()` |
| Variables | `snake_case` | `block_frames` |
| Modules | `snake_case` | `capture_ring` |
| Types and traits | `UpperCamelCase` | `PlaybackRing` |
| Constants | `SCREAMING_SNAKE_CASE` | `FLUSH_FADE_FRAMES` |
| Enum variants | `UpperCamelCase` | `SvfMode::Lowpass` |
| Generic parameters | short capitals or meaningful names | `T`, `Sample` |

- **NAM-01** A function name expresses its purpose, not an implementation detail.
- **NAM-02** Name a boolean so that it is obvious what `true` means (`is_playing`, not `state`).
- **NAM-03** Do not use unclear abbreviations unless they are established in the domain.
  Domain abbreviations such as `svf`, `adsr`, `lufs` and `pdc` are allowed.
- **NAM-04** Avoid names like `Manager`, `Helper` and `Processor` unless they describe a
  concrete responsibility. `Processor` is a Sonara domain term (an audio processor), so it is
  allowed in that meaning.
- **NAM-05** Distinguish operations by their semantics: `parse`, `validate`, `load`, `save`,
  `convert`.
- **NAM-06** Prefer short functions with one main idea, but do not split trivial operations
  into dozens of wrappers that add nothing.
- **NAM-07** Do not repeat the module name in a function name without a reason
  (`ring::push`, not `ring::ring_push`).
- **NAM-08** Keep one style across the whole project, and automate formatting with `cargo fmt`
  (QG-01). Indentation is four spaces.

Names that the golden data or the contracts record keep their recorded spelling. Examples are
`ar_smooth`, `osc_bank` and the `stats()` keys.
