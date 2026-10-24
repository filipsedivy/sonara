//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `SciPy`'s `lfilter` for recursive filters, with persistent state.

use std::cell::Cell;
use std::fmt;

/// Why [`Lfilter::new`] rejected a filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LfilterError {
    /// `b` or `a` is empty.
    EmptyCoefficients,
    /// `a` has a single coefficient. `SciPy` then runs `np.convolve` (an FIR, whose summation order
    /// Accelerate's SIMD dot product sets), which this type does
    /// not reproduce.
    NotRecursive,
    /// `a[0]` is zero, so the coefficients cannot be normalized.
    ZeroLeadingDenominator,
}

impl fmt::Display for LfilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyCoefficients => "the filter coefficients are empty",
            Self::NotRecursive => "the denominator has a single coefficient (an FIR filter)",
            Self::ZeroLeadingDenominator => "the leading denominator coefficient is zero",
        })
    }
}

impl std::error::Error for LfilterError {}

/// Direct-form II transposed IIR filter with persistent state per channel: `SciPy`'s
/// `lfilter(b, a, x, axis=0, zi)` for `len(a) > 1`. The prototype uses it for the compressor's
/// RMS detector and the utility's DC blocker (both first order).
///
/// Like `SciPy`'s C kernel, `b` and `a` are zero-padded to the same length `n` and divided by
/// `a[0]` once. Per channel and sample, with state `Z[0..n-1]` (`SciPy`'s `zi`), the arithmetic
/// is bit-exact with `SciPy` on the recording host:
///
/// ```text
/// y      = fma(b0, x, Z0)
/// Z[k]   = fma(-y, a[k+1], fma(x, b[k+1], Z[k+1]))   for k < n - 2
/// Z[n-2] = fma(b[n-1], x, -(a[n-1] * y))
/// ```
///
/// The middle line is the clang contraction of `SciPy`'s `Z[k+1] + x*b[k+1] - y*a[k+1]`. Fact F5
/// of gives a different middle form; it was verified for first-order
/// filters only, where no middle delay exists. Measured on the recording host, F5's form differs
/// from `SciPy` in most samples of orders 2, 4 and 6, and this form matches all of them.
#[derive(Clone, Debug)]
pub struct Lfilter {
    channels: usize,
    /// Normalized numerator, length `order + 1`.
    b: Vec<f64>,
    /// Normalized denominator, length `order + 1`; `a[0]` is 1 and unused.
    a: Vec<f64>,
    /// `Z` per channel, laid out `[channel][k]`, `order` values each.
    state: Vec<f64>,
}

impl Lfilter {
    /// A filter with numerator `b` and denominator `a` for `channels` interleaved channels, with
    /// zero state.
    ///
    /// Setup: allocates, so it runs before the stream starts, off the real-time thread.
    ///
    /// # Errors
    ///
    /// [`LfilterError`] when `b` or `a` is empty, `a` has one coefficient, or `a[0]` is zero.
    #[expect(
        clippy::disallowed_macros,
        reason = "setup: runs before the stream starts, off the real-time thread"
    )]
    pub fn new(b: &[f64], a: &[f64], channels: usize) -> Result<Self, LfilterError> {
        let Some(&a0) = a.first() else {
            return Err(LfilterError::EmptyCoefficients);
        };
        if b.is_empty() {
            return Err(LfilterError::EmptyCoefficients);
        }
        if a.len() < 2 {
            return Err(LfilterError::NotRecursive);
        }
        if a0 == 0.0 {
            return Err(LfilterError::ZeroLeadingDenominator);
        }
        let n = b.len().max(a.len());
        let normalized = |coefficients: &[f64]| -> Vec<f64> {
            let mut out = vec![0.0; n];
            for (o, c) in out.iter_mut().zip(coefficients) {
                *o = c / a0;
            }
            out
        };
        Ok(Self {
            channels,
            b: normalized(b),
            a: normalized(a),
            state: vec![0.0; channels.saturating_mul(n - 1)],
        })
    }

    /// Number of interleaved channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// The filter order: the length of the padded coefficients minus one.
    #[must_use]
    pub fn order(&self) -> usize {
        self.b.len().saturating_sub(1)
    }

    /// The state `Z` of `channel` (`SciPy`'s `zi` along the filtered axis), or `None` for a
    /// channel the filter does not have.
    #[must_use]
    pub fn state(&self, channel: usize) -> Option<&[f64]> {
        // `new` guarantees an order of at least 1, so the chunk size is never 0.
        self.state.chunks_exact(self.order().max(1)).nth(channel)
    }

    /// Zeroes the state.
    pub fn reset(&mut self) {
        self.state.fill(0.0);
    }

    /// Filters the interleaved frames of `x` into `out`.
    pub fn process(&mut self, x: &[f64], out: &mut [f64]) {
        let (channels, order) = (self.channels, self.order());
        if channels == 0 || order == 0 {
            return;
        }
        for (x_frame, y_frame) in x.chunks_exact(channels).zip(out.chunks_exact_mut(channels)) {
            let states = self.state.chunks_exact_mut(order);
            for ((y, v), z) in y_frame.iter_mut().zip(x_frame).zip(states) {
                *y = filter_sample(&self.b, &self.a, z, *v);
            }
        }
    }

    /// Filters the interleaved frames of `buffer` in place.
    pub fn process_in_place(&mut self, buffer: &mut [f64]) {
        let (channels, order) = (self.channels, self.order());
        if channels == 0 || order == 0 {
            return;
        }
        for frame in buffer.chunks_exact_mut(channels) {
            for (v, z) in frame.iter_mut().zip(self.state.chunks_exact_mut(order)) {
                *v = filter_sample(&self.b, &self.a, z, *v);
            }
        }
    }
}

