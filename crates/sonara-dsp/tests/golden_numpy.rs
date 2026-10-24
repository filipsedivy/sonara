//! Golden parity of `sonara_dsp::numpy` (GLD-01).
//!
//! `golden/numpy/` is recorded by `scripts/golden/area_numpy.py` from NumPy 2.5.4 on the
//! recording host: the random streams of the prototype's seeds and shapes, and the reductions
//! and rounding helpers over awkward inputs.
//!
//! Every comparison is exact, bit for bit, with two exceptions:
//!
//! - any NaN matches any NaN, because the bit pattern of a generated NaN differs between CPUs
//!   (arm64 and x86-64 produce default NaNs of opposite sign); NumPy itself does not specify it;
//! - off macOS, a `standard_normal` tail draw may differ by [`TAIL_ULPS`] (one ulp), because its
//!   last bit comes from the platform libm's `log1p`. The justification and the measured draws
//!   are at the constant. On macOS, the product and recording platform, tails stay exact.
//!
//! Streams longer than one block are recorded as SHA-256 digests per block, plus the first block
//! and every tail draw of `standard_normal` (`|x| >= r`) in full. The generators run inside
//! `assert_no_alloc`, including their construction (RT-02).

use std::collections::BTreeMap;

use assert_no_alloc::assert_no_alloc;
use serde_json::Value;
use sonara_dsp::numpy::math::{cumsum, linspace, mean, percentile, py_round, rint, round, sum};
use sonara_dsp::numpy::random::{Generator, Pcg64, Pcg64State, SeedSequence, default_rng};
use sonara_testkit::{Golden, sha256_hex};

#[global_allocator]
static A: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

/// `ziggurat_nor_r`: every draw at or above it in magnitude came from the tail.
const TAIL_THRESHOLD: f64 = 3.654_152_885_361_009;

/// How far a `standard_normal` tail draw may differ from the oracle, in ulps
/// (`|bits(expected) − bits(actual)|`): 0 on macOS, the product and recording platform, and 1
/// elsewhere (GLD-01).
///
/// A tail draw is `±(r + xx)` with `xx = -inv_r * log1p(-u)`, so its last bit depends on the
/// platform libm's `log1p`. Nothing else in the stream does: every other value is plain
/// arithmetic on the generator's bits, and `exp` only decides accepts, which the exact
/// generator-state check covers. macOS libm is correctly rounded on the recorded inputs; glibc
/// 2.41 is not. Measured in the development container (glibc 2.41, aarch64), 2 of the 971 tail
/// draws of the long seed-1234 stream differ by one ulp:
///
/// - draw 78954, `u = 0x1.4097905c4835dp-1`: `log1p(-u)` is `-0x1.f7c40bf49f59fp-1` on macOS
///   (0.49 ulp from the exact value) and `-0x1.f7c40bf49f59ep-1` on glibc (0.51 ulp); the draw is
///   `0xc00f632659aeefdc` in the oracle and `0xc00f632659aeefdb` on glibc;
/// - draw 3727949, `u = 0x1.b8b1990887704p-2`: `-0x1.2022016f51013p-1` (0.39 ulp) on macOS and
///   `-0x1.2022016f51014p-1` (0.61 ulp) on glibc; the draw is `0x400e771bb7003817` and
///   `0x400e771bb7003818`.
///
/// The allowance covers tail values only (`|x| >= r`, at the recorded indices). The recorded
/// value is patched in before a block (or the reverb noise, a prefix of the same stream) is
/// digested, so every other value and every digest still compare exactly, and the generator
/// state after every block is exact on every platform. Each draw that used the allowance is
/// printed (`--nocapture`) and listed in any failure message.
const TAIL_ULPS: u64 = if cfg!(target_os = "macos") { 0 } else { 1 };

fn golden() -> Golden {
    Golden::open().expect("golden data is available (run `git lfs pull` or set SONARA_GOLDEN)")
}

fn f64s(golden: &Golden, rel: &str) -> (Vec<f64>, Value) {
    let array = golden
        .f64_array(rel)
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"));
    (array.data, array.header)
}

