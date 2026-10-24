//! Musical time: tempo map, time signatures and conversions (`sonara.domain.time`, ADR 0003).
//!
//! Timeline positions are `f64` quarter-note beats. The tempo map converts beats to seconds in
//! closed form; a segment has either a constant tempo or a linear tempo ramp (tempo linear in
//! beats), whose integral is logarithmic.

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

use crate::EPSILON;
use crate::error::{DomainError, ErrorCode, Result, require, require_finite};
use crate::pyfmt::{float_repr, str_repr};

/// Lowest tempo in BPM.
pub const MIN_BPM: f64 = 10.0;
/// Highest tempo in BPM.
pub const MAX_BPM: f64 = 999.0;
/// Allowed time signature denominators.
pub const VALID_DENOMINATORS: [u32; 6] = [1, 2, 4, 8, 16, 32];

/// Below this slope (BPM per beat) a segment counts as constant.
const SLOPE_EPSILON: f64 = 1e-12;

/// A tempo change at `beat`. With `ramp`, the tempo changes linearly (in beats) to the next
/// point's tempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoPoint {
    /// Position in beats (≥ 0).
    pub beat: f64,
    /// Tempo in BPM.
    pub bpm: f64,
    /// Whether the tempo ramps linearly to the next point.
    pub ramp: bool,
}

impl TempoPoint {
    /// A validated tempo point.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for a non-finite or negative beat, or a tempo outside
    /// [`MIN_BPM`]..=[`MAX_BPM`].
    pub fn new(beat: f64, bpm: f64, ramp: bool) -> Result<Self> {
        require_finite(beat, "tempo point beat")?;
        require_finite(bpm, "tempo")?;
        require(beat >= 0.0, || "tempo point beat must be >= 0".to_owned())?;
        require((MIN_BPM..=MAX_BPM).contains(&bpm), || {
            format!(
                "tempo must be within {}-{} BPM",
                float_repr(MIN_BPM),
                float_repr(MAX_BPM)
            )
        })?;
        Ok(Self { beat, bpm, ramp })
    }
}

/// The tempo map: one or more tempo points, the first at beat 0.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoMap {
    points: Vec<TempoPoint>,
    /// Cumulative seconds at each point.
    seconds: Vec<f64>,
}

impl TempoMap {
    /// A validated tempo map.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for an empty map, a first point not at beat 0, or points
    /// that are not strictly increasing in beat.
    pub fn new(points: Vec<TempoPoint>) -> Result<Self> {
        require(!points.is_empty(), || {
            "tempo map needs at least one point".to_owned()
        })?;
        require(points.first().is_some_and(|p| p.beat == 0.0), || {
            "first tempo point must be at beat 0".to_owned()
        })?;
        require(
            points.windows(2).all(|w| w[1].beat > w[0].beat + EPSILON),
            || "tempo points must be strictly increasing in beat".to_owned(),
        )?;
        let mut map = Self {
            points,
            seconds: Vec::new(),
        };
        let mut seconds = Vec::with_capacity(map.points.len());
        seconds.push(0.0);
        for i in 1..map.points.len() {
            let previous = seconds[i - 1];
            seconds.push(previous + map.segment_seconds(i - 1, map.points[i].beat));
        }
        map.seconds = seconds;
        Ok(map)
    }

    /// A constant tempo.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for a tempo outside the allowed range.
    pub fn constant(bpm: f64) -> Result<Self> {
        Self::new(vec![TempoPoint::new(0.0, bpm, false)?])
    }

    /// The tempo points, sorted by beat.
    #[must_use]
    pub fn points(&self) -> &[TempoPoint] {
        &self.points
    }

    /// BPM change per beat within segment `i` (0 for constant segments and the last segment).
    fn slope(&self, i: usize) -> f64 {
        let p = self.points[i];
        match self.points.get(i + 1) {
            Some(next) if p.ramp => (next.bpm - p.bpm) / (next.beat - p.beat),
            _ => 0.0,
        }
    }

    /// Seconds from the start of segment `i` to `beat`.
    fn segment_seconds(&self, i: usize, beat: f64) -> f64 {
        let p = self.points[i];
        let d = beat - p.beat;
        let k = self.slope(i);
        if k.abs() < SLOPE_EPSILON {
            60.0 * d / p.bpm
        } else {
            60.0 / k * ((p.bpm + k * d) / p.bpm).ln()
        }
    }

    fn segment_beats(&self, i: usize, seconds: f64) -> f64 {
        let p = self.points[i];
        let k = self.slope(i);
        if k.abs() < SLOPE_EPSILON {
            seconds * p.bpm / 60.0
        } else {
            (p.bpm * (k * seconds / 60.0).exp() - p.bpm) / k
        }
    }

