//! Not real-time: the reference analysis runs on a worker thread over a decoded file, never on
//! the audio render thread. It lives in this crate next to the decoder that feeds it.
//!
//! Descriptive analysis of a reference recording (SON-P15): a tempo estimate with its confidence
//! and alternatives, the energy over time, and a segmentation by energy changes. It describes the
//! recording; it extracts no melody, harmony or arrangement and cannot be used to copy them.
//!
//! The analyzer is streaming: [`ReferenceAnalyzer::feed`] takes interleaved samples in blocks of
//! any size and keeps only a 100 Hz RMS envelope of the mono mix, so a long file costs 800 bytes
//! per second. The result does not depend on the block size.
//!
//! - **Tempo:** the autocorrelation of the half-wave-rectified envelope change (the onset
//!   strength) over 60 to 200 BPM, refined by a parabola through the peak. Of the peaks within
//!   10 % of the strongest the shortest lag wins, so a steady pulse reads as its beat and not as
//!   a multiple. The confidence is the normalized autocorrelation at the peak, 0 to 1; below
//!   0.05 there is no tempo. Half and double the tempo are listed as alternatives. A constant
//!   tempo is assumed; at least [`MIN_TEMPO_SECONDS`] of audio are needed.
//! - **Energy:** the RMS of 0.5 s windows in dBFS of the mono mix (−120 for silence).
//! - **Segments:** boundaries where the mean energy of the next 2 s differs from the previous 2 s
//!   by at least 4 dB (the largest change within 2 s wins), with segments of at least 2 s. Each
//!   segment is labelled low, medium or high relative to the recording; a recording whose
//!   segments span less than 3 dB is all medium.

use serde_json::{Value, json};

/// The envelope's frame rate: one RMS value per 10 ms.
const HOPS_PER_SECOND: f64 = 100.0;
/// Shortest recording that gets any report.
pub const MIN_SECONDS: f64 = 2.0;
/// Shortest recording that gets a tempo estimate.
pub const MIN_TEMPO_SECONDS: f64 = 6.0;
/// The autocorrelation lags of the tempo range of 60 to 200 BPM, in hops (6000 / BPM).
const MIN_LAG: usize = 30;
const MAX_LAG: usize = 100;
/// Below this normalized autocorrelation there is no tempo.
const MIN_CONFIDENCE: f64 = 0.05;
/// Hops per energy window (0.5 s).
const WINDOW_HOPS: usize = 50;
/// Windows on each side of a boundary candidate (2 s).
const SIDE_WINDOWS: usize = 4;
/// The energy change that makes a boundary, in dB.
const BOUNDARY_DB: f64 = 4.0;
/// A recording whose segments span less than this is steady, in dB.
const STEADY_DB: f64 = 3.0;
/// `value` rounded half away from zero to `digits` decimals.
fn round_to(value: f64, digits: i32) -> f64 {
    let scale = 10_f64.powi(digits);
    (value * scale).round() / scale
}

/// The level of silence.
const FLOOR_DBFS: f64 = -120.0;

/// Why a reference cannot be analyzed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReferenceError {
    /// The sample rate is outside 8 kHz to 384 kHz.
    InvalidSampleRate(u32),
    /// The channel count is 0 or above 64.
    InvalidChannels(usize),
    /// The recording has no complete 10 ms frame.
    Empty,
    /// The recording is shorter than [`MIN_SECONDS`]; carries its length.
    TooShort(f64),
}

impl std::fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSampleRate(rate) => {
                write!(f, "sample rate {rate} Hz is outside 8000 to 384000")
            }
            Self::InvalidChannels(n) => write!(f, "{n} channels is outside 1 to 64"),
            Self::Empty => f.write_str("the recording has no audio"),
            Self::TooShort(seconds) => {
                write!(
                    f,
                    "the recording is {seconds:.2} s; at least {MIN_SECONDS} s are needed"
                )
            }
        }
    }
}

impl std::error::Error for ReferenceError {}

/// A tempo estimate.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoEstimate {
    /// The estimate in BPM.
    pub bpm: f64,
    /// Normalized autocorrelation at the estimate, 0 to 1: how regular the pulse is, not how
    /// likely the estimate is musically correct.
    pub confidence: f64,
    /// Half and double the estimate, whichever lie within 30 to 300 BPM: a pulse has no one
    /// right octave.
    pub alternatives_bpm: Vec<f64>,
}