/// A raw little-endian integer array (`<rel>.u64` or `<rel>.i64`) and its header.
fn words(golden: &Golden, rel: &str, ext: &str) -> (Vec<[u8; 8]>, Value) {
    let header: Value = golden
        .json(&format!("{rel}.json"))
        .unwrap_or_else(|e| panic!("cannot read {rel}.json: {e}"));
    assert_eq!(
        header.get("dtype").and_then(Value::as_str),
        Some(format!("{ext}le").as_str()),
        "{rel}: dtype"
    );
    let bytes = golden
        .bytes(&format!("{rel}.{ext}"))
        .unwrap_or_else(|e| panic!("cannot read {rel}.{ext}: {e}"));
    let (chunks, rest) = bytes.as_chunks::<8>();
    assert!(rest.is_empty(), "{rel}: length is not a multiple of 8");
    let count: u64 = header
        .get("shape")
        .and_then(Value::as_array)
        .expect("a shape")
        .iter()
        .map(|d| d.as_u64().expect("a dimension"))
        .product();
    assert_eq!(
        u64::try_from(chunks.len()).expect("fits u64"),
        count,
        "{rel}: shape"
    );
    (chunks.to_vec(), header)
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("no field {key} in {value}"))
}

fn as_u64(value: &Value, key: &str) -> u64 {
    field(value, key)
        .as_u64()
        .unwrap_or_else(|| panic!("{key} is not a u64"))
}

fn as_usize(value: &Value, key: &str) -> usize {
    usize::try_from(as_u64(value, key)).expect("fits usize")
}

fn as_i64(value: &Value, key: &str) -> i64 {
    field(value, key)
        .as_i64()
        .unwrap_or_else(|| panic!("{key} is not an i64"))
}

fn as_f64(value: &Value, key: &str) -> f64 {
    field(value, key)
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is not a number"))
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    field(value, key)
        .as_array()
        .unwrap_or_else(|| panic!("{key} is not a list"))
}

fn u64_list(value: &Value, key: &str) -> Vec<u64> {
    list(value, key)
        .iter()
        .map(|v| v.as_u64().expect("a u64"))
        .collect()
}

fn usize_list(value: &Value, key: &str) -> Vec<usize> {
    list(value, key)
        .iter()
        .map(|v| usize::try_from(v.as_u64().expect("an index")).expect("fits usize"))
        .collect()
}

fn hex_u128(value: &Value, key: &str) -> u128 {
    let text = field(value, key).as_str().expect("a hex string");
    u128::from_str_radix(text.trim_start_matches("0x"), 16).expect("a hex u128")
}

/// NumPy's `bit_generator.state` as recorded by `_pcg_state`.
fn pcg_state(value: &Value) -> Pcg64State {
    Pcg64State {
        state: hex_u128(value, "state"),
        inc: hex_u128(value, "inc"),
        has_uint32: field(value, "has_uint32").as_bool().expect("a bool"),
        uinteger: u32::try_from(as_u64(value, "uinteger")).expect("a u32"),
    }
}

fn check_state(what: &str, rng: &Generator, recorded: &Value) {
    assert_eq!(
        rng.bit_generator().state(),
        pcg_state(recorded),
        "{what}: generator state"
    );
}

fn same(expected: f64, actual: f64) -> bool {
    expected.to_bits() == actual.to_bits() || (expected.is_nan() && actual.is_nan())
}

/// Exact comparison; on the first mismatch, reports the index and both values with their bits.
fn check_exact(what: &str, expected: &[f64], actual: &[f64]) {
    assert_eq!(expected.len(), actual.len(), "{what}: length");
    if let Some(i) = (0..expected.len()).find(|&i| !same(expected[i], actual[i])) {
        let (e, a) = (expected[i], actual[i]);
        let mismatches = (0..expected.len())
            .filter(|&i| !same(expected[i], actual[i]))
            .count();
        panic!(
            "{what}: {mismatches} of {} values differ; first at index {i}: expected {e:e} \
             ({:#018x}), got {a:e} ({:#018x}){}",
            expected.len(),
            e.to_bits(),
            a.to_bits(),
            if e.abs() >= TAIL_THRESHOLD || a.abs() >= TAIL_THRESHOLD {
                " (a standard_normal tail draw: log1p from the platform libm)"
            } else {
                ""
            }
        );
    }
}

