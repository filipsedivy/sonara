//! Golden parity of the Analyzer (GLD-01).
//!
//! Two recordings check it:
//!
//! - `analysis/`, by `scripts/golden/area_analysis.py`: the full report (`to_json` with the
//!   series) and the issues of the 6 complete renders (5 workflows and the coverage batch) and
//!   the 7 reference signals, fed in 65536-frame float64 blocks, the loop of the render service's
//!   `analyze_file`. The inputs are recorded (the renders as float32 WAVs read as float64, the
//!   signals as float64), so the test feeds exactly what the prototype fed and compares every
//!   field.
//! - `workflows/*/outputs/**/*.analysis.json`: the preview sidecars, normalized. The render
//!   service analyzed the float64 render blocks there but stored the preview as float32, so not
//!   even the prototype reproduces the raw float levels from the stored WAV. The test feeds the
//!   WAV in render blocks (512 frames) and compares what the float32 storage cannot change, as
//!   the prototype itself does: the counts, the DC offset, silence, the rounded loudness
//!   series, the issues and the report's keys. The raw float levels of the sidecars stay
//!   unverified until the render path can feed the float64 blocks.
//! - `tests/fixtures/analysis-cases-v1.json`, by `scripts/golden/dsp_oracles.py`: the full report
//!   and the issues of the EBU Tech 3341/3342, BS.1770, true-peak and stereo cases of the
//!   prototype's `tests/audio/test_analysis.py`, compared by the same rules, with the Python test's own checks on top
//!   ([`reference_cases_match_the_prototype`]). Measured on macOS on 2026-10-23: every level
//!   bit-exact; `side_to_mid_db` within 2.4e-15 dB, the centroid within 2 ulps and the band
//!   shares within 2.2e-16.
//!
//! Exactness, per field (F9–F12), measured in the
//! container (aarch64 Linux, glibc) on the 13 reports:
//!
//! - Counts, the DC offset, silence regions and durations, the rounded loudness series,
//!   `kind`, the report's keys and the issues' texts: exact.
//! - `correlation` and `min_correlation_400ms`: exact. They use the recording host's BLAS
//!   `ddot` order for the stereo columns (fused multiply-adds) and `sqrt`, both correctly
//!   rounded everywhere.
//! - The levels follow the prototype's arithmetic (the K-weighting, the reduction orders, the
//!   gates) but end in `log10` and use `pow` and `tan`, which other libms round differently
//!   from macOS libm in a few cases ([`LIBM_EPSILONS`]). In the container `balance_db`,
//!   `integrated_lufs` and `loudness_range_lu` are bit-exact in every report;
//!   `sample_peak_dbfs`, `true_peak_dbtp`, `rms_dbfs`, `crest_factor_db`,
//!   `momentary_max_lufs`, `short_term_max_lufs` and `plr_db` differ by 1 or 2 ulps in a few.
//! - `side_to_mid_db`: the prototype's contiguous `ddot` depends on the buffers' alignment, so
//!   no order reproduces it ([`SIDE_TO_MID_TOLERANCE`]).
//! - The spectrum (`octave_bands_db`, `spectral_centroid_hz`) goes through `realfft` instead of
//!   pocketfft: [`BAND_SHARE_TOLERANCE`] and [`CENTROID_TOLERANCE`].

mod oracles;

use std::collections::BTreeMap;

use serde_json::Value;
use sonara_dsp::analysis::{AnalysisReport, Analyzer};
use sonara_testkit::Golden;

/// The block size of `area_analysis.py` (and of `RenderService.analyze_file`).
const ANALYSIS_BLOCK: usize = 65536;
/// The render block of the preview sidecars (`renderer.DEFAULT_BLOCK`).
const RENDER_BLOCK: usize = 512;

/// Levels that end in one libm call (`log10`, after `pow` and `tan` in the K-weighting) may
/// differ from the recording by this many machine epsilons of `max(|expected|, 1)`. The scale
/// covers the cancellation in `-0.691 + 10 log10(e)` near 0 LUFS. Measured in the container: at
/// most 1.0 ε (1 or 2 ulps where the value is above 1); macOS libm is the oracle's own and
/// should be exact. The margin covers other platforms' libm.
const LIBM_EPSILONS: f64 = 8.0;

