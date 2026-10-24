//! Not real-time: the Analyzer's results and their JSON, built on the render service's and the
//! export's threads (see the parent module).
//!
//! The prototype's `AnalysisReport` dataclass (`dsp/analysis/meter.py`): the fields, `to_json`
//! and `issues`.

use serde_json::{Map, Value};

use crate::numpy::math::py_round;

/// Lowest value [`AnalysisReport::to_json`] writes for a float (the prototype's `_finite`
/// floor).
const JSON_FLOOR: f64 = -200.0;

/// The report's `kind` entry: which fields are measurements and which are estimates.
const MEASURED: [&str; 9] = [
    "sample_peak_dbfs",
    "rms_dbfs",
    "integrated_lufs",
    "momentary_max_lufs",
    "short_term_max_lufs",
    "loudness_range_lu",
    "clipped_samples",
    "correlation",
    "octave_bands_db",
];
const ESTIMATED: [&str; 2] = ["true_peak_dbtp", "plr_db"];

/// The result of an [`Analyzer`](super::Analyzer) run: the prototype's `AnalysisReport`, with
/// its field names and units. Undefined levels are `-inf` (the loudness of silence), as in the
/// prototype; [`to_json`](Self::to_json) maps them to `null`.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisReport {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channels per frame.
    pub channels: usize,
    /// Frames fed, in seconds.
    pub duration_seconds: f64,
    /// Largest absolute sample, dBFS.
    pub sample_peak_dbfs: f64,
    /// Estimated inter-sample peak (oversampled), dBTP.
    pub true_peak_dbtp: f64,
    /// RMS over all channels, dBFS.
    pub rms_dbfs: f64,
    /// Mean per channel, rounded to 6 decimals as NumPy's `round(6)`.
    pub dc_offset: Vec<f64>,
    /// Sample peak minus RMS, dB; 0 for silence.
    pub crest_factor_db: f64,
    /// Gated integrated loudness (BS.1770-4), LUFS.
    pub integrated_lufs: f64,
    /// Largest 400 ms loudness, LUFS.
    pub momentary_max_lufs: f64,
    /// Largest 3 s loudness, LUFS.
    pub short_term_max_lufs: f64,
    /// Loudness range (EBU Tech 3342), LU.
    pub loudness_range_lu: f64,
    /// True peak minus integrated loudness, dB; 0 when either is undefined.
    pub plr_db: f64,
    /// Frames whose largest absolute sample is at least 0.999.
    pub clipped_samples: u64,
    /// Runs of at least 3 such frames.
    pub clip_events: u64,
    /// Samples above 1.0 in magnitude.
    pub over_full_scale_samples: u64,
    /// NaN and infinite samples (counted, then treated as 0).
    pub nonfinite_samples: u64,
    /// Silent stretches of at least 0.5 s, `(start, end)` in seconds.
    pub silence_regions: Vec<(f64, f64)>,
    /// Silence before the first non-silent 50 ms frame, seconds.
    pub leading_silence_seconds: f64,
    /// Silence after the last non-silent 50 ms frame, seconds.
    pub trailing_silence_seconds: f64,
    /// L/R correlation; `None` for mono or a silent channel.
    pub correlation: Option<f64>,
    /// Smallest correlation over non-overlapping 400 ms windows; `None` for mono or without a
    /// window.
    pub min_correlation_400ms: Option<f64>,
    /// Side energy relative to mid energy, dB (−120 without side energy); `None` for mono.
    pub side_to_mid_db: Option<f64>,
    /// Left energy relative to right energy, dB; `None` for mono or a silent channel.
    pub balance_db: Option<f64>,
    /// Energy share per octave band, dB relative to the total, in band order, keyed like the
    /// prototype (`f"{centre:g}"`: `"31.5"`, `"63"`, …, `"16000"`).
    pub octave_bands_db: Vec<(&'static str, f64)>,
    /// Spectral centroid of the averaged power spectrum, Hz.
    pub spectral_centroid_hz: f64,
    /// Short-term loudness every second, rounded to 2 decimals (−120 for silence).
    pub short_term_lufs: Vec<f64>,
}

/// How serious an [`Issue`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    /// The signal is broken.
    Error,
    /// A likely problem.
    Warning,
    /// Worth knowing.
    Info,
}

