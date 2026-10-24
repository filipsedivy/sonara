//! Real-time: arithmetic of the convolver's spectra, used on the audio render thread (see
//! docs/audio-engine.md).
//!
//! The float type of the impulse-response spectra, the frequency-domain delay line and the
//! inverse transform.

use realfft::FftNum;

mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// What a convolver block's samples are, in the prototype's NumPy dtypes.
///
/// The reverb mixes the convolver's output with a Python float (`wet * m`); NumPy computes that
/// product in the array's dtype, so a processor needs to know which one the prototype had.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputPrecision {
    /// float32 values: the single-precision `irfft` output of an ordinary block.
    Single,
    /// float64 values: the double-precision mode, a crossfade block (the fade is float64) or the
    /// zeros of a convolver without an impulse response.
    Double,
}

/// The float type of a [`PartitionedConvolver`](super::PartitionedConvolver): `f64` is the
/// prototype's default (`complex128`), `f32` its `single_precision` mode (`complex64` spectra
/// and delay line, a float32 `irfft`). Implemented for `f32` and `f64` only.
pub trait SpectrumFloat: FftNum + sealed::Sealed {
    /// The precision of a block that only went through the inverse transform.
    const OUTPUT: OutputPrecision;

    /// Rounds a float64 value to this type, as NumPy's `astype` (round to nearest, ties to
    /// even).
    fn narrow(value: f64) -> Self;

    /// The value as a float64, exactly.
    fn widen(self) -> f64;

    /// A length as this type, exactly for every FFT size the convolver plans.
    fn from_len(len: usize) -> Self;

    /// `self · a + b`, rounded once (the fused multiply-add NumPy's compiled `einsum` uses).
    #[must_use]
    fn fused_mul_add(self, a: Self, b: Self) -> Self;
}

impl SpectrumFloat for f64 {
    const OUTPUT: OutputPrecision = OutputPrecision::Double;

    fn narrow(value: f64) -> Self {
        value
    }

    fn widen(self) -> f64 {
        self
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "FFT sizes are far below 2^53, exactly representable"
    )]
    fn from_len(len: usize) -> Self {
        len as f64
    }

    fn fused_mul_add(self, a: Self, b: Self) -> Self {
        self.mul_add(a, b)
    }
}

impl SpectrumFloat for f32 {
    const OUTPUT: OutputPrecision = OutputPrecision::Single;

    #[expect(
        clippy::cast_possible_truncation,
        reason = "NumPy's astype(complex64) rounds to the nearest float32, as `as` does"
    )]
    fn narrow(value: f64) -> Self {
        value as f32
    }

    fn widen(self) -> f64 {
        f64::from(self)
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "FFT sizes are below 2^24, exactly representable"
    )]
    fn from_len(len: usize) -> Self {
        len as f32
    }

    fn fused_mul_add(self, a: Self, b: Self) -> Self {
        self.mul_add(a, b)
    }
}
