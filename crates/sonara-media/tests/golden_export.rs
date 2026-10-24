//! Golden parity of the export (GLD-01).
//!
//! `export/` is recorded by `scripts/golden/area_export.py` through the prototype's own
//! `Exporter` (libsndfile 1.2.2 with libFLAC 1.4.3, LAME 3.100, libvorbis 1.3.7 and libopus
//! 1.4; python-soxr) from deterministic float64 renders: every container and encoding, mono
//! and stereo, resampling, dither and no dither, every normalization mode, BWF, the metadata
//! rules and the errors. Each case is replayed here with the same blocks, a clock frozen at
//! the recorder's `FIXED_NOW` and the recorder's Ogg serial.
//!
//! Parity targets:
//!
//! - **WAV, BWF, RF64, Wave64, AIFF, CAF:** byte-identical files: headers, strings, `PEAK`,
//!   `bext` and every sample, also for the resampled outputs;
//! - **FLAC:** byte-identical files on macOS, where libFLAC 1.4.3 is the recording's build
//!   (`scripts/native/build-codecs.sh`); elsewhere ([`FLAC_BYTE_EXACT`]) the decoded PCM,
//!   `STREAMINFO` without the frame sizes, and the comments equal. Both checks run on macOS;
//! - **MP3:** the same LAME 3.100 driven the same way: the file size, the LAME tag fields, the
//!   ID3 tags and the decoded audio (within [`MP3_RESIDUAL_DB`]);
//! - **Vorbis and Opus:** other codec builds (aoTuV libvorbis, libopus 1.6.1), so the decoded
//!   audio is compared statistically ([`LOSSY_LOUDNESS_LU`], [`LOSSY_PEAK_DB`]) and the
//!   comments are equal;
//! - **results:** every field of `ExportResult.to_json()`, with the analysis exact; the sizes
//!   of lossy files (and of FLAC files off macOS) and the decoded true peak of Vorbis and Opus
//!   are compared with the tolerances above.
//!
//! The golden workflow exports (`golden/workflows/*/outputs`, `golden/coverage/outputs`) are
//! checked too: the 24-bit WAV and FLAC files and the float previews are reproduced from
//! their own decoded audio (byte for byte; for FLAC off macOS, PCM, `STREAMINFO` and comments),
//! and
//! exporting the recorded `render.wav` (float32, where the prototype exported its float64
//! render) gives the same PCM within one LSB ([`CHAIN_LSB_SHARE`]).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;
use sonara_media::export::{
    Container, Dither, ExportClock, ExportOptions, ExportResult, Exporter, Normalize,
    NormalizeSpec, OutputSpec, PcmContainer, PcmFileWriter, SampleFormat, analyze_file,
    interp_level, read_bext, validate_output,
};
use sonara_media::{DirectReader, MediaReader};
use sonara_testkit::Golden;

/// The recorder's Ogg stream serial ("SONA").
const OGG_SERIAL: u32 = 0x534F_4E41;
/// The recorder's frozen clock, 2026-01-01T12:00:00Z.
const FIXED_NOW: u64 = 1_767_268_800;

/// The vendor string of the recording's libFLAC (soundfile 0.14's libsndfile 1.2.2).
const RECORDING_FLAC_VENDOR: &str = "reference libFLAC 1.4.3 20230623";
/// Whether FLAC files must equal the prototype's byte for byte. They must on macOS, the
/// recording and product platform, where `scripts/native/build-codecs.sh` builds the recorded
/// libFLAC 1.4.3 with the recording's configuration. Elsewhere the comparison falls back to the
/// decoded PCM, `STREAMINFO` without the frame sizes, and the comments, because the bytes come
/// from another libFLAC: the development container has only Debian trixie's libFLAC 1.5.0,
/// whose vendor string differs from the recording's in every file, and whose LPC analysis is
/// compiled by GCC (other floating-point contraction) and, on the x86-64 CI runner, picks
/// SSE/AVX/FMA routines at run time; any of these can choose other predictor coefficients and so
/// other bytes for the same samples. The fallback still pins the lossless content exactly (GLD-01).
const FLAC_BYTE_EXACT: bool = cfg!(target_os = "macos");

/// Largest difference of MP3 decodes relative to the decoded signal, in dB: both files come
/// from LAME 3.100 with the same settings, calls and input, and differ only where the two LAME
/// builds round differently (the prototype's is statically linked into soundfile's macOS
/// libsndfile, the container's is Debian's), which flips some quantization decisions. Measured
/// in the container: 5 of 17 files byte-identical, the others at −51.1 dB (320 kbps) to
/// −100 dB, with equal sizes, LAME tags and ID3 tags.
const MP3_RESIDUAL_DB: f64 = -40.0;
/// Largest difference of the integrated loudness of Vorbis and Opus decodes, in LU: another
/// libvorbis (aoTuV with the Lancer optimizations, against Xiph 1.3.7) and libopus (1.6.1
/// against 1.4). Measured in the container: at most 0.018 LU.
const LOSSY_LOUDNESS_LU: f64 = 0.2;
/// Largest difference of the decoded true peak of Vorbis and Opus files, in dB: the codecs
/// shape their noise differently, which moves the overshoots. Measured in the container: at
/// most 0.46 dB.
const LOSSY_PEAK_DB: f64 = 1.0;
/// Largest share of samples that may differ by one LSB when `render.wav` (float32) stands in
/// for the prototype's float64 render: rounding a float64 to float32 moves a 24-bit sample by
/// up to half an LSB. Measured: 3.0 % (`sidechain_preview`) and 2.0 % (`mix_analysis`); none
/// differs by more than one LSB.
const CHAIN_LSB_SHARE: f64 = 0.1;

fn golden() -> Golden {
    Golden::open().expect("golden data is available (set SONARA_GOLDEN to a recording)")
}

fn clock() -> ExportClock {
    Arc::new(|| UNIX_EPOCH + Duration::from_secs(FIXED_NOW))
}

fn field<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap_or_else(|| panic!("no {key} in {v}"))
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// `a / b` for counts and sizes (far below 2^53, where the conversion is exact).
#[expect(
    clippy::cast_precision_loss,
    reason = "counts and sizes stay far below 2^53"
)]
fn ratio(a: usize, b: usize) -> f64 {
    a as f64 / b as f64
}