impl Severity {
    /// The prototype's name: `error`, `warning` or `info`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// What an [`Issue`] is about; the prototype's codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IssueCode {
    /// NaN or infinite samples.
    Nonfinite,
    /// Samples above full scale.
    OverFullScale,
    /// Probable clipping.
    Clipping,
    /// True peak above −1 dBTP.
    TruePeak,
    /// Integrated loudness above −8 LUFS.
    VeryLoud,
    /// Integrated loudness below −30 LUFS.
    Quiet,
    /// No gated loudness.
    Silent,
    /// Negative L/R correlation.
    Phase,
    /// DC offset above 1 % of full scale.
    DcOffset,
    /// The 63 Hz octave dominates.
    LowEndHeavy,
}

impl IssueCode {
    /// The prototype's code, such as `OVER_FULL_SCALE`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nonfinite => "NONFINITE",
            Self::OverFullScale => "OVER_FULL_SCALE",
            Self::Clipping => "CLIPPING",
            Self::TruePeak => "TRUE_PEAK",
            Self::VeryLoud => "VERY_LOUD",
            Self::Quiet => "QUIET",
            Self::Silent => "SILENT",
            Self::Phase => "PHASE",
            Self::DcOffset => "DC_OFFSET",
            Self::LowEndHeavy => "LOW_END_HEAVY",
        }
    }
}

/// A plain-language finding for agents and the GUI (the prototype's `issues()` entries).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// How serious it is.
    pub severity: Severity,
    /// What it is about.
    pub code: IssueCode,
    /// The prototype's message, numbers formatted as it formats them.
    pub message: String,
}

impl Issue {
    fn new(severity: Severity, code: IssueCode, message: String) -> Self {
        Self {
            severity,
            code,
            message,
        }
    }

    /// `{"severity", "code", "message"}`, in that order.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("severity".into(), self.severity.as_str().into());
        map.insert("code".into(), self.code.as_str().into());
        map.insert("message".into(), self.message.clone().into());
        Value::Object(map)
    }
}

impl AnalysisReport {
    /// The prototype's `to_json(include_series)`: the fields in declaration order (without
    /// `short_term_lufs` unless `include_series`), then `kind`. Float fields go through the
    /// prototype's `_finite`: NaN and `-inf` become `null`, values below −200 become −200, and
    /// `+inf`, which JSON cannot hold, becomes `null` as well (the prototype would write
    /// `Infinity`; only a sum of squares that overflows can produce it). Silence regions are
    /// rounded to 3 decimals with Python's `round`.
    #[must_use]
    pub fn to_json(&self, include_series: bool) -> Value {
        let mut map = Map::new();
        let mut put = |key: &str, value: Value| {
            map.insert(key.into(), value);
        };
        put("sample_rate", self.sample_rate.into());
        put("channels", self.channels.into());
        put("duration_seconds", finite(self.duration_seconds));
        put("sample_peak_dbfs", finite(self.sample_peak_dbfs));
        put("true_peak_dbtp", finite(self.true_peak_dbtp));
        put("rms_dbfs", finite(self.rms_dbfs));
        put(
            "dc_offset",
            self.dc_offset.iter().map(|&d| Value::from(d)).collect(),
        );
        put("crest_factor_db", finite(self.crest_factor_db));
        put("integrated_lufs", finite(self.integrated_lufs));
        put("momentary_max_lufs", finite(self.momentary_max_lufs));
        put("short_term_max_lufs", finite(self.short_term_max_lufs));
        put("loudness_range_lu", finite(self.loudness_range_lu));
        put("plr_db", finite(self.plr_db));
        put("clipped_samples", self.clipped_samples.into());
        put("clip_events", self.clip_events.into());
        put(
            "over_full_scale_samples",
            self.over_full_scale_samples.into(),
        );
        put("nonfinite_samples", self.nonfinite_samples.into());
        put(
            "silence_regions",
            self.silence_regions
                .iter()
                .map(|&(start, end)| {
                    [py_round(start, 3), py_round(end, 3)]
                        .into_iter()
                        .collect::<Value>()
                })
                .collect(),
        );
        put(
            "leading_silence_seconds",
            finite(self.leading_silence_seconds),
        );
        put(
            "trailing_silence_seconds",
            finite(self.trailing_silence_seconds),
        );
        put("correlation", optional(self.correlation));
        put(
            "min_correlation_400ms",
            optional(self.min_correlation_400ms),
        );
        put("side_to_mid_db", optional(self.side_to_mid_db));
        put("balance_db", optional(self.balance_db));
        put(
            "octave_bands_db",
            Value::Object(
                self.octave_bands_db
                    .iter()
                    .map(|&(name, db)| (name.to_owned(), finite(db)))
                    .collect(),
            ),
        );
        put("spectral_centroid_hz", finite(self.spectral_centroid_hz));
        if include_series {
            put(
                "short_term_lufs",
                self.short_term_lufs
                    .iter()
                    .map(|&v| Value::from(v))
                    .collect(),
            );
        }
        let mut kind = Map::new();
        kind.insert("measured".into(), MEASURED.into_iter().collect());
        kind.insert("estimated".into(), ESTIMATED.into_iter().collect());
        put("kind", Value::Object(kind));
        Value::Object(map)
    }

