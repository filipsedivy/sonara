//! Golden parity of the filter primitives (GLD-01).
//!
//! `filters/` is recorded by `scripts/golden/area_filters.py` through the prototype's own
//! functions on seeded inputs, with the state carried across uneven chunks (`index.json` →
//! `chunks`, repeating). Every case replays the same input with the same chunking and compares
//! every output sample and the state after every chunk.
//!
//! Exactness:
//!
//! - `sosfilt`, `lfilter` (F5 with the corrected middle delays, see `Lfilter`), `upfirdn`,
//!   the `SosFilter` state rules, `SmoothedValue`, `ensure_finite`, the backward minimum
//!   and the delay line call no libm function: bit-exact on every platform.
//! - The RBJ designs call `cos`, `sin` and `pow`. The recorder stored the libm values the
//!   prototype used, so the test splits the two concerns and is exact on every platform: a
//!   reference of the cookbook formulas fed the recorded libm values reproduces the prototype's
//!   rows bit for bit, and `biquad`/`butterworth` equal that reference fed this platform's libm
//!   values bit for bit. Where this platform's libm agrees with macOS libm, the designs therefore
//!   equal the prototype's exactly; the test reports how often that is the case.
//! - `db_to_amp`, `amp_to_db` and `time_coefficient` are one libm call each: within
//!   [`LEVEL_ULPS`] ulps (exact with macOS libm).
//!
//! Processing calls run inside `assert_no_alloc` with the `AllocDisabler` global allocator, so
//! they are shown to allocate and free nothing (RT-02). Comparisons run outside the closures.

use std::ops::Range;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::Value;
use sonara_dsp::filters::{
    BackwardMinimum, BiquadType, ButterworthSlope, DelayLine, Lfilter, Section, Smoothed,
    SmoothedValue, SosFilter, amp_to_db, biquad, butterworth, butterworth_qs, db_to_amp,
    ensure_finite, time_coefficient, upfirdn,
};
use sonara_testkit::{F64Array, Golden};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

/// Largest distance in ulps between a level conversion and the recording: one libm call (`pow`,
/// `log10` or `exp`), whose result may differ by an ulp from macOS libm, plus the exactly
/// rounded arithmetic around it (`20 · log10` can turn one ulp into two).
const LEVEL_ULPS: u64 = 2;

fn golden() -> Golden {
    Golden::open().expect("golden data is available (set SONARA_GOLDEN to a recording)")
}

fn array(golden: &Golden, rel: &str) -> F64Array {
    golden
        .f64_array(rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("no field {key} in {value}"))
}

/// A recorded float; the recorder writes non-finite values as `{"non_finite": "nan"}`.
fn float(value: &Value) -> f64 {
    if let Some(v) = value.as_f64() {
        return v;
    }
    match value.get("non_finite").and_then(Value::as_str) {
        Some("nan") => f64::NAN,
        Some("inf") => f64::INFINITY,
        Some("-inf") => f64::NEG_INFINITY,
        _ => panic!("not a number: {value}"),
    }
}

fn floats(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(float)
        .collect()
}

fn usize_of(value: &Value) -> usize {
    usize::try_from(
        value
            .as_u64()
            .unwrap_or_else(|| panic!("not a count: {value}")),
    )
    .expect("a count fits usize")
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|v| v.to_bits()).collect()
}

fn assert_bits(what: &str, actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{what}: length");
    let differing = actual
        .iter()
        .zip(expected)
        .position(|(a, e)| a.to_bits() != e.to_bits());
    if let Some(i) = differing {
        panic!(
            "{what}: sample {i} is {:e}, expected {:e}",
            actual[i], expected[i]
        );
    }
}

struct Index {
    chunks: Vec<usize>,
    cases: Vec<String>,
}

fn index(golden: &Golden) -> Index {
    let index: Value = golden
        .json("filters/index.json")
        .expect("filters/index.json");
    Index {
        chunks: field(&index, "chunks")
            .as_array()
            .expect("chunks")
            .iter()
            .map(usize_of)
            .collect(),
        cases: field(&index, "cases")
            .as_array()
            .expect("cases")
            .iter()
            .map(|c| c.as_str().expect("a case name").to_owned())
            .collect(),
    }
}