fn digest(values: &[f64]) -> String {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    sha256_hex(&bytes)
}

/// Indices (offset by `start`) and values of the tail draws in `values`.
fn tails(values: &[f64], start: usize) -> (Vec<usize>, Vec<f64>) {
    values
        .iter()
        .enumerate()
        .filter(|(_, v)| v.abs() >= TAIL_THRESHOLD)
        .map(|(i, &v)| (start + i, v))
        .unzip()
}

/// The recorded tail draws of the long seed-1234 stream, by stream index.
fn recorded_tails(golden: &Golden) -> BTreeMap<usize, f64> {
    let (values, header) = f64s(golden, "numpy/standard_normal/long.tails");
    let indices = usize_list(&header, "indices");
    assert_eq!(
        indices.len(),
        values.len(),
        "long.tails: one index per value"
    );
    indices.into_iter().zip(values).collect()
}

/// What the tail checks found: draws within [`TAIL_ULPS`] that used the allowance, and failures.
#[derive(Default)]
struct TailReport {
    allowed: Vec<String>,
    failed: Vec<String>,
}

impl TailReport {
    /// Compares the tail draws of `values`, which start at stream index `start`, with the
    /// recorded ones. The indices must match exactly. A value within [`TAIL_ULPS`] of the
    /// recorded one is replaced by it, so a later digest of `values` stays an exact check of
    /// everything else.
    #[cfg_attr(
        target_os = "macos",
        expect(
            clippy::absurd_extreme_comparisons,
            reason = "TAIL_ULPS is 0 on macOS, where tail draws compare exactly"
        )
    )]
    fn check(
        &mut self,
        what: &str,
        values: &mut [f64],
        start: usize,
        recorded: &BTreeMap<usize, f64>,
    ) {
        let (indices, _) = tails(values, start);
        let expected: Vec<usize> = recorded
            .range(start..start + values.len())
            .map(|(&i, _)| i)
            .collect();
        if indices != expected {
            self.failed.push(format!(
                "{what}: tail draw indices differ (an accept decision flipped): recorded \
                 {expected:?}, got {indices:?}"
            ));
            return;
        }
        for i in indices {
            let (e, slot) = (recorded[&i], &mut values[i - start]);
            let ulps = e.to_bits().abs_diff(slot.to_bits());
            if ulps == 0 {
                continue;
            }
            let line = format!(
                "{what}: draw {i}: expected {e:?} ({:#018x}), got {:?} ({:#018x}), {ulps} ulp",
                e.to_bits(),
                *slot,
                slot.to_bits()
            );
            if ulps <= TAIL_ULPS {
                *slot = e;
                self.allowed.push(line);
            } else {
                self.failed.push(line);
            }
        }
    }

    /// Prints the draws that used the allowance and fails on any other difference.
    fn finish(&self, what: &str) {
        for line in &self.allowed {
            eprintln!("tail allowance ({TAIL_ULPS} ulp, platform log1p) used: {line}");
        }
        assert!(
            self.failed.is_empty(),
            "{what}: {} tail draws differ beyond {TAIL_ULPS} ulp (the tail value is \
             r - inv_r * log1p(-u), and log1p comes from the platform libm):\n{}\n\
             within the allowance:\n{}",
            self.failed.len(),
            self.failed.join("\n"),
            self.allowed.join("\n")
        );
    }
}

