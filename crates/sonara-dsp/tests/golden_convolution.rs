//! Golden parity of the uniformly partitioned convolver (GLD-01).
//!
//! `convolution/` is recorded by `scripts/golden/area_convolution.py` through the prototype's own
//! `PartitionedConvolver`, in both modes: `double` (complex128) and `single` (complex64 spectra
//! and delay line, a float32 `irfft`). Every case replays the recorded input block by block with
//! the recorded script of impulse-response changes (`set_ir` with and without crossfade,
//! `reset`) and compares every output sample, each block's dtype and `partitions`.
//!
//! Exactness: the delay-line arithmetic, the einsum order (its fused multiply-adds) and the
//! crossfade follow the prototype bit for bit (see `sonara_dsp::convolution`). The real FFTs
//! are `realfft`'s, whose factorization differs from NumPy's pocketfft, so the output agrees to
//! the last ulps only; [`DOUBLE_TOLERANCE`] and [`SINGLE_TOLERANCE`] bound the error relative to
//! each case's output peak. Block dtypes and partition counts are exact.
//!
//! Processing, swaps (`install`) and resets run inside `assert_no_alloc` with the `AllocDisabler`
//! global allocator, which shows that they allocate and free nothing (RT-02); preparing an
//! impulse response and dropping the retired one are setup and run outside.

use std::collections::BTreeMap;

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::Value;
use sonara_dsp::convolution::{OutputPrecision, PartitionedConvolver, PreparedIr, SpectrumFloat};
use sonara_testkit::Golden;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

/// Largest error of the double-precision output relative to the case's output peak. The
/// prototype's FFT is NumPy's pocketfft and ours is `realfft` (rustfft's scalar path): both are
/// accurate to a few ulps, with different rounding, and rustfft's twiddles come from the
/// platform's `sin`/`cos`. Measured in the container (aarch64 Linux): 7.2e-16 (`fixture`),
/// 5.3e-16 (`partitions`) and 8.4e-16 (`block-480`), about 4 ulps of the peak; 7 to 12 % of the
/// samples are bit-exact. The bound leaves a factor of about 2.4 for other platforms' libm.
const DOUBLE_TOLERANCE: f64 = 2e-15;

/// Largest error of the single-precision output relative to the case's output peak: the
/// float32 inverse FFTs of pocketfft and `realfft` round differently (F11; the complex64 delay
/// line and the einsum sums match the prototype). Measured in the container: 1.7e-7
/// (`fixture`), 1.7e-7 (`partitions`) and 2.0e-7 (`block-480`), under two float32 ulps of the
/// peak; 18 to 23 % of the samples are bit-exact. The bound leaves a factor of 2.
const SINGLE_TOLERANCE: f64 = 4e-7;

fn golden() -> Golden {
    Golden::open().expect("golden data is available (set SONARA_GOLDEN to a recording)")
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("no field {key} in {value}"))
}

fn text(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("not a string: {value}"))
}

fn count(value: &Value) -> usize {
    usize::try_from(
        value
            .as_u64()
            .unwrap_or_else(|| panic!("not a count: {value}")),
    )
    .expect("a count fits usize")
}

/// An impulse response: interleaved samples and the channel count.
struct Ir {
    samples: Vec<f64>,
    channels: usize,
}

fn impulse_responses(golden: &Golden, index: &Value) -> BTreeMap<String, Ir> {
    let manifest = &golden.manifest().files;
    field(index, "irs")
        .as_object()
        .expect("irs")
        .iter()
        .map(|(name, entry)| {
            let ir = if let Some(file) = entry.get("file") {
                let file = text(file);
                assert_eq!(
                    manifest.get(file).map(String::as_str),
                    Some(text(field(entry, "sha256"))),
                    "{file}: the recording used these bytes"
                );
                let audio = golden.wav(file).expect("the IR fixture");
                Ir {
                    samples: audio.samples.iter().map(|&s| f64::from(s)).collect(),
                    channels: usize::from(audio.channels),
                }
            } else {
                let array = golden
                    .f64_array(text(field(entry, "array")))
                    .expect("an IR array");
                let channels = *array.shape.get(1).expect("(taps, channels)");
                Ir {
                    samples: array.data,
                    channels,
                }
            };
            (name.clone(), ir)
        })
        .collect()
}

/// The result of replaying one case.
struct Replay {
    output: Vec<f64>,
    dtypes: Vec<&'static str>,
    partitions: Vec<usize>,
}