fn cases<'a>(index: &'a Index, family: &str, expected: usize) -> Vec<&'a str> {
    let found: Vec<&str> = index
        .cases
        .iter()
        .map(String::as_str)
        .filter(|c| c.starts_with(&format!("{family}/")) || *c == family)
        .collect();
    assert_eq!(found.len(), expected, "{family}: cases in the index");
    found
}

/// The recorder's chunking: sizes repeat; the last chunk is cut at `len` frames.
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

fn samples(frames: &Range<usize>, channels: usize) -> Range<usize> {
    frames.start * channels..frames.end * channels
}

fn sections(data: &[f64]) -> Vec<Section> {
    let (rows, rest) = data.as_chunks::<6>();
    assert!(rest.is_empty(), "SOS data is a whole number of rows");
    rows.to_vec()
}

/// `zi` as `SciPy` shapes it: `(sections, 2)` for 1-D, `(sections, 2, channels)` for 2-D; returned
/// per channel and section as `[z0, z1]`.
fn recorded_sos_state(zi: &Value, channels: usize) -> Vec<Vec<[f64; 2]>> {
    let rows = zi.as_array().expect("zi rows");
    (0..channels)
        .map(|c| {
            rows.iter()
                .map(|row| {
                    let pair = row.as_array().expect("zi pair");
                    let at = |k: usize| {
                        let v = &pair[k];
                        if channels == 1 && v.is_number() {
                            float(v)
                        } else {
                            float(&v.as_array().expect("zi channels")[c])
                        }
                    };
                    [at(0), at(1)]
                })
                .collect()
        })
        .collect()
}

fn sos_state(filter: &SosFilter) -> Vec<Vec<[f64; 2]>> {
    (0..filter.channels())
        .map(|c| filter.state(c).expect("a channel").to_vec())
        .collect()
}

fn assert_sos_state(what: &str, filter: &SosFilter, zi: &Value) {
    let expected = recorded_sos_state(zi, filter.channels());
    let actual = sos_state(filter);
    let flat = |s: &Vec<Vec<[f64; 2]>>| -> Vec<u64> {
        s.iter()
            .flatten()
            .flat_map(|[a, b]| [a.to_bits(), b.to_bits()])
            .collect()
    };
    assert_eq!(flat(&actual), flat(&expected), "{what}: state");
}

#[test]
fn sosfilt_matches_golden() {
    let golden = golden();
    let index = index(&golden);
    let mono = array(&golden, "filters/sosfilt/mono-input").data;
    let stereo = array(&golden, "filters/sosfilt/stereo-input");
    assert_eq!(stereo.shape, [mono.len(), 2]);
    for case in cases(&index, "sosfilt", 3) {
        let design = sections(&array(&golden, &format!("filters/{case}/sos")).data);

        // 1-D, `zi` carried across chunks.
        let recorded = array(&golden, &format!("filters/{case}/mono"));
        let states = field(&recorded.header, "states")
            .as_array()
            .expect("states");
        let mut filter = SosFilter::new(1, design.len());
        filter.set_sections(&design).expect("capacity");
        let mut out = vec![0.0; mono.len()];
        let ranges = chunk_ranges(mono.len(), &index.chunks);
        assert_eq!(ranges.len(), states.len(), "{case}: chunks");
        for (i, (range, zi)) in ranges.iter().zip(states).enumerate() {
            let (x, y) = (&mono[range.clone()], &mut out[range.clone()]);
            assert_no_alloc(|| filter.process(x, y));
            assert_sos_state(&format!("{case} mono, chunk {i}"), &filter, zi);
        }
        assert_bits(&format!("{case} mono"), &out, &recorded.data);

        // (n, 2) through the prototype's SosFilter, in place.
        let recorded = array(&golden, &format!("filters/{case}/stereo"));
        let states = field(&recorded.header, "states")
            .as_array()
            .expect("states");
        let mut filter = SosFilter::new(2, design.len());
        filter.set_sections(&design).expect("capacity");
        let mut buffer = stereo.data.clone();
        for (i, (range, zi)) in ranges.iter().zip(states).enumerate() {
            let block = &mut buffer[samples(range, 2)];
            assert_no_alloc(|| filter.process_in_place(block));
            assert_sos_state(&format!("{case} stereo, chunk {i}"), &filter, zi);
        }
        assert_bits(&format!("{case} stereo"), &buffer, &recorded.data);

        // Zero state along axis 0, in one call.
        let recorded = array(&golden, &format!("filters/{case}/stereo-zero-state"));
        let mut filter = SosFilter::new(2, design.len());
        filter.set_sections(&design).expect("capacity");
        let mut out = vec![0.0; stereo.data.len()];
        assert_no_alloc(|| filter.process(&stereo.data, &mut out));
        assert_bits(&format!("{case} zero state"), &out, &recorded.data);
    }
}

