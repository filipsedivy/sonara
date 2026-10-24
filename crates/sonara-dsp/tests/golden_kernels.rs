//! Golden parity of the four kernels (GLD-01).
//!
//! `golden/kernels/` was recorded by `scripts/golden/area_kernels.py` through the prototype's
//! native C++ kernels: uneven chunks (`index.json` → `chunks`, repeating) with the state carried
//! across calls. Every case replays the same input with the same chunking, and compares every
//! output sample and the state after every chunk.
//!
//! The kernels use the oracle's fused multiply-adds (the forms
//! were read from clang's arm64 output for `native/src/kernels.hpp` and reproduce every golden
//! vector bit for bit on the recording host). Tolerances per family:
//!
//! - `ar_smooth` and `gate_hold` call no libm function: outputs and states are bit-exact on
//!   every platform.
//! - `svf` calls `tan`, `osc_bank` calls `exp2` and `sin`. Where the platform's libm is not
//!   macOS libm, those differ by an ulp or two in some samples (glibc's `tan` in 1267 of the 4096
//!   cutoffs of the sweep, up to 2 ulp), so these two keep the relative
//!   tolerance 1e-12, measured against a floor (see [`FLOOR_REL`]).

use std::ops::Range;

use serde_json::Value;
use sonara_dsp::kernels::{
    GateState, Oscillator, SvfMode, Wave, ar_smooth, clamp_cutoff, clamp_q, gate_hold, osc_bank,
    svf,
};
use sonara_testkit::{F64Array, Golden, max_relative_error};

/// Relative tolerance of
const TOLERANCE: f64 = 1e-12;

/// The error is `|actual − expected| / max(|expected|, FLOOR_REL × peak)`, where `peak` is the
/// largest magnitude of the case's recorded output.
///
/// A pure relative error is meaningless near zero. A value there is the difference of larger
/// terms (zero crossings of oscillators, a high-pass in its stopband: `v0 − k·v1 − v2`), so its
/// rounding noise is that of the *terms* (about 1e-16 of the signal scale), not of the value.
///
/// With 1e-2 the bound is "relative 1e-12, or about 1e-14 × peak absolute near zero". Measured
/// before the kernels used the oracle's fused forms: the absolute error of every `svf` case was
/// at most 2e-15 × peak, but its pure relative error reached 9e-10 in the high-pass stopband, and
/// a one-ulp change of a single `tan()` result (macOS libm recorded the golden data, glibc ran
/// this test) moves it by the same amount. With the fused forms, the remaining difference is the
/// libm one only; measured in the Linux container: `svf` 1.6e-13, `osc_bank` 3.4e-14 against
/// this floor. Values above the floor are all compared at 1e-12 relative.
/// Exact zeros need no special case: they only have to stay within the absolute bound.
const FLOOR_REL: f64 = 1e-2;

fn golden() -> Golden {
    Golden::open().expect("golden data is available (run `git lfs pull` or set SONARA_GOLDEN)")
}

fn array(golden: &Golden, rel: &str) -> F64Array {
    golden
        .f64_array(rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .expect("a JSON array")
        .iter()
        .map(|v| v.as_f64().expect("a JSON number"))
        .collect()
}

fn header_numbers(header: &Value, key: &str) -> Vec<f64> {
    numbers(
        header
            .get(key)
            .unwrap_or_else(|| panic!("header has no {key}")),
    )
}

fn header_f64(header: &Value, key: &str) -> f64 {
    header
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("header has no number {key}"))
}

/// Per-chunk vectors recorded in the header (`states`, `phases`), flattened per chunk.
fn chunk_states(header: &Value, key: &str) -> Vec<Vec<f64>> {
    header
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("header has no {key}"))
        .iter()
        .map(|chunk| {
            let mut flat = Vec::new();
            flatten(chunk, &mut flat);
            flat
        })
        .collect()
}

fn flatten(value: &Value, out: &mut Vec<f64>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| flatten(item, out)),
        other => out.push(other.as_f64().expect("a JSON number")),
    }
}

/// The recorder's chunk sizes and the cases of one kernel family.
struct Index {
    chunks: Vec<usize>,
    sample_rate: f64,
    cases: Vec<String>,
}