/// A directory for the test's files below the target directory, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// Decodes a file this crate reads: channels and interleaved samples.
fn decode(path: &Path) -> (usize, Vec<f64>) {
    let reader = DirectReader::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut reader = MediaReader::Direct(reader);
    let frames = usize::try_from(reader.frames()).expect("frames");
    let channels = reader.channels();
    (channels, reader.read(0, frames).expect("decoded"))
}

// ---------------------------------------------------------------------------------------------
// The recorded cases
// ---------------------------------------------------------------------------------------------

/// One replayed case.
struct CaseRun {
    name: String,
    case: Value,
    golden_dir: PathBuf,
    result: Result<ExportResult, String>,
}

impl CaseRun {
    fn result(&self) -> &ExportResult {
        self.result
            .as_ref()
            .unwrap_or_else(|e| panic!("{}: {e}", self.name))
    }

    fn expected(&self) -> &Value {
        field(&self.case, "result")
    }

    /// Our file and the prototype's for every output.
    fn files(&self) -> Vec<(Container, PathBuf, PathBuf)> {
        self.result()
            .outputs
            .iter()
            .map(|o| {
                let name = o.path.file_name().expect("file name");
                (o.container, o.path.clone(), self.golden_dir.join(name))
            })
            .collect()
    }
}

fn spec_from(out: &Value, dir: &Path) -> OutputSpec {
    let container: Container = text(field(out, "container")).parse().expect("container");
    let mut spec = OutputSpec::new(dir.join(format!(
        "{}{}",
        text(field(out, "name")),
        container.extension()
    )));
    spec.container = container;
    let u32_of = |key: &str| {
        out.get(key)
            .and_then(Value::as_u64)
            .map(|v| u32::try_from(v).expect("u32"))
    };
    if let Some(depth) = u32_of("bit_depth") {
        spec.bit_depth = depth;
    }
    spec.floating_point = out
        .get("floating_point")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    spec.sample_rate = u32_of("sample_rate");
    spec.channels = u32_of("channels");
    spec.bitrate_kbps = u32_of("bitrate_kbps");
    spec.variable_bitrate = out
        .get("variable_bitrate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    spec.dither = match out.get("dither").and_then(Value::as_str) {
        Some("none") => Dither::None,
        _ => Dither::Tpdf,
    };
    spec
}

fn normalize_from(v: &Value) -> NormalizeSpec {
    let mut spec = NormalizeSpec::default();
    if v.is_null() {
        return spec;
    }
    spec.mode = match text(field(v, "mode")) {
        "peak" => Normalize::Peak,
        "true_peak" => Normalize::TruePeak,
        "loudness" => Normalize::Loudness,
        _ => Normalize::None,
    };
    if let Some(t) = v.get("target_db").and_then(Value::as_f64) {
        spec.target_db = t;
    }
    if let Some(c) = v.get("true_peak_ceiling_db") {
        spec.true_peak_ceiling_db = c.as_f64();
    }
    spec
}

fn metadata_from(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .expect("metadata pairs")
        .iter()
        .map(|pair| (text(&pair[0]).to_owned(), text(&pair[1]).to_owned()))
        .collect()
}

/// Every recorded case, replayed once and shared by the tests.
fn runs() -> &'static [CaseRun] {
    static RUNS: OnceLock<Vec<CaseRun>> = OnceLock::new();
    RUNS.get_or_init(|| {
        let golden = golden();
        let cases: Value = golden.json("export/cases.json").unwrap_or_else(|e| {
            panic!(
                "the export area is not in {} ({e}); run `git lfs pull`, or point \
                 SONARA_GOLDEN at a golden directory that includes it",
                golden.root().display()
            )
        });
        let root = scratch("golden-export");
        let mut inputs: BTreeMap<String, (Vec<f64>, u32)> = BTreeMap::new();
        let mut runs = Vec::new();
        for case in cases.as_array().expect("case list") {
            let name = text(field(case, "name")).to_owned();
            let input = text(field(case, "input")).to_owned();
            let (samples, rate) = inputs.entry(input.clone()).or_insert_with(|| {
                let array = golden
                    .f64_array(&format!("export/inputs/{input}"))
                    .expect("input");
                let rate = array.header["sample_rate"].as_u64().expect("rate");
                (array.data, u32::try_from(rate).expect("rate"))
            });
            let dir = root.join(&name);
            let outputs: Vec<OutputSpec> = field(case, "outputs")
                .as_array()
                .expect("outputs")
                .iter()
                .map(|o| spec_from(o, &dir))
                .collect();
            let metadata = metadata_from(field(case, "metadata"));
            let block = usize::try_from(field(case, "block_frames").as_u64().expect("block"))
                .expect("block");
            let options = ExportOptions {
                normalize: normalize_from(field(case, "normalize")),
                metadata: &metadata,
                overwrite: true,
                time_reference: field(case, "time_reference").as_u64().expect("time ref"),
                ogg_serial: Some(OGG_SERIAL),
                ..ExportOptions::default()
            };
            let exporter = Exporter::new(dir.join("cache")).with_clock(clock());
            let result = exporter
                .export(samples.chunks(block * 2), *rate, &outputs, &options)
                .map_err(|e| e.to_string());
            runs.push(CaseRun {
                golden_dir: golden.root().join("export/outputs").join(&name),
                name,
                case: case.clone(),
                result,
            });
        }
        runs
    })
}