#[test]
fn sos_filter_state_rules_match_golden() {
    let golden = golden();
    let index = index(&golden);
    cases(&index, "sos_scenario", 1);
    let input = array(&golden, "filters/sos_scenario/input").data;
    let script: Value = golden
        .json("filters/sos_scenario/script.json")
        .expect("script");
    let mut filter = SosFilter::new(2, 4);
    for (i, step) in field(&script, "steps")
        .as_array()
        .expect("steps")
        .iter()
        .enumerate()
    {
        let what = format!("step {i} ({step})");
        match field(step, "op").as_str().expect("an op") {
            "set" => {
                let name = field(step, "design").as_str().expect("a design");
                let design = if name == "identity" {
                    Vec::new()
                } else {
                    let rel = format!("filters/sos_scenario/design-{name}");
                    sections(&array(&golden, &rel).data)
                };
                let result = assert_no_alloc(|| filter.set_sections(&design));
                result.expect("capacity");
            }
            "reset" => assert_no_alloc(|| filter.reset()),
            "process" => {
                let frames = usize_of(field(step, "frames"));
                let offset = usize_of(field(step, "offset"));
                let range = samples(&(offset..offset + frames), 2);
                let x = &input[range];
                let mut out = vec![0.0; x.len()];
                assert_no_alloc(|| filter.process(x, &mut out));
                let expected = array(&golden, &format!("filters/sos_scenario/step-{i}")).data;
                assert_bits(&what, &out, &expected);
                assert_eq!(
                    Some(filter.is_identity()),
                    field(step, "identity").as_bool(),
                    "{what}"
                );
            }
            other => panic!("unknown op {other}"),
        }
        assert_sos_state(&what, &filter, field(step, "zi"));
    }
}

#[test]
fn lfilter_matches_golden() {
    let golden = golden();
    let index = index(&golden);
    for case in cases(&index, "lfilter", 6) {
        let input = array(&golden, &format!("filters/{case}/input"));
        let recorded = array(&golden, &format!("filters/{case}/output"));
        let b = floats(field(&recorded.header, "b"));
        let a = floats(field(&recorded.header, "a"));
        let channels = input.shape.get(1).copied().unwrap_or(1);
        let frames = input.shape[0];
        let mut filter = Lfilter::new(&b, &a, channels).expect("a recursive filter");
        let order = filter.order();
        let states = field(&recorded.header, "states")
            .as_array()
            .expect("states");
        let ranges = chunk_ranges(frames, &index.chunks);
        assert_eq!(ranges.len(), states.len(), "{case}: chunks");
        let mut out = vec![0.0; input.data.len()];
        for (i, (range, zi)) in ranges.iter().zip(states).enumerate() {
            let range = samples(range, channels);
            let (x, y) = (&input.data[range.clone()], &mut out[range]);
            assert_no_alloc(|| filter.process(x, y));
            // SciPy's zi: (order,) for 1-D, (order, channels) for axis 0.
            let zi = zi.as_array().expect("zi");
            assert_eq!(zi.len(), order, "{case}: order");
            for c in 0..channels {
                let expected: Vec<f64> = zi
                    .iter()
                    .map(|z| {
                        if channels == 1 {
                            float(z)
                        } else {
                            float(&z.as_array().expect("zi channels")[c])
                        }
                    })
                    .collect();
                let actual = filter.state(c).expect("a channel");
                assert_bits(&format!("{case}, chunk {i}, state {c}"), actual, &expected);
            }
        }
        assert_bits(case, &out, &recorded.data);
    }
}

