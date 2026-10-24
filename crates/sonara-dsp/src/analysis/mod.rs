//! Not real-time: the Analyzer runs on the render service's and the export's worker threads,
//! never on the audio render thread (see docs/audio-engine.md). It accumulates results per
//! block, so [`Analyzer::feed`] and [`Analyzer::finish`] allocate for their accumulators; the
//! functions that do say so with a per-item lint expectation. It lives in this crate because the
//! export and the preview use it next to the processors.
//!
//! Streaming audio analysis: the prototype's `Analyzer` and `AnalysisReport`
//! (`dsp/analysis/meter.py`), computed in one pass over audio of any length fed in blocks of any
//! size (`meter.py`). The state carries across blocks exactly as
//! in the prototype, whose per-block accumulations make some values depend on the block size
//! (the RMS sums, the stereo dot products); the render service and `analyze_file` feed 65536
//! frames.
//!
//! - **Levels:** sample peak; per channel `Σx²` and `Σx` per block (sequential per column for
//!   two or more channels, pairwise for one: NumPy's reduction order, F9), RMS, DC offset with
//!   NumPy's `round(6)`; samples above full scale; NaN and infinite samples, counted and then
//!   treated as 0; clipping runs of 3 or more frames at or above 0.999, carried across blocks.
//! - **Loudness:** K-weighting ([`k_weighting_sos`], `sosfilt` with carried state), 100 ms
//!   sub-blocks (a partial last one is dropped), momentary, short-term, gated integrated
//!   loudness and the loudness range with NumPy's percentile.
//! - **True peak:** 4× oversampling below 88.2 kHz, 2× below 176.4 kHz, none above: `SciPy`'s
//!   `upfirdn` (evaluated polyphase with the same sums) with 25 history rows, so it is
//!   block-size invariant; it is not flushed at the end.
//! - **Silence:** 50 ms RMS frames of the mono sum below −60 dBFS for at least 0.5 s.
//! - **Spectrum:** Welch on the mono sum: periodic Hann of 8192 points, hop 4096,
//!   `|rfft|²` averaged; octave bands from 31.5 Hz to 16 kHz and the centroid.
//! - **Stereo:** correlation over the signal and its minimum over 400 ms windows, side-to-mid
//!   and balance, with the recording host's BLAS dot order (see the `stereo` module).
//!
//! The FFT is `realfft`'s, which agrees with NumPy's pocketfft to the last ulps, so the
//! spectrum values are not bit-exact; everything else follows the prototype's arithmetic and
//! differs only where a platform's libm differs from macOS libm. The golden test documents
//! the tolerances (`tests/golden_analysis.rs`).

mod loudness;
mod report;
mod spectrum;
mod stereo;
mod true_peak;

pub use loudness::k_weighting_sos;
pub use report::{AnalysisReport, Issue, IssueCode, Severity};

use std::fmt;

use crate::numpy::math::{mean, round, sum};

use loudness::{LoudnessMeter, len_f64};
use report::python_max;
use spectrum::SpectrumMeter;
use stereo::StereoMeter;
use true_peak::TruePeak;

/// Frames at or above this absolute level count as clipped.
const CLIP_LEVEL: f64 = 0.999;
/// Clipped frames in a row that make a clipping event.
const CLIP_RUN: u64 = 3;
/// Silence threshold of the 50 ms RMS frames, dBFS.
const SILENCE_DB: f64 = -60.0;
/// Shortest silence region, seconds.
const SILENCE_MIN_SECONDS: f64 = 0.5;

/// Error of [`Analyzer::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatError {
    /// The requested sample rate in Hz.
    pub sample_rate: u32,
    /// The requested channel count.
    pub channels: usize,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot analyze {} channels at {} Hz",
            self.channels, self.sample_rate
        )
    }
}

impl std::error::Error for FormatError {}

