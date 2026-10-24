//! Not real-time: the Analyzer's true-peak interpolator (see the parent module).

use crate::tables::{Oversampling, true_peak_taps};

use super::report::python_max;

/// The prototype's `_true_peak`: `upfirdn(taps, [history; block], up)` over the block's output
/// frames, 25 history rows carried across blocks.
///
/// It is evaluated polyphase, one contiguous window per output sample, instead of through
/// [`upfirdn`](crate::filters::upfirdn): the same sum in the same order (from 0, ascending input
/// frames, a fused multiply-add per tap, F7), so the values are identical, which the unit test
/// checks against `upfirdn`. Measured in a debug build: `upfirdn` took 9.6 s per million stereo
/// frames, which made the golden tests' 47 million frames take minutes (QG-11).
#[derive(Clone, Debug)]
pub(super) struct TruePeak {
    factor: usize,
    /// Per phase `p`: the taps `h[p], h[p + up], …`, reversed, so that they pair with ascending
    /// input frames.
    phases: Vec<Vec<f64>>,
    /// History frames (`len(taps) // up + 1`).
    rows: usize,
    /// Per channel: the history followed by the block.
    signal: Vec<Vec<f64>>,
    pub(super) peak: f64,
}

impl TruePeak {
    /// 4× below 88.2 kHz, 2× below 176.4 kHz, otherwise the sample peak.
    pub(super) fn new(sample_rate: u32, channels: usize) -> Self {
        let (factor, taps): (usize, &'static [f64]) = if sample_rate < 88200 {
            (4, true_peak_taps(Oversampling::X4))
        } else if sample_rate < 176_400 {
            (2, true_peak_taps(Oversampling::X2))
        } else {
            (1, &[])
        };
        Self::with_taps(factor, taps, channels)
    }

    #[expect(
        clippy::disallowed_macros,
        reason = "not real-time: sizes the analysis buffers"
    )]
    fn with_taps(factor: usize, taps: &[f64], channels: usize) -> Self {
        let phases = (0..factor)
            .map(|p| taps.iter().skip(p).step_by(factor).rev().copied().collect())
            .collect();
        let rows = if factor > 1 {
            taps.len() / factor + 1
        } else {
            0
        };
        Self {
            factor,
            phases,
            rows,
            signal: vec![vec![0.0; rows]; channels],
            peak: 0.0,
        }
    }

    /// Adds one interleaved block.
    pub(super) fn feed(&mut self, x: &[f64], channels: usize) {
        if self.factor <= 1 {
            let block_peak = x.iter().map(|v| v.abs()).fold(0.0, f64::max);
            self.peak = python_max(self.peak, block_peak);
            return;
        }
        let frames = x.len() / channels.max(1);
        let mut block_peak: Option<f64> = None;
        for (c, signal) in self.signal.iter_mut().enumerate() {
            signal.extend(
                x.chunks_exact(channels)
                    .map(|f| f.get(c).copied().unwrap_or(0.0)),
            );
            for m in self.rows..self.rows + frames {
                let peak = self
                    .phases
                    .iter()
                    .map(|taps| interpolate(signal, m, taps).abs())
                    .fold(0.0, f64::max);
                block_peak = Some(block_peak.map_or(peak, |p| p.max(peak)));
            }
            // Keep the last `rows` frames.
            let drop = signal.len() - self.rows;
            signal.drain(..drop);
        }
        if let Some(block_peak) = block_peak {
            self.peak = python_max(self.peak, block_peak);
        }
    }
}

/// Output sample `up · m + p` of `upfirdn`: the window of input frames ending at `m` against the
/// reversed taps of phase `p`.
fn interpolate(signal: &[f64], m: usize, taps: &[f64]) -> f64 {
    let start = (m + 1).saturating_sub(taps.len());
    signal.get(start..=m).map_or(0.0, |window| {
        window
            .iter()
            .zip(taps)
            .fold(0.0, |acc, (&v, &h)| v.mul_add(h, acc))
    })
}

#[cfg(test)]
mod tests {
    use super::{TruePeak, interpolate};
    use crate::filters::upfirdn;
    use crate::tables::{Oversampling, true_peak_taps};

    #[test]
    fn polyphase_windows_equal_upfirdn() {
        for (os, up) in [(Oversampling::X4, 4), (Oversampling::X2, 2)] {
            let taps = true_peak_taps(os);
            let meter = TruePeak::with_taps(up, taps, 1);
            let x: Vec<f64> = (0..200)
                .map(|i| (f64::from(i) * 0.731).sin() * 1.3)
                .collect();
            let first = meter.rows * up;
            let frames = (x.len() - meter.rows) * up;
            let mut reference: Vec<f64> = (0..frames).map(|_| 0.0).collect();
            upfirdn(taps, &x, 1, up, first, &mut reference);
            let polyphase: Vec<f64> = (meter.rows..x.len())
                .flat_map(|m| meter.phases.iter().map(move |t| (m, t)))
                .map(|(m, t)| interpolate(&x, m, t))
                .collect();
            let bits = |v: &[f64]| v.iter().map(|s| s.to_bits()).collect::<Vec<_>>();
            assert_eq!(bits(&polyphase), bits(&reference), "{up}x");
        }
    }

    #[test]
    fn the_peak_is_block_size_invariant() {
        let x: Vec<f64> = (0..6000).map(|i| (f64::from(i) * 0.9).sin()).collect();
        let mut whole = TruePeak::new(48000, 2);
        whole.feed(&x, 2);
        let mut pieces = TruePeak::new(48000, 2);
        for piece in x.chunks(2 * 7) {
            pieces.feed(piece, 2);
        }
        assert_eq!(whole.peak.to_bits(), pieces.peak.to_bits());
        assert!(
            whole.peak > 1.0,
            "the interpolated peak exceeds the samples"
        );
        let mut high = TruePeak::new(192_000, 2);
        high.feed(&[0.5, -0.75], 2);
        assert_eq!(high.peak.to_bits(), 0.75_f64.to_bits());
    }
}
