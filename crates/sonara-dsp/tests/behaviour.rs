//! Behaviour of the built-in processors and the kernels against the prototype
//! (TST-01, TST-03, TST-10,
//! GLD-01).
//!
//! The golden plugin area renders each processor's defaults and presets through seven
//! reference signals. The properties below are what the prototype's `tests/audio` tests
//! assert beyond that: the compressor's RMS detector, time constants and auto makeup, the
//! gate's expander mode, robustness on DC and near Nyquist, the decay to silence, the utility's
//! width and DC blocker, the filter's modes, aliasing, click bounds after parameter changes,
//! true-peak limiting, the reverb's RT60, and the instruments' pitch, velocity and unmapped
//! notes.
//!
//! **Oracles.** `tests/fixtures/behaviour-v1.json` was recorded from the prototype by
//! `scripts/golden/dsp_oracles.py`, which runs each case with `tests/audio/conftest.py`'s own
//! `make_processor` and `run`. Each test here:
//!
//! 1. regenerates the recorded input from its spec and checks it against the recorded input
//!    fingerprint ([`INPUT_TOLERANCE`]);
//! 2. runs the processor as `run` does (fixed 512-frame blocks, zero-padded; the parameter
//!    changes at the start of their block; instrument events per block) and compares the
//!    output with the prototype's fingerprint at the renderer's tolerances
//!    ([`OUTPUT_SUM_TOLERANCE`], [`OUTPUT_SAMPLE_TOLERANCE`]), and `report()` with the
//!    prototype's;
//! 3. computes the Python test's measurement from the Rust output and compares it with the
//!    prototype's own measurement ([`METRIC_TOLERANCE`]; the reverb's float32 wet path has
//!    [`REVERB_SUM_TOLERANCE`] and [`REVERB_METRIC_TOLERANCE`]);
//! 4. asserts the Python test's bound on the Rust measurement, with the Python test's own
//!    tolerance.
//!
//! The measurements follow the Python helpers: `rms_db`, `test_effects._schroeder_rt60`, the
//! aliasing ratio and `test_instruments.fundamental` (a Hann window, `rfft` with 8x zero
//! padding, log-parabolic interpolation). `realfft` and NumPy's pocketfft round differently,
//! which the metric tolerance covers. Run with
//! `--nocapture` to see each deviation from the prototype.

mod oracles;

use std::f64::consts::{E, PI};

use oracles::{check_fingerprint, fixture, signal};
use realfft::RealFftPlanner;
use serde_json::Value;
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{PluginDescriptor, builtin_registry};
use sonara_dsp::analysis::analyze;
use sonara_dsp::kernels::{Oscillator, SvfMode, Wave, clamp_cutoff, clamp_q, osc_bank, svf};
use sonara_dsp::numpy::math::mean;
use sonara_dsp::processor::{BlockContext, EventKind, InstrumentEvent, ParamSlots, ProcessSpec};
use sonara_dsp::processors::{create, param_slots};

/// The runner's block size (`conftest.BLOCK`) and rate (`conftest.SR`).
const BLOCK: usize = 512;
const SR: u32 = 48_000;
const SRF: f64 = 48_000.0;

/// The output against the prototype's: `Σ|x|` within a relative 1e-9, the peak and the 48
/// sampled frames within 1e-6, the −120 dBFS null-test threshold of (as
/// `sonara-engine`'s `renderer.rs`).
const OUTPUT_SUM_TOLERANCE: f64 = 1e-9;
const OUTPUT_SAMPLE_TOLERANCE: f64 = 1e-6;

/// The regenerated input against the recorded one. The generators reproduce the Python
/// expressions' order of operations, so the inputs are exact where `sin` is correctly rounded
/// (macOS libm, the recording host's); the margin covers another platform's libm.
const INPUT_TOLERANCE: f64 = 1e-12;

/// A measurement of the Rust output against the prototype's measurement of its own output,
/// relative to `max(|recorded|, 1)`. The outputs agree to the last bits (measured on macOS:
/// bit-exact, or within 3e-14 where the limiter's true-peak FIR rounds differently, F6), and
/// the measurements reduce them through `realfft` instead of pocketfft: at most 4e-15.
const METRIC_TOLERANCE: f64 = 1e-9;

/// The algorithmic reverb's `Σ|x|` and measurements. Its wet path is float32, and
/// `realfft` and pocketfft round the inverse FFT differently, so a float32 rounding can flip:
/// an ulp of float32 (6e-8 of the wet level) on some samples, the source of the golden reverb
/// renders' residual (−128.9 dBFS at worst, `golden_plugins.rs`). The samples stay within
/// [`OUTPUT_SAMPLE_TOLERANCE`] (measured on macOS on 2026-10-23: at most 8.9e-8); `Σ|x|` moved by
/// at most 1.1e-9 relative and the measurements by at most 4.5e-8, hence these bounds.
const REVERB_SUM_TOLERANCE: f64 = 1e-8;
const REVERB_METRIC_TOLERANCE: f64 = 1e-7;