/// The streaming analyzer: feed interleaved `(frames, channels)` blocks with
/// [`feed`](Self::feed), then call [`finish`](Self::finish), as with the prototype's
/// `Analyzer`.
#[derive(Clone, Debug)]
pub struct Analyzer {
    sample_rate: u32,
    channels: usize,
    frames: u64,
    peak: f64,
    sum_sq: Vec<f64>,
    sum: Vec<f64>,
    nonfinite: u64,
    over: u64,
    clipped: u64,
    clip_events: u64,
    run: u64,
    true_peak: TruePeak,
    loudness: LoudnessMeter,
    silence: SilenceMeter,
    spectrum: SpectrumMeter,
    stereo: Option<StereoMeter>,
    /// The block with non-finite samples replaced by 0.
    clean: Vec<f64>,
    /// The block's mono sum, `x.mean(axis=1)`.
    mono: Vec<f64>,
    /// Squares of a mono block, for the pairwise sums.
    squares: Vec<f64>,
}

impl Analyzer {
    /// An analyzer for `channels` interleaved channels at `sample_rate`.
    ///
    /// # Errors
    ///
    /// [`FormatError`] for zero channels, or for a rate below 11 Hz, where the prototype's
    /// 50 ms frames would be empty (it raises `ZeroDivisionError`).
    #[expect(
        clippy::disallowed_macros,
        reason = "not real-time: sizes the analysis buffers"
    )]
    pub fn new(sample_rate: u32, channels: usize) -> Result<Self, FormatError> {
        let error = FormatError {
            sample_rate,
            channels,
        };
        let hop = rounded_frames(0.1, sample_rate);
        let silence_hop = rounded_frames(0.05, sample_rate);
        if channels == 0 || hop == 0 || silence_hop == 0 {
            return Err(error);
        }
        Ok(Self {
            sample_rate,
            channels,
            frames: 0,
            peak: 0.0,
            sum_sq: vec![0.0; channels],
            sum: vec![0.0; channels],
            nonfinite: 0,
            over: 0,
            clipped: 0,
            clip_events: 0,
            run: 0,
            true_peak: TruePeak::new(sample_rate, channels),
            loudness: LoudnessMeter::new(sample_rate, channels, hop),
            silence: SilenceMeter::new(silence_hop),
            spectrum: SpectrumMeter::new(),
            stereo: (channels >= 2).then(|| StereoMeter::new(rounded_frames(0.4, sample_rate))),
            clean: Vec::new(),
            mono: Vec::new(),
            squares: Vec::new(),
        })
    }

    /// Sample rate in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Channels per frame.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Analyzes the next interleaved block. A trailing partial frame is ignored; an empty block
    /// changes nothing.
    pub fn feed(&mut self, block: &[f64]) {
        let channels = self.channels;
        let frames = block.len() / channels;
        let Some(block) = block.get(..frames * channels) else {
            return;
        };
        if frames == 0 {
            return;
        }
        self.clean.clear();
        self.clean.extend_from_slice(block);
        for v in &mut self.clean {
            if !v.is_finite() {
                self.nonfinite += 1;
                *v = 0.0;
            }
        }
        self.frames += u64::try_from(frames).unwrap_or(u64::MAX);
        let block_peak = self.clean.iter().map(|v| v.abs()).fold(0.0, f64::max);
        self.peak = python_max(self.peak, block_peak);
        self.accumulate_sums();
        let over = self.clean.iter().filter(|v| v.abs() > 1.0).count();
        self.over += u64::try_from(over).unwrap_or(u64::MAX);
        self.clip_runs();
        self.true_peak.feed(&self.clean, channels);
        self.loudness.feed(&self.clean);
        self.mono.clear();
        self.mono
            .extend(self.clean.chunks_exact(channels).map(mean));
        self.silence.feed(&self.mono);
        self.spectrum.feed(&self.mono);
        if let Some(stereo) = self.stereo.as_mut() {
            stereo.feed(&self.clean, channels);
        }
    }

    /// `np.sum(x * x, axis=0)` and `np.sum(x, axis=0)`, added to the running sums.
    fn accumulate_sums(&mut self) {
        let channels = self.channels;
        if channels == 1 {
            // A `(n, 1)` reduction drops the length-1 axis and sums pairwise.
            self.squares.clear();
            self.squares.extend(self.clean.iter().map(|v| v * v));
            for (sq, s) in self.sum_sq.iter_mut().zip(self.sum.iter_mut()) {
                *sq += sum(&self.squares);
                *s += sum(&self.clean);
            }
            return;
        }
        for (c, (sq, s)) in self.sum_sq.iter_mut().zip(self.sum.iter_mut()).enumerate() {
            let column = self.clean.chunks_exact(channels).filter_map(|f| f.get(c));
            let (block_sq, block_sum) = column.fold((0.0, 0.0), |(q, t), v| (q + v * v, t + v));
            *sq += block_sq;
            *s += block_sum;
        }
    }

    /// The prototype's `_clip_runs`, frame by frame (the run-length encoding with carry-over
    /// counts the same events).
    fn clip_runs(&mut self) {
        for frame in self.clean.chunks_exact(self.channels) {
            let level = frame.iter().map(|v| v.abs()).fold(0.0, f64::max);
            if level >= CLIP_LEVEL {
                self.clipped += 1;
                self.run += 1;
            } else {
                if self.run >= CLIP_RUN {
                    self.clip_events += 1;
                }
                self.run = 0;
            }
        }
    }

    /// The report of everything fed so far (the prototype's `finish`). The analyzer is left
    /// unchanged, so feeding can continue.
    #[must_use]
    pub fn finish(&self) -> AnalysisReport {
        let frames = len_u64(self.frames.max(1));
        let sr = f64::from(self.sample_rate);
        let loudness = self.loudness.measure();
        let rms = (mean(&self.sum_sq) / frames).sqrt();
        let tp = python_max(self.true_peak.peak, self.peak);
        let silence = self.silence.regions(sr);
        let (octave_bands_db, spectral_centroid_hz) = self.spectrum.bands(self.sample_rate);
        let stereo = self.stereo.as_ref();
        let correlation = stereo
            .filter(|s| s.ll > 1e-12 && s.rr > 1e-12)
            .map(|s| s.lr / (s.ll * s.rr).sqrt());
        let min_correlation_400ms = stereo.and_then(|s| {
            s.windows
                .iter()
                .copied()
                .reduce(|m, v| if v < m { v } else { m })
        });
        let side_to_mid_db = stereo.map(|s| {
            if s.mid > 0.0 && s.side > 0.0 {
                10.0 * (s.side / s.mid).log10()
            } else {
                -120.0
            }
        });
        let balance_db = stereo
            .filter(|s| s.ll > 0.0 && s.rr > 0.0)
            .map(|s| 10.0 * (s.ll / s.rr).log10());
        AnalysisReport {
            sample_rate: self.sample_rate,
            channels: self.channels,
            duration_seconds: len_u64(self.frames) / sr,
            sample_peak_dbfs: db(self.peak),
            true_peak_dbtp: db(tp),
            rms_dbfs: db(rms),
            dc_offset: self.sum.iter().map(|s| round(s / frames, 6)).collect(),
            crest_factor_db: if rms > 0.0 {
                db(self.peak) - db(rms)
            } else {
                0.0
            },
            integrated_lufs: loudness.integrated,
            momentary_max_lufs: loudness.momentary_max,
            short_term_max_lufs: loudness.short_term_max,
            loudness_range_lu: loudness.range,
            plr_db: if loudness.integrated.is_finite() && tp > 0.0 {
                db(tp) - loudness.integrated
            } else {
                0.0
            },
            clipped_samples: self.clipped,
            clip_events: self.clip_events + u64::from(self.run >= CLIP_RUN),
            over_full_scale_samples: self.over,
            nonfinite_samples: self.nonfinite,
            silence_regions: silence.regions,
            leading_silence_seconds: silence.leading,
            trailing_silence_seconds: silence.trailing,
            correlation,
            min_correlation_400ms,
            side_to_mid_db,
            balance_db,
            octave_bands_db,
            spectral_centroid_hz,
            short_term_lufs: loudness.series,
        }
    }
}