#[test]
fn seed_sequences_and_pcg64_seeding_match_golden() {
    let golden = golden();
    let recorded: Value = golden.json("numpy/seed_sequence.json").unwrap();
    let cases = list(&recorded, "cases");
    assert_eq!(cases.len(), 10);
    for case in cases {
        let seed = as_u64(case, "seed");
        let seq = SeedSequence::new(seed);
        let pool: Vec<u64> = seq.pool().iter().map(|&w| u64::from(w)).collect();
        assert_eq!(pool, u64_list(case, "pool"), "seed {seed}: pool");
        let mut state_u32 = [0_u32; 8];
        seq.generate_state_u32(&mut state_u32);
        let state_u32: Vec<u64> = state_u32.iter().map(|&w| u64::from(w)).collect();
        assert_eq!(state_u32, u64_list(case, "state_u32"), "seed {seed}: u32");
        let mut state_u64 = [0_u64; 4];
        seq.generate_state_u64(&mut state_u64);
        assert_eq!(
            state_u64.to_vec(),
            u64_list(case, "state_u64"),
            "seed {seed}: u64"
        );
        assert_eq!(
            Pcg64::new(seed).state(),
            pcg_state(field(case, "pcg64")),
            "seed {seed}: PCG64 state after seeding"
        );
    }
}

#[test]
fn raw_and_uniform_streams_match_golden() {
    let golden = golden();
    let index: Value = golden.json("numpy/index.json").unwrap();
    let seeds = u64_list(&index, "raw_seeds");
    assert_eq!(seeds.len(), 8);
    for seed in seeds {
        let (expected, _) = words(&golden, &format!("numpy/raw/seed-{seed}"), "u64");
        let expected: Vec<u64> = expected.into_iter().map(u64::from_le_bytes).collect();
        let mut raw = vec![0_u64; expected.len()];
        assert_no_alloc(|| Pcg64::new(seed).random_raw_fill(&mut raw));
        assert_eq!(raw, expected, "seed {seed}: random_raw");

        let (expected, header) = f64s(&golden, &format!("numpy/random/seed-{seed}"));
        let mut uniform = vec![0.0; expected.len()];
        let rng = assert_no_alloc(|| {
            let mut rng = default_rng(seed);
            rng.random_fill(&mut uniform);
            rng
        });
        check_exact(&format!("seed {seed}: random"), &expected, &uniform);
        check_state(
            &format!("seed {seed}: random"),
            &rng,
            field(&header, "after"),
        );
    }
}

#[test]
fn long_standard_normal_stream_matches_golden() {
    let golden = golden();
    let recorded: Value = golden.json("numpy/standard_normal/long.json").unwrap();
    let count = as_usize(&recorded, "count");
    let block = as_usize(&recorded, "block");
    let blocks = list(&recorded, "blocks");
    let (head, _) = f64s(&golden, "numpy/standard_normal/long.head");
    let recorded_tails = recorded_tails(&golden);
    assert_eq!(as_f64(&recorded, "tail_threshold"), TAIL_THRESHOLD);
    assert_eq!(blocks.len(), count.div_ceil(block));
    assert!(
        count >= 3_000_000 && recorded_tails.len() > 500,
        "the stream must reach the tail often"
    );

    let mut rng = default_rng(as_u64(&recorded, "seed"));
    let mut buffer = vec![0.0; block];
    let mut report = TailReport::default();
    let mut bad_blocks = Vec::new();
    for (b, info) in blocks.iter().enumerate() {
        let n = as_usize(info, "count");
        let values = &mut buffer[..n];
        assert_no_alloc(|| rng.standard_normal_fill(values));
        // Tails first: they are recorded value by value, so a difference is located exactly.
        report.check(&format!("block {b}"), values, b * block, &recorded_tails);
        if b == 0 {
            check_exact("standard_normal, first block", &head, values);
        }
        let state_matches = rng.bit_generator().state() == pcg_state(field(info, "after"));
        if digest(values) != field(info, "sha256").as_str().expect("a digest") || !state_matches {
            bad_blocks.push((b, state_matches));
        }
    }
    report.finish("long seed-1234 stream");
    assert!(
        bad_blocks.is_empty(),
        "standard_normal blocks of {block} draws differ outside the tail draws, as (block, \
         generator state after it matches): {bad_blocks:?}. A matching state means the same \
         number of draws, so a value differs; a different state means an accept decision flipped"
    );
    let mut next = [0_u64; 8];
    rng.bit_generator_mut().random_raw_fill(&mut next);
    assert_eq!(
        next.to_vec(),
        u64_list(&recorded, "next_raw"),
        "stream position"
    );
}