/// `report()` values against the prototype's, relative to `max(|recorded|, 1)` (as the golden
/// runner's `REPORT_TOLERANCE`).
const REPORT_TOLERANCE: f64 = 1e-9;

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
    reason = "frame counts and indices are exact in f64, as NumPy's are"
)]
fn as_f64(i: usize) -> f64 {
    i as f64
}

/// Recorded parameter values: JSON integers are `Int`, other numbers `Float`.
fn params(values: &Value) -> Params {
    values
        .as_object()
        .expect("a parameter object")
        .iter()
        .map(|(key, value)| {
            let value = match value {
                Value::Bool(b) => ParamValue::Bool(*b),
                Value::String(s) => ParamValue::Str(s.clone()),
                Value::Number(n) => n
                    .as_i64()
                    .map_or_else(|| ParamValue::Float(number(value)), ParamValue::Int),
                other => panic!("parameter {key}: {other}"),
            };
            (key.clone(), value)
        })
        .collect()
}

/// `make_processor`: `prepare(ProcessSpec(sr, 512))`, then `set_params` with the resolved,
/// validated values.
fn slots(descriptor: &PluginDescriptor, values: &Params) -> ParamSlots {
    let validated = descriptor
        .validate_param_values(values)
        .unwrap_or_else(|e| panic!("{}: {e}", descriptor.id));
    param_slots(descriptor, &validated).unwrap_or_else(|e| panic!("{}: {e}", descriptor.id))
}

fn event_kind(name: &str) -> EventKind {
    match name {
        "note_on" => EventKind::NoteOn,
        "note_off" => EventKind::NoteOff,
        "cc" => EventKind::Cc,
        "pitch_bend" => EventKind::PitchBend,
        other => panic!("unknown event kind {other}"),
    }
}

/// One recorded case, run in Rust and checked against the recording (steps 1 and 2 of the
/// module docs).
struct Case {
    name: String,
    record: Value,
    input: Vec<[f64; 2]>,
    output: Vec<[f64; 2]>,
    metric_tolerance: f64,
}

impl Case {
    fn run(fixture: &Value, name: &str) -> Self {
        let record = fixture["cases"]
            .get(name)
            .unwrap_or_else(|| panic!("no case {name}"))
            .clone();
        let (samples, channels) = signal(&record["signal"]);
        assert_eq!(channels, 2, "{name}: a stereo input");
        check_fingerprint(
            &format!("{name} (input)"),
            &samples,
            2,
            &record["input"],
            INPUT_TOLERANCE,
            INPUT_TOLERANCE,
        );
        let input = samples.as_chunks::<2>().0.to_vec();
        let output = process(&record, &input);
        let reverb = record["plugin"] == "sonara.reverb";
        check_fingerprint(
            name,
            output.as_flattened(),
            2,
            &record["output"],
            if reverb {
                REVERB_SUM_TOLERANCE
            } else {
                OUTPUT_SUM_TOLERANCE
            },
            OUTPUT_SAMPLE_TOLERANCE,
        );
        Self {
            name: name.to_owned(),
            record,
            input,
            output,
            metric_tolerance: if reverb {
                REVERB_METRIC_TOLERANCE
            } else {
                METRIC_TOLERANCE
            },
        }
    }

    /// The prototype's measurement `key` of its own output; checks the Rust measurement
    /// `actual` against it and returns `actual`.
    fn metric(&self, key: &str, actual: f64) -> f64 {
        check_metric(&self.name, &self.record, key, actual, self.metric_tolerance)
    }

    /// Channel `c` of the output, from frame `from` on.
    fn channel(&self, c: usize, from: usize) -> Vec<f64> {
        self.output[from..].iter().map(|f| f[c]).collect()
    }
}

/// Checks the Rust measurement `actual` against the prototype's measurement `key` in
/// `record["metrics"]`, relative to `max(|recorded|, 1)`; returns `actual`.
fn check_metric(label: &str, record: &Value, key: &str, actual: f64, tolerance: f64) -> f64 {
    let recorded = number(&record["metrics"][key]);
    let deviation = (actual - recorded).abs() / recorded.abs().max(1.0);
    eprintln!("{label}: {key} {deviation:.2e}");
    assert!(
        deviation <= tolerance,
        "{label}: {key} is {actual:e}, the prototype measured {recorded:e}"
    );
    actual
}