#[test]
fn upfirdn_matches_golden() {
    let golden = golden();
    let index = index(&golden);
    let all = cases(&index, "upfirdn", 3);
    for case in all.iter().filter(|c| c.contains("true-peak")) {
        let taps = array(&golden, &format!("filters/{case}/taps")).data;
        let input = array(&golden, &format!("filters/{case}/input"));
        let recorded = array(&golden, &format!("filters/{case}/output"));
        let up = usize_of(field(&recorded.header, "up"));
        let history = usize_of(field(&recorded.header, "history_rows"));
        let frames = input.shape[0];
        // The Analyzer's block loop: history rows, then the block; the valid window starts after
        // the history's output.
        let mut joined: Vec<f64> = vec![0.0; history * 2];
        let mut out = vec![0.0; frames * up * 2];
        for range in chunk_ranges(frames, &index.chunks) {
            joined.truncate(history * 2);
            joined.extend_from_slice(&input.data[samples(&range, 2)]);
            let window = &mut out[samples(&(range.start * up..range.end * up), 2)];
            assert_no_alloc(|| upfirdn(&taps, &joined, 2, up, history * up, window));
            let tail = joined.split_off(joined.len() - history * 2);
            joined = tail;
        }
        assert_bits(case, &out, &recorded.data);
    }
    let case = "upfirdn/odd-length";
    let taps = array(&golden, &format!("filters/{case}/taps")).data;
    let input = array(&golden, &format!("filters/{case}/input")).data;
    let recorded = array(&golden, &format!("filters/{case}/output"));
    let up = usize_of(field(&recorded.header, "up"));
    assert_eq!(recorded.data.len(), (input.len() - 1) * up + taps.len());
    let mut out = vec![0.0; recorded.data.len()];
    assert_no_alloc(|| upfirdn(&taps, &input, 1, up, 0, &mut out));
    assert_bits(case, &out, &recorded.data);
}

fn biquad_type(name: &str) -> BiquadType {
    match name {
        "lowpass" => BiquadType::Lowpass,
        "highpass" => BiquadType::Highpass,
        "bandpass" => BiquadType::Bandpass,
        "notch" => BiquadType::Notch,
        "peak" => BiquadType::Peak,
        "low_shelf" => BiquadType::LowShelf,
        "high_shelf" => BiquadType::HighShelf,
        "allpass" => BiquadType::Allpass,
        other => panic!("unknown biquad type {other}"),
    }
}