#[test]
fn every_case_runs_and_the_intermediate_files_are_removed() {
    let failures: Vec<String> = runs()
        .iter()
        .filter_map(|r| r.result.as_ref().err().map(|e| format!("{}: {e}", r.name)))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
    for run in runs() {
        for (_, ours, _) in run.files() {
            let cache = ours.parent().expect("dir").join("cache");
            let left: Vec<_> = fs::read_dir(&cache)
                .map(|d| d.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            assert!(left.is_empty(), "{}: {left:?}", run.name);
            let partial = ours.with_file_name(format!(
                ".{}.partial",
                ours.file_name().expect("name").to_string_lossy()
            ));
            assert!(!partial.exists(), "{}", partial.display());
        }
    }
}

#[test]
fn uncompressed_files_are_byte_identical() {
    let mut checked = 0;
    for run in runs() {
        for (container, ours, theirs) in run.files() {
            if PcmContainerKind::of(container) {
                assert!(
                    read(&ours) == read(&theirs),
                    "{}: {} differs from the prototype's file",
                    run.name,
                    ours.display()
                );
                checked += 1;
            }
        }
    }
    eprintln!("{checked} uncompressed files byte-identical");
    assert!(checked >= 70, "{checked}");
}

/// Whether a container is written by the own uncompressed writer.
struct PcmContainerKind;

impl PcmContainerKind {
    fn of(c: Container) -> bool {
        matches!(
            c,
            Container::Wav
                | Container::Bwf
                | Container::Rf64
                | Container::W64
                | Container::Aiff
                | Container::Caf
        )
    }
}

/// Compares `ours` and `theirs`, recording each difference under its path.
fn diff(path: &str, ours: &Value, theirs: &Value, out: &mut Vec<String>) {
    match (ours, theirs) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: Vec<&String> = a
                .keys()
                .chain(b.keys().filter(|k| !a.contains_key(*k)))
                .collect();
            for key in keys {
                diff(
                    &format!("{path}.{key}"),
                    a.get(key).unwrap_or(&Value::Null),
                    b.get(key).unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{i}]"), x, y, out);
            }
        }
        (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => {}
        _ if ours == theirs => {}
        _ => out.push(format!("{path}: {ours} vs {theirs}")),
    }
}

/// Levels that end in one libm call may differ by this many machine epsilons of
/// `max(|expected|, 1)` (the Analyzer's documented tolerance, `sonara-dsp`'s
/// `tests/golden_analysis.rs`).
const LIBM_EPSILONS: f64 = 8.0;
/// `side_to_mid_db`, absolute dB (NumPy's alignment-dependent `ddot` order).
const SIDE_TO_MID_TOLERANCE: f64 = 1e-12;
/// Octave bands as energy shares of the total, absolute (`realfft` against pocketfft).
const BAND_SHARE_TOLERANCE: f64 = 1e-14;
/// `spectral_centroid_hz`, relative.
const CENTROID_TOLERANCE: f64 = 1e-14;