/// `side_to_mid_db`, absolute dB. The prototype's `mid` and `side` energies come from NumPy's
/// contiguous `ddot`, whose summation order depends on the buffers' alignment on the recording
/// host, so a different order of the same sums is unavoidable: about `sqrt(n)` ulps
/// relative over blocks of 65536 terms, times 4.34 dB. Measured in the container: 9.9e-14 dB
/// (283 ulps); 7 of 13 reports bit-exact.
const SIDE_TO_MID_TOLERANCE: f64 = 1e-12;

/// Octave bands, compared as energy shares of the total (`10^(dB/10)`), absolute. `realfft` and
/// pocketfft round differently; their errors are relative to the segment's energy, not to
/// a band's, so bands at the FFT's rounding floor (−100 dB and below, such as the far bands of a
/// sine) are noise in dB but agree as shares. An FFT of 8192 points is accurate to about
/// `ε · log2(8192)` ≈ 3e-15 of the total. Measured in the container: 2.2e-16 (and 1.2e-8 dB on a
/// band at −182 dB); 71 of 110 bands bit-exact.
const BAND_SHARE_TOLERANCE: f64 = 1e-14;

/// `spectral_centroid_hz`, relative. The power spectrum differs by FFT rounding, the
/// centroid by about an ulp. Measured in the container: 2 ulps (2.3e-13 Hz); 5 of 13 reports
/// bit-exact.
const CENTROID_TOLERANCE: f64 = 1e-14;

fn golden() -> Golden {
    Golden::open().expect("golden data is available (set SONARA_GOLDEN to a recording)")
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("no field {key} in {value}"))
}

/// Interleaved float64 samples, the channel count and the sample rate of a recorded source.
fn source(golden: &Golden, rel: &str) -> (Vec<f64>, usize, u32) {
    if let Some(base) = rel.strip_suffix(".f64") {
        let array = golden.f64_array(base).expect("a recorded signal");
        let channels = *array.shape.get(1).expect("(frames, channels)");
        let rate = field(&array.header, "sample_rate")
            .as_u64()
            .and_then(|r| u32::try_from(r).ok())
            .expect("a sample rate");
        return (array.data, channels, rate);
    }
    let audio = golden.wav(rel).expect("a recorded WAV");
    let samples = audio.samples.iter().map(|&s| f64::from(s)).collect();
    (samples, usize::from(audio.channels), audio.sample_rate)
}

fn analyze(samples: &[f64], channels: usize, rate: u32, block: usize) -> AnalysisReport {
    let mut analyzer = Analyzer::new(rate, channels).expect("a supported format");
    for piece in samples.chunks(block * channels) {
        analyzer.feed(piece);
    }
    analyzer.finish()
}

/// How a report field is compared.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Rule {
    /// Equal values (floats bit for bit).
    Exact,
    /// Within [`LIBM_EPSILONS`] epsilons of `max(|expected|, 1)`.
    Libm,
    /// Within an absolute tolerance.
    Absolute(f64),
    /// Within a tolerance relative to the expected value.
    Relative(f64),
    /// Octave bands: energy shares within [`BAND_SHARE_TOLERANCE`].
    Bands,
}

fn rule(key: &str) -> Rule {
    match key {
        "sample_peak_dbfs"
        | "true_peak_dbtp"
        | "rms_dbfs"
        | "crest_factor_db"
        | "integrated_lufs"
        | "momentary_max_lufs"
        | "short_term_max_lufs"
        | "loudness_range_lu"
        | "plr_db"
        | "balance_db" => Rule::Libm,
        "side_to_mid_db" => Rule::Absolute(SIDE_TO_MID_TOLERANCE),
        "spectral_centroid_hz" => Rule::Relative(CENTROID_TOLERANCE),
        "octave_bands_db" => Rule::Bands,
        _ => Rule::Exact,
    }
}

/// Whether `actual` is within the rule's tolerance of `expected`.
fn within(rule: Rule, actual: f64, expected: f64) -> bool {
    let diff = (actual - expected).abs();
    match rule {
        Rule::Exact => actual.to_bits() == expected.to_bits(),
        Rule::Libm => diff <= LIBM_EPSILONS * f64::EPSILON * expected.abs().max(1.0),
        Rule::Absolute(tolerance) => diff <= tolerance,
        Rule::Relative(tolerance) => diff <= tolerance * expected.abs(),
        Rule::Bands => diff <= BAND_SHARE_TOLERANCE,
    }
}

fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    if a.is_sign_negative() != b.is_sign_negative() {
        return u64::MAX;
    }
    a.to_bits().abs_diff(b.to_bits())
}