/// `conftest.run` with the case's parameter changes and events; also checks `report()`.
fn process(record: &Value, input: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let registry = builtin_registry();
    let plugin = record["plugin"].as_str().expect("a plugin");
    let descriptor = registry.get(plugin).expect("a built-in plugin");
    let base = params(&record["params"]);
    let mut processor = create(plugin).unwrap_or_else(|| panic!("no processor for {plugin}"));
    processor.prepare(ProcessSpec {
        sample_rate: SR,
        block_size: BLOCK,
        channels: 2,
    });
    processor.set_params(&slots(descriptor, &base));
    // `on_block`: `set_params({**p._params, **values})` before the block is processed.
    let changes: Vec<(usize, ParamSlots)> = items(&record["changes"])
        .iter()
        .map(|change| {
            let mut merged = base.clone();
            merged.extend(params(&change[1]));
            (count(&change[0]), slots(descriptor, &merged))
        })
        .collect();
    let events: Vec<(usize, InstrumentEvent)> = items(&record["events"])
        .iter()
        .map(|e| {
            let byte = |v: &Value| u8::try_from(count(v)).expect("a MIDI byte");
            let event = InstrumentEvent {
                offset: 0,
                kind: event_kind(e[1].as_str().expect("an event kind")),
                pitch: byte(&e[2]),
                velocity: byte(&e[3]),
                channel: 0,
                value: i32::try_from(e[4].as_i64().expect("a value")).expect("an i32"),
                number: byte(&e[5]),
                articulation: None,
            };
            (count(&e[0]), event)
        })
        .collect();
    let bpm = number(&record["bpm"]);
    let blocks = input.len().div_ceil(BLOCK);
    let mut output = vec![[0.0; 2]; blocks * BLOCK];
    output[..input.len()].copy_from_slice(input);
    for (b, block) in output.as_chunks_mut::<BLOCK>().0.iter_mut().enumerate() {
        for (_, change) in changes.iter().filter(|(at, _)| *at == b) {
            processor.set_params(change);
        }
        let a = b * BLOCK;
        let block_events: Vec<InstrumentEvent> = events
            .iter()
            .filter(|(frame, _)| (a..a + BLOCK).contains(frame))
            .map(|&(frame, event)| InstrumentEvent {
                offset: frame - a,
                ..event
            })
            .collect();
        let ctx = BlockContext {
            frame: i64::try_from(a).expect("a frame"),
            sample_rate: SR,
            beat: as_f64(a) / SRF * bpm / 60.0,
            bpm,
            playing: true,
            sidechain: None,
            events: &block_events,
        };
        processor.process(block, &ctx);
    }
    output.truncate(input.len());
    let report = processor.report();
    let recorded = record["report"].as_object().expect("a report");
    assert_eq!(
        report.entries().len(),
        recorded.len(),
        "{plugin}: report keys"
    );
    for (key, value) in report.entries() {
        let expected = number(&recorded[*key]);
        assert!(
            (value - expected).abs() <= REPORT_TOLERANCE * expected.abs().max(1.0),
            "{plugin}: report {key} is {value}, the prototype's {expected}"
        );
    }
    output
}

// -- measurements (the Python tests' helpers) ---------------------------------------------------

/// `conftest.rms_db`: `20 log10(sqrt(mean(x²)))`, or −400 for silence (NumPy's pairwise mean).
fn rms_db(x: &[f64]) -> f64 {
    let squares: Vec<f64> = x.iter().map(|v| v * v).collect();
    let rms = mean(&squares).sqrt();
    if rms > 0.0 {
        20.0 * rms.log10()
    } else {
        -400.0
    }
}

fn peak(x: &[f64]) -> f64 {
    x.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
}

fn max_jump(x: &[f64]) -> f64 {
    x.windows(2)
        .fold(0.0_f64, |m, w| m.max((w[1] - w[0]).abs()))
}

/// `np.hanning(n)`: `0.5 + 0.5 cos(pi k / (n - 1))` for `k = 1 - n, 3 - n, …, n - 1`.
fn hanning(n: usize) -> Vec<f64> {
    let m = as_f64(n) - 1.0;
    (0..n)
        .map(|i| 0.5 + 0.5 * (PI * (2.0 * as_f64(i) - m) / m).cos())
        .collect()
}

/// `np.abs(np.fft.rfft(x, size))`: `x` zero-padded to `size`.
fn rfft_magnitudes(x: &[f64], size: usize) -> Vec<f64> {
    let fft = RealFftPlanner::<f64>::new().plan_fft_forward(size);
    let mut input = fft.make_input_vec();
    input[..x.len()].copy_from_slice(x);
    let mut bins = fft.make_output_vec();
    fft.process(&mut input, &mut bins)
        .expect("an FFT of its own size");
    bins.iter().map(|z| z.norm()).collect()
}

/// `np.fft.rfftfreq(size, 1 / SR)[k]`.
fn bin_hz(k: usize, size: usize) -> f64 {
    as_f64(k) * (1.0 / (as_f64(size) * (1.0 / SRF)))
}

