//! Not real-time: K-weighted loudness (BS.1770-4, EBU R128 and Tech 3342) for the Analyzer (see
//! the parent module).

use crate::filters::{Section, SosFilter};
use crate::numpy::math::{cumsum, mean, percentile, py_round, sum};

use super::report::python_max;

/// Absolute gate, LUFS.
const ABS_GATE: f64 = -70.0;
/// Relative gate of the integrated loudness, LU.
const REL_GATE_I: f64 = -10.0;
/// Relative gate of the loudness range, LU.
const REL_GATE_LRA: f64 = -20.0;
/// 100 ms sub-blocks per momentary (400 ms) block.
const MOMENTARY: usize = 4;
/// 100 ms sub-blocks per short-term (3 s) block.
const SHORT_TERM: usize = 30;
/// Sub-blocks between loudness-range blocks (1 s).
const LRA_STEP: usize = 10;
/// Short-term values between entries of the loudness series (1 s).
const SERIES_STEP: usize = 10;
/// The series' value for a block without energy.
const SERIES_SILENCE: f64 = -120.0;
/// Weight of channels 3 and up (surround) in the channel sum.
const SURROUND_WEIGHT: f64 = 1.41;

/// The BS.1770 K-weighting filter (pre-filter shelf, then the RLB high-pass) for any sample
/// rate, in closed form: the prototype's `k_weighting_sos`, with its constants and order of
/// operations. It calls `tan` and `pow`, so it is exact where the platform's libm agrees with
/// macOS libm; the container's glibc does at 48 kHz.
#[must_use]
pub fn k_weighting_sos(sample_rate: u32) -> [Section; 2] {
    let fs = f64::from(sample_rate);
    let (f0, gain_db, q) = (
        1_681.974_450_955_533,
        3.999_843_853_973_347,
        0.707_175_236_955_419_6,
    );
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let vh = 10.0_f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = [
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
        1.0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    ];
    let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let d = 1.0 + k / q + k * k;
    let highpass = [
        1.0,
        -2.0,
        1.0,
        1.0,
        2.0 * (k * k - 1.0) / d,
        (1.0 - k / q + k * k) / d,
    ];
    [shelf, highpass]
}

/// `-0.691 + 10 log10(energy)`, `-inf` without energy (the prototype's `_energy_to_lufs`).
pub(super) fn energy_to_lufs(energy: f64) -> f64 {
    if energy > 0.0 {
        -0.691 + 10.0 * energy.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// The K-weighted mean square per channel of every complete 100 ms sub-block.
#[derive(Clone, Debug)]
pub(super) struct LoudnessMeter {
    channels: usize,
    filter: SosFilter,
    /// The K-weighted block.
    weighted: Vec<f64>,
    /// Frames per sub-block (`round(0.1 · sr)`).
    hop: usize,
    /// The incomplete sub-block, interleaved.
    pending: Vec<f64>,
    /// Squares of one mono sub-block, for the pairwise mean.
    squares: Vec<f64>,
    /// Mean squares, `(sub-blocks, channels)`.
    energies: Vec<f64>,
}

impl LoudnessMeter {
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: sizes the analysis buffers"
    )]
    pub(super) fn new(sample_rate: u32, channels: usize, hop: usize) -> Self {
        let mut filter = SosFilter::new(channels, 2);
        // Two sections fit the capacity of two.
        let _fits = filter.set_sections(&k_weighting_sos(sample_rate));
        Self {
            channels,
            filter,
            weighted: Vec::new(),
            hop,
            pending: Vec::with_capacity(hop.saturating_mul(channels)),
            squares: Vec::with_capacity(hop),
            energies: Vec::new(),
        }
    }

    /// Filters `x` (`sosfilt` with the state carried) and closes every complete sub-block.
    pub(super) fn feed(&mut self, x: &[f64]) {
        self.weighted.resize(x.len(), 0.0);
        self.filter.process(x, &mut self.weighted);
        let full = self.hop.saturating_mul(self.channels);
        let weighted = std::mem::take(&mut self.weighted);
        let mut rest = weighted.as_slice();
        while !rest.is_empty() {
            let take = full.saturating_sub(self.pending.len()).min(rest.len());
            let (now, later) = rest.split_at_checked(take).unwrap_or((rest, &[]));
            self.pending.extend_from_slice(now);
            rest = later;
            if self.pending.len() == full {
                self.close_sub_block();
            }
        }
        self.weighted = weighted;
    }

    /// `np.mean(segs * segs, axis=1)` of one sub-block: sequential per channel for two or more
    /// channels, pairwise for one (NumPy drops the length-1 axis).
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: appends to the analysis accumulator"
    )]
    fn close_sub_block(&mut self) {
        let hop = len_f64(self.hop);
        if self.channels == 1 {
            self.squares.clear();
            self.squares.extend(self.pending.iter().map(|v| v * v));
            self.energies.push(mean(&self.squares));
        } else {
            for c in 0..self.channels {
                let total = self
                    .pending
                    .chunks_exact(self.channels)
                    .filter_map(|frame| frame.get(c))
                    .fold(0.0, |acc, v| acc + v * v);
                self.energies.push(total / hop);
            }
        }
        self.pending.clear();
    }

    /// The loudness values from the closed sub-blocks.
    pub(super) fn measure(&self) -> Loudness {
        measure(&self.energies, self.channels)
    }
}