/// The cookbook formulas of `biquad.py` given the libm values `cos(w0)`, `sin(w0)` and
/// `a = 10 ** (gain_db / 40)`, and the clamped `q`: a second transcription, which the recorded
/// libm values validate against the prototype.
#[expect(
    clippy::manual_midpoint,
    reason = "the prototype's `(1 + cw) / 2`, written as it computes it"
)]
fn rbj_reference(kind: BiquadType, cw: f64, sw: f64, q: f64, a: f64) -> Section {
    let alpha = sw / (2.0 * q);
    let sa = 2.0 * a.sqrt() * alpha;
    let (b, den) = match kind {
        BiquadType::Lowpass => (
            [(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0],
            [1.0 + alpha, -2.0 * cw, 1.0 - alpha],
        ),
        BiquadType::Highpass => (
            [(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0],
            [1.0 + alpha, -2.0 * cw, 1.0 - alpha],
        ),
        BiquadType::Bandpass => ([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        BiquadType::Notch => ([1.0, -2.0 * cw, 1.0], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        BiquadType::Allpass => (
            [1.0 - alpha, -2.0 * cw, 1.0 + alpha],
            [1.0 + alpha, -2.0 * cw, 1.0 - alpha],
        ),
        BiquadType::Peak => (
            [1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a],
        ),
        BiquadType::LowShelf => (
            [
                a * ((a + 1.0) - (a - 1.0) * cw + sa),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                a * ((a + 1.0) - (a - 1.0) * cw - sa),
            ],
            [
                (a + 1.0) + (a - 1.0) * cw + sa,
                -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                (a + 1.0) + (a - 1.0) * cw - sa,
            ],
        ),
        BiquadType::HighShelf => (
            [
                a * ((a + 1.0) + (a - 1.0) * cw + sa),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                a * ((a + 1.0) + (a - 1.0) * cw - sa),
            ],
            [
                (a + 1.0) - (a - 1.0) * cw + sa,
                2.0 * ((a - 1.0) - (a + 1.0) * cw),
                (a + 1.0) - (a - 1.0) * cw - sa,
            ],
        ),
    };
    [
        b[0] / den[0],
        b[1] / den[0],
        b[2] / den[0],
        1.0,
        den[1] / den[0],
        den[2] / den[0],
    ]
}

/// `w0` as `biquad.py` computes it, with Python's `min`/`max` (exact arithmetic, no libm).
fn reference_w0(freq: f64, sample_rate: f64) -> f64 {
    let f = if 1.0 > freq { 1.0 } else { freq };
    let f = if 0.499 * sample_rate < f {
        0.499 * sample_rate
    } else {
        f
    };
    2.0 * std::f64::consts::PI * f / sample_rate
}

#[test]
fn biquad_designs_match_golden() {
    let golden = golden();
    let index = index(&golden);
    cases(&index, "rbj", 1);

    let recorded = array(&golden, "filters/rbj/biquad");
    let rows = sections(&recorded.data);
    let meta = field(&recorded.header, "cases").as_array().expect("cases");
    assert_eq!(rows.len(), meta.len());
    let mut libm_agrees = 0;
    for (row, case) in rows.iter().zip(meta) {
        let kind = biquad_type(field(case, "kind").as_str().expect("a kind"));
        let [freq, sr, q, gain_db] =
            ["freq", "sample_rate", "q", "gain_db"].map(|k| float(field(case, k)));
        let [w0, cw, sw, a] = ["w0", "cos", "sin", "pow"].map(|k| float(field(case, k)));
        let q = if 1e-3 > q { 1e-3 } else { q };
        // The reference with the prototype's libm values is the prototype's row.
        assert_eq!(reference_w0(freq, sr).to_bits(), w0.to_bits(), "{case}: w0");
        assert_bits(
            &format!("{case}: reference"),
            &rbj_reference(kind, cw, sw, q, a),
            row,
        );
        // `biquad` is the reference with this platform's libm values.
        let local = (w0.cos(), w0.sin(), 10.0_f64.powf(gain_db / 40.0));
        let design = biquad(kind, freq, sr, float(field(case, "q")), gain_db);
        assert_bits(
            &format!("{case}: biquad"),
            &design,
            &rbj_reference(kind, local.0, local.1, q, local.2),
        );
        if bits(&[local.0, local.1, local.2]) == bits(&[cw, sw, a]) {
            libm_agrees += 1;
            assert_bits(&format!("{case}: prototype"), &design, row);
        }
    }
    println!(
        "biquad: {libm_agrees} of {} designs bit-exact with the prototype here (libm agrees)",
        rows.len()
    );
}

#[test]
fn butterworth_designs_match_golden() {
    let golden = golden();
    let index = index(&golden);
    cases(&index, "rbj", 1);

    // Butterworth Qs: `1 / (2 sin(...))`, from the recorded and from this platform's `sin`.
    let qs: Value = golden
        .json("filters/rbj/butterworth_qs.json")
        .expect("butterworth_qs");
    let mut recorded_qs = std::collections::HashMap::new();
    for case in field(&qs, "cases").as_array().expect("cases") {
        let order = field(case, "order").as_u64().expect("an order");
        let slope =
            ButterworthSlope::from_db(i64::try_from(order * 6).expect("small")).expect("a slope");
        let expected = floats(field(case, "qs"));
        let sines = floats(field(case, "sin"));
        let reference: Vec<f64> = sines.iter().map(|s| 1.0 / (2.0 * s)).collect();
        assert_bits(
            &format!("qs of order {order}: reference"),
            &reference,
            &expected,
        );
        let local: Vec<f64> = butterworth_qs(slope).collect();
        let local_sines: Vec<f64> = (1..=order / 2)
            .map(|k| {
                #[expect(clippy::cast_precision_loss, reason = "small integers")]
                let (k, order) = ((2 * k - 1) as f64, (2 * order) as f64);
                (k * std::f64::consts::PI / order).sin()
            })
            .collect();
        let local_reference: Vec<f64> = local_sines.iter().map(|s| 1.0 / (2.0 * s)).collect();
        assert_bits(&format!("qs of order {order}"), &local, &local_reference);
        recorded_qs.insert(order, (expected, bits(&local_sines) == bits(&sines)));
    }

    let recorded = array(&golden, "filters/rbj/butterworth");
    let rows = sections(&recorded.data);
    let mut rows = rows.iter();
    let meta = field(&recorded.header, "cases").as_array().expect("cases");
    let (mut agree, mut total) = (0, 0);
    for case in meta {
        let kind = biquad_type(field(case, "kind").as_str().expect("a kind"));
        let [freq, sr, w0, cw, sw] =
            ["freq", "sample_rate", "w0", "cos", "sin"].map(|k| float(field(case, k)));
        let slope_db = field(case, "slope_db").as_i64().expect("a slope");
        let slope = ButterworthSlope::from_db(slope_db).expect("a slope");
        let count = usize_of(field(case, "sections"));
        let expected: Vec<Section> = rows.by_ref().take(count).copied().collect();
        let (qs, qs_agree) = &recorded_qs[&u64::from(slope.order())];
        assert_eq!(reference_w0(freq, sr).to_bits(), w0.to_bits(), "{case}: w0");
        let reference: Vec<f64> = qs
            .iter()
            .flat_map(|&q| rbj_reference(kind, cw, sw, q, 1.0))
            .collect();
        assert_bits(
            &format!("{case}: reference"),
            &reference,
            expected.as_flattened(),
        );
        let cascade = assert_no_alloc(|| butterworth(kind, freq, sr, slope));
        let local: Vec<f64> = butterworth_qs(slope)
            .flat_map(|q| rbj_reference(kind, w0.cos(), w0.sin(), q, 1.0))
            .collect();
        assert_bits(
            &format!("{case}: butterworth"),
            cascade.sections().as_flattened(),
            &local,
        );
        total += 1;
        if *qs_agree && bits(&[w0.cos(), w0.sin()]) == bits(&[cw, sw]) {
            agree += 1;
            assert_bits(
                &format!("{case}: prototype"),
                cascade.sections().as_flattened(),
                expected.as_flattened(),
            );
        }
    }
    assert!(rows.next().is_none(), "every recorded section is compared");
    println!("butterworth: {agree} of {total} cascades bit-exact with the prototype here");
}

#[test]
fn smoothed_value_matches_golden() {
    let golden = golden();
    let index = index(&golden);
    cases(&index, "smoothed", 1);
    let recorded: Value = golden.json("filters/smoothed.json").expect("smoothed");
    for sequence in field(&recorded, "sequences").as_array().expect("sequences") {
        let name = field(sequence, "name").as_str().expect("a name");
        let mut s = match field(sequence, "init") {
            Value::Null => SmoothedValue::new(),
            init => {
                let init = init.as_array().expect("[value, ramp]");
                let ramp = u32::try_from(init[1].as_u64().expect("a ramp")).expect("small");
                SmoothedValue::with_value(float(&init[0]), ramp)
            }
        };
        for (i, step) in field(sequence, "steps")
            .as_array()
            .expect("steps")
            .iter()
            .enumerate()
        {
            let what = format!("{name}, step {i} ({})", field(step, "op"));
            let args = field(step, "args").as_array().expect("args");
            match field(step, "op").as_str().expect("an op") {
                "next" => {
                    let mut out = vec![f64::NAN; usize_of(&args[0])];
                    let result = assert_no_alloc(|| s.next_block(&mut out));
                    match (field(step, "kind").as_str().expect("a kind"), result) {
                        ("scalar", Smoothed::Steady(value)) => {
                            let expected = float(field(step, "value"));
                            assert_eq!(value.to_bits(), expected.to_bits(), "{what}");
                        }
                        ("ramp", Smoothed::Ramp) => {
                            assert_bits(&what, &out, &floats(field(step, "values")));
                        }
                        (kind, result) => panic!("{what}: recorded {kind}, got {result:?}"),
                    }
                }
                "target" => assert_no_alloc(|| s.set_target(float(&args[0]))),
                "immediate" => assert_no_alloc(|| s.set_immediate(float(&args[0]))),
                "ramp" => {
                    let ramp = u32::try_from(args[0].as_u64().expect("a ramp")).expect("small");
                    s.set_ramp(ramp);
                }
                other => panic!("unknown op {other}"),
            }
            assert_eq!(
                (s.current().to_bits(), s.target().to_bits()),
                (
                    float(field(step, "current")).to_bits(),
                    float(field(step, "target")).to_bits()
                ),
                "{what}: current and target"
            );
            assert_eq!(
                Some(s.is_smoothing()),
                field(step, "smoothing").as_bool(),
                "{what}"
            );
        }
    }
}

fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    assert_eq!(
        a.is_sign_negative(),
        b.is_sign_negative(),
        "{a} vs {b}: sign"
    );
    a.to_bits().abs_diff(b.to_bits())
}

#[test]
fn level_functions_match_golden() {
    let golden = golden();
    let index = index(&golden);
    cases(&index, "level", 1);
    let recorded: Value = golden.json("filters/level.json").expect("level");
    let pairs = |key: &str| -> Vec<Vec<f64>> {
        field(&recorded, key)
            .as_array()
            .expect("cases")
            .iter()
            .map(floats)
            .collect()
    };
    let (mut exact, mut total) = (0, 0);
    let mut check = |what: String, actual: f64, expected: f64| {
        let distance = ulps(actual, expected);
        assert!(distance <= LEVEL_ULPS, "{what}: {actual:e} vs {expected:e}");
        total += 1;
        exact += usize::from(distance == 0);
    };
    for case in pairs("db_to_amp") {
        check(
            format!("db_to_amp({})", case[0]),
            db_to_amp(case[0]),
            case[1],
        );
    }
    for case in pairs("amp_to_db") {
        check(
            format!("amp_to_db({})", case[0]),
            amp_to_db(case[0], -200.0),
            case[1],
        );
    }
    for case in pairs("time_coefficient") {
        let what = format!("time_coefficient({}, {})", case[0], case[1]);
        check(what, time_coefficient(case[0], case[1]), case[2]);
    }
    println!("level: {exact} of {total} values bit-exact here");

    let finite = field(&recorded, "ensure_finite");
    let mut buffer = floats(field(finite, "in"));
    let replaced = assert_no_alloc(|| ensure_finite(&mut buffer));
    assert_eq!(replaced, 3);
    assert_bits("ensure_finite", &buffer, &floats(field(finite, "out")));
}

#[test]
fn backward_minimum_matches_golden() {
    let golden = golden();
    let index = index(&golden);
    let input = array(&golden, "filters/minimum/input").data;
    for case in cases(&index, "minimum", 5) {
        let recorded = array(&golden, &format!("filters/{case}"));
        let lookahead = usize_of(field(&recorded.header, "lookahead"));
        let initial = float(field(&recorded.header, "initial"));
        let mut filter = BackwardMinimum::new(lookahead, initial);
        let mut out = vec![0.0; input.len()];
        for range in chunk_ranges(input.len(), &index.chunks) {
            let (x, y) = (&input[range.clone()], &mut out[range]);
            assert_no_alloc(|| filter.process(x, y));
        }
        assert_bits(case, &out, &recorded.data);
    }
}

#[test]
fn delay_line_matches_golden() {
    let golden = golden();
    let index = index(&golden);
    let input = array(&golden, "filters/delay/input");
    let frames = input.shape[0];
    for case in cases(&index, "delay", 4) {
        let recorded = array(&golden, &format!("filters/{case}"));
        let delay = usize_of(field(&recorded.header, "delay"));
        let mut line = DelayLine::new(delay, 2);
        let mut in_place = DelayLine::new(delay, 2);
        let mut out = vec![0.0; input.data.len()];
        let mut buffer = input.data.clone();
        for range in chunk_ranges(frames, &index.chunks) {
            let range = samples(&range, 2);
            let (x, y) = (&input.data[range.clone()], &mut out[range.clone()]);
            assert_no_alloc(|| line.process(x, y));
            let block = &mut buffer[range];
            assert_no_alloc(|| in_place.process_in_place(block));
        }
        assert_bits(case, &out, &recorded.data);
        assert_bits(&format!("{case} in place"), &buffer, &recorded.data);
    }
}