/// The largest deviation seen per field, printed for the record of the tolerances.
#[derive(Default)]
struct Deviations {
    /// Field → (largest ulps, largest absolute difference, largest difference in epsilons of
    /// `max(|expected|, 1)`, comparisons, bit-exact ones).
    fields: BTreeMap<String, (u64, f64, f64, usize, usize)>,
}

impl Deviations {
    fn record(&mut self, key: &str, actual: f64, expected: f64) {
        let entry = self.fields.entry(key.to_owned()).or_default();
        let diff = (actual - expected).abs();
        entry.0 = entry.0.max(ulps(actual, expected));
        entry.1 = entry.1.max(diff);
        entry.2 = entry.2.max(diff / (f64::EPSILON * expected.abs().max(1.0)));
        entry.3 += 1;
        entry.4 += usize::from(actual.to_bits() == expected.to_bits());
    }

    fn print(&self) {
        for (key, (ulps, abs, eps, n, exact)) in &self.fields {
            eprintln!(
                "  {key}: {exact}/{n} bit-exact, max {ulps} ulps, max |diff| {abs:.3e} \
                 ({eps:.2} eps of max(|e|, 1))"
            );
        }
    }
}

fn number(value: &Value, what: &str) -> Option<f64> {
    if value.is_null() {
        return None;
    }
    Some(
        value
            .as_f64()
            .unwrap_or_else(|| panic!("{what}: not a number: {value}")),
    )
}

/// Compares one field and records its deviation; panics outside the field's tolerance.
fn compare_field(
    what: &str,
    key: &str,
    actual: &Value,
    expected: &Value,
    deviations: &mut Deviations,
) {
    let rule = rule(key);
    match rule {
        Rule::Exact => assert_eq!(actual, expected, "{what}: {key}"),
        Rule::Libm | Rule::Absolute(_) | Rule::Relative(_) => {
            let (a, e) = (number(actual, what), number(expected, what));
            let (Some(a), Some(e)) = (a, e) else {
                assert_eq!(a, e, "{what}: {key} (null)");
                return;
            };
            deviations.record(key, a, e);
            assert!(
                within(rule, a, e),
                "{what}: {key} is {a:e}, expected {e:e} ({} ulps)",
                ulps(a, e)
            );
        }
        Rule::Bands => {
            let (a, e) = (
                actual.as_object().expect("bands"),
                expected.as_object().expect("bands"),
            );
            assert_eq!(keys(actual), keys(expected), "{what}: band names");
            for (name, expected_db) in e {
                let actual_db = number(&a[name], what).expect("a band level");
                let expected_db = number(expected_db, what).expect("a band level");
                let share = |db: f64| 10f64.powf(db / 10.0);
                deviations.record("octave_bands_db (dB)", actual_db, expected_db);
                deviations.record(
                    "octave_bands_db (share)",
                    share(actual_db),
                    share(expected_db),
                );
                assert!(
                    within(rule, share(actual_db), share(expected_db)),
                    "{what}: band {name} is {actual_db} dB, expected {expected_db} dB"
                );
            }
        }
    }
}

fn keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn issues_json(report: &AnalysisReport) -> Value {
    Value::Array(
        report
            .issues()
            .iter()
            .map(sonara_dsp::analysis::Issue::to_json)
            .collect(),
    )
}

#[test]
fn analysis_reports_match_the_prototype() {
    let golden = golden();
    let files: Vec<String> = golden
        .manifest()
        .files
        .keys()
        .filter(|rel| {
            rel.starts_with("analysis/")
                && std::path::Path::new(rel.as_str())
                    .extension()
                    .is_some_and(|ext| ext == "json")
        })
        .cloned()
        .collect();
    assert_eq!(files.len(), 13, "6 renders and 7 signals");
    let mut deviations = Deviations::default();
    for rel in &files {
        let recorded: Value = golden.json(rel).expect("an analysis report");
        let src = field(&recorded, "source").as_str().expect("a source");
        let (samples, channels, rate) = source(&golden, src);
        assert_eq!(
            field(&recorded, "block").as_u64(),
            Some(ANALYSIS_BLOCK as u64)
        );
        assert_eq!(
            field(&recorded, "frames").as_u64(),
            Some((samples.len() / channels) as u64),
            "{rel}: frames"
        );
        let report = analyze(&samples, channels, rate, ANALYSIS_BLOCK);
        let actual = report.to_json(true);
        let expected = field(&recorded, "analysis");
        assert_eq!(keys(&actual), keys(expected), "{rel}: report keys");
        for (key, value) in expected.as_object().expect("a report") {
            compare_field(rel, key, &actual[key], value, &mut deviations);
        }
        assert_eq!(
            &issues_json(&report),
            field(&recorded, "issues"),
            "{rel}: issues"
        );
    }
    eprintln!("analysis/ ({} reports):", files.len());
    deviations.print();
}