/// The Analyzer's loudness results.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Loudness {
    pub(super) integrated: f64,
    pub(super) momentary_max: f64,
    pub(super) short_term_max: f64,
    pub(super) range: f64,
    pub(super) series: Vec<f64>,
}

/// The prototype's `_loudness` from the per-channel sub-block energies.
fn measure(energies: &[f64], channels: usize) -> Loudness {
    if energies.is_empty() || channels == 0 {
        return Loudness {
            integrated: f64::NEG_INFINITY,
            momentary_max: f64::NEG_INFINITY,
            short_term_max: f64::NEG_INFINITY,
            range: 0.0,
            series: Vec::new(),
        };
    }
    // `np.sum(sub_energy, axis=1)`: pairwise over each contiguous row. Above three channels the
    // prototype weights the surround channels with a BLAS matrix-vector product, whose order is
    // not reproduced here (no golden data has more than two channels).
    let sub: Vec<f64> = energies
        .chunks_exact(channels)
        .map(|row| {
            if channels > 3 {
                row.iter().enumerate().fold(0.0, |acc, (c, e)| {
                    acc + e * if c >= 3 { SURROUND_WEIGHT } else { 1.0 }
                })
            } else {
                sum(row)
            }
        })
        .collect();

    let momentary = blocks(&sub, MOMENTARY, 1);
    let absolute = 10.0_f64.powf((ABS_GATE + 0.691) / 10.0);
    let gated: Vec<f64> = momentary
        .iter()
        .copied()
        .filter(|&e| e > absolute)
        .collect();
    let integrated = if gated.is_empty() {
        f64::NEG_INFINITY
    } else {
        let relative = energy_to_lufs(mean(&gated)) + REL_GATE_I;
        let threshold = 10.0_f64.powf((relative + 0.691) / 10.0);
        let kept: Vec<f64> = gated.iter().copied().filter(|&e| e > threshold).collect();
        if kept.is_empty() {
            f64::NEG_INFINITY
        } else {
            energy_to_lufs(mean(&kept))
        }
    };
    let momentary_max = largest(&momentary).map_or(f64::NEG_INFINITY, energy_to_lufs);
    let short = blocks(&sub, SHORT_TERM, 1);
    let short_term_max = largest(&short).map_or(f64::NEG_INFINITY, energy_to_lufs);

    let mut range = 0.0;
    let lra: Vec<f64> = blocks(&sub, SHORT_TERM, LRA_STEP)
        .into_iter()
        .filter(|&e| e > 0.0)
        .map(energy_to_lufs)
        .filter(|&l| l > ABS_GATE)
        .collect();
    if !lra.is_empty() {
        let powers: Vec<f64> = lra
            .iter()
            .map(|l| 10.0_f64.powf((l + 0.691) / 10.0))
            .collect();
        let relative = energy_to_lufs(mean(&powers)) + REL_GATE_LRA;
        let mut kept: Vec<f64> = lra.into_iter().filter(|&l| l > relative).collect();
        if kept.len() >= 2
            && let (Some(high), Some(low)) =
                (percentile(&mut kept, 95.0), percentile(&mut kept, 10.0))
        {
            range = high - low;
        }
    }
    let series = short
        .iter()
        .step_by(SERIES_STEP)
        .map(|&e| {
            if e > 0.0 {
                py_round(energy_to_lufs(e), 2)
            } else {
                SERIES_SILENCE
            }
        })
        .collect();
    Loudness {
        integrated,
        momentary_max,
        short_term_max,
        range,
        series,
    }
}