    /// Index of the last point at or before `beat` (binary search, like the prototype).
    fn segment_index(&self, beat: f64) -> usize {
        let (mut lo, mut hi) = (0, self.points.len() - 1);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if self.points[mid].beat <= beat {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }

    /// Tempo at `beat` (negative beats use the first point).
    #[must_use]
    pub fn bpm_at(&self, beat: f64) -> f64 {
        let beat = beat.max(0.0);
        let i = self.segment_index(beat);
        self.points[i].bpm + self.slope(i) * (beat - self.points[i].beat)
    }

    /// Seconds at `beat`; negative beats extrapolate the first tempo.
    #[must_use]
    pub fn beat_to_seconds(&self, beat: f64) -> f64 {
        if beat < 0.0 {
            return 60.0 * beat / self.points[0].bpm;
        }
        let i = self.segment_index(beat);
        self.seconds[i] + self.segment_seconds(i, beat)
    }

    /// Beat at `seconds`; negative times extrapolate the first tempo.
    #[must_use]
    pub fn seconds_to_beat(&self, seconds: f64) -> f64 {
        if seconds < 0.0 {
            return seconds * self.points[0].bpm / 60.0;
        }
        let (mut lo, mut hi) = (0, self.seconds.len() - 1);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if self.seconds[mid] <= seconds {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        self.points[lo].beat + self.segment_beats(lo, seconds - self.seconds[lo])
    }

    /// The nearest sample frame for `beat` (Python's `round`: ties to even).
    #[must_use]
    pub fn beat_to_sample(&self, beat: f64, sample_rate: u32) -> i64 {
        python_round(self.beat_to_seconds(beat) * f64::from(sample_rate))
    }

    /// Beat at sample frame `sample`.
    #[must_use]
    pub fn sample_to_beat(&self, sample: f64, sample_rate: u32) -> f64 {
        self.seconds_to_beat(sample / f64::from(sample_rate))
    }

    /// Inserts or replaces the tempo point at `beat`.
    ///
    /// # Errors
    ///
    /// Validation errors of the new point or map.
    pub fn with_point(&self, beat: f64, bpm: f64, ramp: bool) -> Result<Self> {
        let mut kept: Vec<TempoPoint> = self
            .points
            .iter()
            .copied()
            .filter(|p| (p.beat - beat).abs() > EPSILON)
            .collect();
        kept.push(TempoPoint::new(beat, bpm, ramp)?);
        kept.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        Self::new(kept)
    }

    /// Removes the tempo point at `beat`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for beat 0, [`ErrorCode::NotFound`] when there is no
    /// point at `beat`.
    pub fn without_point(&self, beat: f64) -> Result<Self> {
        require(beat.abs() > EPSILON, || {
            "the tempo point at beat 0 cannot be removed".to_owned()
        })?;
        let kept: Vec<TempoPoint> = self
            .points
            .iter()
            .copied()
            .filter(|p| (p.beat - beat).abs() > EPSILON)
            .collect();
        if kept.len() == self.points.len() {
            return Err(DomainError::new(
                ErrorCode::NotFound,
                format!("no tempo point at beat {}", float_repr(beat)),
            ));
        }
        Self::new(kept)
    }
}

/// A time signature starting at `bar` (0-based bar index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSignaturePoint {
    /// 0-based bar index.
    pub bar: i64,
    /// Beats per bar in denominator units.
    pub numerator: u32,
    /// Note value of one beat.
    pub denominator: u32,
}

impl TimeSignaturePoint {
    /// A validated time signature point.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for a negative bar, a numerator outside 1–64 or a
    /// denominator outside [`VALID_DENOMINATORS`].
    pub fn new(bar: i64, numerator: u32, denominator: u32) -> Result<Self> {
        require(bar >= 0, || "time signature bar must be >= 0".to_owned())?;
        require((1..=64).contains(&numerator), || {
            "time signature numerator must be within 1-64".to_owned()
        })?;
        require(VALID_DENOMINATORS.contains(&denominator), || {
            "time signature denominator must be one of (1, 2, 4, 8, 16, 32)".to_owned()
        })?;
        Ok(Self {
            bar,
            numerator,
            denominator,
        })
    }

    /// Bar length in quarter-note beats.
    #[must_use]
    pub fn beats_per_bar(&self) -> f64 {
        f64::from(self.numerator) * 4.0 / f64::from(self.denominator)
    }
}

impl fmt::Display for TimeSignaturePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

/// A position for display: 1-based bar and beat (in denominator units) and the remainder in
/// quarter-note beats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarPosition {
    /// 1-based bar.
    pub bar: i64,
    /// 1-based beat within the bar.
    pub beat: i64,
    /// Remainder in quarter-note beats.
    pub fraction: f64,
}