/// The sidecar fields that the float32 storage of the preview cannot change (see the module
/// docs); `duration_seconds` is volatile in the recording.
const SIDECAR_EXACT: [&str; 12] = [
    "sample_rate",
    "channels",
    "dc_offset",
    "clipped_samples",
    "clip_events",
    "over_full_scale_samples",
    "nonfinite_samples",
    "silence_regions",
    "leading_silence_seconds",
    "trailing_silence_seconds",
    "short_term_lufs",
    "kind",
];

#[test]
fn preview_sidecars_match_where_the_stored_preview_allows() {
    let golden = golden();
    let sidecars: Vec<String> = golden
        .manifest()
        .files
        .keys()
        .filter(|rel| rel.starts_with("workflows/") && rel.ends_with(".analysis.json"))
        .cloned()
        .collect();
    assert_eq!(sidecars.len(), 7, "the workflows' previews");
    for rel in &sidecars {
        let recorded: Value = golden.json(rel).expect("a sidecar");
        let wav = rel.replace(".analysis.json", ".wav");
        let (samples, channels, rate) = source(&golden, &wav);
        let report = analyze(&samples, channels, rate, RENDER_BLOCK);
        let actual = report.to_json(true);
        let expected = field(&recorded, "analysis");
        assert_eq!(keys(&actual), keys(expected), "{rel}: report keys");
        for key in SIDECAR_EXACT {
            assert_eq!(&actual[key], field(expected, key), "{rel}: {key}");
        }
        assert_eq!(
            &issues_json(&report),
            field(&recorded, "issues"),
            "{rel}: issues"
        );
    }
}

/// The BS.1770 K-weighting at 48 kHz (`test_k_weighting_coefficients_at_48k_match_bs1770`).
const BS1770_48K: [[f64; 6]; 2] = [
    [
        1.535_124_859_586_97,
        -2.691_696_189_406_38,
        1.198_392_810_852_85,
        1.0,
        -1.690_659_293_182_41,
        0.732_480_774_215_85,
    ],
    [
        1.0,
        -2.0,
        1.0,
        1.0,
        -1.990_047_454_833_98,
        0.990_072_250_366_21,
    ],
];

/// The reference cases of `tests/audio/test_analysis.py` beyond the golden reports:
/// EBU Tech 3341 (the stereo sine levels, the absolute
/// and relative gates) and 3342 (loudness range), the rate independence of the K-weighting,
/// inter-sample peaks and true peak at 44.1, 96 and 192 kHz, correlation and width of equal,
/// inverted, independent and mono noise, and the octave slope of white noise.
///
/// `tests/fixtures/analysis-cases-v1.json` holds the prototype's full report and issues for
/// each signal, recorded by `scripts/golden/dsp_oracles.py`. Each signal is regenerated from its
/// spec and checked against the recorded input; the report is compared field by field with the
/// rules of [`analysis_reports_match_the_prototype`], and the Python test's own check, with its
/// tolerance, holds on the Rust report.
#[test]
fn reference_cases_match_the_prototype() {
    let fixture = oracles::fixture("analysis-cases-v1.json");
    let mut deviations = Deviations::default();
    // A1: the K-weighting sections against BS.1770 at 48 kHz (1e-12, as the Python test), and
    // against the prototype's at every rate the cases use (one `tan` and one `pow` each).
    let recorded = fixture["k_weighting"]
        .as_object()
        .expect("the K-weighting by rate");
    assert_eq!(recorded.len(), 4, "44.1, 48, 96 and 192 kHz");
    for (rate, sections) in recorded {
        let rate: u32 = rate.parse().expect("a rate");
        let actual = sonara_dsp::analysis::k_weighting_sos(rate);
        let sections = sections.as_array().expect("two sections");
        assert_eq!(sections.len(), actual.len(), "{rate}: sections");
        for (s, (section, expected)) in actual.iter().zip(sections).enumerate() {
            for (i, &value) in section.iter().enumerate() {
                let prototype = number(&expected[i], "a coefficient").expect("a coefficient");
                deviations.record("k_weighting", value, prototype);
                assert!(
                    within(Rule::Libm, value, prototype),
                    "{rate}: section {s}[{i}]"
                );
            }
        }
    }
    for (s, (section, table)) in sonara_dsp::analysis::k_weighting_sos(48_000)
        .iter()
        .zip(BS1770_48K)
        .enumerate()
    {
        for (i, (&actual, expected)) in section.iter().zip(table).enumerate() {
            assert!(
                (actual - expected).abs() <= 1e-12,
                "BS.1770: section {s}[{i}]"
            );
        }
    }
    let cases = fixture["cases"].as_object().expect("the cases");
    assert_eq!(cases.len(), 19, "the recorded signals");
    for (name, case) in cases {
        let (samples, channels) = oracles::signal(field(case, "signal"));
        oracles::check_fingerprint(name, &samples, channels, field(case, "input"), 1e-12, 1e-12);
        let rate =
            u32::try_from(field(case, "sample_rate").as_u64().expect("a rate")).expect("a rate");
        let report = analyze(&samples, channels, rate, ANALYSIS_BLOCK);
        let actual = report.to_json(true);
        let expected = field(case, "analysis");
        assert_eq!(keys(&actual), keys(expected), "{name}: report keys");
        for (key, value) in expected.as_object().expect("a report") {
            if LEVEL_DIFFERENCES.contains(&key.as_str()) {
                compare_level_difference(name, key, &actual, expected, rate, &mut deviations);
            } else if rate == ILL_CONDITIONED_RATE && rule(key) == Rule::Libm {
                compare_ill_conditioned(name, key, &actual[key], value, &mut deviations);
            } else {
                compare_field(name, key, &actual[key], value, &mut deviations);
            }
        }
        assert_eq!(
            &issues_json(&report),
            field(case, "issues"),
            "{name}: issues"
        );
        python_check(name, &actual, &issues_json(&report));
    }
    eprintln!("analysis cases ({}):", cases.len());
    deviations.print();
}

