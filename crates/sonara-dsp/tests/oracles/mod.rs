//! The prototype oracles of `scripts/golden/dsp_oracles.py` (GLD-01): the fixtures in
//! `tests/fixtures/`, the input signals their specs describe, and the fingerprints they record.
//!
//! - [`fixture`] reads one fixture.
//! - [`signal`] regenerates a recorded input from its spec, in the order of operations of the
//!   Python expression that made it, so that the samples equal the prototype's: `sin` and the
//!   arithmetic are correctly rounded the same way on the recording host (macOS libm, which
//!   NumPy's float64 `sin` calls), and NumPy's `default_rng` is ported bit for bit
//!   (`sonara_dsp::numpy::random`).
//! - [`check_fingerprint`] compares samples with a recorded fingerprint: `Σ|x|`, the peak and
//!   the sampled frames.

use std::path::Path;

use serde_json::Value;
use sonara_dsp::filters::{Section, SosFilter};
use sonara_dsp::numpy::math::sum;
use sonara_dsp::numpy::random::default_rng;

/// A fixture of `tests/fixtures/`.
pub fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_slice(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{value} is not a number"))
}

fn count(value: &Value) -> usize {
    value
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or_else(|| panic!("{value} is not a count"))
}

fn items(value: &Value) -> &[Value] {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is not an array"))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "frame indices of a few seconds of audio are exact in f64, as NumPy's arange is"
)]
fn as_f64(i: usize) -> f64 {
    i as f64
}

/// Python's `int(seconds * sr)`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Python's int() truncates a positive frame count"
)]
fn frames_of(seconds: f64, sample_rate: f64) -> usize {
    (seconds * sample_rate) as usize
}

/// The interleaved samples and the channel count of the signal `spec` describes (the kinds of
/// `dsp_oracles.py`'s `Sig`).
pub fn signal(spec: &Value) -> (Vec<f64>, usize) {
    let kind = spec["kind"].as_str().expect("a signal kind");
    match kind {
        "sine" | "sine_index" => sine(spec),
        "dc" => {
            let value: Vec<f64> = items(&spec["value"]).iter().map(number).collect();
            let data = value.repeat(count(&spec["frames"]));
            (data, value.len())
        }
        "impulse" => {
            let mut data = vec![0.0; count(&spec["frames"]) * 2];
            let at = count(&spec["at"]) * 2;
            data[at..at + 2].fill(number(&spec["amp"]));
            (data, 2)
        }
        "normal" => normal(spec),
        // A mono signal on two channels, each times its sign.
        "stereo_of" => {
            let (mono, channels) = signal(&spec["input"]);
            assert_eq!(channels, 1, "stereo_of a mono signal");
            let signs: Vec<f64> = items(&spec["signs"]).iter().map(number).collect();
            let data = mono
                .iter()
                .flat_map(|&x| [x * signs[0], x * signs[1]])
                .collect();
            (data, 2)
        }
        "concat" => {
            let mut data = Vec::new();
            let mut channels = None;
            for part in items(&spec["parts"]) {
                let (samples, c) = signal(part);
                assert_eq!(*channels.get_or_insert(c), c, "parts of one layout");
                data.extend(samples);
            }
            (data, channels.expect("parts"))
        }
        // `parts[0] + parts[1] + …`, left to right.
        "sum" => {
            let parts = items(&spec["parts"]);
            let (mut data, channels) = signal(&parts[0]);
            for part in &parts[1..] {
                let (samples, c) = signal(part);
                assert_eq!(
                    (samples.len(), c),
                    (data.len(), channels),
                    "parts of one shape"
                );
                for (x, y) in data.iter_mut().zip(samples) {
                    *x += y;
                }
            }
            (data, channels)
        }
        "sosfilt" => sosfilt(spec),
        other => panic!("unknown signal kind {other}"),
    }
}