impl fmt::Display for BarPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{:03}",
            self.bar,
            self.beat,
            python_round(self.fraction * 960.0)
        )
    }
}

/// The time signature map: one or more points, the first at bar 0.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSignatureMap {
    points: Vec<TimeSignaturePoint>,
    start_beats: Vec<f64>,
}

impl Default for TimeSignatureMap {
    fn default() -> Self {
        Self {
            points: vec![TimeSignaturePoint {
                bar: 0,
                numerator: 4,
                denominator: 4,
            }],
            start_beats: vec![0.0],
        }
    }
}

impl TimeSignatureMap {
    /// A validated time signature map.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for an empty map, a first point not at bar 0, or points
    /// that are not strictly increasing in bar.
    pub fn new(points: Vec<TimeSignaturePoint>) -> Result<Self> {
        require(!points.is_empty(), || {
            "time signature map needs at least one point".to_owned()
        })?;
        require(points.first().is_some_and(|p| p.bar == 0), || {
            "first time signature must start at bar 0".to_owned()
        })?;
        require(points.windows(2).all(|w| w[1].bar > w[0].bar), || {
            "time signature points must be strictly increasing in bar".to_owned()
        })?;
        let mut start_beats = Vec::with_capacity(points.len());
        start_beats.push(0.0);
        for w in points.windows(2) {
            let previous = start_beats.last().copied().unwrap_or(0.0);
            #[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
            let bars = (w[1].bar - w[0].bar) as f64;
            start_beats.push(previous + bars * w[0].beats_per_bar());
        }
        Ok(Self {
            points,
            start_beats,
        })
    }

    /// The points, sorted by bar.
    #[must_use]
    pub fn points(&self) -> &[TimeSignaturePoint] {
        &self.points
    }

    #[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
    fn index_for_bar(&self, bar: f64) -> usize {
        let mut i = 0;
        while i + 1 < self.points.len() && (self.points[i + 1].bar as f64) <= bar {
            i += 1;
        }
        i
    }

    fn index_for_beat(&self, beat: f64) -> usize {
        let mut i = 0;
        while i + 1 < self.start_beats.len() && self.start_beats[i + 1] <= beat + EPSILON {
            i += 1;
        }
        i
    }

    /// The signature in effect at 0-based `bar`.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
    pub fn at_bar(&self, bar: i64) -> TimeSignaturePoint {
        self.points[self.index_for_bar(bar as f64)]
    }

    /// The signature in effect at `beat`.
    #[must_use]
    pub fn at_beat(&self, beat: f64) -> TimeSignaturePoint {
        self.points[self.index_for_beat(beat)]
    }

    /// Beat at the start of 0-based `bar` (fractional bars allowed).
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
    pub fn bar_to_beat(&self, bar: f64) -> f64 {
        let i = self.index_for_bar(bar);
        let p = self.points[i];
        self.start_beats[i] + (bar - p.bar as f64) * p.beats_per_bar()
    }

    /// Fractional 0-based bar at `beat`.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
    pub fn beat_to_bar(&self, beat: f64) -> f64 {
        let i = self.index_for_beat(beat);
        let p = self.points[i];
        p.bar as f64 + (beat - self.start_beats[i]) / p.beats_per_bar()
    }

    /// Length of 0-based `bar` in beats.
    #[must_use]
    pub fn bar_length(&self, bar: i64) -> f64 {
        self.at_bar(bar).beats_per_bar()
    }

    /// Musical position of `beat` for display.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "floored bar and beat counts fit in i64"
    )]
    pub fn position(&self, beat: f64) -> BarPosition {
        let i = self.index_for_beat(beat);
        let p = self.points[i];
        let rel = beat - self.start_beats[i];
        let bar_offset = (rel / p.beats_per_bar() + EPSILON).floor();
        let in_bar = rel - bar_offset * p.beats_per_bar();
        let unit = 4.0 / f64::from(p.denominator);
        let beat_index = (in_bar / unit + EPSILON).floor();
        let fraction = (in_bar - beat_index * unit).max(0.0);
        BarPosition {
            bar: p.bar + bar_offset as i64 + 1,
            beat: beat_index as i64 + 1,
            fraction,
        }
    }

    /// Inserts or replaces the signature at 0-based `bar`.
    ///
    /// # Errors
    ///
    /// Validation errors of the new point or map.
    pub fn with_point(&self, bar: i64, numerator: u32, denominator: u32) -> Result<Self> {
        let mut kept: Vec<TimeSignaturePoint> = self
            .points
            .iter()
            .copied()
            .filter(|p| p.bar != bar)
            .collect();
        kept.push(TimeSignaturePoint::new(bar, numerator, denominator)?);
        kept.sort_by_key(|p| p.bar);
        Self::new(kept)
    }

    /// Removes the signature at 0-based `bar`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for bar 0, [`ErrorCode::NotFound`] when there is none at
    /// `bar`.
    pub fn without_point(&self, bar: i64) -> Result<Self> {
        require(bar != 0, || {
            "the time signature at bar 0 cannot be removed".to_owned()
        })?;
        let kept: Vec<TimeSignaturePoint> = self
            .points
            .iter()
            .copied()
            .filter(|p| p.bar != bar)
            .collect();
        if kept.len() == self.points.len() {
            return Err(DomainError::new(
                ErrorCode::NotFound,
                format!("no time signature at bar {}", bar + 1),
            ));
        }
        Self::new(kept)
    }
}