fn windowed(x: &[f64]) -> Vec<f64> {
    x.iter().zip(hanning(x.len())).map(|(v, w)| v * w).collect()
}

/// `test_instruments.fundamental`: the peak of the Hann-windowed spectrum, 8x zero-padded,
/// above 20 Hz, refined by log-parabolic interpolation.
fn fundamental(segment: &[f64]) -> f64 {
    let size = 8 * segment.len();
    let mut spectrum = rfft_magnitudes(&windowed(segment), size);
    for (k, s) in spectrum.iter_mut().enumerate() {
        if bin_hz(k, size) < 20.0 {
            *s = 0.0;
        }
    }
    let top = argmax(&spectrum);
    let [below, at, above] = [top - 1, top, top + 1].map(|k| (spectrum[k] + 1e-300).ln());
    let shift = 0.5 * (below - above) / (below - 2.0 * at + above);
    bin_hz(top, size) + shift * (bin_hz(1, size) - bin_hz(0, size))
}

/// NumPy's `argmax`: the first index of the largest value.
fn argmax(x: &[f64]) -> usize {
    let mut best = 0;
    for (i, v) in x.iter().enumerate() {
        if *v > x[best] {
            best = i;
        }
    }
    best
}

fn cents(f: f64, reference: f64) -> f64 {
    1200.0 * (f / reference).log2()
}

fn interleaved(frames: &[[f64; 2]]) -> Vec<f64> {
    frames.as_flattened().to_vec()
}

fn true_peak_dbtp(frames: &[[f64; 2]]) -> f64 {
    analyze(&interleaved(frames), SR, 2, 65_536)
        .expect("a stereo analysis")
        .true_peak_dbtp
}

// -- V2: the compressor's RMS detector, time constants and auto makeup; the expander ----------

/// `test_compressor_rms_detector`: a −6 dBFS sine, 10 ms RMS window: the output peak follows
/// the RMS level's static curve within 0.7 dB.
#[test]
fn compressor_rms_detector() {
    let fixture = fixture("behaviour-v1.json");
    let case = Case::run(&fixture, "compressor_rms_detector");
    let out_peak = case.metric(
        "out_peak_db",
        20.0 * peak(&case.channel(0, case.output.len() - 12_000)).log10(),
    );
    let rms_level = -6.0 - 3.0103;
    let gr = rms_level - (-20.0 + (rms_level + 20.0) / 4.0);
    assert!((out_peak - (-6.0 - gr)).abs() <= 0.7, "{out_peak}");
}

/// `test_compressor_attack_and_release_time_constants`: one attack time constant after a step
/// the reduction is `1 − 1/e` of its final value, after five it is settled, and one release
/// time constant after the drop it is `1/e` of where it was.
#[test]
fn compressor_attack_and_release_time_constants() {
    let fixture = fixture("behaviour-v1.json");
    let case = Case::run(&fixture, "compressor_attack_release");
    let gr: Vec<f64> = case
        .input
        .iter()
        .zip(&case.output)
        .map(|(x, y)| 20.0 * x[0].log10() - 20.0 * y[0].log10())
        .collect();
    let (step, rel) = (9_600, 9_600 + 24_000);
    let final_gr = case.metric("final_gr_db", gr[step + 21_600]);
    let attack = case.metric("attack_fraction", gr[step + 480] / final_gr);
    let settled = case.metric("attack_fraction_50ms", gr[step + 2_400] / final_gr);
    let release = case.metric("release_fraction", gr[rel + 4_800] / gr[rel]);
    assert!(
        (attack - (1.0 - (-1.0_f64).exp())).abs() <= 0.05,
        "{attack}"
    );
    assert!(settled > 0.99, "{settled}");
    assert!((release - 1.0 / E).abs() <= 0.05, "{release}");
}

/// Auto makeup, which no Python test sets: it adds half the reduction a 0 dBFS signal receives
/// (`compressor.py`) to the manual makeup. On full-scale DC (threshold −20 dB, ratio 4) the
/// static curve gives −15 dB, so the output settles at `10^((−15 + 7.5 + makeup) / 20)`; the
/// tolerance is `test_compressor_parallel_mix_and_makeup`'s.
#[test]
fn compressor_auto_makeup() {
    let fixture = fixture("behaviour-v1.json");
    for (name, makeup_db) in [
        ("compressor_auto_makeup", 0.0),
        ("compressor_auto_and_manual_makeup", 3.0),
    ] {
        let case = Case::run(&fixture, name);
        let last = case.metric("last", case.output.last().expect("output")[0]);
        let expected = 10f64.powf((-15.0 + 7.5 + makeup_db) / 20.0);
        let recorded = number(&case.record["metrics"]["expected_last"]);
        assert!((expected - recorded).abs() <= 1e-12, "{name}: the curve");
        assert!(
            (last - expected).abs() <= 1e-4 * expected,
            "{name}: {last} vs {expected}"
        );
    }
}

