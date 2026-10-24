//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! Sonara's DSP: the per-sample kernels, filters, NumPy-compatible numerics and the built-in
//! processors. Pure computation without I/O; everything has a reference in the
//! prototype's `sonara.dsp` and `sonara.instruments`.
#![forbid(unsafe_code)]
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

pub mod analysis;
pub mod convolution;
pub mod filters;
pub mod kernels;
pub mod numpy;
pub mod processor;
pub mod processors;
pub mod tables;
