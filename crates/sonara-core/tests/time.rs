//! The time base follows the prototype (`tests/unit/test_time.py`): tempo and meter maps,
//! conversions, validation messages, position parsing and snapping. Expected messages were taken
//! from the prototype.

use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::time::{
    SnapMode, TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint, parse_position, snap,
};

/// `pytest.approx` with its defaults: relative 1e-6, absolute 1e-12.
fn approx(actual: f64, expected: f64) {
    within(actual, expected, (expected.abs() * 1e-6).max(1e-12));
}

fn within(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} ± {tolerance}, got {actual}"
    );
}

fn tempo(beat: f64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint::new(beat, bpm, ramp).expect("valid tempo point")
}

fn tempo_map(points: &[(f64, f64, bool)]) -> TempoMap {
    TempoMap::new(points.iter().map(|&(b, bpm, r)| tempo(b, bpm, r)).collect())
        .expect("valid tempo map")
}

fn meter(bar: i64, numerator: u32, denominator: u32) -> TimeSignaturePoint {
    TimeSignaturePoint::new(bar, numerator, denominator).expect("valid time signature")
}

fn meter_map(points: &[(i64, u32, u32)]) -> TimeSignatureMap {
    TimeSignatureMap::new(points.iter().map(|&(b, n, d)| meter(b, n, d)).collect())
        .expect("valid time signature map")
}

fn assert_error<T: std::fmt::Debug>(
    result: Result<T, DomainError>,
    code: ErrorCode,
    message: &str,
) {
    let err = result.expect_err(message);
    assert_eq!((err.code, err.message.as_str()), (code, message));
}

/// `numpy.linspace(start, stop, count)`.
fn linspace(start: f64, stop: f64, count: u32) -> Vec<f64> {
    let increment = (stop - start) / f64::from(count - 1);
    (0..count)
        .map(|i| start + f64::from(i) * increment)
        .collect()
}

#[test]
fn constant_tempo_conversions() {
    let tm = TempoMap::constant(120.0).expect("valid");
    approx(tm.beat_to_seconds(4.0), 2.0);
    approx(tm.seconds_to_beat(2.0), 4.0);
    assert_eq!(tm.beat_to_sample(1.0, 48_000), 24_000);
    approx(tm.sample_to_beat(24_000.0, 48_000), 1.0);
    assert!((tm.bpm_at(100.0) - 120.0).abs() < f64::EPSILON);
}

#[test]
fn negative_positions_extrapolate_first_tempo() {
    let tm = tempo_map(&[(0.0, 60.0, false), (4.0, 120.0, false)]);
    approx(tm.beat_to_seconds(-2.0), -2.0);
    approx(tm.seconds_to_beat(-3.0), -3.0);
}

#[test]
fn multiple_step_changes() {
    let tm = tempo_map(&[
        (0.0, 120.0, false),
        (8.0, 60.0, false),
        (12.0, 240.0, false),
    ]);
    // 8 beats at 120 = 4 s; 4 beats at 60 = 4 s; then 240 BPM.
    approx(tm.beat_to_seconds(8.0), 4.0);
    approx(tm.beat_to_seconds(12.0), 8.0);
    approx(tm.beat_to_seconds(16.0), 9.0);
    assert!((tm.bpm_at(9.0) - 60.0).abs() < f64::EPSILON);
    for b in [0.0, 3.3, 8.0, 10.5, 12.0, 20.25] {
        within(tm.seconds_to_beat(tm.beat_to_seconds(b)), b, 1e-9);
    }
}

#[test]
fn linear_ramp_closed_form() {
    let tm = tempo_map(&[(0.0, 120.0, true), (8.0, 60.0, false)]);
    let k = (60.0 - 120.0) / 8.0;
    let expected = 60.0 / k * (60.0_f64 / 120.0).ln();
    approx(tm.beat_to_seconds(8.0), expected);
    approx(tm.bpm_at(4.0), 90.0);
    // After the ramp the last tempo holds.
    approx(tm.beat_to_seconds(10.0), expected + 2.0);
    for b in linspace(0.0, 12.0, 25) {
        within(tm.seconds_to_beat(tm.beat_to_seconds(b)), b, 1e-9);
    }
    // The prototype's values.
    within(tm.beat_to_seconds(8.0), 5.545_177_444_479_562, 1e-12);
    within(tm.beat_to_seconds(10.0), 7.545_177_444_479_562, 1e-12);
}