/// Analyzes interleaved `audio` fed in blocks of `block` frames: the prototype's `analyze`
/// (default block 65536, as the render service's `analyze_file`).
///
/// # Errors
///
/// The errors of [`Analyzer::new`].
pub fn analyze(
    audio: &[f64],
    sample_rate: u32,
    channels: usize,
    block: usize,
) -> Result<AnalysisReport, FormatError> {
    let mut analyzer = Analyzer::new(sample_rate, channels)?;
    for piece in audio.chunks(block.max(1).saturating_mul(channels)) {
        analyzer.feed(piece);
    }
    Ok(analyzer.finish())
}

/// `20 log10(x)`, `-inf` for `x <= 0` (the prototype's `_db`).
fn db(x: f64) -> f64 {
    if x > 0.0 {
        20.0 * x.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// Python's `round(seconds * sample_rate)` as a frame count.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a non-negative, integral value far below usize::MAX"
)]
fn rounded_frames(seconds: f64, sample_rate: u32) -> usize {
    (seconds * f64::from(sample_rate)).round_ties_even() as usize
}

/// A frame count as a float, exactly below 2^53.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts are far below 2^53, exactly representable"
)]
fn len_u64(count: u64) -> f64 {
    count as f64
}

/// The 50 ms RMS frames of the mono sum.
#[derive(Clone, Debug)]
struct SilenceMeter {
    hop: usize,
    /// Squares of the incomplete frame.
    pending: Vec<f64>,
    /// RMS per complete frame.
    frames: Vec<f64>,
}