/// How energetic a segment is relative to the rest of the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyLevel {
    /// The quietest third of the range.
    Low,
    /// The middle third, or every segment of a steady recording.
    Medium,
    /// The loudest third of the range.
    High,
}

impl EnergyLevel {
    /// The level's name in the JSON report.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// A stretch of the recording with a similar energy.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Where it starts, in seconds.
    pub start_seconds: f64,
    /// Where it ends, in seconds.
    pub end_seconds: f64,
    /// Its RMS level in dBFS.
    pub mean_rms_dbfs: f64,
    /// Its energy relative to the other segments.
    pub energy: EnergyLevel,
}

/// The result of [`ReferenceAnalyzer::finish`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceReport {
    /// The analyzed length (whole 10 ms frames), in seconds.
    pub duration_seconds: f64,
    /// The sample rate of the input.
    pub sample_rate: u32,
    /// The RMS level of the whole recording in dBFS.
    pub mean_rms_dbfs: f64,
    /// The tempo, `None` when the recording is too short or has no regular pulse.
    pub tempo: Option<TempoEstimate>,
    /// The segments in order; they cover the recording.
    pub segments: Vec<Segment>,
    /// The RMS level of every 0.5 s window in dBFS.
    pub energy_dbfs: Vec<f64>,
    /// Samples that were NaN or infinite and counted as 0.
    pub non_finite_samples: u64,
    /// What the analysis does not do, and why the tempo is missing if it is.
    pub limitations: Vec<String>,
}

impl ReferenceReport {
    /// The report as JSON: levels rounded to 2 decimals, tempo to 1, times to 2.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let tempo = self.tempo.as_ref().map_or(Value::Null, |t| {
            json!({
                "bpm": round_to(t.bpm, 1),
                "confidence": round_to(t.confidence, 2),
                "alternatives_bpm": t.alternatives_bpm.iter().map(|b| round_to(*b, 1)).collect::<Vec<_>>(),
            })
        });
        let segments: Vec<Value> = self
            .segments
            .iter()
            .map(|s| {
                json!({
                    "start_seconds": round_to(s.start_seconds, 2),
                    "end_seconds": round_to(s.end_seconds, 2),
                    "mean_rms_dbfs": round_to(s.mean_rms_dbfs, 2),
                    "energy": s.energy.as_str(),
                })
            })
            .collect();
        json!({
            "duration_seconds": round_to(self.duration_seconds, 2),
            "sample_rate": self.sample_rate,
            "mean_rms_dbfs": round_to(self.mean_rms_dbfs, 2),
            "tempo": tempo,
            "segments": segments,
            "energy_dbfs": self.energy_dbfs.iter().map(|e| round_to(*e, 2)).collect::<Vec<_>>(),
            "non_finite_samples": self.non_finite_samples,
            "limitations": self.limitations,
        })
    }
}

/// The streaming reference analyzer.
#[derive(Debug)]
pub struct ReferenceAnalyzer {
    sample_rate: u32,
    channels: usize,
    hop_frames: usize,
    /// Sum of squares of the mono mix in the current hop, and its frame count.
    sum_squares: f64,
    frames_in_hop: usize,
    /// The RMS of each complete hop.
    envelope: Vec<f64>,
    non_finite: u64,
}

impl ReferenceAnalyzer {
    /// An analyzer for audio of `sample_rate` Hz with `channels` interleaved channels. Setup:
    /// allocates.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::InvalidSampleRate`] outside 8 kHz to 384 kHz and
    /// [`ReferenceError::InvalidChannels`] for 0 or more than 64 channels.
    pub fn new(sample_rate: u32, channels: usize) -> Result<Self, ReferenceError> {
        if !(8_000..=384_000).contains(&sample_rate) {
            return Err(ReferenceError::InvalidSampleRate(sample_rate));
        }
        if !(1..=64).contains(&channels) {
            return Err(ReferenceError::InvalidChannels(channels));
        }
        let hop_frames = hop_frames(sample_rate);
        Ok(Self {
            sample_rate,
            channels,
            hop_frames,
            sum_squares: 0.0,
            frames_in_hop: 0,
            envelope: Vec::new(),
            non_finite: 0,
        })
    }