#[test]
fn reverb_ir_draws_match_golden() {
    let golden = golden();
    let index: Value = golden.json("numpy/index.json").unwrap();
    let cases = list(&index, "reverb");
    assert_eq!(cases.len(), 7);
    // The noise is a prefix of the long seed-1234 stream, whose tail draws are recorded.
    let long: Value = golden.json("numpy/standard_normal/long.json").unwrap();
    let recorded_tails = recorded_tails(&golden);
    let mut skipped = 0;
    for case in cases {
        let name = case.as_str().expect("a case name");
        let (uniforms, header) = f64s(&golden, &format!("numpy/reverb/{name}.uniforms"));
        let (signs, signs_header) = f64s(&golden, &format!("numpy/reverb/{name}.signs"));
        let frames = as_usize(&header, "frames");
        let n_early = as_usize(&header, "n_early");
        let spread = as_f64(&header, "spread_ms");
        let sample_rate = as_f64(&header, "sample_rate");
        let delays: Vec<i64> = list(&header, "delays")
            .iter()
            .map(|d| d.as_i64().expect("a delay"))
            .collect();

        let mut noise = vec![0.0; 2 * frames];
        let mut got_uniforms = vec![0.0; n_early];
        let mut got_signs = vec![0.0; signs.len()];
        let mut got_delays = vec![0_i64; n_early];
        // No panics inside the closure: a panic message allocates.
        let (rng, after_noise, draws_fit) = assert_no_alloc(|| {
            let mut rng = default_rng(1234);
            rng.standard_normal_fill(&mut noise);
            let after_noise = rng.bit_generator().state();
            let mut sign_rows = got_signs.as_chunks_mut::<2>().0.iter_mut();
            let mut draws_fit = true;
            for (u, delay) in got_uniforms.iter_mut().zip(&mut got_delays) {
                *u = rng.random();
                // `int((0.002 + u * spread_ms * 0.001) * sample_rate)`, truncated like int().
                #[expect(clippy::cast_possible_truncation, reason = "Python's int() truncates")]
                let d = ((0.002 + *u * spread * 0.001) * sample_rate) as i64;
                *delay = d;
                if usize::try_from(d).is_ok_and(|d| d < frames) {
                    // `choice([-1.0, 1.0], size=2)` only when the reflection fits.
                    match sign_rows.next() {
                        Some(row) => draws_fit &= rng.choice(&[-1.0, 1.0], row).is_ok(),
                        None => draws_fit = false,
                    }
                }
            }
            (rng, after_noise, draws_fit && sign_rows.next().is_none())
        });
        assert!(
            draws_fit,
            "{name}: the choice draws match the recorded rows"
        );
        assert_eq!(
            after_noise,
            pcg_state(field(&signs_header, "after_noise")),
            "{name}: state after the noise"
        );
        check_exact(&format!("{name}: uniforms"), &uniforms, &got_uniforms);
        assert_eq!(got_delays, delays, "{name}: delays");
        check_exact(&format!("{name}: signs"), &signs, &got_signs);
        check_state(name, &rng, field(&signs_header, "after"));
        assert!(
            noise.len() <= as_usize(&long, "count"),
            "{name}: noise within the long stream"
        );
        assert_eq!(
            recorded_tails
                .range(..noise.len())
                .map(|(&i, _)| i)
                .collect::<Vec<_>>(),
            usize_list(&signs_header, "noise_tails"),
            "{name}: the recorded noise tails are the long stream's"
        );
        let mut report = TailReport::default();
        report.check(&format!("{name} noise"), &mut noise, 0, &recorded_tails);
        report.finish(name);
        assert_eq!(
            digest(&noise),
            field(&signs_header, "noise_sha256")
                .as_str()
                .expect("a digest"),
            "{name}: noise outside the tail draws (a prefix of the long seed-1234 stream)"
        );
        skipped += n_early - signs.len() / 2;
    }
    assert!(skipped > 0, "some reflections skip the choice draw");
}

