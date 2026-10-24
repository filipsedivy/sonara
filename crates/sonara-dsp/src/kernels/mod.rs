//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! The four per-sample recursive kernels of the prototype: [`ar_smooth`], [`svf`], [`gate_hold`]
//! and [`osc_bank`]. They reproduce the prototype's native C++ implementation
//! (`native/src/kernels.hpp`), which is what the golden data in `golden/kernels/` recorded.
//!
//! Common conventions:
//!
//! - Slices in, slices out. State is owned by the caller and passed as `&mut`, so processing a
//!   signal in one call or split across several calls gives bit-identical results.
//! - Nothing here allocates, locks, blocks or panics (RT-02, RT-03, RT-05). Slices are walked
//!   with iterators, never indexed.
//! - When the slices of one call differ in length, the kernel processes the common length (the
//!   shortest slice, counted in frames for [`svf`]) and leaves the rest of the longer slices
//!   untouched. The state then reflects the processed prefix only. Callers size their buffers
//!   before the stream starts, so this never happens on the audio path; it exists so that a
//!   mistake cannot panic the callback.
//! - Numerics keep the oracle's order of operations (RT-11). The oracle is compiled by clang with
//!   floating-point contraction on arm64, so `mul_add` appears
//!   exactly where its machine code fuses a multiply and an add, read from the arm64 assembly
//!   and verified bit-exact against the golden vectors on the recording host. Nothing is
//!   reassociated.

mod ar_smooth;
mod gate_hold;
mod osc_bank;
mod svf;

pub use ar_smooth::ar_smooth;
pub use gate_hold::{GateState, gate_hold};
pub use osc_bank::{MAX_OSCILLATORS, Oscillator, Wave, osc_bank};
pub use svf::{SvfMode, clamp_cutoff, clamp_q, svf};