/// `test_expander_ratio`: 10 dB below the threshold at ratio 2 the expander takes 10 dB more,
/// within 1.5 dB.
#[test]
fn expander_ratio() {
    let fixture = fixture("behaviour-v1.json");
    let case = Case::run(&fixture, "expander_ratio");
    let from = case.output.len() - 12_000;
    let x: Vec<f64> = case.input[from..].iter().map(|f| f[0]).collect();
    let gain = case.metric("gain_db", rms_db(&case.channel(0, from)) - rms_db(&x));
    assert!((gain - -10.0).abs() <= 1.5, "{gain}");
}

// -- E1: robustness ------------------------------------------------------------------------------

/// The built-in effects of `test_effects.EFFECTS`.
const EFFECTS: [&str; 10] = [
    "sonara.utility",
    "sonara.eq.parametric",
    "sonara.filter",
    "sonara.dynamics.compressor",
    "sonara.dynamics.limiter",
    "sonara.dynamics.gate",
    "sonara.delay",
    "sonara.reverb",
    "sonara.convolver",
    "sonara.saturation",
];

/// `test_finite_and_bounded` (full-scale DC, a full-scale 19 kHz sine) and
/// `test_decays_to_silence_without_blowup` (0.3 s of loud noise, then 6 s of silence) for every
/// effect with its defaults: finite, below 20, and below 1e-3 in the last half second. The
/// other signals of `test_finite_and_bounded` are golden renders.
#[test]
fn every_effect_is_finite_bounded_and_decays() {
    let fixture = fixture("behaviour-v1.json");
    for plugin in EFFECTS {
        for signal in ["dc", "hf_sine", "decay"] {
            let case = Case::run(&fixture, &format!("{plugin}/{signal}"));
            let all = interleaved(&case.output);
            assert!(all.iter().all(|x| x.is_finite()), "{plugin}/{signal}");
            let level = case.metric("peak", peak(&all));
            assert!(level < 20.0, "{plugin}/{signal}: {level}");
            if signal == "decay" {
                let tail = interleaved(&case.output[case.output.len() - 24_000..]);
                let tail = case.metric("tail_peak", peak(&tail));
                assert!(tail < 1e-3, "{plugin}: {tail}");
            }
        }
    }
}

// -- E4 to E10 -----------------------------------------------------------------------------------

/// `test_utility_width_polarity_swap_gain`, width 0 and 2 (the swap, the polarity and the gain
/// are unit tests of `processors::utility`), and `test_utility_dc_block`.
#[test]
fn utility_width_and_dc_block() {
    let fixture = fixture("behaviour-v1.json");
    let mono = Case::run(&fixture, "utility_width_0");
    let tail = &mono.output[1024..];
    let difference = mono.metric(
        "channel_difference",
        tail.iter().fold(0.0_f64, |m, f| m.max((f[0] - f[1]).abs())),
    );
    let mid = mono.metric(
        "mid_difference",
        tail.iter()
            .zip(&mono.input[1024..])
            .fold(0.0_f64, |m, (y, x)| {
                m.max((y[0] - f64::midpoint(x[0], x[1])).abs())
            }),
    );
    assert!(difference <= 1e-12 && mid <= 1e-12, "{difference} {mid}");
    let wide = Case::run(&fixture, "utility_width_2");
    let last = wide.output.last().expect("output");
    let left = wide.metric("last_left", last[0]);
    let right = wide.metric("last_right", last[1]);
    assert!((left - 1.5).abs() <= 1e-9 && (right - -0.5).abs() <= 1e-9);
    let blocked = Case::run(&fixture, "utility_dc_block");
    let tail = blocked.channel(0, blocked.output.len() - 24_000);
    let tail_mean = blocked.metric("tail_mean", mean(&tail));
    assert!(tail_mean.abs() < 1e-3, "{tail_mean}");
}

/// `test_multimode_filter_lowpass_and_highpass` through `_steady_gain_db`: the low-pass passes
/// 100 Hz and cuts 8 kHz, the high-pass cuts 100 Hz and passes 10 kHz, and the band-pass peaks
/// at its cutoff by more than 10 dB over two octaves away.
#[test]
fn multimode_filter_modes() {
    let fixture = fixture("behaviour-v1.json");
    let gain = |name: &str| {
        let case = Case::run(&fixture, name);
        let x: Vec<f64> = case.input[12_000..].iter().map(|f| f[0]).collect();
        case.metric("gain_db", rms_db(&case.channel(0, 12_000)) - rms_db(&x))
    };
    assert!(gain("filter_lowpass_100").abs() <= 0.3);
    assert!(gain("filter_lowpass_8000") < -30.0);
    assert!(gain("filter_highpass_100") < -30.0);
    assert!(gain("filter_highpass_10000").abs() <= 0.5);
    let peak = gain("filter_bandpass_1000");
    assert!(peak > gain("filter_bandpass_250") + 10.0);
    assert!(peak > gain("filter_bandpass_4000") + 10.0);
}

