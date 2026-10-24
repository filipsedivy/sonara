//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! The filter primitives of the prototype's processors, ported with the oracle's exact
//! arithmetic (section 0):
//!
//! - [`SosFilter`]: `SciPy`'s `sosfilt` with persistent state and the state rules of the
//!   prototype's `SosFilter` (`dsp/filters/biquad.py`). Bit-exact. [`response_db`] is
//!   `sosfreqz`'s magnitude in dB, for drawing a design's response (not bit-exact).
//! - [`Lfilter`]: `SciPy`'s `lfilter` for recursive filters (`len(a) > 1`). Bit-exact (F5, with the
//!   middle-delay form corrected; see [`Lfilter`]).
//! - [`upfirdn`]: `SciPy`'s `upfirdn` for upsampling (`down = 1`). Bit-exact.
//! - [`biquad`], [`butterworth`] and [`butterworth_qs`]: the RBJ cookbook designs of `biquad.py`
//!   in plain f64 arithmetic in the prototype's order. They call `sin`, `cos` and `pow`,
//!   so they are exact where the platform's libm agrees with macOS libm.
//! - [`SmoothedValue`], [`db_to_amp`], [`amp_to_db`], [`time_coefficient`] and
//!   [`ensure_finite`] from `dsp/base.py`.
//! - [`DelayLine`] (the graph's latency compensation) and [`BackwardMinimum`] (the limiter's
//!   `minimum_filter1d` hold).
//!
//! Common conventions, as in [`crate::kernels`]:
//!
//! - Multichannel buffers are interleaved `(frames, channels)`, row-major, which is the
//!   prototype's C-order `(n, channels)` array filtered along `axis=0`.
//! - Constructors and `new` functions are setup: they allocate and run before the stream starts,
//!   off the real-time thread. Every other method allocates, locks, blocks and panics never
//!   (RT-02, RT-03, RT-05), and walks slices with iterators instead of indexing.
//! - When the slices of one call differ in length, the common length in frames is processed and
//!   the rest is left untouched, so that a sizing mistake cannot panic the callback.
//! - Numerics keep the oracle's order of operations (RT-11). `mul_add` appears exactly where the
//!   oracle's compiled code fuses a multiply and an add; everything else is plain
//!   arithmetic, never reassociated.

mod biquad;
mod delay;
mod level;
mod lfilter;
mod minimum;
mod smoothed;
mod sos;
mod upfirdn;

pub use biquad::{
    BiquadType, ButterworthSlope, Cascade, MAX_CASCADE_SECTIONS, biquad, butterworth,
    butterworth_qs,
};
pub use delay::DelayLine;
pub use level::{amp_to_db, db_to_amp, ensure_finite, time_coefficient};
pub use lfilter::{Lfilter, LfilterError};
pub use minimum::BackwardMinimum;
pub use smoothed::{Smoothed, SmoothedValue};
pub use sos::{IDENTITY_SECTION, Section, SectionCapacityError, SosFilter, response_db};
pub use upfirdn::upfirdn;