/// Both channels of a sine: `"sine"` is `conftest.sine`, `amp * sin(2 * pi * freq * t + phase)`
/// with `t = arange(n) / sr`; `"sine_index"` is `amp * sin(2 * pi * freq * arange(n) / sr +
/// phase)`, the order of `test_true_peak_finds_intersample_peak`.
fn sine(spec: &Value) -> (Vec<f64>, usize) {
    let sr = number(&spec["sr"]);
    let (freq, amp, phase) = (
        number(&spec["freq"]),
        number(&spec["amp"]),
        number(&spec["phase"]),
    );
    let by_index = spec["kind"] == "sine_index";
    let n = if by_index {
        count(&spec["frames"])
    } else {
        frames_of(number(&spec["seconds"]), sr)
    };
    let w = 2.0 * std::f64::consts::PI * freq;
    let data = (0..n)
        .flat_map(|i| {
            let angle = if by_index {
                w * as_f64(i) / sr
            } else {
                w * (as_f64(i) / sr)
            };
            let x = amp * (angle + phase).sin();
            [x, x]
        })
        .collect();
    (data, 2)
}

/// `default_rng(seed).standard_normal((frames, channels)) * scale` after `skip` earlier draws.
fn normal(spec: &Value) -> (Vec<f64>, usize) {
    let mut rng = default_rng(spec["seed"].as_u64().expect("a seed"));
    let mut skipped = vec![0.0; count(&spec["skip"])];
    rng.standard_normal_fill(&mut skipped);
    let channels = count(&spec["channels"]);
    let mut data = vec![0.0; count(&spec["frames"]) * channels];
    rng.standard_normal_fill(&mut data);
    let scale = number(&spec["scale"]);
    for x in &mut data {
        *x *= scale;
    }
    (data, channels)
}

/// SciPy's `sosfilt(sos, x, axis=0)`: every channel through the same cascade.
fn sosfilt(spec: &Value) -> (Vec<f64>, usize) {
    let (x, channels) = signal(&spec["input"]);
    let sections: Vec<Section> = items(&spec["sos"])
        .iter()
        .map(|s| {
            let s: Vec<f64> = items(s).iter().map(number).collect();
            s.try_into().expect("six coefficients")
        })
        .collect();
    let mut filter = SosFilter::new(channels, sections.len());
    filter.set_sections(&sections).expect("the cascade fits");
    let mut data = vec![0.0; x.len()];
    filter.process(&x, &mut data);
    (data, channels)
}

/// Compares the interleaved `data` of `channels` channels with a recorded fingerprint:
/// `Σ|x|` within `sum_tolerance` relative (exactly for a silent recording), the peak and every
/// sampled frame within `sample_tolerance` absolute. `Σ|x|` is NumPy's pairwise `np.sum` over
/// the C-ordered samples, as the recorder computes it. Returns the largest sample deviation.
pub fn check_fingerprint(
    label: &str,
    data: &[f64],
    channels: usize,
    expected: &Value,
    sum_tolerance: f64,
    sample_tolerance: f64,
) -> f64 {
    let frames = data.len() / channels;
    assert_eq!(frames, count(&expected["frames"]), "{label}: frames");
    let magnitudes: Vec<f64> = data.iter().map(|x| x.abs()).collect();
    let abs_sum = sum(&magnitudes);
    let peak = data.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let recorded_sum = number(&expected["abs_sum"]);
    let sum_deviation = if recorded_sum > 0.0 {
        (abs_sum - recorded_sum).abs() / recorded_sum
    } else {
        abs_sum
    };
    assert!(
        (abs_sum - recorded_sum).abs() <= sum_tolerance * recorded_sum,
        "{label}: Σ|x| {abs_sum:e} vs the prototype's {recorded_sum:e}"
    );
    let recorded_peak = number(&expected["peak"]);
    assert!(
        (peak - recorded_peak).abs() <= sample_tolerance,
        "{label}: peak {peak:e} vs the prototype's {recorded_peak:e}"
    );
    let mut worst = 0.0_f64;
    for sample in items(&expected["samples"]) {
        let pair = items(sample);
        let frame = count(&pair[0]);
        for (c, value) in items(&pair[1]).iter().enumerate() {
            let (actual, value) = (data[frame * channels + c], number(value));
            let deviation = (actual - value).abs();
            worst = worst.max(deviation);
            assert!(
                deviation <= sample_tolerance,
                "{label}: frame {frame}, channel {c}: {actual:e} vs the prototype's {value:e}"
            );
        }
    }
    eprintln!("{label}: Σ|x| {sum_deviation:.2e} relative, samples {worst:.2e}");
    worst
}