    /// The prototype's `issues()`: findings in its order, with its thresholds and message
    /// texts (`:.2f` and `:.1f` round the exact binary value half to even, as Rust's `{:.N}`
    /// does).
    #[must_use]
    #[expect(
        clippy::disallowed_macros,
        reason = "not real-time: formats the messages on the analysis thread"
    )]
    pub fn issues(&self) -> Vec<Issue> {
        let integrated = self.integrated_lufs;
        let loud = integrated.is_finite() && integrated > -8.0;
        let quiet = integrated.is_finite() && !loud && integrated < -30.0;
        let dc = self.dc_offset.iter().map(|d| d.abs()).reduce(python_max);
        // `octave_bands_db.get("63", -99.0)`, only when there are bands.
        let low = (!self.octave_bands_db.is_empty()).then(|| {
            self.octave_bands_db
                .iter()
                .find(|(name, _)| *name == "63")
                .map_or(-99.0, |&(_, db)| db)
        });
        let found = [
            (self.nonfinite_samples > 0).then(|| {
                Issue::new(
                    Severity::Error,
                    IssueCode::Nonfinite,
                    format!(
                        "{} NaN/infinite samples in the signal",
                        self.nonfinite_samples
                    ),
                )
            }),
            (self.over_full_scale_samples > 0).then(|| {
                Issue::new(
                    Severity::Error,
                    IssueCode::OverFullScale,
                    format!(
                        "{} samples exceed 0 dBFS (will clip in fixed-point exports)",
                        self.over_full_scale_samples
                    ),
                )
            }),
            (self.clip_events > 0).then(|| {
                Issue::new(
                    Severity::Warning,
                    IssueCode::Clipping,
                    format!(
                        "{} probable clipping events ({} samples at full scale)",
                        self.clip_events, self.clipped_samples
                    ),
                )
            }),
            (self.true_peak_dbtp > -1.0 && self.true_peak_dbtp.is_finite()).then(|| {
                Issue::new(
                    Severity::Warning,
                    IssueCode::TruePeak,
                    format!(
                        "true peak {:.2} dBTP is above -1 dBTP (risk of codec clipping)",
                        self.true_peak_dbtp
                    ),
                )
            }),
            loud.then(|| {
                Issue::new(
                    Severity::Warning,
                    IssueCode::VeryLoud,
                    format!(
                        "integrated loudness {integrated:.1} LUFS is very loud (heavily limited?)"
                    ),
                )
            }),
            quiet.then(|| {
                Issue::new(
                    Severity::Info,
                    IssueCode::Quiet,
                    format!("integrated loudness {integrated:.1} LUFS is quiet for music delivery"),
                )
            }),
            (!integrated.is_finite()).then(|| {
                Issue::new(
                    Severity::Warning,
                    IssueCode::Silent,
                    "the signal is silent (no gated loudness)".to_owned(),
                )
            }),
            self.correlation.filter(|&c| c < 0.0).map(|c| {
                Issue::new(
                    Severity::Warning,
                    IssueCode::Phase,
                    format!("negative L/R correlation ({c:.2}): mono compatibility problems"),
                )
            }),
            dc.is_some_and(|dc| dc > 0.01).then(|| {
                Issue::new(
                    Severity::Warning,
                    IssueCode::DcOffset,
                    "DC offset above 1% of full scale".to_owned(),
                )
            }),
            low.is_some_and(|low| low > -4.0).then(|| {
                Issue::new(
                    Severity::Info,
                    IssueCode::LowEndHeavy,
                    "the 63 Hz octave dominates the spectrum".to_owned(),
                )
            }),
        ];
        found.into_iter().flatten().collect()
    }
}

/// Python's `max(a, b)`: `b` only when it is larger.
pub(super) fn python_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The prototype's `_finite`, as a JSON value.
fn finite(x: f64) -> Value {
    if x.is_nan() || x.is_infinite() {
        // NaN and -inf are None; +inf is not representable in JSON.
        return Value::Null;
    }
    Value::from(python_max(x, JSON_FLOOR))
}

/// An optional float through `_finite`; `None` is `null`.
fn optional(x: Option<f64>) -> Value {
    x.map_or(Value::Null, finite)
}