/// Silence results.
struct Silence {
    regions: Vec<(f64, f64)>,
    leading: f64,
    trailing: f64,
}

impl SilenceMeter {
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: sizes the analysis buffers"
    )]
    fn new(hop: usize) -> Self {
        Self {
            hop,
            pending: Vec::with_capacity(hop),
            frames: Vec::new(),
        }
    }

    /// The prototype's `_silence`: `sqrt(mean(seg²))` per frame, pairwise over the frame.
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: appends to the analysis accumulator"
    )]
    fn feed(&mut self, mono: &[f64]) {
        for v in mono {
            self.pending.push(v * v);
            if self.pending.len() == self.hop {
                self.frames.push(mean(&self.pending).sqrt());
                self.pending.clear();
            }
        }
    }

    /// The prototype's silence regions, leading and trailing silence.
    #[expect(
        clippy::disallowed_methods,
        reason = "not real-time: collects the regions on the analysis thread"
    )]
    fn regions(&self, sr: f64) -> Silence {
        let threshold = 10.0_f64.powf(SILENCE_DB / 20.0);
        let hop_s = len_f64(self.hop) / sr;
        let mut regions = Vec::new();
        let mut start: Option<usize> = None;
        for (i, &r) in self.frames.iter().enumerate() {
            if r < threshold {
                start = start.or(Some(i));
            } else if let Some(s) = start.take()
                && len_f64(i - s) * hop_s >= SILENCE_MIN_SECONDS
            {
                regions.push((len_f64(s) * hop_s, len_f64(i) * hop_s));
            }
        }
        let count = self.frames.len();
        if let Some(s) = start
            && len_f64(count - s) * hop_s >= SILENCE_MIN_SECONDS
        {
            regions.push((len_f64(s) * hop_s, len_f64(count) * hop_s));
        }
        let audible = |r: &f64| *r >= threshold;
        let leading = self.frames.iter().position(audible).unwrap_or(count);
        let trailing = self.frames.iter().rev().position(audible).unwrap_or(count);
        Silence {
            regions,
            leading: len_f64(leading) * hop_s,
            trailing: len_f64(trailing) * hop_s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Analyzer, FormatError, analyze};

    fn tone(frames: usize, amplitude: f64) -> Vec<f64> {
        (0..frames)
            .flat_map(|i| {
                let t = f64::from(u32::try_from(i).unwrap()) / 48000.0;
                let v = amplitude * (2.0 * std::f64::consts::PI * 997.0 * t).sin();
                [v, v]
            })
            .collect()
    }

    #[test]
    fn rejects_unusable_formats() {
        assert_eq!(
            Analyzer::new(48000, 0).unwrap_err(),
            FormatError {
                sample_rate: 48000,
                channels: 0
            }
        );
        assert!(Analyzer::new(10, 1).is_err());
        assert!(Analyzer::new(11, 1).is_ok());
    }

    #[test]
    fn block_size_does_not_change_counts_and_peaks() {
        let mut x = tone(48000, 1.2);
        if let Some(v) = x.get_mut(101) {
            *v = f64::NAN;
        }
        let whole = analyze(&x, 48000, 2, 65536).unwrap();
        let pieces = analyze(&x, 48000, 2, 777).unwrap();
        assert_eq!(whole.nonfinite_samples, 1);
        assert_eq!(whole.clipped_samples, pieces.clipped_samples);
        assert_eq!(whole.clip_events, pieces.clip_events);
        assert_eq!(
            whole.over_full_scale_samples,
            pieces.over_full_scale_samples
        );
        assert_eq!(whole.sample_peak_dbfs, pieces.sample_peak_dbfs);
        assert_eq!(whole.true_peak_dbtp, pieces.true_peak_dbtp);
        assert_eq!(whole.short_term_lufs, pieces.short_term_lufs);
        assert_eq!(whole.silence_regions, pieces.silence_regions);
        assert!(whole.true_peak_dbtp >= whole.sample_peak_dbfs);
        assert!(whole.clip_events > 0 && whole.over_full_scale_samples > 0);
    }

    #[test]
    fn a_full_scale_sine_is_near_minus_three_lufs() {
        // BS.1770: a 997 Hz sine at 0 dBFS in both channels reads about -0.0 LUFS per channel
        // pair summed, i.e. +3 dB over one channel's -3.01 LUFS.
        let report = analyze(&tone(48000 * 4, 1.0), 48000, 2, 65536).unwrap();
        assert!(
            (report.integrated_lufs - 0.0).abs() < 0.1,
            "{}",
            report.integrated_lufs
        );
        assert!((report.rms_dbfs + 3.0103).abs() < 1e-3);
        assert_eq!(report.correlation, Some(1.0));
        assert_eq!(report.side_to_mid_db, Some(-120.0));
        assert_eq!(report.silence_regions.len(), 0);
        assert_eq!(report.leading_silence_seconds, 0.0);
    }

    #[test]
    fn silence_is_reported_with_regions() {
        let zeros: Vec<f64> = std::iter::repeat_n(0.0, 2 * 48000).collect();
        let silent = analyze(&zeros, 48000, 2, 4096).unwrap();
        assert_eq!(silent.integrated_lufs, f64::NEG_INFINITY);
        assert_eq!(silent.silence_regions, [(0.0, 1.0)]);
        assert_eq!(silent.leading_silence_seconds, 1.0);
        assert_eq!(silent.trailing_silence_seconds, 1.0);
        assert_eq!(silent.correlation, None);
        assert_eq!(silent.crest_factor_db, 0.0);
        let mono = analyze(zeros.get(..4800).unwrap(), 48000, 1, 4096).unwrap();
        assert_eq!(mono.side_to_mid_db, None);
        assert_eq!(mono.dc_offset, [0.0]);
    }
}