fn index(golden: &Golden, family: &str, expected_cases: usize) -> Index {
    let index: Value = golden
        .json("kernels/index.json")
        .expect("kernels/index.json");
    let chunks = header_numbers(&index, "chunks")
        .into_iter()
        .map(|c| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let size = c as usize;
            size
        })
        .collect();
    let cases: Vec<String> = index
        .get("cases")
        .and_then(Value::as_array)
        .expect("index has cases")
        .iter()
        .filter_map(|c| c.as_str())
        .filter(|c| c.starts_with(&format!("{family}/")))
        .map(str::to_owned)
        .collect();
    assert_eq!(cases.len(), expected_cases, "{family}: cases in the index");
    Index {
        chunks,
        sample_rate: header_f64(&index, "sample_rate"),
        cases,
    }
}

/// The recorder's chunking: sizes repeat; the last chunk is cut at `len`.
fn chunk_ranges(len: usize, sizes: &[usize]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for &size in sizes.iter().cycle() {
        if start >= len {
            break;
        }
        let end = len.min(start + size);
        ranges.push(start..end);
        start = end;
    }
    ranges
}

/// Compares and reports the worst relative error; fails above [`TOLERANCE`].
fn check(what: &str, expected: &[f64], actual: &[f64], peak: f64) -> f64 {
    assert_eq!(expected.len(), actual.len(), "{what}: length");
    let error = max_relative_error(expected, actual, FLOOR_REL * peak);
    assert!(
        error <= TOLERANCE,
        "{what}: relative error {error:e} > {TOLERANCE:e}"
    );
    error
}

fn peak(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |peak, v| peak.max(v.abs()))
}

fn check_states(what: &str, expected: &[Vec<f64>], actual: &[Vec<f64>], peak: f64) -> f64 {
    assert_eq!(expected.len(), actual.len(), "{what}: number of chunks");
    expected
        .iter()
        .zip(actual)
        .enumerate()
        .map(|(i, (e, a))| check(&format!("{what}, state after chunk {i}"), e, a, peak))
        .fold(0.0, f64::max)
}

#[test]
fn ar_smooth_matches_golden() {
    let golden = golden();
    let index = index(&golden, "ar_smooth", 2);
    for case in &index.cases {
        let x = array(&golden, &format!("kernels/{case}.in")).data;
        let recorded = array(&golden, &format!("kernels/{case}.out"));
        let attack = header_f64(&recorded.header, "attack");
        let release = header_f64(&recorded.header, "release");

        let mut out = vec![0.0; x.len()];
        let mut state = 0.0;
        let mut states = Vec::new();
        for range in chunk_ranges(x.len(), &index.chunks) {
            ar_smooth(
                &x[range.clone()],
                &mut out[range],
                attack,
                release,
                &mut state,
            );
            states.push(vec![state]);
        }
        // No libm call: bit-exact on every platform.
        assert_eq!(bits(&out), bits(&recorded.data), "{case}: outputs");
        let expected_states = chunk_states(&recorded.header, "states");
        assert_eq!(states.len(), expected_states.len(), "{case}: chunks");
        for (i, (actual, expected)) in states.iter().zip(&expected_states).enumerate() {
            assert_eq!(
                bits(actual),
                bits(expected),
                "{case}: state after chunk {i}"
            );
        }
    }
}

/// Bit patterns, so that equality means identical (and `-0.0` differs from `0.0`).
fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn svf_matches_golden() {
    let golden = golden();
    let index = index(&golden, "svf", 24);
    let input = array(&golden, "kernels/svf/input");
    let cutoff = array(&golden, "kernels/svf/cutoff").data;
    assert_eq!(input.shape, [cutoff.len(), 2], "stereo input");
    // The recorder went through the Python wrapper, which clamps cutoff and Q before the native
    // kernel; the kernel itself does not (the sweep ends above 0.49 × the sample rate).
    let cutoff: Vec<f64> = cutoff
        .iter()
        .map(|c| clamp_cutoff(*c, index.sample_rate))
        .collect();
    let channels = input.shape[1];

    let mut worst = 0.0_f64;
    for case in &index.cases {
        let recorded = array(&golden, &format!("kernels/{case}"));
        let q = clamp_q(header_f64(&recorded.header, "q"));
        #[allow(clippy::cast_possible_truncation)]
        let mode = SvfMode::from_raw(header_f64(&recorded.header, "mode") as i32);

        let mut out = vec![0.0; input.data.len()];
        let mut state = vec![[0.0; 2]; channels];
        let mut states = Vec::new();
        for range in chunk_ranges(cutoff.len(), &index.chunks) {
            let samples = range.start * channels..range.end * channels;
            svf(
                &input.data[samples.clone()],
                &mut out[samples],
                &cutoff[range],
                q,
                mode,
                index.sample_rate,
                &mut state,
            );
            states.push(state.iter().flatten().copied().collect());
        }
        worst = worst
            .max(check(case, &recorded.data, &out, peak(&recorded.data)))
            .max(check_states(
                case,
                &chunk_states(&recorded.header, "states"),
                &states,
                peak(&recorded.data),
            ));
    }
    println!("svf: max relative error {worst:e}");
}