/// The report's fields that are differences of two levels: PLR (true peak − integrated
/// loudness), the loudness range (two percentiles of the short-term loudness), the crest
/// factor (peak − RMS) and the balance (left − right loudness).
const LEVEL_DIFFERENCES: [&str; 4] = [
    "plr_db",
    "loudness_range_lu",
    "crest_factor_db",
    "balance_db",
];

/// The levels a [`LEVEL_DIFFERENCES`] field is taken from.
const LEVELS: [&str; 6] = [
    "sample_peak_dbfs",
    "true_peak_dbtp",
    "rms_dbfs",
    "integrated_lufs",
    "momentary_max_lufs",
    "short_term_max_lufs",
];

/// The rate whose K-weighting is ill-conditioned for another libm: its poles lie close to the
/// unit circle, so an ulp of `tan` or `pow` in a coefficient moves the K-weighted energy by
/// about 1e-13. Measured in the container on 2026-10-23: one of the 48 coefficients of the four
/// rates one ulp from the prototype's (all are checked by [`Rule::Libm`] above). The golden
/// reports never analyze at this rate.
const ILL_CONDITIONED_RATE: u32 = 192_000;

/// The golden relative tolerance, for the loudness of [`ILL_CONDITIONED_RATE`]. Measured in the
/// container (aarch64 Linux, glibc) on 2026-10-23: the integrated, momentary and short-term
/// loudness of the 192 kHz sine 125 to 132 ε of `max(|e|, 1)` apart (1.8e-13 LU, a relative
/// 3e-14), every other case within [`LIBM_EPSILONS`]. On macOS, the oracle's libm, they are
/// bit-exact.
const SPEC_RELATIVE_TOLERANCE: f64 = 1e-12;

fn compare_ill_conditioned(
    name: &str,
    key: &str,
    actual: &Value,
    expected: &Value,
    deviations: &mut Deviations,
) {
    let (Some(a), Some(e)) = (number(actual, name), number(expected, name)) else {
        assert_eq!(actual, expected, "{name}: {key} (null)");
        return;
    };
    deviations.record(key, a, e);
    assert!(
        (a - e).abs() <= SPEC_RELATIVE_TOLERANCE * e.abs().max(1.0),
        "{name}: {key} is {a:e}, expected {e:e} ({} ulps)",
        ulps(a, e)
    );
}