#[test]
fn ramp_numeric_integration_matches() {
    let tm = tempo_map(&[(0.0, 80.0, true), (16.0, 160.0, false)]);
    // numpy.trapezoid(60 / bpm, beats) over 200 001 samples.
    let beats = linspace(0.0, 16.0, 200_001);
    let seconds: Vec<f64> = beats
        .iter()
        .map(|b| 60.0 / (80.0 + (160.0 - 80.0) * b / 16.0))
        .collect();
    let integral: f64 = beats
        .windows(2)
        .zip(seconds.windows(2))
        .map(|(b, s)| (b[1] - b[0]) * (s[0] + s[1]) / 2.0)
        .sum();
    let actual = tm.beat_to_seconds(16.0);
    within(actual, integral, integral.abs() * 1e-8);
    within(actual, 8.317_766_166_719_343, 1e-12);
}

/// The prototype's `test_vectorized_conversions_match_scalar` checks numpy conversions against
/// the scalar ones; Rust has only the scalar API, so the round trip of the same beats is ported.
#[test]
fn vectorized_conversions_match_scalar() {
    let maps = [
        tempo_map(&[(0.0, 120.0, false)]),
        tempo_map(&[
            (0.0, 100.0, false),
            (4.0, 140.0, false),
            (10.0, 70.0, false),
        ]),
        tempo_map(&[(0.0, 90.0, true), (8.0, 150.0, true), (12.0, 60.0, false)]),
    ];
    let beats = [-3.0, 0.0, 0.5, 3.999, 4.0, 7.25, 8.0, 11.0, 12.0, 30.0];
    for tm in &maps {
        for b in beats {
            within(tm.seconds_to_beat(tm.beat_to_seconds(b)), b, 1e-9);
        }
    }
}

#[test]
fn with_and_without_point() {
    let tm = TempoMap::constant(120.0)
        .and_then(|m| m.with_point(8.0, 90.0, false))
        .and_then(|m| m.with_point(4.0, 100.0, false))
        .expect("valid");
    let beats: Vec<f64> = tm.points().iter().map(|p| p.beat).collect();
    assert_eq!(beats, [0.0, 4.0, 8.0]);
    let tm2 = tm.with_point(4.0, 110.0, false).expect("valid");
    let bpms: Vec<f64> = tm2.points().iter().map(|p| p.bpm).collect();
    assert_eq!(bpms, [120.0, 110.0, 90.0]);
    let first = tm.with_point(0.0, 60.0, false).expect("valid").points()[0];
    assert!((first.bpm - 60.0).abs() < f64::EPSILON);
    let tm3 = tm2.without_point(4.0).expect("valid");
    let beats: Vec<f64> = tm3.points().iter().map(|p| p.beat).collect();
    assert_eq!(beats, [0.0, 8.0]);
    assert_error(
        tm3.without_point(5.0),
        ErrorCode::NotFound,
        "no tempo point at beat 5.0",
    );
    assert_error(
        tm3.without_point(0.0),
        ErrorCode::InvalidArgument,
        "the tempo point at beat 0 cannot be removed",
    );
}

#[test]
fn tempo_validation() {
    let invalid = ErrorCode::InvalidArgument;
    assert_error(
        TempoPoint::new(0.0, 5.0, false),
        invalid,
        "tempo must be within 10.0-999.0 BPM",
    );
    assert_error(
        TempoPoint::new(-1.0, 120.0, false),
        invalid,
        "tempo point beat must be >= 0",
    );
    assert_error(
        TempoPoint::new(0.0, f64::NAN, false),
        invalid,
        "tempo must be a finite number",
    );
    assert_error(
        TempoMap::new(vec![tempo(1.0, 120.0, false)]),
        invalid,
        "first tempo point must be at beat 0",
    );
    assert_error(
        TempoMap::new(vec![
            tempo(0.0, 120.0, false),
            tempo(4.0, 100.0, false),
            tempo(4.0, 90.0, false),
        ]),
        invalid,
        "tempo points must be strictly increasing in beat",
    );
    assert_error(
        TempoMap::new(Vec::new()),
        invalid,
        "tempo map needs at least one point",
    );
}

/// The prototype also checks that equal maps hash equally; `TempoMap` is not hashable in Rust.
#[test]
fn equality_ignores_cache() {
    let a = tempo_map(&[(0.0, 120.0, false), (4.0, 90.0, false)]);
    let b = tempo_map(&[(0.0, 120.0, false), (4.0, 90.0, false)]);
    assert_eq!(a, b);
}