/// The prototype's `blocks(length, step)`: means over `length` sub-blocks every `step`, from a
/// cumulative sum `(c[s + L] − c[s]) / L`; fewer sub-blocks than `length` give one block of
/// their (pairwise) mean.
#[expect(
    clippy::disallowed_methods,
    reason = "not real-time: builds the block series on the analysis thread"
)]
fn blocks(sub: &[f64], length: usize, step: usize) -> Vec<f64> {
    if sub.len() < length {
        return if sub.is_empty() {
            Vec::new()
        } else {
            Vec::from([mean(sub)])
        };
    }
    // `concatenate([[0.0], cumsum(sub)])`.
    let mut c = Vec::with_capacity(sub.len() + 1);
    c.push(0.0);
    c.extend_from_slice(sub);
    if let Some(tail) = c.get_mut(1..) {
        cumsum(tail);
    }
    let divisor = len_f64(length);
    let count = sub.len() - length + 1;
    (0..count)
        .step_by(step.max(1))
        .filter_map(|s| Some((c.get(s + length)? - c.get(s)?) / divisor))
        .collect()
}

/// NumPy's `max` of a non-empty array of non-NaN values.
fn largest(values: &[f64]) -> Option<f64> {
    values.iter().copied().reduce(python_max)
}

/// A count as a float, exactly for every count below 2^53.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame and block counts are far below 2^53, exactly representable"
)]
pub(super) fn len_f64(len: usize) -> f64 {
    len as f64
}

#[cfg(test)]
mod tests {
    use super::{blocks, energy_to_lufs, k_weighting_sos, measure};

    #[test]
    fn k_weighting_has_unit_leading_coefficients() {
        let [shelf, highpass] = k_weighting_sos(48000);
        assert_eq!(shelf[3], 1.0);
        assert_eq!(highpass[..4], [1.0, -2.0, 1.0, 1.0]);
        // The shelf gains about 4 dB at Nyquist.
        let gain_at_nyquist = (shelf[0] - shelf[1] + shelf[2]) / (shelf[3] - shelf[4] + shelf[5]);
        assert!((20.0 * gain_at_nyquist.log10() - 4.0).abs() < 0.1);
    }

    #[test]
    fn blocks_use_a_cumulative_sum_or_the_mean() {
        let sub = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(blocks(&sub, 2, 1), [1.5, 2.5, 3.5, 4.5]);
        assert_eq!(blocks(&sub, 2, 3), [1.5, 4.5]);
        assert_eq!(blocks(&sub, 9, 1), [3.0]);
        assert_eq!(blocks(&[], 4, 1).len(), 0);
    }

    #[test]
    fn silence_has_no_loudness() {
        let silent = measure(&[0.0; 80], 2);
        assert_eq!(silent.integrated, f64::NEG_INFINITY);
        assert_eq!(silent.momentary_max, f64::NEG_INFINITY);
        assert_eq!(silent.range, 0.0);
        // 40 sub-blocks give 11 short-term blocks; the series takes every tenth.
        assert_eq!(silent.series, [-120.0, -120.0]);
        assert_eq!(energy_to_lufs(0.0), f64::NEG_INFINITY);
        // A steady signal: every block equals the sub-block energy, the range is zero.
        let steady = measure(&[0.5; 400], 2);
        assert_eq!(steady.integrated, energy_to_lufs(1.0));
        assert_eq!(steady.range, 0.0);
    }
}