/// Compares an analysis report with the Analyzer's documented tolerances; every other field
/// exactly. The recorder's volatile duration is skipped (it is frames / rate).
fn check_analysis(at: &str, ours: &Value, theirs: &Value, problems: &mut Vec<String>) {
    let levels = [
        "sample_peak_dbfs",
        "true_peak_dbtp",
        "rms_dbfs",
        "crest_factor_db",
        "integrated_lufs",
        "momentary_max_lufs",
        "short_term_max_lufs",
        "loudness_range_lu",
        "plr_db",
        "balance_db",
    ];
    // Non-finite values are recorded as objects and compared exactly below.
    let close = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol;
    let mut rest_a = ours.clone();
    let mut rest_b = theirs.clone();
    for key in levels.iter().chain(&[
        "side_to_mid_db",
        "spectral_centroid_hz",
        "octave_bands_db",
        "duration_seconds",
    ]) {
        let (a, b) = (ours[*key].clone(), theirs[*key].clone());
        rest_a[*key] = Value::Null;
        rest_b[*key] = Value::Null;
        let ok = match *key {
            "duration_seconds" => true,
            "octave_bands_db" => {
                // The recording's keys are sorted; the bands pair up by name.
                let share = |v: &Value| -> BTreeMap<String, f64> {
                    let bands: BTreeMap<String, f64> = v
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .filter_map(|(k, d)| {
                                    d.as_f64().map(|d| (k.clone(), 10f64.powf(d / 10.0)))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let total: f64 = bands.values().sum();
                    bands
                        .into_iter()
                        .map(|(k, e)| (k, e / total.max(1e-300)))
                        .collect()
                };
                let (sa, sb) = (share(&a), share(&b));
                sa.len() == sb.len()
                    && sa.iter().all(|(k, x)| {
                        sb.get(k)
                            .is_some_and(|y| (x - y).abs() <= BAND_SHARE_TOLERANCE)
                    })
            }
            _ => match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => {
                    let tol = match *key {
                        "side_to_mid_db" => SIDE_TO_MID_TOLERANCE,
                        "spectral_centroid_hz" => CENTROID_TOLERANCE * y.abs(),
                        _ => LIBM_EPSILONS * f64::EPSILON * y.abs().max(1.0),
                    };
                    close(x, y, tol)
                }
                _ => a == b,
            },
        };
        if !ok {
            problems.push(format!("{at}.analysis_before_gain.{key}: {a} vs {b}"));
        }
    }
    diff(
        &format!("{at}.analysis_before_gain"),
        &rest_a,
        &rest_b,
        problems,
    );
}

/// Compares one output entry of the results (sizes and decoded peaks of the codecs that are
/// not the prototype's with the documented tolerances).
fn check_output(
    case: &str,
    o: &Value,
    e: &Value,
    problems: &mut Vec<String>,
    peaks: &mut Vec<(String, f64)>,
) {
    let container = Container::from_str_value(text(&o["container"]));
    let file = |v: &Value| {
        Path::new(text(&v["path"]))
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    };
    let at = format!("{case}.{}", file(o).unwrap_or_default());
    if file(o) != file(e) {
        problems.push(format!("{at}: path {} vs {}", o["path"], e["path"]));
    }
    for key in [
        "container",
        "sample_rate",
        "channels",
        "bit_depth",
        "floating_point",
        "frames",
        "seconds",
        "clipped_samples",
        "dithered",
        "resampled_from",
        "metadata_written",
        "verified",
    ] {
        diff(&format!("{at}.{key}"), &o[key], &e[key], problems);
    }
    // LAME and, on macOS, libFLAC are the prototype's: equal sizes give equal bitrates.
    let exact_size = match container {
        Container::Ogg | Container::Opus => false,
        Container::Flac => FLAC_BYTE_EXACT,
        _ => true,
    };
    if exact_size {
        diff(
            &format!("{at}.size_bytes"),
            &o["size_bytes"],
            &e["size_bytes"],
            problems,
        );
        diff(
            &format!("{at}.bitrate_kbps"),
            &o["bitrate_kbps"],
            &e["bitrate_kbps"],
            problems,
        );
    }
    match container {
        Container::Ogg | Container::Opus => {
            let ours = o["decoded_true_peak_dbtp"].as_f64().expect("decoded peak");
            let theirs = e["decoded_true_peak_dbtp"].as_f64().expect("decoded peak");
            peaks.push((at.clone(), ours - theirs));
            if (ours - theirs).abs() > LOSSY_PEAK_DB {
                problems.push(format!("{at}: decoded true peak {ours} vs {theirs}"));
            }
        }
        Container::Mp3 => {
            let (ours, theirs) = (&o["decoded_true_peak_dbtp"], &e["decoded_true_peak_dbtp"]);
            let close = match (ours.as_f64(), theirs.as_f64()) {
                (Some(a), Some(b)) => (a - b).abs() <= 0.01 + 1e-9,
                _ => ours == theirs,
            };
            if !close {
                problems.push(format!("{at}: decoded true peak {ours} vs {theirs}"));
            }
        }
        _ => diff(
            &format!("{at}.decoded_true_peak_dbtp"),
            &o["decoded_true_peak_dbtp"],
            &e["decoded_true_peak_dbtp"],
            problems,
        ),
    }
}

#[test]
fn results_match_the_prototype() {
    let mut problems = Vec::new();
    let mut peaks = Vec::new();
    for run in runs() {
        let ours = run.result().to_json();
        let expected = run.expected();
        for key in [
            "gain_db",
            "final_integrated_lufs",
            "final_true_peak_dbtp",
            "normalization_target_reached",
            "warnings",
            "analysis_before_gain",
        ] {
            if key == "analysis_before_gain" {
                check_analysis(&run.name, &ours[key], &expected[key], &mut problems);
            } else {
                diff(
                    &format!("{}.{key}", run.name),
                    &ours[key],
                    &expected[key],
                    &mut problems,
                );
            }
        }
        let outputs = field(expected, "outputs").as_array().expect("outputs");
        assert_eq!(
            ours["outputs"].as_array().map(Vec::len),
            Some(outputs.len())
        );
        for (o, e) in ours["outputs"]
            .as_array()
            .expect("outputs")
            .iter()
            .zip(outputs)
        {
            check_output(&run.name, o, e, &mut problems, &mut peaks);
        }
    }
    let worst = peaks.iter().map(|(_, d)| d.abs()).fold(0.0, f64::max);
    eprintln!("Vorbis/Opus decoded true peak: largest difference {worst:.2} dB");
    assert!(problems.is_empty(), "{problems:#?}");
}

/// `Container::from_str` for the tests' JSON values.
trait FromValue {
    fn from_str_value(s: &str) -> Self;
}

impl FromValue for Container {
    fn from_str_value(s: &str) -> Self {
        s.parse().expect("container")
    }
}

// ---------------------------------------------------------------------------------------------
// FLAC
// ---------------------------------------------------------------------------------------------

/// The metadata blocks of a FLAC file: `(type, body)`.
fn flac_blocks(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
    assert_eq!(&bytes[..4], b"fLaC");
    let mut pos = 4;
    let mut blocks = Vec::new();
    loop {
        let header = bytes[pos];
        let len = usize::from(bytes[pos + 1]) << 16
            | usize::from(bytes[pos + 2]) << 8
            | usize::from(bytes[pos + 3]);
        blocks.push((header & 0x7F, bytes[pos + 4..pos + 4 + len].to_vec()));
        pos += 4 + len;
        if header & 0x80 != 0 {
            return blocks;
        }
    }
}

/// A Vorbis comment body: the vendor and the comments.
fn vorbis_comments(body: &[u8]) -> (String, Vec<String>) {
    let u32_at = |p: usize| u32::from_le_bytes(body[p..p + 4].try_into().expect("u32")) as usize;
    let vendor_len = u32_at(0);
    let vendor = String::from_utf8_lossy(&body[4..4 + vendor_len]).into_owned();
    let mut pos = 4 + vendor_len;
    let count = u32_at(pos);
    pos += 4;
    let mut comments = Vec::new();
    for _ in 0..count {
        let len = u32_at(pos);
        comments.push(String::from_utf8_lossy(&body[pos + 4..pos + 4 + len]).into_owned());
        pos += 4 + len;
    }
    (vendor, comments)
}

/// Checks one of our FLAC files against the prototype's: on macOS byte for byte
/// ([`FLAC_BYTE_EXACT`]); everywhere the decoded PCM, `STREAMINFO` without the frame sizes, the
/// comments, and that libFLAC wrote both. Returns whether the files are byte-identical.
fn check_flac(ours: &Path, theirs: &Path) -> Result<bool, String> {
    let (a, b) = (read(ours), read(theirs));
    let identical = a == b;
    if FLAC_BYTE_EXACT && !identical {
        let at = a
            .iter()
            .zip(&b)
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        let vendor = vorbis_comments(&flac_blocks(&a)[1].1).0;
        return Err(format!(
            "not byte-identical: {} vs {} bytes, first difference at byte {at}; our vendor \
             {vendor:?} (expected {RECORDING_FLAC_VENDOR:?})",
            a.len(),
            b.len()
        ));
    }
    let (blocks_a, blocks_b) = (flac_blocks(&a), flac_blocks(&b));
    let kinds = |blocks: &[(u8, Vec<u8>)]| blocks.iter().map(|(t, _)| *t).collect::<Vec<_>>();
    if kinds(&blocks_a) != [0, 4] || kinds(&blocks_b) != [0, 4] {
        return Err(format!(
            "blocks {:?} vs {:?}",
            kinds(&blocks_a),
            kinds(&blocks_b)
        ));
    }
    let (si_a, si_b) = (&blocks_a[0].1, &blocks_b[0].1);
    // min/max block size (4 bytes), then frame sizes (6 bytes, encoder-specific), then rate,
    // channels, bits, total samples (8 bytes) and the MD5 (16 bytes).
    if si_a[..4] != si_b[..4] || si_a[10..] != si_b[10..] {
        return Err(format!("STREAMINFO {si_a:02x?} vs {si_b:02x?}"));
    }
    let (vendor_a, comments_a) = vorbis_comments(&blocks_a[1].1);
    let (vendor_b, comments_b) = vorbis_comments(&blocks_b[1].1);
    if comments_a != comments_b {
        return Err(format!("comments {comments_a:?} vs {comments_b:?}"));
    }
    if !vendor_a.starts_with("reference libFLAC ") || vendor_b != RECORDING_FLAC_VENDOR {
        return Err(format!("vendors {vendor_a} vs {vendor_b}"));
    }
    if decode(ours) != decode(theirs) {
        return Err("decoded PCM differs".to_owned());
    }
    Ok(identical)
}

#[test]
fn flac_files_are_the_prototypes() {
    let mut checked = 0;
    let mut identical = 0;
    let mut vendor = String::new();
    for run in runs() {
        for (container, ours, theirs) in run.files() {
            if container == Container::Flac {
                let same =
                    check_flac(&ours, &theirs).unwrap_or_else(|e| panic!("{}: {e}", run.name));
                identical += usize::from(same);
                vendor = vorbis_comments(&flac_blocks(&read(&ours))[1].1).0;
                checked += 1;
            }
        }
    }
    eprintln!(
        "{checked} FLAC files ({identical} byte-identical, exact check {}); encoder {vendor:?}",
        if FLAC_BYTE_EXACT { "on" } else { "off" }
    );
    assert!(checked >= 10, "{checked}");
}

// ---------------------------------------------------------------------------------------------
// MP3
// ---------------------------------------------------------------------------------------------

/// The Xing/Info tag and LAME extension of the first frame (after an optional `ID3v2` tag).
#[derive(Debug, PartialEq)]
struct LameTag {
    id: [u8; 4],
    flags: u32,
    frames: u32,
    bytes: u32,
    toc: Vec<u8>,
    quality: u32,
    encoder: String,
    revision_method: u8,
    lowpass: u8,
    encoding_flags: u8,
    bitrate: u8,
    delay: u16,
    padding: u16,
    misc: u8,
    preset: u16,
    music_length: u32,
}

fn lame_tag(bytes: &[u8]) -> (LameTag, [u8; 2]) {
    let start = if bytes.starts_with(b"ID3") {
        let size = bytes[6..10]
            .iter()
            .fold(0usize, |acc, &b| (acc << 7) | usize::from(b & 0x7F));
        10 + size
    } else {
        0
    };
    let frame = &bytes[start..start + 400];
    let at = frame
        .windows(4)
        .position(|w| w == b"Info" || w == b"Xing")
        .expect("Xing/Info tag");
    let t = &frame[at..];
    let be32 = |p: usize| u32::from_be_bytes(t[p..p + 4].try_into().expect("u32"));
    let flags = be32(4);
    assert_eq!(
        flags & 0xF,
        0xF,
        "frames, bytes, TOC and quality are present"
    );
    let lame = &t[120..];
    let delay_padding = u32::from(lame[21]) << 16 | u32::from(lame[22]) << 8 | u32::from(lame[23]);
    (
        LameTag {
            id: t[..4].try_into().expect("id"),
            flags,
            frames: be32(8),
            bytes: be32(12),
            toc: t[16..116].to_vec(),
            quality: be32(116),
            encoder: String::from_utf8_lossy(&lame[..9]).into_owned(),
            revision_method: lame[9],
            lowpass: lame[10],
            encoding_flags: lame[19],
            bitrate: lame[20],
            delay: u16::try_from(delay_padding >> 12).expect("12 bits"),
            padding: u16::try_from(delay_padding & 0xFFF).expect("12 bits"),
            misc: lame[24],
            preset: u16::from_be_bytes([lame[26], lame[27]]),
            music_length: u32::from_be_bytes(lame[28..32].try_into().expect("u32")),
        },
        [lame[32], lame[33]],
    )
}

/// Residual of `ours` against `theirs` relative to `theirs`, in dB.
fn residual_db(ours: &[f64], theirs: &[f64]) -> f64 {
    let n = ours.len().min(theirs.len());
    let (mut err, mut sig) = (0.0, 0.0);
    for (a, b) in ours[..n].iter().zip(&theirs[..n]) {
        err += (a - b) * (a - b);
        sig += b * b;
    }
    if err == 0.0 {
        return f64::NEG_INFINITY;
    }
    10.0 * (err / sig.max(1e-30)).log10()
}

#[test]
fn mp3_files_have_the_prototypes_lame_tag_id3_tags_and_audio() {
    let mut checked = 0;
    let mut identical = 0;
    let mut worst = f64::NEG_INFINITY;
    let mut residuals = Vec::new();
    for run in runs() {
        for (container, ours, theirs) in run.files() {
            if container != Container::Mp3 {
                continue;
            }
            let at = format!("{}: {}", run.name, ours.display());
            let (a, b) = (read(&ours), read(&theirs));
            assert_eq!(a.len(), b.len(), "{at}: size");
            if a == b {
                identical += 1;
            }
            let ((tag_a, _), (tag_b, _)) = (lame_tag(&a), lame_tag(&b));
            assert_eq!(tag_a, tag_b, "{at}: LAME tag");
            assert_eq!(tag_a.encoder, "LAME3.100");
            // ID3v1 trailer and ID3v2 header.
            assert_eq!(a[a.len() - 128..], b[b.len() - 128..], "{at}: trailer");
            assert_eq!(a.starts_with(b"ID3"), b.starts_with(b"ID3"), "{at}: ID3v2");
            if a.starts_with(b"ID3") {
                let size = 10
                    + b[6..10]
                        .iter()
                        .fold(0usize, |acc, &x| (acc << 7) | usize::from(x & 0x7F));
                assert_eq!(a[..size], b[..size], "{at}: ID3v2 tag");
            }
            let ((_, da), (_, db)) = (decode(&ours), decode(&theirs));
            assert_eq!(da.len(), db.len(), "{at}: decoded length");
            let r = residual_db(&da, &db);
            eprintln!("{at}: residual {r:.1} dB, identical {}", a == b);
            worst = worst.max(r);
            residuals.push((at, r));
            checked += 1;
        }
    }
    eprintln!("{checked} MP3 files ({identical} byte-identical); worst residual {worst:.1} dB");
    for (at, r) in &residuals {
        assert!(*r <= MP3_RESIDUAL_DB, "{at}: residual {r:.1} dB");
    }
    assert!(checked >= 17, "{checked}");
}

// ---------------------------------------------------------------------------------------------
// Vorbis and Opus
// ---------------------------------------------------------------------------------------------

/// The comment packet (the second packet) of an Ogg stream: vendor and comments.
fn ogg_comments(path: &Path, opus: bool) -> (String, Vec<String>, u32) {
    let file = fs::File::open(path).expect("open");
    let mut reader = ogg::reading::PacketReader::new(std::io::BufReader::new(file));
    let first = reader.read_packet().expect("ogg").expect("first packet");
    let second = reader.read_packet().expect("ogg").expect("second packet");
    let body = if opus {
        assert_eq!(&second.data[..8], b"OpusTags");
        &second.data[8..]
    } else {
        assert_eq!(&second.data[..7], b"\x03vorbis");
        &second.data[7..]
    };
    let (vendor, comments) = vorbis_comments(body);
    (vendor, comments, first.stream_serial())
}

#[test]
fn vorbis_and_opus_files_match_statistically_with_the_prototypes_comments() {
    let mut checked = 0;
    let (mut loud, mut peak) = (0.0f64, 0.0f64);
    for run in runs() {
        for (container, ours, theirs) in run.files() {
            let opus = container == Container::Opus;
            if !opus && container != Container::Ogg {
                continue;
            }
            let at = format!("{}: {}", run.name, ours.display());
            let (vendor_a, comments_a, serial_a) = ogg_comments(&ours, opus);
            let (vendor_b, comments_b, serial_b) = ogg_comments(&theirs, opus);
            assert_eq!(comments_a, comments_b, "{at}: comments");
            assert_eq!(serial_a, serial_b, "{at}: serial");
            if opus {
                assert!(vendor_a.starts_with("libopus "), "{vendor_a}");
                assert_eq!(vendor_b, "libopus 1.4");
            } else {
                assert_eq!(
                    vendor_b,
                    "Xiph.Org libVorbis I 20200704 (Reducing Environment)"
                );
            }
            let (a, b) = (
                analyze_file(&ours).expect("ours"),
                analyze_file(&theirs).expect("theirs"),
            );
            assert_eq!(a.sample_rate, b.sample_rate, "{at}");
            assert_eq!(a.channels, b.channels, "{at}");
            assert!(
                (a.duration_seconds - b.duration_seconds).abs() < 1e-9,
                "{at}: duration"
            );
            if b.integrated_lufs.is_finite() {
                let d = (a.integrated_lufs - b.integrated_lufs).abs();
                loud = loud.max(d);
                assert!(
                    d <= LOSSY_LOUDNESS_LU,
                    "{at}: loudness {} vs {}",
                    a.integrated_lufs,
                    b.integrated_lufs
                );
            }
            let d = (a.true_peak_dbtp - b.true_peak_dbtp).abs();
            peak = peak.max(d);
            assert!(
                d <= LOSSY_PEAK_DB,
                "{at}: true peak {} vs {}",
                a.true_peak_dbtp,
                b.true_peak_dbtp
            );
            checked += 1;
        }
    }
    eprintln!("{checked} Ogg files; loudness within {loud:.3} LU, true peak within {peak:.2} dB");
    assert!(checked >= 14, "{checked}");
}

// ---------------------------------------------------------------------------------------------
// Errors, tables, BWF
// ---------------------------------------------------------------------------------------------

#[test]
fn errors_match_the_prototype() {
    let golden = golden();
    let errors: Value = golden.json("export/errors.json").expect("errors");
    let dir = scratch("golden-export-errors");
    let exporter = Exporter::new(dir.join("cache")).with_clock(clock());
    let silence = vec![0.0; 2000];
    for e in errors.as_array().expect("errors") {
        let case = text(field(e, "case"));
        let outcome = if let Some(spec) = e.get("spec") {
            let mut spec_json = spec.clone();
            spec_json["name"] = Value::from(case);
            validate_output(&spec_from(&spec_json, &dir)).map_err(|e| e.to_domain_error())
        } else {
            let existing = dir.join("exists.wav");
            fs::write(&existing, b"x").expect("write");
            let directory = dir.join("dir.wav");
            fs::create_dir_all(&directory).expect("mkdir");
            let cancelled = || true;
            let (specs, blocks, options): (Vec<OutputSpec>, Vec<&[f64]>, ExportOptions) = match case
            {
                "no-outputs" => (vec![], vec![&silence], ExportOptions::default()),
                "exists" => (
                    vec![OutputSpec::new(&existing)],
                    vec![&silence],
                    ExportOptions::default(),
                ),
                "directory" => (
                    vec![OutputSpec::new(&directory)],
                    vec![&silence],
                    ExportOptions {
                        overwrite: true,
                        ..ExportOptions::default()
                    },
                ),
                "nothing-rendered" => (
                    vec![OutputSpec::new(dir.join("empty.wav"))],
                    vec![],
                    ExportOptions::default(),
                ),
                "cancelled" => (
                    vec![OutputSpec::new(dir.join("cancel.wav"))],
                    vec![&silence],
                    ExportOptions {
                        cancelled: Some(&cancelled),
                        ..ExportOptions::default()
                    },
                ),
                other => panic!("unknown case {other}"),
            };
            exporter
                .export(blocks, 48_000, &specs, &options)
                .map(|_| ())
                .map_err(|e| e.to_domain_error())
        };
        match outcome {
            Ok(()) => assert_eq!(e.get("ok"), Some(&Value::Bool(true)), "{case}: {e}"),
            Err(err) => {
                assert_eq!(err.code.as_str(), text(field(e, "code")), "{case}");
                let root = dir.to_string_lossy().into_owned();
                let message = err.message.replace(&root, "<ROOT>/errors");
                assert_eq!(message, text(field(e, "message")), "{case}");
                let details: Value = Value::Object(err.details.clone());
                let details = serde_json::to_string(&details)
                    .expect("json")
                    .replace(&root, "<ROOT>/errors");
                assert_eq!(
                    serde_json::from_str::<Value>(&details).expect("json"),
                    e["details"],
                    "{case}"
                );
            }
        }
    }
}

#[test]
fn rates_and_compression_levels_match_the_prototype() {
    let tables: Value = golden().json("export/tables.json").expect("tables");
    assert_eq!(tables["chunk"], 65536);
    for row in tables["resolved_rates"].as_array().expect("rates") {
        let container: Container = text(&row["container"]).parse().expect("container");
        let mut spec = OutputSpec::new("x");
        spec.container = container;
        spec.sample_rate = row["requested"]
            .as_u64()
            .map(|r| u32::try_from(r).expect("rate"));
        let source = u32::try_from(row["source"].as_u64().expect("source")).expect("rate");
        assert_eq!(
            u64::from(spec.resolved_rate(source)),
            row["resolved"].as_u64().expect("resolved"),
            "{row}"
        );
    }
    for row in tables["levels"].as_array().expect("levels") {
        let container: Container = text(&row["container"]).parse().expect("container");
        let mut spec = OutputSpec::new("x");
        spec.container = container;
        spec.variable_bitrate = row["variable"].as_bool().expect("variable");
        spec.bitrate_kbps = Some(u32::try_from(row["kbps"].as_u64().expect("kbps")).expect("kbps"));
        let expected = row["level"].as_f64().expect("level");
        let table: &[(f64, f64)] = match (container, spec.variable_bitrate) {
            (Container::Mp3, false) => &[
                (0.0, 320.0),
                (0.25, 256.0),
                (0.5, 160.0),
                (0.75, 96.0),
                (0.9, 56.0),
                (0.99, 32.0),
            ],
            (Container::Mp3, true) => &[
                (0.0, 268.0),
                (0.1, 214.0),
                (0.25, 179.0),
                (0.5, 126.0),
                (0.75, 104.0),
                (0.9, 79.0),
                (0.99, 64.0),
            ],
            (Container::Ogg, _) => &[
                (0.0, 560.0),
                (0.1, 364.0),
                (0.25, 255.0),
                (0.5, 171.0),
                (0.75, 105.0),
                (0.9, 85.0),
                (0.99, 70.0),
            ],
            _ => &[
                (0.0, 514.0),
                (0.1, 466.0),
                (0.25, 390.0),
                (0.5, 265.0),
                (0.75, 139.0),
                (0.9, 64.0),
                (0.99, 14.0),
            ],
        };
        let kbps = row["kbps"].as_f64().expect("kbps");
        assert_eq!(
            interp_level(table, kbps).to_bits(),
            expected.to_bits(),
            "{row}"
        );
    }
}

#[test]
fn bwf_chunks_read_back_like_the_prototypes() {
    let mut checked = 0;
    for run in runs() {
        let files = field(&run.case, "files").as_array().expect("files");
        for (entry, out) in files.iter().zip(&run.result().outputs) {
            let Some(expected) = entry.get("bext") else {
                continue;
            };
            let ours = read_bext(&out.path).expect("read").expect("bext");
            assert_eq!(&ours.to_json(), expected, "{}", run.name);
            checked += 1;
        }
    }
    assert!(checked >= 8, "{checked}");
}

// ---------------------------------------------------------------------------------------------
// The golden workflow exports
// ---------------------------------------------------------------------------------------------

/// The `LIST`/`INFO` strings of a RIFF file as exporter metadata, in file order.
fn info_metadata(wav: &[u8]) -> Vec<(String, String)> {
    let list = wav
        .windows(8)
        .position(|w| &w[..4] == b"LIST" && &w[4..] != b"\0\0\0\0")
        .expect("LIST chunk");
    let size = u32::from_le_bytes(wav[list + 4..list + 8].try_into().expect("u32")) as usize;
    let mut pos = list + 12;
    let mut out = Vec::new();
    while pos < list + 8 + size {
        let id = &wav[pos..pos + 4];
        let len = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().expect("u32")) as usize;
        let value = &wav[pos + 8..pos + 8 + len];
        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
        let key = match id {
            b"INAM" => "title",
            b"IART" => "artist",
            b"IGNR" => "genre",
            b"ICMT" => "comment",
            b"ICOP" => "copyright",
            b"ICRD" => "date",
            b"IPRD" => "album",
            b"ITRK" => "tracknumber",
            b"ISFT" => "software",
            other => panic!("unexpected INFO field {other:?}"),
        };
        out.push((
            key.to_owned(),
            String::from_utf8_lossy(&value[..end]).into_owned(),
        ));
        pos += 8 + len;
    }
    out
}