/// `test_saturation_oversampling_reduces_aliasing`: every harmonic of a hard-clipped 15 kHz sine
/// lies above Nyquist, so whatever else is in band is aliasing; 4x oversampling keeps it 20 dB
/// lower than 1x.
#[test]
fn saturation_oversampling_reduces_aliasing() {
    let fixture = fixture("behaviour-v1.json");
    let alias_db = |name: &str| {
        let case = Case::run(&fixture, name);
        let y = case.channel(0, 12_000);
        let size = y.len();
        let power: Vec<f64> = rfft_magnitudes(&windowed(&y), size)
            .iter()
            .map(|m| m * m)
            .collect();
        let (mut fundamental, mut alias) = (0.0, 0.0);
        for (k, p) in power.iter().enumerate() {
            let f = bin_hz(k, size);
            if (f - 15_000.0).abs() < 50.0 || f < 20.0 {
                fundamental += p;
            } else {
                alias += p;
            }
        }
        case.metric("alias_db", 10.0 * (alias / fundamental).log10())
    };
    let one = alias_db("saturation_alias_1x");
    let four = alias_db("saturation_alias_4x");
    assert!(four < one - 20.0, "{four} vs {one}");
}

/// `test_delay_time_change_is_click_free`: after the delay time changes at block 50, no step
/// exceeds three times the sine's slope over `1 − feedback`.
#[test]
fn delay_time_change_is_click_free() {
    let fixture = fixture("behaviour-v1.json");
    let case = Case::run(&fixture, "delay_time_change");
    let jump = case.metric("max_jump", max_jump(&case.channel(0, 19_200)));
    let max_dx = 2.0 * PI * 220.0 / SRF * 0.5;
    assert!(jump < 3.0 * max_dx / (1.0 - 0.3), "{jump}");
}

/// `test_limiter_true_peak_on_band_limited_material` (drives of 6, 12 and 18 dB: within 0.1 dB
/// of the −1 dBTP ceiling) and `…_on_full_band_noise_has_bounded_overshoot` (seeds 1 to 3:
/// within 0.75 dB), measured by the Analyzer after the first half second.
#[test]
fn limiter_true_peak_ceiling() {
    let fixture = fixture("behaviour-v1.json");
    for (name, bound) in [
        ("limiter_band_limited_6", 0.1),
        ("limiter_band_limited_12", 0.1),
        ("limiter_band_limited_18", 0.1),
        ("limiter_full_band_noise_1", 0.75),
        ("limiter_full_band_noise_2", 0.75),
        ("limiter_full_band_noise_3", 0.75),
    ] {
        let case = Case::run(&fixture, name);
        let tp = case.metric("true_peak_dbtp", true_peak_dbtp(&case.output[24_000..]));
        assert!(tp <= -1.0 + bound, "{name}: {tp} dBTP");
    }
}

