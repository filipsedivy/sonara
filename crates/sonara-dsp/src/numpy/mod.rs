//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! NumPy-compatible numerics: the parts of NumPy 2.5.4 whose exact results the prototype's
//! golden data depends on (F12–F15).
//!
//! - [`random`]: `np.random.default_rng(seed)`, bit for bit: `SeedSequence`, `PCG64`, and the
//!   `Generator` methods the prototype calls (`random`, `integers`, `choice`,
//!   `standard_normal`).
//! - [`math`]: pairwise `sum` and `mean`, `cumsum`, `linspace`, `rint`, NumPy's `round`,
//!   Python's `round(x, ndigits)` and `percentile`.
//!
//! Nothing here allocates, locks or panics (RT-02, RT-03, RT-05). Arrays are caller-owned
//! slices in C order. Floating-point operations keep NumPy's order (RT-11); the only fused
//! multiply-add is the ziggurat's wedge test, where NumPy's C code is contracted as well (see
//! [`random::Generator::standard_normal`]).

pub mod math;
pub mod random;
mod ziggurat;