    /// Adds interleaved samples; a trailing partial frame is ignored. NaN and infinite samples
    /// count as 0 and are reported.
    pub fn feed(&mut self, interleaved: &[f64]) {
        for frame in interleaved.chunks_exact(self.channels) {
            let mut sum = 0.0;
            for &sample in frame {
                if sample.is_finite() {
                    sum += sample;
                } else {
                    self.non_finite += 1;
                }
            }
            let mono = sum / count(self.channels);
            self.sum_squares += mono * mono;
            self.frames_in_hop += 1;
            if self.frames_in_hop == self.hop_frames {
                self.envelope
                    .push((self.sum_squares / count(self.hop_frames)).sqrt());
                self.sum_squares = 0.0;
                self.frames_in_hop = 0;
            }
        }
    }

    /// The report. A partial last 10 ms hop is dropped.
    ///
    /// # Errors
    ///
    /// [`ReferenceError::Empty`] without a complete hop, [`ReferenceError::TooShort`] under
    /// [`MIN_SECONDS`].
    pub fn finish(self) -> Result<ReferenceReport, ReferenceError> {
        if self.envelope.is_empty() {
            return Err(ReferenceError::Empty);
        }
        let duration = count(self.envelope.len()) / HOPS_PER_SECOND;
        if duration < MIN_SECONDS {
            return Err(ReferenceError::TooShort(duration));
        }
        let energy = window_energy(&self.envelope);
        let segments = segment(&energy, duration);
        let mean_power =
            self.envelope.iter().map(|e| e * e).sum::<f64>() / count(self.envelope.len());
        let mut limitations = vec![
            "Descriptive only: no melody, harmony or arrangement is extracted.".to_owned(),
            "Segments follow energy changes, not musical form.".to_owned(),
        ];
        let tempo = if duration < MIN_TEMPO_SECONDS {
            limitations.push(format!(
                "No tempo: the recording is shorter than {MIN_TEMPO_SECONDS} s."
            ));
            None
        } else {
            let tempo = estimate_tempo(&self.envelope);
            if tempo.is_none() {
                limitations.push("No tempo: the recording has no regular pulse.".to_owned());
            } else {
                limitations.push(
                    "Tempo assumes a constant pulse of 60 to 200 BPM; half or double may be the intended beat."
                        .to_owned(),
                );
            }
            tempo
        };
        if self.non_finite > 0 {
            limitations.push(format!(
                "{} NaN or infinite samples were counted as 0.",
                self.non_finite
            ));
        }
        Ok(ReferenceReport {
            duration_seconds: duration,
            sample_rate: self.sample_rate,
            mean_rms_dbfs: db(mean_power),
            tempo,
            segments,
            energy_dbfs: energy.iter().map(|power| db(*power)).collect(),
            non_finite_samples: self.non_finite,
            limitations,
        })
    }
}

/// A count as a float, exactly below 2^53.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts of frames and windows are far below 2^53"
)]
fn count(n: usize) -> f64 {
    n as f64
}

/// The frames of one 10 ms hop at `sample_rate`, at least 1.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rate of at most 384 kHz gives a small, positive count"
)]
fn hop_frames(sample_rate: u32) -> usize {
    ((f64::from(sample_rate) / HOPS_PER_SECOND).round() as usize).max(1)
}

/// 10·log10 of a power, floored at silence.
fn db(power: f64) -> f64 {
    if power > 0.0 {
        (10.0 * power.log10()).max(FLOOR_DBFS)
    } else {
        FLOOR_DBFS
    }
}

/// The mean power of each complete 0.5 s window; the rest of the envelope joins the last one.
fn window_energy(envelope: &[f64]) -> Vec<f64> {
    let windows = (envelope.len() / WINDOW_HOPS).max(1);
    (0..windows)
        .map(|i| {
            let start = i * WINDOW_HOPS;
            let end = if i + 1 == windows {
                envelope.len()
            } else {
                start + WINDOW_HOPS
            };
            let part = envelope.get(start..end).unwrap_or_default();
            part.iter().map(|e| e * e).sum::<f64>() / count(part.len().max(1))
        })
        .collect()
}

/// The mean of `energy` (powers) over `range`, in dB.
fn mean_db(energy: &[f64], start: usize, end: usize) -> f64 {
    let part = energy.get(start..end).unwrap_or_default();
    db(part.iter().sum::<f64>() / count(part.len().max(1)))
}