/// One sample of one channel; `b` and `a` have `z.len() + 1` coefficients.
#[inline]
fn filter_sample(numerator: &[f64], denominator: &[f64], state: &mut [f64], input: f64) -> f64 {
    let b0 = numerator.first().copied().unwrap_or(0.0);
    let state = Cell::from_mut(state).as_slice_of_cells();
    let output = b0.mul_add(input, state.first().map_or(0.0, Cell::get));
    // Ascending k reads Z[k + 1] before it is overwritten, as SciPy's loop does.
    let middle = numerator.iter().zip(denominator).skip(1);
    for (pair, (&bk, &ak)) in state.windows(2).zip(middle) {
        if let [zk, next] = pair {
            zk.set((-output).mul_add(ak, input.mul_add(bk, next.get())));
        }
    }
    if let (Some(last), Some(&bn), Some(&an)) = (state.last(), numerator.last(), denominator.last())
    {
        last.set(bn.mul_add(input, -(an * output)));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{Lfilter, LfilterError};

    #[test]
    fn rejects_filters_that_are_not_recursive() {
        assert_eq!(
            Lfilter::new(&[], &[1.0, 0.5], 1).err(),
            Some(LfilterError::EmptyCoefficients)
        );
        assert_eq!(
            Lfilter::new(&[1.0, 2.0], &[1.0], 1).err(),
            Some(LfilterError::NotRecursive)
        );
        assert_eq!(
            Lfilter::new(&[1.0], &[0.0, 1.0], 1).err(),
            Some(LfilterError::ZeroLeadingDenominator)
        );
    }

    #[test]
    fn one_pole_smoother_follows_the_difference_equation() {
        // y[n] = 0.5 x[n] + 0.5 y[n-1], the compressor's RMS form.
        let mut f = Lfilter::new(&[0.5], &[1.0, -0.5], 1).unwrap();
        assert_eq!(f.order(), 1);
        let mut out = [0.0; 3];
        f.process(&[1.0, 1.0, 1.0], &mut out);
        assert_eq!(out, [0.5, 0.75, 0.875]);
        assert_eq!(f.state(0), Some(&[0.4375][..]));
    }

    #[test]
    fn normalizes_by_a0_and_pads() {
        let mut scaled = Lfilter::new(&[1.0, -1.0], &[2.0, -1.0], 1).unwrap();
        let mut unit = Lfilter::new(&[0.5, -0.5], &[1.0, -0.5], 1).unwrap();
        let x = [1.0, 0.25, -0.5, 2.0];
        let (mut a, mut b) = ([0.0; 4], [0.0; 4]);
        scaled.process(&x, &mut a);
        unit.process(&x, &mut b);
        assert_eq!(a, b);

        // A numerator longer than the denominator pads `a` with zeros: an FIR tail.
        let mut padded = Lfilter::new(&[1.0, 2.0, 3.0], &[1.0, 0.0], 1).unwrap();
        let mut out = [0.0; 4];
        padded.process(&[1.0, 0.0, 0.0, 0.0], &mut out);
        assert_eq!(out, [1.0, 2.0, 3.0, 0.0]);
    }

    #[test]
    fn channels_split_and_in_place_agree() {
        let x = [1.0, -1.0, 0.5, 0.25, -0.75, 2.0];
        let mut a = Lfilter::new(&[1.0, -1.0], &[1.0, -0.99], 2).unwrap();
        let mut out = [0.0; 6];
        a.process(&x, &mut out);
        let mut b = Lfilter::new(&[1.0, -1.0], &[1.0, -0.99], 2).unwrap();
        let mut buffer = x;
        b.process_in_place(&mut buffer);
        assert_eq!(buffer, out);
        assert_eq!(a.state(1), b.state(1));
        a.reset();
        assert_eq!(a.state(0), Some(&[0.0][..]));
        assert_eq!(a.state(2), None);
    }
}