#[test]
fn time_signature_changes() {
    let sigs = meter_map(&[(0, 4, 4), (2, 3, 4), (4, 7, 8)]);
    // Bars 0, 1: 4 beats each; bars 2, 3: 3 beats; bar 4 on: 3.5 beats.
    approx(sigs.bar_to_beat(2.0), 8.0);
    approx(sigs.bar_to_beat(4.0), 14.0);
    approx(sigs.bar_to_beat(5.0), 17.5);
    approx(sigs.bar_to_beat(2.5), 9.5);
    for bar in [0.0, 1.5, 2.0, 3.25, 4.0, 6.75] {
        approx(sigs.beat_to_bar(sigs.bar_to_beat(bar)), bar);
    }
    assert_eq!(sigs.at_bar(3).numerator, 3);
    assert_eq!(sigs.at_beat(15.0).denominator, 8);
    approx(sigs.bar_length(4), 3.5);
    assert_eq!(sigs.at_bar(4).to_string(), "7/8");
}

#[test]
fn bar_position_display() {
    let sigs = meter_map(&[(0, 4, 4), (2, 3, 4), (4, 7, 8)]);
    assert_eq!(sigs.position(0.0).to_string(), "1.1.000");
    assert_eq!(sigs.position(4.5).to_string(), "2.1.480");
    let pos = sigs.position(9.0);
    assert_eq!((pos.bar, pos.beat), (3, 2));
    // 7/8: the beat unit is an eighth (0.5 quarter beats).
    let pos = sigs.position(14.0 + 1.5);
    assert_eq!((pos.bar, pos.beat), (5, 4));
    approx(pos.fraction, 0.0);
    assert_eq!(pos.to_string(), "5.4.000");
}

#[test]
fn time_signature_edit_and_validation() {
    let sigs = TimeSignatureMap::default()
        .with_point(4, 6, 8)
        .and_then(|m| m.with_point(2, 3, 4))
        .expect("valid");
    let bars: Vec<i64> = sigs.points().iter().map(|p| p.bar).collect();
    assert_eq!(bars, [0, 2, 4]);
    let without = sigs.without_point(2).expect("valid");
    let bars: Vec<i64> = without.points().iter().map(|p| p.bar).collect();
    assert_eq!(bars, [0, 4]);
    let invalid = ErrorCode::InvalidArgument;
    assert_error(
        sigs.without_point(0),
        invalid,
        "the time signature at bar 0 cannot be removed",
    );
    assert_error(
        sigs.without_point(7),
        ErrorCode::NotFound,
        "no time signature at bar 8",
    );
    assert_error(
        TimeSignaturePoint::new(0, 4, 3),
        invalid,
        "time signature denominator must be one of (1, 2, 4, 8, 16, 32)",
    );
    assert_error(
        TimeSignaturePoint::new(0, 0, 4),
        invalid,
        "time signature numerator must be within 1-64",
    );
    assert_error(
        TimeSignatureMap::new(vec![meter(1, 4, 4)]),
        invalid,
        "first time signature must start at bar 0",
    );
}

#[test]
fn parse_positions() {
    let sigs = TimeSignatureMap::default()
        .with_point(2, 3, 4)
        .expect("valid");
    let parse = |text| parse_position(text, &sigs).expect("valid position");
    assert_eq!(
        [
            parse("1"),
            parse("2:1"),
            parse("2:3"),
            parse("3.2"),
            parse("1:1:480")
        ],
        [0.0, 4.0, 6.0, 9.0, 0.5]
    );
    for (bad, message) in [
        ("", "cannot parse musical position ''"),
        ("abc", "cannot parse musical position 'abc'"),
        ("0:1", "bar and beat are 1-based"),
        ("1:0", "bar and beat are 1-based"),
        ("3:4", "beat 4 exceeds 3/4 bar"),
    ] {
        assert_error(
            parse_position(bad, &sigs),
            ErrorCode::InvalidArgument,
            message,
        );
    }
}

#[test]
fn snap_modes() {
    approx(snap(1.13, 0.25, SnapMode::Nearest), 1.25);
    approx(snap(1.12, 0.25, SnapMode::Nearest), 1.0);
    assert_eq!(
        [
            snap(1.99, 1.0, SnapMode::Floor),
            snap(2.0 + 1e-12, 1.0, SnapMode::Floor),
            snap(1.01, 1.0, SnapMode::Ceil),
            snap(2.0 - 1e-12, 1.0, SnapMode::Ceil),
            snap(1.234, 0.0, SnapMode::Nearest),
            snap(1.234, -1.0, SnapMode::Nearest),
        ],
        [1.0, 2.0, 2.0, 2.0, 1.234, 1.234]
    );
}