fn segment(energy: &[f64], duration: f64) -> Vec<Segment> {
    let windows = energy.len();
    let window_seconds = duration / count(windows);
    let novelty = |i: usize| {
        (mean_db(energy, i, i + SIDE_WINDOWS) - mean_db(energy, i - SIDE_WINDOWS, i)).abs()
    };
    let mut cuts = vec![0];
    if windows >= 2 * SIDE_WINDOWS {
        for i in SIDE_WINDOWS..=windows - SIDE_WINDOWS {
            let here = novelty(i);
            let peak = (i.saturating_sub(SIDE_WINDOWS).max(SIDE_WINDOWS)
                ..=(i + SIDE_WINDOWS).min(windows - SIDE_WINDOWS))
                .all(|j| novelty(j) <= here);
            let far_enough = cuts.last().is_some_and(|last| i - last >= SIDE_WINDOWS);
            if here >= BOUNDARY_DB && peak && far_enough && windows - i >= SIDE_WINDOWS {
                cuts.push(i);
            }
        }
    }
    cuts.push(windows);
    let levels: Vec<f64> = cuts
        .windows(2)
        .map(|w| mean_db(energy, w[0], w[1]))
        .collect();
    let low = levels.iter().copied().fold(f64::INFINITY, f64::min);
    let high = levels.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    cuts.windows(2)
        .zip(&levels)
        .map(|(w, &level)| {
            let energy_level = if high - low < STEADY_DB {
                EnergyLevel::Medium
            } else {
                let t = (level - low) / (high - low);
                if t < 1.0 / 3.0 {
                    EnergyLevel::Low
                } else if t > 2.0 / 3.0 {
                    EnergyLevel::High
                } else {
                    EnergyLevel::Medium
                }
            };
            Segment {
                start_seconds: count(w[0]) * window_seconds,
                end_seconds: if w[1] == windows {
                    duration
                } else {
                    count(w[1]) * window_seconds
                },
                mean_rms_dbfs: level,
                energy: energy_level,
            }
        })
        .collect()
}

fn estimate_tempo(envelope: &[f64]) -> Option<TempoEstimate> {
    let mut onset: Vec<f64> = envelope
        .windows(2)
        .map(|w| (w[1] - w[0]).max(0.0))
        .collect();
    let mean = onset.iter().sum::<f64>() / count(onset.len());
    for o in &mut onset {
        *o -= mean;
    }
    let (min_lag, max_lag) = (MIN_LAG, MAX_LAG);
    if onset.len() <= max_lag + 1 {
        return None;
    }
    // Normalized autocorrelation for the lags around the range, so that its edges have neighbours.
    let r: Vec<f64> = (min_lag - 1..=max_lag + 1)
        .map(|lag| {
            let (head, tail) = (&onset[..onset.len() - lag], &onset[lag..]);
            let cross: f64 = head.iter().zip(tail).map(|(a, b)| a * b).sum();
            let norm = (head.iter().map(|a| a * a).sum::<f64>()
                * tail.iter().map(|b| b * b).sum::<f64>())
            .sqrt();
            if norm > 0.0 { cross / norm } else { 0.0 }
        })
        .collect();
    let at = |lag: usize| r.get(lag - (min_lag - 1)).copied().unwrap_or(0.0);
    let peaks: Vec<usize> = (min_lag..=max_lag)
        .filter(|&lag| at(lag) > at(lag - 1) && at(lag) >= at(lag + 1))
        .collect();
    let best = peaks
        .iter()
        .map(|&lag| at(lag))
        .fold(f64::NEG_INFINITY, f64::max);
    if !best.is_finite() || best < MIN_CONFIDENCE {
        return None;
    }
    let lag = *peaks.iter().find(|&&lag| at(lag) >= 0.9 * best)?;
    let (left, mid, right) = (at(lag - 1), at(lag), at(lag + 1));
    let curve = left - 2.0 * mid + right;
    let shift = if curve < 0.0 {
        (0.5 * (left - right) / curve).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let bpm = HOPS_PER_SECOND * 60.0 / (count(lag) + shift);
    let alternatives_bpm = [bpm / 2.0, bpm * 2.0]
        .into_iter()
        .filter(|b| (30.0..=300.0).contains(b))
        .collect();
    Some(TempoEstimate {
        bpm,
        confidence: mid.clamp(0.0, 1.0),
        alternatives_bpm,
    })
}