/// Re-exports decoded 24-bit PCM without dither: the exact integers again.
fn reexport(samples: &[f64], channels: usize, spec: OutputSpec, metadata: &[(String, String)]) {
    let stereo: Vec<f64> = if channels == 1 {
        samples.iter().flat_map(|&v| [v, v]).collect()
    } else {
        samples.to_vec()
    };
    let options = ExportOptions {
        metadata,
        overwrite: true,
        ..ExportOptions::default()
    };
    let dir = spec.path.parent().expect("dir").join("cache");
    Exporter::new(dir)
        .with_clock(clock())
        .export(stereo.chunks(65536 * 2), 48_000, &[spec], &options)
        .expect("export");
}

/// The 24-bit WAV and FLAC exports of the golden workflows and stems.
fn golden_exports(golden: &Golden) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for rel in ["workflows", "coverage/outputs"] {
        let root = golden.root().join(rel);
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).expect("dir").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.to_string_lossy().contains("/outputs/") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

#[test]
fn golden_24_bit_exports_are_reproduced_from_their_pcm() {
    let golden = golden();
    let dir = scratch("golden-export-workflows");
    let mut checked = 0;
    let (mut flac, mut flac_identical) = (0, 0);
    for path in golden_exports(&golden) {
        let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let bytes = read(&path);
        // The large WAV renders take minutes in a debug build; the small ones cover the layout.
        // Every FLAC file is checked: libFLAC re-encodes even the two long workflow renders
        // (17 million frames) in seconds, and they are the longest byte-exact FLAC oracle.
        if bytes.len() > 20_000_000 && ext.as_deref() != Some("flac") {
            continue;
        }
        match ext.as_deref() {
            Some("wav") if bytes[20..22] == [1, 0] && bytes[34..36] == [24, 0] => {
                let (channels, samples) = decode(&path);
                let mut spec = OutputSpec::new(dir.join(&name));
                spec.dither = Dither::None;
                spec.channels = Some(u32::try_from(channels).expect("channels"));
                reexport(&samples, channels, spec, &info_metadata(&bytes));
                assert!(
                    read(&dir.join(&name)) == bytes,
                    "{} is not reproduced",
                    path.display()
                );
                checked += 1;
            }
            Some("flac") => {
                let (channels, samples) = decode(&path);
                let (_, comments) = vorbis_comments(&flac_blocks(&bytes)[1].1);
                let metadata: Vec<(String, String)> = comments
                    .iter()
                    .map(|c| {
                        let (k, v) = c.split_once('=').expect("key=value");
                        (k.to_owned(), v.to_owned())
                    })
                    .collect();
                let mut spec = OutputSpec::new(dir.join(&name));
                spec.container = Container::Flac;
                spec.dither = Dither::None;
                spec.channels = Some(u32::try_from(channels).expect("channels"));
                reexport(&samples, channels, spec, &metadata);
                let same = check_flac(&dir.join(&name), &path)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                flac_identical += usize::from(same);
                flac += 1;
                checked += 1;
            }
            _ => {}
        }
    }
    eprintln!(
        "{checked} golden 24-bit exports reproduced, {flac} of them FLAC ({flac_identical} \
         byte-identical)"
    );
    assert!(checked >= 6, "{checked}");
    assert!(
        flac >= 11,
        "the coverage stems and the workflow renders: {flac}"
    );
}