fn replay<T: SpectrumFloat>(header: &Value, input: &[f64], irs: &BTreeMap<String, Ir>) -> Replay {
    let block = count(field(header, "block"));
    let channels = count(field(header, "channels"));
    let blocks = field(header, "dtypes").as_array().expect("dtypes").len();
    let script = field(header, "script").as_array().expect("script");
    let mut conv = PartitionedConvolver::<T>::new(block, channels).expect("a valid layout");
    let samples = block * channels;
    let mut output = vec![0.0; blocks * samples];
    let mut dtypes = Vec::with_capacity(blocks);
    let mut partitions = Vec::with_capacity(blocks);
    for (i, out) in output.chunks_exact_mut(samples).enumerate() {
        for event in script.iter().filter(|e| count(field(e, "block")) == i) {
            match text(field(event, "action")) {
                "set_ir" => {
                    let ir = &irs[text(field(event, "ir"))];
                    let crossfade = field(event, "crossfade").as_bool().expect("a flag");
                    let prepared = PreparedIr::<T>::new(block, channels, &ir.samples, ir.channels)
                        .expect("a usable IR");
                    let retired = assert_no_alloc(|| conv.install(prepared, crossfade))
                        .expect("the same layout");
                    drop(retired);
                }
                "reset" => assert_no_alloc(|| conv.reset()),
                other => panic!("unknown action {other}"),
            }
        }
        let x = &input[i * samples..(i + 1) * samples];
        let precision = assert_no_alloc(|| conv.process(x, out)).expect("a full block");
        drop(conv.take_retired());
        dtypes.push(match precision {
            OutputPrecision::Single => "float32",
            OutputPrecision::Double => "float64",
        });
        partitions.push(conv.partitions());
    }
    Replay {
        output,
        dtypes,
        partitions,
    }
}

/// Largest absolute error relative to the expected output's peak, and the share of samples that
/// are bit-exact.
fn compare(actual: &[f64], expected: &[f64]) -> (f64, f64) {
    assert_eq!(actual.len(), expected.len(), "output length");
    let peak = expected.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let worst = actual
        .iter()
        .zip(expected)
        .fold(0.0_f64, |m, (a, e)| m.max((a - e).abs()));
    let exact = actual
        .iter()
        .zip(expected)
        .filter(|(a, e)| a.to_bits() == e.to_bits())
        .count();
    #[expect(clippy::cast_precision_loss, reason = "a share for the report")]
    let share = exact as f64 / expected.len() as f64;
    (if peak > 0.0 { worst / peak } else { worst }, share)
}

#[test]
fn partitioned_convolver_matches_the_prototype() {
    let golden = golden();
    let index: Value = golden
        .json("convolution/index.json")
        .expect("convolution/index.json");
    let irs = impulse_responses(&golden, &index);
    let input = golden
        .f64_array(text(field(&index, "input")))
        .expect("the input")
        .data;
    let mut cases = 0;
    for mode in field(&index, "modes").as_array().expect("modes") {
        let mode = text(mode);
        for case in field(&index, "cases").as_array().expect("cases") {
            let case = text(case);
            let recorded = golden
                .f64_array(&format!("convolution/{mode}/{case}/output"))
                .expect("a recorded output");
            let header = &recorded.header;
            let single = field(header, "single_precision").as_bool().expect("a flag");
            assert_eq!(single, mode == "single", "{mode}/{case}: mode");
            let run = if single {
                replay::<f32>(header, &input, &irs)
            } else {
                replay::<f64>(header, &input, &irs)
            };
            let dtypes: Vec<&str> = field(header, "dtypes")
                .as_array()
                .expect("dtypes")
                .iter()
                .map(text)
                .collect();
            assert_eq!(run.dtypes, dtypes, "{mode}/{case}: block dtypes");
            let partitions: Vec<usize> = field(header, "partitions")
                .as_array()
                .expect("partitions")
                .iter()
                .map(count)
                .collect();
            assert_eq!(run.partitions, partitions, "{mode}/{case}: partitions");
            let (error, exact) = compare(&run.output, &recorded.data);
            let tolerance = if single {
                SINGLE_TOLERANCE
            } else {
                DOUBLE_TOLERANCE
            };
            eprintln!(
                "{mode}/{case}: max error {error:.3e} of the output peak, {:.1} % bit-exact",
                exact * 100.0
            );
            assert!(
                error <= tolerance,
                "{mode}/{case}: error {error:e} of the output peak exceeds {tolerance:e}"
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 6, "every recorded case ran");
}