#[test]
fn synth_streams_match_golden() {
    let golden = golden();
    let (phases, header) = f64s(&golden, "numpy/synth/initial_phases");
    let orders = u64_list(&header, "orders");
    let count = phases.len() / orders.len();
    let mut got = vec![0.0; phases.len()];
    assert_no_alloc(|| {
        for (&order, row) in orders.iter().zip(got.chunks_exact_mut(count)) {
            default_rng(order).random_fill(row);
        }
    });
    check_exact("synth initial phases", &phases, &got);

    let (noise, header) = f64s(&golden, "numpy/synth/noise");
    let cases = list(&header, "cases");
    let count = noise.len() / cases.len();
    let seeds: Vec<u64> = cases
        .iter()
        .map(|case| {
            let (seed, age) = (as_u64(case, "seed"), as_u64(case, "age"));
            // `(seed * 1_000_003 + age) & 0xFFFFFFFF`: Python's integers are unbounded, and the
            // low 32 bits of the wrapped u64 arithmetic are the same.
            let rng_seed = seed.wrapping_mul(1_000_003).wrapping_add(age) & 0xffff_ffff;
            assert_eq!(rng_seed, as_u64(case, "rng_seed"));
            rng_seed
        })
        .collect();
    let mut got = vec![0.0; noise.len()];
    assert_no_alloc(|| {
        for (&seed, row) in seeds.iter().zip(got.chunks_exact_mut(count)) {
            // Reseeded per block segment on the audio thread.
            default_rng(seed).standard_normal_fill(row);
        }
    });
    check_exact("synth noise", &noise, &got);
}

#[test]
fn drum_streams_match_golden() {
    let golden = golden();
    let index: Value = golden.json("numpy/index.json").unwrap();
    let names = list(&index, "drums");
    assert_eq!(names.len(), 29);
    for name in names {
        let name = name.as_str().expect("a voice name");
        let (expected, header) = f64s(&golden, &format!("numpy/drums/{name}"));
        let mut got = vec![0.0; expected.len()];
        let seed = as_u64(&header, "seed");
        assert_no_alloc(|| default_rng(seed).standard_normal_fill(&mut got));
        check_exact(&format!("drum {name} (seed {seed})"), &expected, &got);
    }
}

#[test]
fn export_dither_matches_golden() {
    let golden = golden();
    let index: Value = golden.json("numpy/index.json").unwrap();
    for case in list(&index, "dither") {
        let case = case.as_array().expect("[name, channels, chunks]");
        let name = case[0].as_str().expect("a name");
        let mut rng = default_rng(0x5A0A);
        for (i, _) in case[2].as_array().expect("chunks").iter().enumerate() {
            let (expected, header) = f64s(&golden, &format!("numpy/dither/{name}-chunk-{i}"));
            let mut a = vec![0.0; expected.len()];
            let mut b = vec![0.0; expected.len()];
            assert_no_alloc(|| {
                // `random(shape) - random(shape)`: two full arrays, the left one first.
                rng.random_fill(&mut a);
                rng.random_fill(&mut b);
                for (x, y) in a.iter_mut().zip(&b) {
                    *x -= y;
                }
            });
            check_exact(&format!("dither {name} chunk {i}"), &expected, &a);
            check_state(
                &format!("dither {name} chunk {i}"),
                &rng,
                field(&header, "after"),
            );
        }
    }
}