#[test]
fn golden_float_previews_are_reproduced_from_their_samples() {
    let golden = golden();
    let dir = scratch("golden-export-previews");
    let mut checked = 0;
    for path in golden_exports(&golden) {
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let bytes = read(&path);
        let is_wav = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
        if !is_wav || bytes[20..22] != [3, 0] || bytes.len() > 20_000_000 {
            continue;
        }
        let audio = golden
            .wav(
                &path
                    .strip_prefix(golden.root())
                    .expect("inside")
                    .to_string_lossy(),
            )
            .expect("float WAV");
        let out = dir.join(&name);
        let mut writer = PcmFileWriter::create(
            &out,
            PcmContainer::Wav,
            SampleFormat::Float,
            audio.channels,
            audio.sample_rate,
            &[],
            clock(),
        )
        .expect("create");
        for block in audio.samples.chunks(65536 * usize::from(audio.channels)) {
            writer.write_f32(block).expect("write");
        }
        writer.finish().expect("finish");
        // The recorder strips libsndfile's time-stamped PEAK chunk (golden_common.py).
        assert!(
            strip_peak(&read(&out)) == bytes,
            "{} is not reproduced",
            path.display()
        );
        checked += 1;
    }
    eprintln!("{checked} golden float previews reproduced");
    assert!(checked >= 3, "{checked}");
}