#[cfg(test)]
mod tests {
    use super::{AnalysisReport, IssueCode};
    use serde_json::Value;

    fn report() -> AnalysisReport {
        AnalysisReport {
            sample_rate: 48000,
            channels: 2,
            duration_seconds: 1.0,
            sample_peak_dbfs: -0.5,
            true_peak_dbtp: -0.125,
            rms_dbfs: -250.0,
            dc_offset: Vec::from([0.02, 0.0]),
            crest_factor_db: 3.0,
            integrated_lufs: f64::NEG_INFINITY,
            momentary_max_lufs: f64::NAN,
            short_term_max_lufs: -10.0,
            loudness_range_lu: 0.0,
            plr_db: 0.0,
            clipped_samples: 4,
            clip_events: 1,
            over_full_scale_samples: 0,
            nonfinite_samples: 2,
            silence_regions: Vec::from([(0.0005, 1.2345)]),
            leading_silence_seconds: 0.0,
            trailing_silence_seconds: 0.05,
            correlation: Some(-0.125),
            min_correlation_400ms: None,
            side_to_mid_db: Some(-120.0),
            balance_db: None,
            octave_bands_db: Vec::from([("31.5", -300.0), ("63", -3.0)]),
            spectral_centroid_hz: 100.0,
            short_term_lufs: Vec::from([-10.0]),
        }
    }

    fn field<'a>(json: &'a Value, key: &str) -> &'a Value {
        json.get(key).unwrap()
    }

    fn keys(json: &Value) -> Vec<&str> {
        json.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn json_keeps_the_prototype_key_order_and_finite_rules() {
        let json = report().to_json(true);
        // The dataclass field order of `AnalysisReport` in meter.py, then `kind`.
        let expected = [
            "sample_rate",
            "channels",
            "duration_seconds",
            "sample_peak_dbfs",
            "true_peak_dbtp",
            "rms_dbfs",
            "dc_offset",
            "crest_factor_db",
            "integrated_lufs",
            "momentary_max_lufs",
            "short_term_max_lufs",
            "loudness_range_lu",
            "plr_db",
            "clipped_samples",
            "clip_events",
            "over_full_scale_samples",
            "nonfinite_samples",
            "silence_regions",
            "leading_silence_seconds",
            "trailing_silence_seconds",
            "correlation",
            "min_correlation_400ms",
            "side_to_mid_db",
            "balance_db",
            "octave_bands_db",
            "spectral_centroid_hz",
            "short_term_lufs",
            "kind",
        ];
        assert_eq!(keys(&json), expected);
        assert_eq!(field(&json, "rms_dbfs").as_f64(), Some(-200.0));
        assert!(field(&json, "integrated_lufs").is_null());
        assert!(field(&json, "momentary_max_lufs").is_null());
        let regions = field(&json, "silence_regions").as_array().unwrap();
        let region: Vec<f64> = regions
            .iter()
            .flat_map(|r| r.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()))
            .collect();
        // Python's round: 0.0005 is slightly above the tie in binary, 1.2345 slightly below.
        assert_eq!(region, [0.001, 1.234]);
        let bands = field(&json, "octave_bands_db");
        assert_eq!(field(bands, "31.5").as_f64(), Some(-200.0));
        assert!(field(&json, "sample_rate").is_u64() && field(&json, "duration_seconds").is_f64());
        assert_eq!(keys(field(&json, "kind")), ["measured", "estimated"]);
        assert!(report().to_json(false).get("short_term_lufs").is_none());
    }

    #[test]
    fn issues_follow_the_prototype_order_and_texts() {
        let issues = report().issues();
        let codes: Vec<IssueCode> = issues.iter().map(|i| i.code).collect();
        assert_eq!(
            codes,
            [
                IssueCode::Nonfinite,
                IssueCode::Clipping,
                IssueCode::TruePeak,
                IssueCode::Silent,
                IssueCode::Phase,
                IssueCode::DcOffset,
                IssueCode::LowEndHeavy,
            ]
        );
        let message = |i: usize| issues.get(i).map(|issue| issue.message.as_str());
        // 0.125 is an exact tie: Python's `:.2f` rounds it half to even.
        assert_eq!(
            message(2),
            Some("true peak -0.12 dBTP is above -1 dBTP (risk of codec clipping)")
        );
        assert_eq!(
            message(4),
            Some("negative L/R correlation (-0.12): mono compatibility problems")
        );
        let json = issues.first().unwrap().to_json();
        assert_eq!(keys(&json), ["severity", "code", "message"]);
        assert_eq!(field(&json, "severity").as_str(), Some("error"));
    }
}