#[test]
fn integers_and_choice_match_golden() {
    let golden = golden();
    let index: Value = golden.json("numpy/index.json").unwrap();
    for range in list(&index, "integers") {
        let range = range.as_array().expect("[low, high]");
        let (low, high) = (range[0].as_i64().unwrap(), range[1].as_i64().unwrap());
        let rel = format!("numpy/integers/{low}_{high}");
        let (expected, header) = words(&golden, &rel, "i64");
        let expected: Vec<Option<i64>> = expected
            .into_iter()
            .map(|w| Some(i64::from_le_bytes(w)))
            .collect();
        assert_eq!(
            (as_i64(&header, "low"), as_i64(&header, "high")),
            (low, high)
        );
        let seed = as_u64(&header, "seed");
        let mut got = vec![None; expected.len()];
        let rng = assert_no_alloc(|| {
            let mut rng = default_rng(seed);
            for v in &mut got {
                *v = rng.integers(low, high).ok();
            }
            rng
        });
        assert_eq!(got, expected, "integers({low}, {high})");
        check_state(&rel, &rng, field(&header, "after"));
    }
    for name in list(&index, "choice") {
        let name = name.as_str().expect("a population name");
        let (expected, header) = f64s(&golden, &format!("numpy/choice/{name}"));
        let population: Vec<f64> = list(&header, "population")
            .iter()
            .map(|v| v.as_f64().expect("a number"))
            .collect();
        let size = usize::try_from(field(&header, "shape")[1].as_u64().unwrap()).unwrap();
        let seed = as_u64(&header, "seed");
        let mut got = vec![0.0; expected.len()];
        let (rng, all_ok) = assert_no_alloc(|| {
            let mut rng = default_rng(seed);
            let mut all_ok = true;
            for row in got.chunks_exact_mut(size) {
                all_ok &= rng.choice(&population, row).is_ok();
            }
            (rng, all_ok)
        });
        assert!(all_ok, "choice {name}: a valid population");
        check_exact(&format!("choice {name}"), &expected, &got);
        check_state(&format!("choice {name}"), &rng, field(&header, "after"));
    }
}

#[test]
fn interleaved_calls_share_the_buffered_half() {
    let golden = golden();
    let recorded: Value = golden.json("numpy/sequence.json").unwrap();
    let mut rng = default_rng(as_u64(&recorded, "seed"));
    for (i, op) in list(&recorded, "ops").iter().enumerate() {
        let kind = field(op, "op").as_str().expect("an op");
        let out = field(op, "out");
        let what = format!("op {i} ({kind})");
        match kind {
            "integers2" => assert_eq!(rng.integers(0, 2).ok(), out.as_i64(), "{what}"),
            "integers7" => assert_eq!(rng.integers(-3, 4).ok(), out.as_i64(), "{what}"),
            "integers32" => assert_eq!(rng.integers(0, 1 << 32).ok(), out.as_i64(), "{what}"),
            "random" => check_exact(&what, &[out.as_f64().unwrap()], &[rng.random()]),
            "raw" => assert_eq!(Some(rng.bit_generator_mut().random_raw()), out.as_u64()),
            "choice" => {
                let mut pair = [0.0; 2];
                rng.choice(&[-1.0, 1.0], &mut pair).unwrap();
                let expected: Vec<f64> = out
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                check_exact(&what, &expected, &pair);
            }
            "normal" => check_exact(&what, &[out.as_f64().unwrap()], &[rng.standard_normal()]),
            other => panic!("unknown op {other}"),
        }
        let state = rng.bit_generator().state();
        assert_eq!(
            (state.has_uint32, u64::from(state.uinteger)),
            (
                field(op, "has_uint32").as_bool().unwrap(),
                as_u64(op, "uinteger")
            ),
            "{what}: 32-bit buffer"
        );
    }
}

/// The `[name, size]` cases of a concatenated input array, as slices.
fn split_cases<'a>(data: &'a [f64], header: &Value) -> Vec<(String, &'a [f64])> {
    let mut rest = data;
    let cases = list(header, "cases")
        .iter()
        .map(|case| {
            let case = case.as_array().expect("[name, size]");
            let size = usize::try_from(case[1].as_u64().expect("a size")).expect("fits");
            let (head, tail) = rest.split_at(size);
            rest = tail;
            (case[0].as_str().expect("a name").to_owned(), head)
        })
        .collect();
    assert!(rest.is_empty(), "case sizes cover the input");
    cases
}

#[test]
fn sum_and_mean_match_golden() {
    let golden = golden();
    let (input, header) = f64s(&golden, "numpy/math/reduce.in");
    let (expected, _) = f64s(&golden, "numpy/math/reduce.out");
    let cases = split_cases(&input, &header);
    assert!(cases.len() > 300);
    for ((name, values), pair) in cases.iter().zip(expected.as_chunks::<2>().0) {
        let got = assert_no_alloc(|| [sum(values), mean(values)]);
        check_exact(&format!("sum and mean of {name}"), pair, &got);
    }
    let big: Value = golden.json("numpy/math/reduce_big.json").unwrap();
    for case in list(&big, "cases") {
        let mut values = vec![0.0; as_usize(case, "count")];
        default_rng(as_u64(case, "seed")).standard_normal_fill(&mut values);
        let got = assert_no_alloc(|| [sum(&values), mean(&values)]);
        check_exact(
            &format!("sum and mean of {} normals", values.len()),
            &[as_f64(case, "sum"), as_f64(case, "mean")],
            &got,
        );
    }
}