/// `golden_common.strip_peak_chunk`.
fn strip_peak(wav: &[u8]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    let mut pos = 12;
    while pos + 8 <= wav.len() {
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().expect("u32")) as usize;
        let end = (pos + 8 + size + (size & 1)).min(wav.len());
        if &wav[pos..pos + 4] != b"PEAK" {
            body.extend_from_slice(&wav[pos..end]);
        }
        pos = end;
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).expect("size").to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// Exports a workflow's `render.wav` like its `export_audio` step and compares the PCM of the
/// common frames with the golden WAV.
fn check_render_chain(workflow: &str, wav: &str, normalize: NormalizeSpec) -> (usize, usize) {
    let golden = golden();
    let base = format!("workflows/{workflow}");
    let transcript: Value = golden
        .json(&format!("{base}/transcript.json"))
        .expect("transcript");
    let step = transcript["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|s| s["tool"] == "export_audio")
        .expect("export step");
    let analysis = &step["result"]["result"]["analysis_before_gain"];
    // The prototype's `_gain` on its float64 analysis (the float32 render.wav is shorter and
    // rounded, so its own analysis would differ).
    let gain_db = match normalize.mode {
        Normalize::Loudness => {
            let gain = normalize.target_db - analysis["integrated_lufs"].as_f64().expect("lufs");
            let limit = normalize.true_peak_ceiling_db.expect("ceiling")
                - analysis["true_peak_dbtp"].as_f64().expect("tp");
            gain.min(limit)
        }
        _ => 0.0,
    };
    let gain = 10f64.powf(gain_db / 20.0);
    let render = golden.wav(&format!("{base}/render.wav")).expect("render");
    let blocks: Vec<f64> = render
        .samples
        .iter()
        // `x * 1.0` is `x`, so multiplying always equals the prototype's `if gain != 1.0`.
        .map(|&v| f64::from(v) * gain)
        .collect();
    let golden_path = golden.root().join(format!("{base}/outputs/{wav}"));
    let metadata = info_metadata(&read(&golden_path));
    let dir = scratch(&format!("golden-export-chain-{workflow}"));
    let out = dir.join("chain.wav");
    let options = ExportOptions {
        metadata: &metadata,
        overwrite: true,
        ..ExportOptions::default()
    };
    Exporter::new(dir.join("cache"))
        .with_clock(clock())
        .export(
            blocks.chunks(512 * 2),
            render.sample_rate,
            &[OutputSpec::new(&out)],
            &options,
        )
        .expect("export");
    let (_, ours) = decode(&out);
    let (_, theirs) = decode(&golden_path);
    let n = ours.len().min(theirs.len());
    let mut off_by_one = 0;
    for (a, b) in ours[..n].iter().zip(&theirs[..n]) {
        let d = ((a - b) * 8_388_608.0).abs();
        assert!(d <= 1.0, "{workflow}: a sample differs by {d} LSB");
        if d != 0.0 {
            off_by_one += 1;
        }
    }
    (off_by_one, n)
}

#[test]
fn exporting_the_recorded_renders_gives_the_golden_pcm_within_one_lsb() {
    let loudness = NormalizeSpec {
        mode: Normalize::Loudness,
        target_db: -14.0,
        true_peak_ceiling_db: Some(-1.0),
    };
    // examples/sidechain_preview.json and examples/mix_analysis.json.
    for (workflow, wav, normalize) in [
        (
            "sidechain_preview",
            "examples/output/sidechain_preview/sidechain-preview.wav",
            NormalizeSpec::default(),
        ),
        (
            "mix_analysis",
            "examples/output/mix_analysis/fixed-mix.wav",
            loudness,
        ),
    ] {
        let (off, n) = check_render_chain(workflow, wav, normalize);
        let share = ratio(off, n);
        eprintln!(
            "{workflow}: {off} of {n} samples differ by one LSB ({:.2} %)",
            share * 100.0
        );
        assert!(share <= CHAIN_LSB_SHARE, "{workflow}: {share}");
    }
}
