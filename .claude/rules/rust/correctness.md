# Rust correctness rules: ownership, types, errors

These are SHOULD rules unless stated otherwise (see `quality-gate.md`). In real-time code the
RT rules in `realtime.md` override them and are stricter.

## MEM: ownership, borrowing, lifetimes

- **MEM-01 Prefer borrowing.** If a function only reads data, take a reference instead of
  ownership.
- **MEM-02 Take ownership on purpose.** Pass `String`, `Vec<T>` and other owned data by value
  only when the function really takes ownership.
- **MEM-03 Minimize `clone()`.** Do not copy data just to silence the borrow checker. First check
  whether the API design should change.
- **MEM-04 Minimize mutability.** Use `mut` only where a change of state is really needed.
- **MEM-05 Do not add lifetimes early.** Write explicit lifetime annotations only when the
  relationship between references requires them.
- **MEM-06 Do not reach for `'static`.** Never use `'static` as a universal fix for lifetime
  problems.
- **MEM-07 Prefer RAII.** Release resources through ownership and `Drop`, not through manual
  lifetime management where that is not necessary. For example, the miniaudio context and
  device handles are owned by wrapper types whose `Drop` uninitializes them, on a non-real-time
  thread (RT-09).
- **MEM-08 Limit shared mutability.** Use `Arc<Mutex<T>>` and `Arc<RwLock<T>>` only where shared
  access is really needed. They are never reachable from real-time code, which uses no locks at
  all (RT-03, RT-04). Outside real-time code, prefer transferring ownership or passing messages.
- **MEM-09 `unsafe` needs a reason.** Every `unsafe` block must have clearly stated safety
  preconditions. In this repository RT-10 applies, which is stricter: `unsafe` is allowed in three
  modules only, and every block needs a `// SAFETY:` comment.
- **MEM-10 Respect the borrow checker.** Do not work around its limits without understanding
  the cause of the problem.

Accept the widest borrowed type the function needs:

```rust
/// Largest absolute sample value in `block`; 0.0 for an empty block.
fn peak_abs(block: &[f32]) -> f32 {
    block.iter().fold(0.0, |peak, sample| peak.max(sample.abs()))
}
```

The function needs neither ownership nor a `Vec<f32>`. Taking `&[f32]` gives a wider and simpler
interface.

## TYP: types and domain modelling

- **TYP-01 Use domain types.** Create a separate type for a value with its own rules, such as a
  generation, a timeline frame, a channel count or a sample rate.
- **TYP-02 Prefer `enum` for variants.** Model a finite set of states with an enum.
- **TYP-03 No magic values.** Do not use, for example, `-1` as an implicit marker for a missing
  value.
- **TYP-04 Use `Option<T>`.** Represent a missing value explicitly.
- **TYP-05 Use `Result<T, E>`.** Give operations that can fail a typed result.
- **TYP-06 Validate at the boundary.** Convert external data into a validated domain type as
  early as possible. Here that means where project JSON, tool arguments, IPC messages, CLI
  arguments or media files enter the application.
- **TYP-07 Distinguish identifiers.** Do not use one integer type for all kinds of ID when mixing
  them up could cause a defect. Track IDs and device IDs, for example, are separate types.
- **TYP-08 Avoid needless conversions.** Keep clear boundaries between transport (JSON, IPC),
  domain and storage types.
- **TYP-09 Limit `Any` and dynamic typing.** Use dynamic representations only where they are
  needed, for example `serde_json::Value` at a generic serialization boundary.
- **TYP-10 Respect trait semantics.** Implement `Eq`, `Hash`, `Ord` and the other traits
  consistently with their contracts and with each other. For example, `f64` is not `Eq`, so do
  not wrap it and pretend that it is.

An enum is better than several booleans that can contradict each other. A conversion at the
boundary must keep the behaviour that the golden data records (GLD-01):

```rust
/// Response of the state-variable filter (the `mode` parameter of the SVF kernel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvfMode {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peak,
    Allpass,
}

impl SvfMode {
    /// Maps the raw mode as the golden kernel vectors record it:
    /// 0–4 select a response, and every other value selects `Allpass`.
    #[must_use]
    pub fn from_raw(mode: i32) -> Self {
        match mode {
            0 => Self::Lowpass,
            1 => Self::Highpass,
            2 => Self::Bandpass,
            3 => Self::Notch,
            4 => Self::Peak,
            _ => Self::Allpass,
        }
    }
}
```

A newtype keeps semantically different integers apart. It costs nothing at run time:

```rust
/// Ring block generation; `flush` makes older generations stale. Wraps on overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation(u32);
```

## ERR: errors and failure

- **ERR-01 No thoughtless `unwrap()`.** Handle errors explicitly in production logic.
  - `unwrap()` and `expect()` are legitimate in tests, when initializing invariants, and where
    failure really is fatal. Even there, use them only for a known reason (QG-03).
  - Real-time code has no exceptions to this rule (RT-05).
- **ERR-02 Propagate with `?`.** Do not repeat the same handling logic by hand for every
  failure.
- **ERR-03 Use meaningful error types.** Distinguish, for example, a validation error, a device
  error and a MIDI backend error.
- **ERR-04 Keep the cause.** When converting an error, do not drop the original context.
  `source()` must return the cause.
- **ERR-05 Add context at boundaries.** An error must say which operation failed.
- **ERR-06 Do not suppress errors.** Never ignore a `Result` (`let _ = …`, `.ok()`) when the
  error affects the correctness of the result.
- **ERR-07 Separate errors from panics.** A panic is not the normal mechanism for operational
  errors. Catching a panic at a worker-thread boundary is a safety net for defects, not an error
  path.
- **ERR-08 Decide recoverability.** Whether an operation can be retried is a decision that
  belongs to the application logic.

A failing operation returns a `Result`. The caller decides what happens next:

```rust
/// Audio context requested by `init_context`.
pub enum ContextKind {
    System,
    Null,
}

impl std::str::FromStr for ContextKind {
    type Err = UnknownContext;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "default" => Ok(Self::System),
            "null" => Ok(Self::Null),
            other => Err(UnknownContext(other.to_owned())),
        }
    }
}
```

**Sonara specifics:**

- **Error types in library crates.** Library crates define error enums that implement
  `std::error::Error` with `source()`. `thiserror` is the expected tool for this (QG-10).
  `anyhow` and `Box<dyn Error>` do not appear in library APIs.
- **The agent boundary is part of the contract (CTR-01).** Tool errors keep the recorded error
  codes and structure, for example `ENGINE_UNAVAILABLE` and `CONFIRMATION_REQUIRED`. Typed Rust
  errors map to them in `sonara-agent`, checked against the golden transcripts.
- **Errors on the real-time thread** cannot be returned. They are counted and flagged, and a
  non-real-time thread reports them (RT-08, RT-12).