/// `test_effects._schroeder_rt60`: the energy decay curve's slope between −5 and −35 dB, by a
/// least-squares line (`np.polyfit(t, edc_db, 1)`), extrapolated to −60 dB.
fn schroeder_rt60(ir: &[f64]) -> f64 {
    let mut edc: Vec<f64> = ir.iter().rev().map(|x| x * x).collect();
    let mut total = 0.0;
    for e in &mut edc {
        total += *e;
        *e = total;
    }
    edc.reverse();
    let edc_db: Vec<f64> = edc
        .iter()
        .map(|e| 10.0 * (e / edc[0]).max(1e-30).log10())
        .collect();
    let first_below = |db: f64| edc_db.iter().position(|v| *v < db).unwrap_or(0);
    let (i5, i35) = (first_below(-5.0), first_below(-35.0));
    let t: Vec<f64> = (i5..i35).map(|i| as_f64(i) / SRF).collect();
    let y = &edc_db[i5..i35];
    let n = as_f64(t.len());
    let (mt, my) = (t.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let sxy: f64 = t.iter().zip(y).map(|(a, b)| (a - mt) * (b - my)).sum();
    let sxx: f64 = t.iter().map(|a| (a - mt) * (a - mt)).sum();
    -60.0 / (sxy / sxx)
}

/// `test_reverb_rt60_and_normalization`: the plate's RT60 within 15 % of its decay time, and
/// its impulse response normalized to unit energy within 5 %; and
/// `test_reverb_parameter_change_crossfades`: a decay and character change at block 100
/// crossfades, so no step around it is more than 2.5 times the largest step before it.
#[test]
fn reverb_rt60_normalization_and_crossfade() {
    let fixture = fixture("behaviour-v1.json");
    for (name, decay) in [("reverb_rt60_1.0", 1.0), ("reverb_rt60_2.5", 2.5)] {
        let case = Case::run(&fixture, name);
        let rt60 = case.metric("rt60", schroeder_rt60(&case.channel(0, 0)));
        let energy = (0..2)
            .map(|c| case.output.iter().map(|f| f[c] * f[c]).sum::<f64>())
            .fold(0.0_f64, f64::max);
        let energy = case.metric("energy", energy);
        assert!((rt60 - decay).abs() <= 0.15 * decay, "{name}: RT60 {rt60}");
        assert!((energy - 1.0).abs() <= 0.05, "{name}: energy {energy}");
    }
    let case = Case::run(&fixture, "reverb_parameter_change");
    let y = case.channel(0, 0);
    let around = case.metric("around", max_jump(&y[99 * 512..=102 * 512]));
    let elsewhere = case.metric("elsewhere", max_jump(&y[60 * 512..=95 * 512]));
    assert!(around < 2.5 * elsewhere, "{around} vs {elsewhere}");
}

/// `test_gain_change_is_smoothed`: a 20 dB gain step at block 40 ramps, so no step of a 100 Hz
/// sine exceeds 1.5 times its slope.
#[test]
fn gain_change_is_smoothed() {
    let fixture = fixture("behaviour-v1.json");
    let case = Case::run(&fixture, "utility_gain_change");
    let jump = case.metric("max_jump", max_jump(&case.channel(0, 0)));
    let max_dx = 2.0 * PI * 100.0 / SRF * 0.5;
    assert!(jump < 1.5 * max_dx, "{jump}");
}

// -- I1 to I3: the instruments -------------------------------------------------------------------

/// `test_synth_pitch_accuracy`, `test_synth_waveforms_fundamental_and_bandlimiting` and
/// `test_synth_detune_and_octave`: a single voice (`PURE`) plays within 2 cents of its pitch,
/// every waveform's fundamental is the note's and stays below 2, and an octave up with 50
/// cents of detune lands 50 cents above A5.
#[test]
fn synth_pitch_waveforms_detune_and_octave() {
    let fixture = fixture("behaviour-v1.json");
    let measure = |name: &str| {
        let case = Case::run(&fixture, name);
        let seg = case.channel(0, 12_000);
        assert!(seg.iter().all(|x| x.is_finite()), "{name}");
        let level = case.metric("peak", peak(&seg));
        (case.metric("fundamental_hz", fundamental(&seg)), level)
    };
    for pitch in [33_i32, 45, 60, 69, 81, 96] {
        let (f, _) = measure(&format!("synth_pitch_{pitch}"));
        let expected = 440.0 * 2f64.powf(f64::from(pitch - 69) / 12.0);
        assert!(cents(f, expected).abs() < 2.0, "{pitch}: {f} Hz");
    }
    for wave in ["saw", "square", "pulse", "triangle"] {
        let (f, level) = measure(&format!("synth_wave_{wave}"));
        assert!(
            cents(f, 440.0).abs() < 2.0 && level < 2.0,
            "{wave}: {f} Hz, {level}"
        );
    }
    let (f, _) = measure("synth_detune_and_octave");
    assert!((cents(f, 440.0) - 50.0).abs() <= 2.0, "{f} Hz");
}

/// `test_synth_velocity_sensitivity` (full sensitivity: velocity 127 is more than 3.5 times as
/// loud as 64) and `test_drum_velocity_scales_level` (a snare at 127 is more than 3 times as
/// loud as at 40).
#[test]
fn velocity_scales_the_level() {
    let fixture = fixture("behaviour-v1.json");
    let synth = |name: &str| {
        let case = Case::run(&fixture, name);
        case.metric("level", peak(&case.channel(0, 12_000)))
    };
    assert!(synth("synth_velocity_127") > 3.5 * synth("synth_velocity_64"));
    let drums = |name: &str| {
        let case = Case::run(&fixture, name);
        case.metric("level", peak(&interleaved(&case.output)))
    };
    assert!(drums("drums_velocity_127") > 3.0 * drums("drums_velocity_40"));
}

/// `test_unmapped_drum_notes_are_ignored_and_counted`: notes 20 and 100 play nothing and are
/// counted in `report()["ignored_notes"]` (checked against the recording by the runner).
#[test]
fn unmapped_drum_notes_are_ignored_and_counted() {
    let fixture = fixture("behaviour-v1.json");
    let case = Case::run(&fixture, "drums_unmapped");
    assert!(case.output.iter().all(|f| *f == [0.0, 0.0]), "silence");
    assert_eq!(case.metric("peak", peak(&interleaved(&case.output))), 0.0);
    assert_eq!(number(&case.record["report"]["ignored_notes"]), 2.0);
}

// -- K1, K2: the kernels -------------------------------------------------------------------------

/// The kernels against the prototype's native kernels (`tests/fixtures/kernels-v1.json`,
/// through the Python wrappers the tests call): relative 1e-12, the tolerance of
/// `golden_kernels.rs`, on `Σ|x|` and, times the peak (at least 1), on every sampled value. On
/// macOS libm the kernels are bit-exact; another libm's `tan`, `exp2` or `sin` may differ by an
/// ulp (`golden_kernels.rs`).
const KERNEL_TOLERANCE: f64 = 1e-12;

fn check_kernel(name: &str, samples: &[f64], channels: usize, record: &Value) {
    let peak = number(&record["output"]["peak"]).max(1.0);
    check_fingerprint(
        name,
        samples,
        channels,
        &record["output"],
        KERNEL_TOLERANCE,
        KERNEL_TOLERANCE * peak,
    );
}

/// `test_svf_lowpass_unity_dc_and_stable_at_extremes`: a low-pass passes DC at unity (within
/// 1e-9 after one second), and a band-pass at a cutoff above Nyquist with a Q of 50 stays
/// finite and below 1e3. The prototype's wrapper clamps the cutoff to 0.49 × the rate and Q to
/// at least 0.05 before the native kernel runs, so the test does too.
#[test]
fn svf_lowpass_unity_dc_and_stable_at_extremes() {
    let fixture = fixture("kernels-v1.json");
    for name in ["svf_lowpass_dc", "svf_bandpass_extreme"] {
        let record = &fixture["cases"][name];
        let (x, channels) = signal(&record["signal"]);
        if let Some(input) = record.get("input") {
            check_fingerprint(name, &x, channels, input, INPUT_TOLERANCE, INPUT_TOLERANCE);
        }
        let frames = x.len() / channels;
        let cutoff = vec![clamp_cutoff(number(&record["cutoff_hz"]), SRF); frames];
        let mode = i32::try_from(record["mode"].as_i64().expect("a mode")).expect("an i32");
        let (mut y, mut state) = (vec![0.0; x.len()], vec![[0.0; 2]; channels]);
        svf(
            &x,
            &mut y,
            &cutoff,
            clamp_q(number(&record["q"])),
            SvfMode::from_raw(mode),
            SRF,
            &mut state,
        );
        check_kernel(name, &y, channels, record);
        assert!(y.iter().all(|v| v.is_finite()), "{name}");
        if name == "svf_lowpass_dc" {
            let last = check_metric(name, record, "last", y[y.len() - 1], METRIC_TOLERANCE);
            assert!((last - 1.0).abs() < 1e-9, "{last}");
        } else {
            let level = check_metric(name, record, "peak", peak(&y), METRIC_TOLERANCE);
            assert!(level < 1e3, "{level}");
        }
    }
}

/// `test_osc_bank_waveforms_are_bounded_and_periodic`: each of the five waveforms at A4 for one
/// second stays finite and within 1.3 (the `PolyBLEP` overshoot), and its Hann-windowed spectrum
/// (1 Hz bins) peaks at 440 Hz within one bin.
#[test]
fn osc_bank_waveforms_are_bounded_and_periodic() {
    let fixture = fixture("kernels-v1.json");
    for wave in 0..5 {
        let name = format!("osc_bank_wave_{wave}");
        let record = &fixture["cases"][&name];
        let n = 48_000;
        let mut oscillators = [Oscillator {
            wave: Wave::from_raw(wave),
            level: 1.0,
            pitch_offset: 0.0,
            pan_l: 1.0,
            pan_r: 1.0,
            phase: 0.0,
        }];
        let (mut left, mut right) = (vec![0.0; n], vec![0.0; n]);
        osc_bank(
            &vec![69.0; n],
            &mut oscillators,
            0.3,
            SRF,
            &mut left,
            &mut right,
        );
        check_kernel(&name, &left, 1, record);
        check_metric(
            &name,
            record,
            "phase",
            oscillators[0].phase,
            KERNEL_TOLERANCE,
        );
        assert!(left.iter().all(|v| v.is_finite()), "{name}");
        let level = check_metric(&name, record, "peak", peak(&left), METRIC_TOLERANCE);
        assert!(level <= 1.3, "{name}: {level}");
        let bin = argmax(&rfft_magnitudes(&windowed(&left), n));
        assert_eq!(
            as_f64(bin),
            number(&record["metrics"]["argmax_bin"]),
            "{name}"
        );
        assert!(bin.abs_diff(440) <= 1, "{name}: {bin}");
    }
}