static BAR_BEAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(\d+)(?:[.:](\d+)(?:[.:](\d+))?)?\s*$").expect("the pattern is valid")
});

/// Parses `"bar"`, `"bar:beat"` or `"bar.beat.ticks"` (1-based, ticks at 960 PPQ) into a beat.
///
/// # Errors
///
/// [`ErrorCode::InvalidArgument`] for text that is not a position, for bar or beat 0, or for a
/// beat beyond the bar.
pub fn parse_position(text: &str, signatures: &TimeSignatureMap) -> Result<f64> {
    let unparsable =
        || DomainError::invalid(format!("cannot parse musical position {}", str_repr(text)));
    let caps = BAR_BEAT.captures(text).ok_or_else(unparsable)?;
    let number = |i: usize, default: i64| -> Result<i64> {
        caps.get(i).map_or(Ok(default), |m| {
            m.as_str().parse().map_err(|_| unparsable())
        })
    };
    let bar = number(1, 0)?;
    let beat = number(2, 1)?;
    let ticks = number(3, 0)?;
    require(bar >= 1 && beat >= 1, || {
        "bar and beat are 1-based".to_owned()
    })?;
    let sig = signatures.at_bar(bar - 1);
    require(beat <= i64::from(sig.numerator), || {
        format!("beat {beat} exceeds {sig} bar")
    })?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "bar, beat and tick counts are far below 2^52"
    )]
    let position = signatures.bar_to_beat((bar - 1) as f64)
        + (beat - 1) as f64 * 4.0 / f64::from(sig.denominator)
        + ticks as f64 / 960.0;
    Ok(position)
}

/// How [`snap`] rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapMode {
    /// To the nearest grid line.
    Nearest,
    /// Down to the grid line at or before.
    Floor,
    /// Up to the grid line at or after.
    Ceil,
}

/// Snaps `beat` to multiples of `grid` beats; `grid <= 0` disables snapping.
#[must_use]
pub fn snap(beat: f64, grid: f64, mode: SnapMode) -> f64 {
    if grid <= 0.0 {
        return beat;
    }
    let q = beat / grid;
    let n = match mode {
        SnapMode::Floor => (q + EPSILON).floor(),
        SnapMode::Ceil => (q - EPSILON).ceil(),
        SnapMode::Nearest => (q + 0.5).floor(),
    };
    n * grid
}

/// Python's `round(x)`: to the nearest integer, ties to even.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "positions in samples fit in i64"
)]
pub fn python_round(value: f64) -> i64 {
    value.round_ties_even() as i64
}

#[cfg(test)]
mod tests {
    use super::{TempoMap, TempoPoint, python_round};

    #[test]
    fn rounding_follows_python() {
        assert_eq!(python_round(0.5), 0);
        assert_eq!(python_round(1.5), 2);
        assert_eq!(python_round(2.5), 2);
        assert_eq!(python_round(-0.5), 0);
    }

    #[test]
    fn ramps_integrate_logarithmically() {
        let map = TempoMap::new(vec![
            TempoPoint::new(0.0, 60.0, true).unwrap(),
            TempoPoint::new(4.0, 120.0, false).unwrap(),
        ])
        .unwrap();
        // 60 → 120 BPM over 4 beats: seconds = 60/k · ln(2), k = 15 BPM per beat.
        let expected = 4.0 * std::f64::consts::LN_2;
        assert!((map.beat_to_seconds(4.0) - expected).abs() < 1e-12);
        assert!((map.seconds_to_beat(expected) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_maps_are_rejected() {
        assert!(TempoPoint::new(0.0, 5.0, false).is_err());
        assert!(
            TempoMap::new(vec![
                TempoPoint::new(0.0, 120.0, false).unwrap(),
                TempoPoint::new(0.0, 90.0, false).unwrap(),
            ])
            .is_err()
        );
        assert!(TempoMap::new(vec![]).is_err());
    }
}