#[test]
fn cumsum_matches_golden() {
    let golden = golden();
    let (input, header) = f64s(&golden, "numpy/math/cumsum.in");
    let (expected, _) = f64s(&golden, "numpy/math/cumsum.out");
    let mut got = input.clone();
    let mut rest = got.as_mut_slice();
    for (_, values) in split_cases(&input, &header) {
        let (head, tail) = rest.split_at_mut(values.len());
        assert_no_alloc(|| cumsum(head));
        rest = tail;
    }
    check_exact("cumsum", &expected, &got);
}

#[test]
fn linspace_matches_golden() {
    let golden = golden();
    let (args, _) = f64s(&golden, "numpy/math/linspace.args");
    let (expected, _) = f64s(&golden, "numpy/math/linspace.out");
    let mut got = vec![0.0; expected.len()];
    let mut rest = got.as_mut_slice();
    for &[start, stop, count] in args.as_chunks::<3>().0 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the recorded count is a small non-negative integer"
        )]
        let n = count as usize;
        let (head, tail) = rest.split_at_mut(n);
        assert_no_alloc(|| linspace(start, stop, head));
        rest = tail;
    }
    assert!(rest.is_empty(), "the counts cover the output");
    check_exact("linspace", &expected, &got);
}

#[test]
fn rounding_matches_golden() {
    let golden = golden();
    let (input, header) = f64s(&golden, "numpy/math/round.in");
    let decimals: Vec<i32> = list(&header, "np_decimals")
        .iter()
        .map(|d| i32::try_from(d.as_i64().unwrap()).unwrap())
        .collect();
    let ndigits: Vec<u32> = list(&header, "py_ndigits")
        .iter()
        .map(|d| u32::try_from(d.as_u64().unwrap()).unwrap())
        .collect();

    let (expected, _) = f64s(&golden, "numpy/math/rint.out");
    let got: Vec<f64> = input.iter().map(|&x| rint(x)).collect();
    check_exact("rint", &expected, &got);

    let (expected, _) = f64s(&golden, "numpy/math/np_round.out");
    let mut got = vec![0.0; expected.len()];
    assert_no_alloc(|| {
        for (&x, row) in input.iter().zip(got.chunks_exact_mut(decimals.len())) {
            for (&d, out) in decimals.iter().zip(row) {
                *out = round(x, d);
            }
        }
    });
    check_exact("np.round", &expected, &got);

    let (expected, _) = f64s(&golden, "numpy/math/py_round.out");
    let mut got = vec![0.0; expected.len()];
    assert_no_alloc(|| {
        for (&x, row) in input.iter().zip(got.chunks_exact_mut(ndigits.len())) {
            for (&nd, out) in ndigits.iter().zip(row) {
                *out = py_round(x, nd);
            }
        }
    });
    check_exact("Python round", &expected, &got);
}

#[test]
fn percentile_matches_golden() {
    let golden = golden();
    let (input, header) = f64s(&golden, "numpy/math/percentile.in");
    let (expected, _) = f64s(&golden, "numpy/math/percentile.out");
    let qs: Vec<f64> = list(&header, "q")
        .iter()
        .map(|q| q.as_f64().unwrap())
        .collect();
    let sizes = usize_list(&header, "sizes");
    let mut scratch = vec![0.0; input.len()];
    let mut got = Vec::with_capacity(expected.len());
    let mut rest = input.as_slice();
    for size in sizes {
        let (values, tail) = rest.split_at(size);
        rest = tail;
        for &q in &qs {
            let buffer = &mut scratch[..size];
            buffer.copy_from_slice(values);
            got.push(assert_no_alloc(|| percentile(buffer, q)).unwrap_or(f64::INFINITY));
        }
    }
    check_exact("percentile", &expected, &got);
}