#[test]
fn gate_hold_matches_golden() {
    let golden = golden();
    let index = index(&golden, "gate_hold", 3);
    let level = array(&golden, "kernels/gate_hold/level").data;
    for case in &index.cases {
        let recorded = array(&golden, &format!("kernels/{case}"));
        let open_db = header_f64(&recorded.header, "open_db");
        let close_db = header_f64(&recorded.header, "close_db");
        let hold = recorded
            .header
            .get("hold")
            .and_then(Value::as_i64)
            .expect("an integer hold");

        let mut out = vec![0.0; level.len()];
        let mut state = GateState::default();
        let mut states = Vec::new();
        for range in chunk_ranges(level.len(), &index.chunks) {
            gate_hold(
                &level[range.clone()],
                &mut out[range],
                open_db,
                close_db,
                hold,
                &mut state,
            );
            #[allow(clippy::cast_precision_loss)]
            states.push(vec![
                f64::from(u8::from(state.is_open)),
                state.hold_remaining as f64,
            ]);
        }
        // A state machine: outputs and states are exact.
        assert_eq!(out, recorded.data, "{case}: outputs");
        assert_eq!(
            states,
            chunk_states(&recorded.header, "states"),
            "{case}: states"
        );
    }
}

#[test]
fn osc_bank_matches_golden() {
    let golden = golden();
    let index = index(&golden, "osc_bank", 2);
    let pitch = array(&golden, "kernels/osc_bank/pitch").data;
    let mut worst = 0.0_f64;
    for case in &index.cases {
        let recorded = array(&golden, &format!("kernels/{case}"));
        let header = &recorded.header;
        assert_eq!(recorded.shape, [pitch.len(), 2], "{case}: shape");
        let pulse_width = header_f64(header, "pulse_width");
        let waves = header_numbers(header, "waves");
        let levels = header_numbers(header, "levels");
        let offsets = header_numbers(header, "offsets");
        let pan_l = header_numbers(header, "pan_l");
        let pan_r = header_numbers(header, "pan_r");
        let phases = header_numbers(header, "initial_phases");
        let mut oscillators: Vec<Oscillator> = (0..waves.len())
            .map(|k| {
                #[allow(clippy::cast_possible_truncation)]
                let wave = Wave::from_raw(waves[k] as i32);
                Oscillator {
                    wave,
                    level: levels[k],
                    pitch_offset: offsets[k],
                    pan_l: pan_l[k],
                    pan_r: pan_r[k],
                    phase: phases[k],
                }
            })
            .collect();

        let (mut out_l, mut out_r) = (vec![0.0; pitch.len()], vec![0.0; pitch.len()]);
        let mut states = Vec::new();
        for range in chunk_ranges(pitch.len(), &index.chunks) {
            osc_bank(
                &pitch[range.clone()],
                &mut oscillators,
                pulse_width,
                index.sample_rate,
                &mut out_l[range.clone()],
                &mut out_r[range],
            );
            states.push(oscillators.iter().map(|o| o.phase).collect());
        }
        let out: Vec<f64> = out_l
            .iter()
            .zip(&out_r)
            .flat_map(|(l, r)| [*l, *r])
            .collect();
        worst = worst
            .max(check(case, &recorded.data, &out, peak(&recorded.data)))
            // A phase is a fraction of a cycle in [0, 1): its scale is 1.
            .max(check_states(
                case,
                &chunk_states(header, "phases"),
                &states,
                1.0,
            ));
    }
    println!("osc_bank: max relative error {worst:e}");
}
