//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `SciPy`'s `upfirdn` for upsampling (`down = 1`).

/// Upsamples by `up` and FIR-filters with `h`: `SciPy`'s `upfirdn(h, x, up=up, axis=0)` (the
/// Analyzer's true-peak interpolator), output frames `first_frame ..` into `out`.
///
/// - `x` and `out` are interleaved `(frames, channels)` buffers.
/// - Output frame `j` of channel `c` is `Σ_i x[i][c] · h[j − up·i]` over the input frames `i`
///   with `0 ≤ j − up·i < h.len()`. `SciPy`'s full output has `(frames − 1)·up + h.len()` frames;
///   frames beyond that are zero by the same sum.
/// - Accumulation is bit-exact with `SciPy` on the recording host: from `0.0` over ascending
///   `i`, `acc = fma(x[i][c], h[j − up·i], acc)`.
///
/// Every frame of `out` is written (its trailing partial frame is ignored). With `channels` or
/// `up` zero nothing is written.
///
/// # Examples
///
/// ```
/// use sonara_dsp::filters::upfirdn;
///
/// // Zero-stuffing by 2 through a 3-tap filter.
/// let mut out = [0.0; 6];
/// upfirdn(&[0.5, 1.0, 0.5], &[2.0, 4.0], 1, 2, 0, &mut out);
/// assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 2.0, 0.0]);
/// ```
pub fn upfirdn(
    h: &[f64],
    x: &[f64],
    channels: usize,
    up: usize,
    first_frame: usize,
    out: &mut [f64],
) {
    if channels == 0 || up == 0 {
        return;
    }
    for (n, frame) in out.chunks_exact_mut(channels).enumerate() {
        let j = first_frame.saturating_add(n);
        // Input frames with a tap at output j: i_lo ..= j / up, the first one with the
        // highest tap index (`j − up·i_lo < h.len()`).
        let i_lo = j.saturating_add(1).saturating_sub(h.len()).div_ceil(up);
        let taps = j
            .checked_sub(up.saturating_mul(i_lo))
            .and_then(|k_hi| h.get(..=k_hi));
        let Some(taps) = taps else {
            frame.fill(0.0);
            continue;
        };
        for (c, y) in frame.iter_mut().enumerate() {
            let inputs = x.chunks_exact(channels).skip(i_lo);
            *y = taps
                .iter()
                .rev()
                .step_by(up)
                .zip(inputs)
                .fold(0.0, |acc, (&tap, input)| {
                    input.get(c).map_or(acc, |&v| v.mul_add(tap, acc))
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::upfirdn;

    /// Direct evaluation of the sum, in the same order.
    fn reference(h: &[f64], x: &[f64], up: usize, j: usize) -> f64 {
        let mut acc = 0.0_f64;
        for (i, &v) in x.iter().enumerate() {
            if let Some(&tap) = j.checked_sub(up * i).and_then(|k| h.get(k)) {
                acc = v.mul_add(tap, acc);
            }
        }
        acc
    }

    #[test]
    fn matches_the_sum_for_every_frame_and_offset() {
        let h: Vec<f64> = (0..7).map(|k| f64::from(k).mul_add(0.3, -0.9)).collect();
        let x: Vec<f64> = (0..10).map(|i| (f64::from(i) * 1.7).sin()).collect();
        for up in 1..5 {
            let full = (x.len() - 1) * up + h.len();
            let mut out = [f64::NAN; 64];
            upfirdn(&h, &x, 1, up, 0, &mut out);
            for (j, y) in out.iter().enumerate() {
                let expected = if j < full {
                    reference(&h, &x, up, j)
                } else {
                    0.0
                };
                assert_eq!(y.to_bits(), expected.to_bits(), "up {up}, frame {j}");
            }
            let mut tail = [f64::NAN; 5];
            upfirdn(&h, &x, 1, up, 3, &mut tail);
            assert_eq!(tail.first(), out.get(3), "up {up}: offset window");
            assert_eq!(tail.last(), out.get(7), "up {up}: offset window");
        }
    }

    #[test]
    fn channels_are_filtered_independently() {
        let h = [0.25, 0.5, 1.0, 0.5];
        let interleaved = [1.0, -1.0, 2.0, 0.5, -3.0, 4.0];
        let mut stereo = [0.0; 16];
        upfirdn(&h, &interleaved, 2, 2, 0, &mut stereo);
        for c in 0..2 {
            let mono: Vec<f64> = interleaved.iter().skip(c).step_by(2).copied().collect();
            let mut out = [0.0; 8];
            upfirdn(&h, &mono, 1, 2, 0, &mut out);
            let channel: Vec<f64> = stereo.iter().skip(c).step_by(2).copied().collect();
            assert_eq!(channel, out);
        }
    }

    #[test]
    fn degenerate_arguments_write_nothing_or_zeros() {
        let mut out = [7.0; 4];
        upfirdn(&[1.0], &[1.0], 0, 2, 0, &mut out);
        upfirdn(&[1.0], &[1.0], 1, 0, 0, &mut out);
        assert_eq!(out, [7.0; 4]);
        upfirdn(&[], &[1.0], 1, 2, 0, &mut out);
        assert_eq!(out, [0.0; 4]);
    }
}