/// A difference of two levels carries their libm ulps at the levels' scale, not at its
/// own. In the golden reports both scales agree (PLRs and ranges of several dB), but the
/// reference sines are steady: their PLR and loudness range are 0 up to rounding, the
/// difference of two values near −23 or −33. So these fields are compared by [`Rule::Libm`]
/// scaled by the largest level of the report (measured in the container on 2026-10-23: at most
/// 4.8 ε of that scale, where 32 ε of 1 would fail), and at [`ILL_CONDITIONED_RATE`] by
/// [`SPEC_RELATIVE_TOLERANCE`] of it (85 ε).
fn compare_level_difference(
    name: &str,
    key: &str,
    actual: &Value,
    expected: &Value,
    rate: u32,
    deviations: &mut Deviations,
) {
    let value = |report: &Value, key: &str| number(field(report, key), key);
    let (Some(a), Some(e)) = (value(actual, key), value(expected, key)) else {
        assert_eq!(actual[key], expected[key], "{name}: {key} (null)");
        return;
    };
    let scale = LEVELS
        .iter()
        .filter_map(|level| value(expected, level))
        .filter(|level| level.is_finite())
        .fold(1.0_f64, |m, level| m.max(level.abs()));
    let tolerance = if rate == ILL_CONDITIONED_RATE {
        SPEC_RELATIVE_TOLERANCE
    } else {
        LIBM_EPSILONS * f64::EPSILON
    };
    deviations.record(key, a, e);
    assert!(
        (a - e).abs() <= tolerance * scale,
        "{name}: {key} is {a:e}, expected {e:e} ({} ulps)",
        ulps(a, e)
    );
}

/// The Python test's assertion for case `name` (`pytest.approx` defaults to a relative 1e-6).
fn python_check(name: &str, report: &Value, issues: &Value) {
    let get = |key: &str| number(field(report, key), key).unwrap_or(f64::NAN);
    let near = |key: &str, expected: f64, tolerance: f64| {
        let value = get(key);
        assert!(
            (value - expected).abs() <= tolerance,
            "{name}: {key} is {value}, expected {expected} ± {tolerance}"
        );
    };
    let approx = |key: &str, expected: f64| near(key, expected, 1e-6 * expected.abs());
    let has_issue = |code: &str| {
        issues
            .as_array()
            .expect("issues")
            .iter()
            .any(|i| i["code"] == code)
    };
    match name {
        "ebu_3341_sine_23" | "ebu_3341_sine_33" => {
            let level = if name.ends_with("23") { -23.0 } else { -33.0 };
            for key in [
                "integrated_lufs",
                "momentary_max_lufs",
                "short_term_max_lufs",
            ] {
                near(key, level, 0.1);
            }
        }
        "ebu_3341_gating"
        | "ebu_3341_relative_gate"
        | "rate_44100"
        | "rate_48000"
        | "rate_96000" => near("integrated_lufs", -23.0, 0.1),
        "ebu_3342_20_30" => near("loudness_range_lu", 10.0, 0.1),
        "ebu_3342_20_15" => near("loudness_range_lu", 5.0, 0.1),
        "ebu_3342_40_20" => near("loudness_range_lu", 20.0, 0.1),
        "true_peak_intersample" => {
            near("sample_peak_dbfs", -3.01, 0.01);
            near("true_peak_dbtp", 0.0, 0.2);
        }
        "true_peak_44100" | "true_peak_96000" | "true_peak_192000" => {
            assert!(
                get("true_peak_dbtp") >= get("sample_peak_dbfs") - 1e-9,
                "{name}"
            );
            near("true_peak_dbtp", -6.0, 0.1);
        }
        "correlation_same" => {
            approx("correlation", 1.0);
            approx("side_to_mid_db", -120.0);
        }
        "correlation_inverted" => {
            approx("correlation", -1.0);
            assert!(has_issue("PHASE"), "{name}: a phase issue");
        }
        "correlation_independent" => {
            assert!(get("correlation").abs() < 0.05, "{name}");
            near("side_to_mid_db", 0.0, 0.2);
            assert!(get("min_correlation_400ms") < 0.2, "{name}");
            near("balance_db", 0.0, 0.1);
        }
        "correlation_mono" => assert!(field(report, "correlation").is_null(), "{name}"),
        "white_noise_bands" => {
            let bands = field(report, "octave_bands_db");
            let band = |b: &str| number(&bands[b], b).expect("a band level");
            for (lo, hi) in [
                ("250", "500"),
                ("500", "1000"),
                ("1000", "2000"),
                ("2000", "4000"),
            ] {
                let slope = band(hi) - band(lo);
                assert!(
                    (slope - 3.01).abs() <= 0.3,
                    "{name}: {lo} to {hi}: {slope} dB"
                );
            }
        }
        other => panic!("no check for case {other}"),
    }
}
